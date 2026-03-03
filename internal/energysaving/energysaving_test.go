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

package energysaving_test

import (
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/energysaving"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

// ── timer factories ──────────────────────────────────────────────────────────

// immediateFakeHandle is the TimerHandle returned by makeFakeFactory.
type immediateFakeHandle struct {
	mu    sync.Mutex
	fired bool
}

func (h *immediateFakeHandle) Stop() bool {
	h.mu.Lock()
	defer h.mu.Unlock()

	return !h.fired
}

// makeFakeFactory returns a TimerFactory that fires immediately (in a new
// goroutine) when delay ≤ 0 and does nothing for delay > 0.
func makeFakeFactory() scheduler.TimerFactory {
	return func(delay time.Duration, callback func()) scheduler.TimerHandle {
		handle := &immediateFakeHandle{}

		if delay <= 0 {
			handle.fired = true

			go callback()
		}

		return handle
	}
}

// controllableHandle is the TimerHandle returned by makeControllableFactory.
type controllableHandle struct {
	mu      sync.Mutex
	fired   bool
	stopped bool
}

func (h *controllableHandle) Stop() bool {
	h.mu.Lock()
	defer h.mu.Unlock()

	if h.fired {
		return false
	}

	h.stopped = true

	return true
}

// controllableFactory stores registered timers so tests can trigger them
// manually without relying on real wall-clock delays.
type controllableFactory struct {
	mu     sync.Mutex
	timers []*controllableEntry
}

type controllableEntry struct {
	handle   *controllableHandle
	callback func()
}

func makeControllableFactory() *controllableFactory {
	return &controllableFactory{}
}

// Factory returns the scheduler.TimerFactory function.
func (cf *controllableFactory) Factory() scheduler.TimerFactory {
	return func(delay time.Duration, callback func()) scheduler.TimerHandle {
		handle := &controllableHandle{}
		entry := &controllableEntry{handle: handle, callback: callback}

		cf.mu.Lock()
		cf.timers = append(cf.timers, entry)
		cf.mu.Unlock()

		if delay <= 0 {
			handle.mu.Lock()
			handle.fired = true
			handle.mu.Unlock()

			go callback()
		}

		return handle
	}
}

// TriggerNext fires the first pending (unfired, unstopped) timer in a new
// goroutine and returns true. Returns false if no pending timer exists.
func (cf *controllableFactory) TriggerNext() bool {
	cf.mu.Lock()
	defer cf.mu.Unlock()

	for _, entry := range cf.timers {
		entry.handle.mu.Lock()
		alreadyDone := entry.handle.fired || entry.handle.stopped
		entry.handle.mu.Unlock()

		if !alreadyDone {
			entry.handle.mu.Lock()
			entry.handle.fired = true
			entry.handle.mu.Unlock()

			go entry.callback()

			return true
		}
	}

	return false
}

// ── helpers ──────────────────────────────────────────────────────────────────

func newTestConfig(start, end string) config.EnergySavingConfig {
	return config.EnergySavingConfig{Start: start, End: end}
}

// ── tests ─────────────────────────────────────────────────────────────────────

// TestActivationAndDeactivationTC03TC04 covers TC-03 (energy-saving activates
// at the start time) and TC-04 (energy-saving deactivates at the end time).
// Window: 02:00–06:00 UTC. Clock starts at 01:59 → inactive; first trigger →
// active; second trigger → inactive.
func TestActivationAndDeactivationTC03TC04(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, 1, 15, 1, 59, 0, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()
	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	var lastActive atomic.Value
	lastActive.Store(false)

	// First callback: activation (TC-03).
	var activationWG sync.WaitGroup
	activationWG.Add(1)

	// Second callback: deactivation (TC-04).
	var deactivationWG sync.WaitGroup
	deactivationWG.Add(1)

	callCount := 0

	var callMu sync.Mutex

	ctrl := energysaving.New(
		newTestConfig("02:00", "06:00"),
		timezone,
		fakeClock,
		sched,
		func(active bool) {
			lastActive.Store(active)

			callMu.Lock()
			callCount++
			currentCount := callCount
			callMu.Unlock()

			switch currentCount {
			case 1:
				activationWG.Done()
			case 2:
				deactivationWG.Done()
			}
		},
	)

	err := ctrl.Start()
	if err != nil {
		t.Fatalf("Start() error: %v", err)
	}

	// Before any transition: inactive at 01:59.
	if ctrl.IsActive() {
		t.Error("before TC-03: IsActive() = true, want false")
	}

	// Advance clock past 02:00 and trigger the activation timer (TC-03).
	fakeClock.Set(time.Date(2024, 1, 15, 2, 1, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("TC-03: no pending timer to trigger for activation")
	}

	activationWG.Wait()

	if !ctrl.IsActive() {
		t.Error("TC-03: after activation trigger, IsActive() = false, want true")
	}

	if active, _ := lastActive.Load().(bool); !active {
		t.Error("TC-03: onChange received false, want true")
	}

	// Advance clock past 06:00 and trigger the deactivation timer (TC-04).
	fakeClock.Set(time.Date(2024, 1, 15, 6, 1, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("TC-04: no pending timer to trigger for deactivation")
	}

	deactivationWG.Wait()
	sched.Stop()

	if ctrl.IsActive() {
		t.Error("TC-04: after deactivation trigger, IsActive() = true, want false")
	}

	if active, _ := lastActive.Load().(bool); active {
		t.Error("TC-04: onChange received true, want false")
	}
}

// TestMidnightSpanningWindowTC09 covers TC-09: a window that spans midnight
// (e.g. "23:00"–"05:00"). Tests initial state at various times and verifies a
// transition from inactive to active at the 23:00 boundary.
func TestMidnightSpanningWindowTC09(t *testing.T) {
	t.Parallel()

	timezone := time.UTC

	// Table-driven initial-state checks.
	cases := []struct {
		hour       int
		minute     int
		wantActive bool
	}{
		{22, 59, false},
		{23, 30, true},
		{4, 59, true},
		{5, 1, false},
	}

	for _, testCase := range cases {
		fakeClock := clock.NewFakeClock(
			time.Date(2024, 1, 15, testCase.hour, testCase.minute, 0, 0, timezone),
		)
		ctx := t.Context()
		sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

		ctrl := energysaving.New(
			newTestConfig("23:00", "05:00"),
			timezone,
			fakeClock,
			sched,
			func(bool) {},
		)

		err := ctrl.Start()
		if err != nil {
			t.Fatalf("Start() at %02d:%02d error: %v", testCase.hour, testCase.minute, err)
		}

		sched.Stop()

		got := ctrl.IsActive()
		if got != testCase.wantActive {
			t.Errorf("at %02d:%02d: IsActive() = %v, want %v",
				testCase.hour, testCase.minute, got, testCase.wantActive)
		}
	}

	// Transition test: clock at 22:59, Start → inactive, TriggerNext → active.
	fakeClock := clock.NewFakeClock(time.Date(2024, 1, 15, 22, 59, 0, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()
	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	var waitGroup sync.WaitGroup
	waitGroup.Add(1)

	var lastActive atomic.Value
	lastActive.Store(false)

	ctrl := energysaving.New(
		newTestConfig("23:00", "05:00"),
		timezone,
		fakeClock,
		sched,
		func(active bool) {
			lastActive.Store(active)
			waitGroup.Done()
		},
	)

	err := ctrl.Start()
	if err != nil {
		t.Fatalf("transition test Start() error: %v", err)
	}

	if ctrl.IsActive() {
		t.Error("TC-09 transition: IsActive() = true before 23:00, want false")
	}

	// Advance clock past 23:00 and trigger.
	fakeClock.Set(time.Date(2024, 1, 15, 23, 1, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("TC-09 transition: no pending timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	if !ctrl.IsActive() {
		t.Error("TC-09 transition: IsActive() = false after 23:00, want true")
	}

	if active, _ := lastActive.Load().(bool); !active {
		t.Error("TC-09 transition: onChange received false, want true")
	}
}

// TestTimezoneAwarenessTC21 covers TC-21: the configured timezone controls
// when the energy-saving window activates. The clock is set to 07:00 UTC which
// equals 02:00 EST (UTC-5), putting it exactly at the window start.
func TestTimezoneAwarenessTC21(t *testing.T) {
	t.Parallel()

	loc, err := time.LoadLocation("America/New_York")
	if err != nil {
		t.Fatalf("time.LoadLocation: %v", err)
	}

	// 07:00 UTC = 02:00 EST (January, UTC-5).
	fakeClock := clock.NewFakeClock(time.Date(2024, 1, 15, 7, 0, 0, 0, time.UTC))
	ctx := t.Context()
	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	ctrl := energysaving.New(
		newTestConfig("02:00", "06:00"),
		loc,
		fakeClock,
		sched,
		func(bool) {},
	)

	startErr := ctrl.Start()
	if startErr != nil {
		t.Fatalf("Start() error: %v", startErr)
	}

	sched.Stop()

	if !ctrl.IsActive() {
		t.Error("TC-21: IsActive() = false at 07:00 UTC (02:00 EST), want true")
	}
}

// TestConcurrentIsActiveReads verifies that concurrent reads of IsActive do
// not trigger the race detector.
func TestConcurrentIsActiveReads(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, 6, 15, 12, 0, 0, 0, timezone))
	ctx := t.Context()
	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	ctrl := energysaving.New(
		newTestConfig("02:00", "06:00"),
		timezone,
		fakeClock,
		sched,
		func(bool) {},
	)

	err := ctrl.Start()
	if err != nil {
		t.Fatalf("Start() error: %v", err)
	}

	const numGoroutines = 50

	var waitGroup sync.WaitGroup
	waitGroup.Add(numGoroutines)

	for range numGoroutines {
		go func() {
			defer waitGroup.Done()

			_ = ctrl.IsActive()
		}()
	}

	waitGroup.Wait()
	sched.Stop()
}
