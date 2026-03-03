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

package scheduler_test

import (
	"context"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

// fakeTimerHandle implements scheduler.TimerHandle for tests.
// It returns false from Stop() if the goroutine was already launched (d<=0),
// and true if the timer was never fired (d>0). This mirrors time.Timer.Stop().
type fakeTimerHandle struct {
	mu    sync.Mutex
	fired bool
}

func (fth *fakeTimerHandle) Stop() bool {
	fth.mu.Lock()
	defer fth.mu.Unlock()

	return !fth.fired
}

// makeFakeFactory returns a TimerFactory that fires immediately (in a goroutine)
// when d <= 0, and does not fire for d > 0. Using a goroutine (not a direct
// call) matches time.AfterFunc semantics and avoids deadlocking on job.mu.
func makeFakeFactory() scheduler.TimerFactory {
	return func(d time.Duration, callback func()) scheduler.TimerHandle {
		handle := &fakeTimerHandle{}

		if d <= 0 {
			handle.fired = true

			go callback()
		}

		return handle
	}
}

// baseTime is a fixed reference instant used throughout the tests.
var baseTime = time.Date(2024, 6, 15, 12, 0, 0, 0, time.UTC)

// farFuture is used as the reschedule target when we want the job to not
// fire again during a test.
var farFuture = baseTime.Add(24 * time.Hour * 365)

// TestJobFiresOnceThenReschedules verifies that a job fires exactly once for
// a past fireAt, then reschedules to a future time without firing again.
func TestJobFiresOnceThenReschedules(t *testing.T) {
	t.Parallel()

	fakeClock := clock.NewFakeClock(baseTime)

	ctx := t.Context()

	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	var callCount atomic.Int32

	pastFireAt := baseTime.Add(-time.Minute) // in the past → delay=0 → fires
	sched.Schedule("once", pastFireAt, func() {
		callCount.Add(1)
	}, func(_ time.Time) time.Time {
		return farFuture // reschedule far in the future → won't fire in test
	})

	// Stop waits for the in-flight action and the rescheduled (non-firing) timer.
	sched.Stop()

	if got := callCount.Load(); got != 1 {
		t.Errorf("action call count = %d, want 1", got)
	}
}

// TestStopBeforeFire verifies that Stop() prevents a future-scheduled job
// from ever calling its action.
func TestStopBeforeFire(t *testing.T) {
	t.Parallel()

	fakeClock := clock.NewFakeClock(baseTime)

	ctx := t.Context()

	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	var callCount atomic.Int32

	futureFireAt := farFuture // d > 0 → fake factory does not launch goroutine
	sched.Schedule("future", futureFireAt, func() {
		callCount.Add(1)
	}, func(fired time.Time) time.Time {
		return fired.Add(24 * time.Hour)
	})

	sched.Stop()

	if got := callCount.Load(); got != 0 {
		t.Errorf("action should not have fired; call count = %d", got)
	}
}

// TestContextCancellationStopsRescheduling verifies that once the context is
// canceled, an in-flight callback exits cleanly and does not reschedule.
func TestContextCancellationStopsRescheduling(t *testing.T) {
	t.Parallel()

	fakeClock := clock.NewFakeClock(baseTime)
	ctx, cancel := context.WithCancel(context.Background())

	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	// Cancel before scheduling so ctx.Done() is already closed when the
	// callback runs.
	cancel()

	var callCount atomic.Int32

	pastFireAt := baseTime.Add(-time.Minute)
	sched.Schedule("ctx-cancel", pastFireAt, func() {
		callCount.Add(1)
	}, func(fired time.Time) time.Time {
		return fired.Add(time.Second) // would fire immediately → infinite loop without ctx guard
	})

	sched.Stop()

	// Action must not have run (context was already done).
	if got := callCount.Load(); got != 0 {
		t.Errorf("action should not run after context cancel; call count = %d", got)
	}
}

// TestTwoConcurrentJobsBothFire verifies that two independently scheduled past
// jobs each fire exactly once under concurrent execution.
func TestTwoConcurrentJobsBothFire(t *testing.T) {
	t.Parallel()

	fakeClock := clock.NewFakeClock(baseTime)

	ctx := t.Context()

	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	var countA, countB atomic.Int32

	pastA := baseTime.Add(-2 * time.Minute)
	pastB := baseTime.Add(-1 * time.Minute)

	sched.Schedule(
		"jobA",
		pastA,
		func() { countA.Add(1) },
		func(_ time.Time) time.Time { return farFuture },
	)
	sched.Schedule(
		"jobB",
		pastB,
		func() { countB.Add(1) },
		func(_ time.Time) time.Time { return farFuture },
	)

	sched.Stop()

	if got := countA.Load(); got != 1 {
		t.Errorf("jobA call count = %d, want 1", got)
	}

	if got := countB.Load(); got != 1 {
		t.Errorf("jobB call count = %d, want 1", got)
	}
}
