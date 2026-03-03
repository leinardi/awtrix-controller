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

package broker

import (
	"context"
	"fmt"
	"sync"

	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/logger"
	mqtt "github.com/wind-c/comqtt/v2/mqtt"
	"github.com/wind-c/comqtt/v2/mqtt/listeners"
)

// newServerMu serializes calls to mqtt.New to avoid a data race in comqtt's
// ensureDefaults(), which writes to a shared DefaultServerCapabilities pointer
// without synchronization when called concurrently.
var newServerMu sync.Mutex //nolint:gochecknoglobals // intentional package-level synchronization

// New creates and configures an MQTT server with the given hook, a TCP
// listener on cfg.MQTT.Port, and an optional WebSocket listener on
// cfg.MQTT.WSPort when non-nil. The server is not yet started; call Serve.
func New(cfg *config.Config, hook *ControllerHook) (*mqtt.Server, error) {
	newServerMu.Lock()
	server := mqtt.New(&mqtt.Options{InlineClient: true, Logger: logger.L()})
	newServerMu.Unlock()

	hookErr := server.AddHook(hook, nil)
	if hookErr != nil {
		return nil, fmt.Errorf("broker: add controller hook: %w", hookErr)
	}

	tcpAddr := fmt.Sprintf(":%d", cfg.MQTT.Port)
	tcpListener := listeners.NewTCP("tcp", tcpAddr, nil)

	tcpErr := server.AddListener(tcpListener)
	if tcpErr != nil {
		return nil, fmt.Errorf("broker: add TCP listener on %s: %w", tcpAddr, tcpErr)
	}

	if cfg.MQTT.WSPort != nil {
		wsAddr := fmt.Sprintf(":%d", *cfg.MQTT.WSPort)
		wsListener := listeners.NewWebsocket("ws", wsAddr, nil)

		wsErr := server.AddListener(wsListener)
		if wsErr != nil {
			return nil, fmt.Errorf("broker: add WebSocket listener on %s: %w", wsAddr, wsErr)
		}
	}

	return server, nil
}

// Serve starts the MQTT server in a background goroutine and blocks until
// ctx is canceled, at which point it calls server.Close and returns.
// Any error from the server's own Serve loop is logged but does not unblock
// the function; the caller must cancel ctx to initiate shutdown.
func Serve(ctx context.Context, server *mqtt.Server) error {
	go func() {
		serveErr := server.Serve()
		if serveErr != nil {
			logger.L().Error("broker: server stopped unexpectedly", "err", serveErr)
		}
	}()

	<-ctx.Done()

	closeErr := server.Close()
	if closeErr != nil {
		return fmt.Errorf("broker: close server: %w", closeErr)
	}

	return nil
}
