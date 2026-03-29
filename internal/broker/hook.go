/*
 * MIT License
 *
 * Copyright (c) 2026 Roberto Leinardi
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

// Package broker wraps the comqtt embedded MQTT broker with authentication,
// client-state tracking, and settings-push on connect.
package broker

import (
	"encoding/json"
	"strings"
	"sync"

	"github.com/leinardi/awtrix-controller/internal/clientstate"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/model"
	mqtt "github.com/wind-c/comqtt/v2/mqtt"
	"github.com/wind-c/comqtt/v2/mqtt/packets"
)

// ControllerHook is a comqtt hook that handles authentication, client-state
// registration, and MQTT topic routing for Awtrix3 devices.
type ControllerHook struct {
	mqtt.HookBase

	username      string
	password      string
	registry      *clientstate.Registry
	pushSettings  func(clientID string) error
	onDeviceReady func(clientID string)
	readyClients  sync.Map // key: clientID string, value: struct{}
	activeClients sync.Map // key: clientID string, value: *mqtt.Client; tracks the current session pointer
}

// NewControllerHook returns a ControllerHook configured with the given
// credentials, registry, settings-push function, and optional device-ready
// callback. pushSettings is called in a goroutine on every successful client
// connection. onDeviceReady (may be nil) is called once per connection when
// the first {clientID}/stats publish is received, indicating the device has
// completed its subscriptions and is ready to receive notifications.
func NewControllerHook(
	username, password string,
	registry *clientstate.Registry,
	pushSettings func(clientID string) error,
	onDeviceReady func(clientID string),
) *ControllerHook {
	return &ControllerHook{
		username:      username,
		password:      password,
		registry:      registry,
		pushSettings:  pushSettings,
		onDeviceReady: onDeviceReady,
	}
}

// ID returns the unique identifier for this hook.
// The receiver is intentionally unnamed: the hook ID is a compile-time constant.
func (*ControllerHook) ID() string {
	return "controller"
}

// Provides reports whether this hook implements the given hook event.
// The receiver is intentionally unnamed: the check is purely value-based.
func (*ControllerHook) Provides(hookEvent byte) bool {
	switch hookEvent {
	case mqtt.OnConnectAuthenticate, mqtt.OnACLCheck,
		mqtt.OnConnect, mqtt.OnDisconnect, mqtt.OnPublish:
		return true
	default:
		return false
	}
}

// OnConnectAuthenticate validates the connecting client's credentials against
// the configured username and password. Returns false to reject the connection.
//
//nolint:gocritic // hugeParam: packets.Packet value type is required by the mqtt.Hook interface
func (h *ControllerHook) OnConnectAuthenticate(_ *mqtt.Client, packet packets.Packet) bool {
	return string(packet.Connect.Username) == h.username &&
		string(packet.Connect.Password) == h.password
}

// OnACLCheck grants all authenticated clients full publish and subscribe access.
// The receiver is intentionally unnamed: the check is unconditional.
//

func (*ControllerHook) OnACLCheck(_ *mqtt.Client, _ string, _ bool) bool {
	return true
}

// OnConnect registers the client in the state registry and pushes the current
// settings payload in a background goroutine so the CONNACK is not delayed.
//
//nolint:gocritic // hugeParam: packets.Packet value type is required by the mqtt.Hook interface
func (h *ControllerHook) OnConnect(client *mqtt.Client, _ packets.Packet) error {
	h.activeClients.Store(client.ID, client)
	h.registry.Register(client.ID)
	logger.L().Info("broker: client connected", "clientID", client.ID)

	go func() {
		defer func() {
			if recovered := recover(); recovered != nil {
				logger.L().Error("broker: panic in pushSettings",
					"clientID", client.ID,
					"panic", recovered,
				)
			}
		}()

		pushErr := h.pushSettings(client.ID)
		if pushErr != nil {
			logger.L().Error("broker: failed to push settings",
				"clientID", client.ID,
				"err", pushErr,
			)
		}
	}()

	return nil
}

// OnDisconnect removes the client from the state registry and clears its
// ready state so that a subsequent reconnect triggers onDeviceReady again.
//
// The unregister is guarded by a pointer check: if the client reconnected before
// this disconnect callback fired (comqtt calls OnConnect for the new session
// before OnDisconnect for the expired old one), the activeClients map already
// holds the new session's pointer. Comparing it against the disconnecting client
// pointer prevents the stale disconnect from wiping the new registration.
func (h *ControllerHook) OnDisconnect(client *mqtt.Client, disconnectErr error, expired bool) {
	if currentClient, ok := h.activeClients.Load(
		client.ID,
	); ok &&
		currentClient.(*mqtt.Client) == client { //nolint:forcetypeassert // activeClients only stores *mqtt.Client values via OnConnect
		h.activeClients.Delete(client.ID)
		h.readyClients.Delete(client.ID)
		h.registry.Unregister(client.ID)
	}

	logger.L().Info("broker: client disconnected",
		"clientID", client.ID,
		"err", disconnectErr,
		"sessionExpired", expired,
	)
}

// OnPublish routes incoming client publishes by topic suffix:
//   - {clientID}/stats            → unmarshal Stats and update registry
//   - {clientID}/stat/currentApp  → update current-app in registry
//   - {clientID}/button/{left,select,right} → log the button event
//
//nolint:gocritic // hugeParam: packets.Packet value type is required by the mqtt.Hook interface
func (h *ControllerHook) OnPublish(
	client *mqtt.Client,
	packet packets.Packet,
) (packets.Packet, error) {
	topic := packet.TopicName

	logger.L().Debug("broker: message received",
		"clientID", client.ID,
		"topic", topic,
		"payload", string(packet.Payload),
	)

	switch {
	case strings.HasSuffix(topic, "/stats"):
		var stats model.Stats

		parseErr := json.Unmarshal(packet.Payload, &stats)
		if parseErr != nil {
			logger.L().Warn("broker: failed to parse stats payload",
				"clientID", client.ID,
				"topic", topic,
				"err", parseErr,
			)
		} else {
			h.registry.UpdateStats(client.ID, &stats)
		}

		if h.onDeviceReady != nil {
			_, alreadyReady := h.readyClients.LoadOrStore(client.ID, struct{}{})
			if !alreadyReady {
				go h.onDeviceReady(client.ID)
			}
		}

	case strings.HasSuffix(topic, "/stat/currentApp"):
		h.registry.UpdateCurrentApp(client.ID, string(packet.Payload))
		logger.L().Debug("broker: current app updated",
			"clientID", client.ID,
			"app", string(packet.Payload),
		)

	case strings.HasSuffix(topic, "/button/left"),
		strings.HasSuffix(topic, "/button/select"),
		strings.HasSuffix(topic, "/button/right"):
		parts := strings.Split(topic, "/")
		buttonName := parts[len(parts)-1]

		logger.L().Info("broker: button event",
			"clientID", client.ID,
			"button", buttonName,
		)

	default:
		logger.L().Debug("broker: unhandled topic",
			"clientID", client.ID,
			"topic", topic,
			"payload", string(packet.Payload),
		)
	}

	return packet, nil
}
