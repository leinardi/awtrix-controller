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

// Package logger provides the application-wide structured logger singleton.
// All packages access the logger through L(), which auto-initializes at Info
// level on first use. Call Init before the first log statement in main to set
// the desired log level.
package logger

import (
	"log/slog"
	"os"
	"sync/atomic"
)

// instance is the global logger pointer. It is set by Init or lazily by L.
var instance atomic.Pointer[slog.Logger]

// Init configures the global logger with a JSON handler writing to stdout at
// the given slog level. Calling Init multiple times replaces the logger.
func Init(level slog.Level) {
	h := slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: level})
	instance.Store(slog.New(h))
}

// Set replaces the global logger with l. Passing nil causes the next call to
// L to auto-initialize at Info level. Intended for use in tests.
func Set(l *slog.Logger) {
	instance.Store(l)
}

// L returns the global logger, auto-initializing at Info level if Init or Set
// has not been called yet.
func L() *slog.Logger {
	if l := instance.Load(); l != nil {
		return l
	}

	h := slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelInfo})
	candidate := slog.New(h)
	// Only the first goroutine wins; subsequent ones see the already-stored value.
	instance.CompareAndSwap(nil, candidate)

	return instance.Load()
}
