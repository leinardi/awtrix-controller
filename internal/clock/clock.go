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

// Package clock defines the Clock interface and its two implementations:
// RealClock (production) and FakeClock (deterministic tests). Every
// time-dependent package takes a Clock parameter so that tests can control time
// without sleeping.
package clock

import (
	"sync"
	"time"
)

// Clock abstracts the current time so that callers can be tested without
// relying on the system clock.
type Clock interface {
	// Now returns the current time.
	Now() time.Time
}

// --- RealClock ---

// RealClock is a Clock that delegates to time.Now().
type RealClock struct{}

// NewRealClock returns a new RealClock.
func NewRealClock() *RealClock {
	return &RealClock{}
}

// Now returns the current wall-clock time via time.Now().
func (RealClock) Now() time.Time {
	return time.Now()
}

// --- FakeClock ---

// FakeClock is a Clock whose current time is controlled by the caller. It is
// safe for concurrent use.
type FakeClock struct {
	mu  sync.Mutex
	now time.Time
}

// NewFakeClock returns a FakeClock initialized to t.
func NewFakeClock(t time.Time) *FakeClock {
	return &FakeClock{now: t}
}

// Now returns the time most recently set by Set or Advance.
func (f *FakeClock) Now() time.Time {
	f.mu.Lock()
	defer f.mu.Unlock()

	return f.now
}

// Set replaces the clock's current time with t.
func (f *FakeClock) Set(t time.Time) {
	f.mu.Lock()
	defer f.mu.Unlock()

	f.now = t
}

// Advance moves the clock forward by d (or backward if d is negative).
func (f *FakeClock) Advance(d time.Duration) {
	f.mu.Lock()
	defer f.mu.Unlock()

	f.now = f.now.Add(d)
}
