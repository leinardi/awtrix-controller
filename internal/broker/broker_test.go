/*
 * MIT License
 *
 * Copyright (c) 2025 Roberto Leinardi
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

package broker_test

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"testing"
	"time"

	paho "github.com/eclipse/paho.mqtt.golang"
	"github.com/leinardi/awtrix-controller/internal/broker"
	"github.com/leinardi/awtrix-controller/internal/clientstate"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/model"
)

const (
	testUsername = "testuser"
	testPassword = "testpass"
)

// freePort returns an available TCP port on 127.0.0.1.
func freePort(t *testing.T) int {
	t.Helper()

	listener, err := (&net.ListenConfig{}).Listen(context.Background(), "tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal("freePort: listen:", err)
	}

	port := listener.Addr().(*net.TCPAddr).Port //nolint:forcetypeassert // net.ListenConfig.Listen("tcp") always returns *net.TCPAddr

	listener.Close()

	return port
}

// waitForBroker polls until the broker's TCP port accepts connections or
// the 5-second deadline is exceeded.
func waitForBroker(t *testing.T, addr string) {
	t.Helper()

	dialer := &net.Dialer{Timeout: 100 * time.Millisecond}
	deadline := time.Now().Add(5 * time.Second)

	for time.Now().Before(deadline) {
		conn, dialErr := dialer.DialContext(context.Background(), "tcp", addr)
		if dialErr == nil {
			conn.Close()

			return
		}

		time.Sleep(10 * time.Millisecond)
	}

	t.Fatalf("broker did not start at %s within 5 seconds", addr)
}

// startBroker creates and starts a broker on a random port. It registers a
// t.Cleanup that cancels the context (stopping the broker) when the test ends.
// The returned address is "tcp://127.0.0.1:PORT".
func startBroker(
	t *testing.T,
	registry *clientstate.Registry,
	pushFn func(string) error,
) string {
	t.Helper()

	port := freePort(t)
	addr := fmt.Sprintf("127.0.0.1:%d", port)

	cfg := &config.Config{
		MQTT: config.MQTTConfig{
			Port:     port,
			Username: testUsername,
			Password: testPassword,
		},
	}

	hook := broker.NewControllerHook(testUsername, testPassword, registry, pushFn)

	srv, err := broker.New(cfg, hook)
	if err != nil {
		t.Fatal("broker.New:", err)
	}

	ctx, cancel := context.WithCancel(context.Background())

	t.Cleanup(func() {
		cancel()
		time.Sleep(100 * time.Millisecond) // allow server to close cleanly
	})

	go func() {
		_ = broker.Serve(ctx, srv)
	}()

	waitForBroker(t, addr)

	return "tcp://" + addr
}

// pahoConnect creates and connects a paho MQTT client.
// It returns the client and a boolean indicating whether the connection succeeded.
//

//nolint:ireturn // paho.Client is the library's own interface; concrete type is not exported
func pahoConnect(
	t *testing.T,
	brokerAddr, clientID, username, password string,
) (paho.Client, bool) {
	t.Helper()

	opts := paho.NewClientOptions()
	opts.AddBroker(brokerAddr)
	opts.SetClientID(clientID)
	opts.SetUsername(username)
	opts.SetPassword(password)
	opts.SetAutoReconnect(false)
	opts.SetConnectRetry(false)
	opts.SetConnectTimeout(5 * time.Second)

	client := paho.NewClient(opts)
	token := client.Connect()

	if !token.WaitTimeout(5 * time.Second) {
		t.Log("pahoConnect: timeout waiting for token")

		return client, false
	}

	return client, token.Error() == nil
}

// TestBrokerConnectValidCredentials verifies that a client with correct
// credentials connects successfully and triggers a pushSettings call (TC-01).
func TestBrokerConnectValidCredentials(t *testing.T) {
	t.Parallel()

	registry := clientstate.NewRegistry()
	pushCalled := make(chan string, 1)

	brokerAddr := startBroker(t, registry, func(clientID string) error {
		pushCalled <- clientID

		return nil
	})

	client, connected := pahoConnect(t, brokerAddr, "device01", testUsername, testPassword)
	if !connected {
		t.Fatal("expected connection to succeed with valid credentials")
	}

	defer client.Disconnect(250)

	// Wait for the async pushSettings goroutine to fire.
	select {
	case gotClientID := <-pushCalled:
		if gotClientID != "device01" {
			t.Errorf("pushSettings called with clientID %q, want %q", gotClientID, "device01")
		}
	case <-time.After(2 * time.Second):
		t.Error("pushSettings was not called within 2 seconds")
	}
}

// TestBrokerConnectInvalidCredentials verifies that wrong credentials result
// in a refused connection (TC-07).
func TestBrokerConnectInvalidCredentials(t *testing.T) {
	t.Parallel()

	registry := clientstate.NewRegistry()

	brokerAddr := startBroker(t, registry, func(_ string) error { return nil })

	_, connected := pahoConnect(t, brokerAddr, "badclient", "wrong", "credentials")
	if connected {
		t.Error("expected connection to be rejected with invalid credentials")
	}
}

// TestBrokerPublishStats verifies that a stats payload published to
// {clientID}/stats is parsed and stored in the registry (TC-08).
func TestBrokerPublishStats(t *testing.T) {
	t.Parallel()

	registry := clientstate.NewRegistry()

	brokerAddr := startBroker(t, registry, func(_ string) error { return nil })

	client, connected := pahoConnect(t, brokerAddr, "device02", testUsername, testPassword)
	if !connected {
		t.Fatal("expected connection to succeed")
	}

	defer client.Disconnect(250)

	stats := model.Stats{Bat: 85, Lux: 450, Ram: 28000}

	statsJSON, marshalErr := json.Marshal(stats)
	if marshalErr != nil {
		t.Fatal("json.Marshal stats:", marshalErr)
	}

	// QoS 1 publish: blocks until PUBACK, ensuring the hook has run.
	pubToken := client.Publish("device02/stats", 1, false, statsJSON)
	if !pubToken.WaitTimeout(5*time.Second) || pubToken.Error() != nil {
		t.Fatal("publish stats failed:", pubToken.Error())
	}

	snap, ok := registry.Snapshot("device02")
	if !ok || snap.Stats == nil {
		t.Fatal("stats not stored in registry after publish")
	}

	if snap.Stats.Bat != stats.Bat {
		t.Errorf("Bat: want %d, got %d", stats.Bat, snap.Stats.Bat)
	}

	if snap.Stats.Lux != stats.Lux {
		t.Errorf("Lux: want %d, got %d", stats.Lux, snap.Stats.Lux)
	}
}

// TestBrokerDisconnect verifies that disconnecting a client removes it from
// the registry (TC-16).
func TestBrokerDisconnect(t *testing.T) {
	t.Parallel()

	registry := clientstate.NewRegistry()

	brokerAddr := startBroker(t, registry, func(_ string) error { return nil })

	client, connected := pahoConnect(t, brokerAddr, "device03", testUsername, testPassword)
	if !connected {
		t.Fatal("expected connection to succeed")
	}

	if len(registry.ConnectedIDs()) == 0 {
		t.Error("client should be in registry after connect")
	}

	client.Disconnect(250)

	// Poll until the OnDisconnect hook removes the client from the registry.
	deadline := time.Now().Add(2 * time.Second)

	for time.Now().Before(deadline) {
		if len(registry.ConnectedIDs()) == 0 {
			return // success
		}

		time.Sleep(10 * time.Millisecond)
	}

	t.Errorf("registry still contains clients after disconnect: %v", registry.ConnectedIDs())
}
