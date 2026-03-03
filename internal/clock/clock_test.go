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

package clock_test

import (
	"sync"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
)

// realClockTolerance is the maximum allowed delta between RealClock.Now() and
// time.Now() in the same test.
const realClockTolerance = time.Second

// TestRealClockNowIsRecent verifies that RealClock.Now() is within one second
// of the system clock.
func TestRealClockNowIsRecent(t *testing.T) {
	t.Parallel()

	clk := clock.NewRealClock()
	before := time.Now()
	got := clk.Now()
	after := time.Now()

	if got.Before(before.Add(-realClockTolerance)) || got.After(after.Add(realClockTolerance)) {
		t.Errorf(
			"RealClock.Now() = %v, want within %v of [%v, %v]",
			got,
			realClockTolerance,
			before,
			after,
		)
	}
}

// TestFakeClockNowReturnsSetValue verifies that FakeClock.Now() returns the
// exact time supplied to NewFakeClock and updated by Set.
func TestFakeClockNowReturnsSetValue(t *testing.T) {
	t.Parallel()

	fixed := time.Date(2024, 6, 15, 12, 0, 0, 0, time.UTC)
	clk := clock.NewFakeClock(fixed)

	if got := clk.Now(); !got.Equal(fixed) {
		t.Errorf("Now() = %v, want %v", got, fixed)
	}

	newTime := fixed.Add(time.Hour)
	clk.Set(newTime)

	if got := clk.Now(); !got.Equal(newTime) {
		t.Errorf("after Set, Now() = %v, want %v", got, newTime)
	}
}

// TestFakeClockAdvance verifies that Advance moves the clock forward by the
// given duration.
func TestFakeClockAdvance(t *testing.T) {
	t.Parallel()

	start := time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)
	clk := clock.NewFakeClock(start)

	clk.Advance(30 * time.Minute)

	want := start.Add(30 * time.Minute)
	if got := clk.Now(); !got.Equal(want) {
		t.Errorf("after Advance, Now() = %v, want %v", got, want)
	}
}

// TestFakeClockConcurrentSafety exercises FakeClock under concurrent reads and
// writes to ensure it is race-detector clean.
func TestFakeClockConcurrentSafety(t *testing.T) {
	t.Parallel()

	clk := clock.NewFakeClock(time.Now())

	const (
		goroutines = 10
		iterations = 100
	)

	var waitGroup sync.WaitGroup
	waitGroup.Add(goroutines * 2)

	// Writers
	for range goroutines {
		go func() {
			defer waitGroup.Done()

			for range iterations {
				clk.Advance(time.Millisecond)
			}
		}()
	}

	// Readers
	for range goroutines {
		go func() {
			defer waitGroup.Done()

			for range iterations {
				_ = clk.Now()
			}
		}()
	}

	waitGroup.Wait()
}
