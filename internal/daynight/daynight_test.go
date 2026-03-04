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

package daynight_test

import (
	"bytes"
	"log/slog"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/daynight"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

// ── timer factories ──────────────────────────────────────────────────────────

// immediateFakeHandle is the TimerHandle returned by makeFakeFactory.
// Stop() returns false if the goroutine was already launched (delay ≤ 0) and
// true otherwise, mirroring the contract of time.Timer.Stop.
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

// capturingFactory records the duration passed when each timer is created,
// without ever firing. Used to assert the scheduled retry delay in TC-17.
type capturingFactory struct {
	mu        sync.Mutex
	durations []time.Duration
}

func makeCapturingFactory() *capturingFactory {
	return &capturingFactory{}
}

// Factory returns the scheduler.TimerFactory function.
func (fac *capturingFactory) Factory() scheduler.TimerFactory {
	return func(delay time.Duration, _ func()) scheduler.TimerHandle {
		fac.mu.Lock()
		fac.durations = append(fac.durations, delay)
		fac.mu.Unlock()

		return &immediateFakeHandle{} // never fired
	}
}

// FirstDuration returns the first captured duration, or 0 if none.
func (fac *capturingFactory) FirstDuration() time.Duration {
	fac.mu.Lock()
	defer fac.mu.Unlock()

	if len(fac.durations) == 0 {
		return 0
	}

	return fac.durations[0]
}

// ── shared helpers ────────────────────────────────────────────────────────────

// floatPtr returns a pointer to the given float64 value.
func floatPtr(v float64) *float64 { return &v }

// normalSunriseFunc returns a SunriseFunc that sets sunrise at 06:00 and
// sunset at 20:00 each day in the given timezone.
func normalSunriseFunc(timezone *time.Location) daynight.SunriseFunc {
	return func(_, _ float64, date time.Time) (time.Time, time.Time, bool) {
		base := time.Date(date.Year(), date.Month(), date.Day(), 0, 0, 0, 0, timezone)

		return base.Add(6 * time.Hour), base.Add(20 * time.Hour), true
	}
}

// neverRisesFunc always returns ok=false, simulating polar night.
func neverRisesFunc(_, _ float64, _ time.Time) (rise, set time.Time, ok bool) {
	return time.Time{}, time.Time{}, false
}

// newTestLoc builds a config.LocationConfig for test use.
func newTestLoc() config.LocationConfig {
	return config.LocationConfig{
		Latitude:  floatPtr(48.0),
		Longitude: floatPtr(11.0),
	}
}

// ── tests ─────────────────────────────────────────────────────────────────────

// TestInitialModeDayAtNoon verifies that a clock set to noon (between 06:00
// rise and 20:00 set) yields an initial mode of Day.
func TestInitialModeDayAtNoon(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, 6, 15, 12, 0, 0, 0, timezone))
	ctx := t.Context()
	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	ctrl := daynight.NewWithSunriseFunc(
		newTestLoc(),
		timezone,
		fakeClock,
		sched,
		func(daynight.Mode) {},
		normalSunriseFunc(timezone),
	)

	err := ctrl.Start()
	if err != nil {
		t.Fatalf("Start() error: %v", err)
	}

	sched.Stop()

	if got := ctrl.CurrentMode(); got != daynight.Day {
		t.Errorf("CurrentMode() = %v, want Day", got)
	}
}

// TestInitialModeNightAt2AM verifies that a clock set to 02:00 (before 06:00
// sunrise) yields an initial mode of Night.
func TestInitialModeNightAt2AM(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, 6, 15, 2, 0, 0, 0, timezone))
	ctx := t.Context()
	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	ctrl := daynight.NewWithSunriseFunc(
		newTestLoc(),
		timezone,
		fakeClock,
		sched,
		func(daynight.Mode) {},
		normalSunriseFunc(timezone),
	)

	err := ctrl.Start()
	if err != nil {
		t.Fatalf("Start() error: %v", err)
	}

	sched.Stop()

	if got := ctrl.CurrentMode(); got != daynight.Night {
		t.Errorf("CurrentMode() = %v, want Night", got)
	}
}

// TestDayToNightTransitionTC02 covers TC-02: the application starts in Day
// mode (clock just before sunset) and transitions to Night when the scheduled
// sunset timer fires.
func TestDayToNightTransitionTC02(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	// 19:59 — one minute before sunset at 20:00
	fakeClock := clock.NewFakeClock(time.Date(2024, 6, 15, 19, 59, 0, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()
	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	var lastMode atomic.Value
	lastMode.Store(daynight.Day)

	var waitGroup sync.WaitGroup
	waitGroup.Add(1)

	ctrl := daynight.NewWithSunriseFunc(
		newTestLoc(),
		timezone,
		fakeClock,
		sched,
		func(mode daynight.Mode) {
			lastMode.Store(mode)
			waitGroup.Done()
		},
		normalSunriseFunc(timezone),
	)

	err := ctrl.Start()
	if err != nil {
		t.Fatalf("Start() error: %v", err)
	}

	// Immediately after Start the mode must be Day (19:59 is between 06:00–20:00).
	if got := ctrl.CurrentMode(); got != daynight.Day {
		t.Errorf("before transition: CurrentMode() = %v, want Day", got)
	}

	// Advance the fake clock past sunset so the sunset timer's delay would be ≤ 0
	// when re-armed, then manually trigger the sunset job.
	fakeClock.Set(time.Date(2024, 6, 15, 20, 1, 0, 0, timezone))

	triggered := factory.TriggerNext()
	if !triggered {
		t.Fatal("no pending timer to trigger")
	}

	// Wait for the onChange callback.
	waitGroup.Wait()
	sched.Stop()

	if got := ctrl.CurrentMode(); got != daynight.Night {
		t.Errorf("after transition: CurrentMode() = %v, want Night", got)
	}

	if got, _ := lastMode.Load().(daynight.Mode); got != daynight.Night {
		t.Errorf("onChange received mode = %v, want Night", got)
	}
}

// TestPolarNightFallbackTC17 covers TC-17: when the sunrise function returns
// ok=false, the Controller logs a Warn, stays in Night mode, and schedules a
// retry for the start of the next calendar day. No crash, no spin-retry.
func TestPolarNightFallbackTC17(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	now := time.Date(2024, 1, 15, 14, 0, 0, 0, timezone) // mid-day in polar winter
	fakeClock := clock.NewFakeClock(now)
	ctx := t.Context()

	factory := makeCapturingFactory()
	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	// Capture log output to verify a Warn is emitted.
	var logBuf bytes.Buffer

	handler := slog.NewJSONHandler(&logBuf, &slog.HandlerOptions{Level: slog.LevelWarn})
	logger.Set(slog.New(handler))

	ctrl := daynight.NewWithSunriseFunc(
		newTestLoc(),
		timezone,
		fakeClock,
		sched,
		func(daynight.Mode) {},
		neverRisesFunc,
	)

	err := ctrl.Start()
	if err != nil {
		t.Fatalf("Start() error: %v", err)
	}

	sched.Stop()

	// Mode must remain Night.
	if got := ctrl.CurrentMode(); got != daynight.Night {
		t.Errorf("CurrentMode() = %v, want Night", got)
	}

	// A Warn must have been logged.
	if !bytes.Contains(logBuf.Bytes(), []byte(`"level":"WARN"`)) {
		t.Errorf("expected a WARN log entry; got:\n%s", logBuf.String())
	}

	// The retry must be scheduled at the start of the next calendar day.
	expectedMidnight := time.Date(now.Year(), now.Month(), now.Day()+1, 0, 0, 0, 0, timezone)
	expectedDelay := expectedMidnight.Sub(now)
	capturedDelay := factory.FirstDuration()

	const tolerance = time.Second

	diff := capturedDelay - expectedDelay
	if diff < -tolerance || diff > tolerance {
		t.Errorf(
			"scheduled delay = %v, want ≈ %v (within %v)",
			capturedDelay,
			expectedDelay,
			tolerance,
		)
	}
}

// TestConcurrentCurrentModeReads verifies that concurrent reads of CurrentMode
// do not trigger the race detector.
func TestConcurrentCurrentModeReads(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, 6, 15, 12, 0, 0, 0, timezone))
	ctx := t.Context()
	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	ctrl := daynight.NewWithSunriseFunc(
		newTestLoc(),
		timezone,
		fakeClock,
		sched,
		func(daynight.Mode) {},
		normalSunriseFunc(timezone),
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

			_ = ctrl.CurrentMode()
		}()
	}

	waitGroup.Wait()
	sched.Stop()
}
