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

// Package scheduler provides a recurring-job scheduler with an injectable timer
// factory, enabling deterministic testing without real sleeps.
package scheduler

import (
	"context"
	"sync"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/logger"
)

// TimerHandle is the control interface returned by a TimerFactory. It allows
// the Scheduler to cancel a pending timer. *time.Timer satisfies this interface.
type TimerHandle interface {
	Stop() bool
}

// TimerFactory creates a one-shot timer that calls f after duration d. The
// default production implementation wraps time.AfterFunc.
type TimerFactory func(d time.Duration, f func()) TimerHandle

// Job is a recurring scheduled task managed by a Scheduler.
type Job struct {
	name       string
	action     func()
	reschedule func(time.Time) time.Time

	mu      sync.Mutex
	handle  TimerHandle
	fireAt  time.Time
	stopped bool
}

// Scheduler runs recurring jobs using injectable timers and a clock. All
// exported methods are safe for concurrent use.
type Scheduler struct {
	ctx          context.Context //nolint:containedctx // scheduler owns its lifecycle context
	clk          clock.Clock
	timerFactory TimerFactory

	jobsMu sync.Mutex
	jobs   []*Job

	wg sync.WaitGroup
}

// New returns a Scheduler that uses time.AfterFunc as its timer factory.
func New(ctx context.Context, clk clock.Clock) *Scheduler {
	return NewWithFactory(ctx, clk, func(d time.Duration, f func()) TimerHandle {
		return time.AfterFunc(d, f)
	})
}

// NewWithFactory returns a Scheduler with a custom timer factory. Intended for
// test injection so that timers fire deterministically without sleeping.
func NewWithFactory(ctx context.Context, clk clock.Clock, factory TimerFactory) *Scheduler {
	return &Scheduler{
		ctx:          ctx,
		clk:          clk,
		timerFactory: factory,
	}
}

// Schedule registers a new recurring Job. It fires at fireAt, then calls
// reschedule(firedTime) to compute the next fire time and rearms itself.
// Schedule is safe to call concurrently.
func (sched *Scheduler) Schedule(
	name string,
	fireAt time.Time,
	action func(),
	reschedule func(time.Time) time.Time,
) *Job {
	job := &Job{
		name:       name,
		action:     action,
		reschedule: reschedule,
	}

	sched.jobsMu.Lock()
	sched.jobs = append(sched.jobs, job)
	sched.jobsMu.Unlock()

	logger.L().Info("scheduler: job registered", "job", name, "fireAt", fireAt)
	sched.arm(job, fireAt)

	return job
}

// Stop cancels all pending timers and waits for any in-flight action callbacks
// to complete before returning.
func (sched *Scheduler) Stop() {
	sched.jobsMu.Lock()
	jobs := make([]*Job, len(sched.jobs))
	copy(jobs, sched.jobs)
	sched.jobsMu.Unlock()

	for _, job := range jobs {
		job.mu.Lock()
		job.stopped = true

		if job.handle != nil {
			if job.handle.Stop() {
				// Timer was stopped before it fired; the callback will never
				// run, so manually balance the wg.Add from arm().
				sched.wg.Done()
			}
			// If Stop() returns false the timer has already fired; the
			// callback goroutine will call wg.Done() via defer.
		}

		job.mu.Unlock()
	}

	sched.wg.Wait()
}

// arm sets or rearms the timer for job to fire at fireAt. Must only be called
// when job.stopped is false (checked under job.mu by the caller or arm itself).
func (sched *Scheduler) arm(job *Job, fireAt time.Time) {
	job.mu.Lock()
	defer job.mu.Unlock()

	if job.stopped {
		return
	}

	delay := max(fireAt.Sub(sched.clk.Now()), 0)

	job.fireAt = fireAt

	// wg.Add must happen before the timer is created so that Stop/wg.Wait
	// cannot return before the callback has a chance to run.
	sched.wg.Add(1)

	job.handle = sched.timerFactory(delay, func() {
		defer sched.wg.Done()

		// Exit early if context is already canceled.
		select {
		case <-sched.ctx.Done():
			return
		default:
		}

		// The action runs unconditionally once the timer has fired —
		// it is considered "in-flight". Stop() only prevents future
		// rescheduling, not an already-launched callback.
		job.action()

		// Do not reschedule if the scheduler has been stopped or the
		// context was canceled while the action was running.
		select {
		case <-sched.ctx.Done():
			return
		default:
		}

		job.mu.Lock()

		if job.stopped {
			job.mu.Unlock()

			return
		}

		fired := job.fireAt
		job.mu.Unlock()

		sched.arm(job, job.reschedule(fired))
	})
}
