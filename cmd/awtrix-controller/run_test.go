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

package main

import (
	"context"
	"fmt"
	"log/slog"
	"net"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// freePort finds and returns an available TCP port on localhost.
func freePort(t *testing.T) int {
	t.Helper()

	var listenCfg net.ListenConfig

	listener, err := listenCfg.Listen(context.Background(), "tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("freePort: failed to listen: %v", err)
	}

	tcpAddr, ok := listener.Addr().(*net.TCPAddr)
	if !ok {
		t.Fatalf("freePort: expected *net.TCPAddr, got %T", listener.Addr())
	}

	port := tcpAddr.Port

	closeErr := listener.Close()
	if closeErr != nil {
		t.Fatalf("freePort: failed to close listener: %v", closeErr)
	}

	return port
}

// writeTempConfig writes content to a temporary config file and returns its path.
func writeTempConfig(t *testing.T, content string) string {
	t.Helper()

	configPath := filepath.Join(t.TempDir(), "config.yaml")

	writeErr := os.WriteFile(configPath, []byte(content), 0o600)
	if writeErr != nil {
		t.Fatalf("writeTempConfig: %v", writeErr)
	}

	return configPath
}

// minimalConfig returns a minimal valid YAML configuration string using the
// given MQTT port.
func minimalConfig(port int) string {
	return fmt.Sprintf(`mqtt:
  username: testuser
  password: testpass
  port: %d
location:
  latitude: 52.5
  longitude: 13.4
`, port)
}

// noEnv is a getenv stub that always returns empty string.
func noEnv(string) string { return "" }

// TestRunWithContext_TC10_GracefulShutdown verifies that canceling the context
// causes runWithContext to return exit code 0 (TC-10: SIGTERM → exit 0).
func TestRunWithContext_TC10_GracefulShutdown(t *testing.T) {
	t.Parallel()

	port := freePort(t)
	configPath := writeTempConfig(t, minimalConfig(port))

	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)

	resultCh := make(chan int, 1)

	go func() {
		resultCh <- runWithContext(ctx, []string{"--config", configPath}, noEnv)
	}()

	// Cancel the context to simulate SIGTERM.
	cancel()

	select {
	case result := <-resultCh:
		if result != 0 {
			t.Errorf("TC-10: expected exit code 0 on graceful shutdown, got %d", result)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("TC-10: runWithContext did not return within 10 s after context cancellation")
	}
}

// TestRunWithContext_TC18_MissingConfig verifies that a non-existent config
// path causes runWithContext to return exit code 1 (TC-18: missing config → exit 1).
func TestRunWithContext_TC18_MissingConfig(t *testing.T) {
	t.Parallel()

	result := runWithContext(
		context.Background(),
		[]string{"--config", "/nonexistent/awtrix-controller/config.yaml"},
		noEnv,
	)

	if result != 1 {
		t.Errorf("TC-18: expected exit code 1 for missing config, got %d", result)
	}
}

// TestEffectiveLevel_TC19_EnvVarSetsLevel verifies that AWTRIX_LOG_LEVEL is
// used when the --log-level flag was not explicitly set (TC-19).
func TestEffectiveLevel_TC19_EnvVarSetsLevel(t *testing.T) {
	t.Parallel()

	level := effectiveLevel("info", "debug", false)

	if level != slog.LevelDebug {
		t.Errorf("TC-19: expected LevelDebug when env=debug and flag not set, got %v", level)
	}
}

// TestEffectiveLevel_TC20_FlagWinsOverEnvVar verifies that an explicitly set
// --log-level flag takes precedence over AWTRIX_LOG_LEVEL (TC-20).
func TestEffectiveLevel_TC20_FlagWinsOverEnvVar(t *testing.T) {
	t.Parallel()

	level := effectiveLevel("warn", "debug", true)

	if level != slog.LevelWarn {
		t.Errorf("TC-20: expected LevelWarn when flag=warn wins over env=debug, got %v", level)
	}
}
