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

package logger_test

import (
	"bytes"
	"log/slog"
	"strings"
	"testing"

	"github.com/leinardi/awtrix-controller/internal/logger"
)

// TestLBeforeInitReturnsNonNil verifies that L() returns a usable logger even
// when Init has never been called.
func TestLBeforeInitReturnsNonNil(
	t *testing.T,
) {
	t.Cleanup(func() { logger.Set(nil) })

	logger.Set(nil) // force uninitialized state

	l := logger.L()
	if l == nil {
		t.Fatal("L() returned nil before Init()")
	}
}

// TestInitDebugEmitsDebugRecords verifies that after setting a Debug-level
// logger, debug records are written to the backing handler.
func TestInitDebugEmitsDebugRecords(
	t *testing.T,
) {
	var buf bytes.Buffer

	h := slog.NewJSONHandler(&buf, &slog.HandlerOptions{Level: slog.LevelDebug})
	logger.Set(slog.New(h))
	t.Cleanup(func() { logger.Set(nil) })

	const msg = "test debug message"
	logger.L().Debug(msg)

	if !strings.Contains(buf.String(), msg) {
		t.Errorf("expected %q in log output, got: %s", msg, buf.String())
	}
}

// TestInitInfoSuppressesDebugRecords verifies that at Info level, debug
// records are not emitted.
func TestInitInfoSuppressesDebugRecords(
	t *testing.T,
) {
	var buf bytes.Buffer

	h := slog.NewJSONHandler(&buf, &slog.HandlerOptions{Level: slog.LevelInfo})
	logger.Set(slog.New(h))
	t.Cleanup(func() { logger.Set(nil) })

	logger.L().Debug("should not appear")

	if strings.Contains(buf.String(), "should not appear") {
		t.Errorf("debug record appeared at Info level: %s", buf.String())
	}
}
