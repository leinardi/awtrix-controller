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

// Package energysaving manages the nightly energy-saving window. During the
// window the display brightness is reduced (BRI=1, ABRI=false); outside the
// window auto-brightness is restored (ABRI=true). The window may span midnight.
// An onChange callback fires on each activation/deactivation so the settings
// layer can push updated payloads to all connected clients.
package energysaving

import (
	"fmt"
	"sync"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

// Controller manages the energy-saving active state and schedules transitions
// between the configured start and end times. All exported methods are safe
// for concurrent use.
type Controller struct {
	cfg      config.EnergySavingConfig
	timezone *time.Location
	clk      clock.Clock
	sched    *scheduler.Scheduler
	onChange func(bool)

	// Populated during Start(); zero until then.
	windowStart time.Duration // duration from midnight for activation
	windowEnd   time.Duration // duration from midnight for deactivation

	mu     sync.RWMutex
	active bool
}

// New returns a Controller configured with the given energy-saving window and
// timezone. Call Start to determine the initial state and schedule transitions.
func New(
	cfg config.EnergySavingConfig,
	timezone *time.Location,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	onChange func(bool),
) *Controller {
	return &Controller{
		cfg:      cfg,
		timezone: timezone,
		clk:      clk,
		sched:    sched,
		onChange: onChange,
	}
}

// Start determines the current active state from the clock, then schedules the
// next window boundary transition. It must be called exactly once before any
// other method. It never blocks; the transition callback runs in the
// Scheduler's goroutine.
func (ctrl *Controller) Start() error {
	startDuration, err := parseHHMM(ctrl.cfg.Start)
	if err != nil {
		return err
	}

	endDuration, err := parseHHMM(ctrl.cfg.End)
	if err != nil {
		return err
	}

	ctrl.windowStart = startDuration
	ctrl.windowEnd = endDuration

	now := ctrl.clk.Now()
	tod := timeOfDay(now, ctrl.timezone)

	ctrl.mu.Lock()
	ctrl.active = isInWindow(tod, ctrl.windowStart, ctrl.windowEnd)
	ctrl.mu.Unlock()

	fireAt := ctrl.nextBoundary(now)
	ctrl.sched.Schedule("energysaving", fireAt, ctrl.onTransition, ctrl.reschedule)

	return nil
}

// IsActive reports whether energy-saving mode is currently active. It is safe
// for concurrent use.
func (ctrl *Controller) IsActive() bool {
	ctrl.mu.RLock()
	defer ctrl.mu.RUnlock()

	return ctrl.active
}

// onTransition is called by the Scheduler when the activation or deactivation
// timer fires. It toggles the active state and invokes the onChange callback.
func (ctrl *Controller) onTransition() {
	ctrl.mu.Lock()
	ctrl.active = !ctrl.active
	newState := ctrl.active
	ctrl.mu.Unlock()

	ctrl.onChange(newState)
}

// reschedule is the Scheduler reschedule function called after each transition.
// It returns the time of the next opposite boundary.
func (ctrl *Controller) reschedule(fired time.Time) time.Time {
	return ctrl.nextBoundary(fired.In(ctrl.timezone))
}

// nextBoundary returns the next activation or deactivation boundary after from.
// If currently active the next boundary is the window end; otherwise it is the
// window start.
func (ctrl *Controller) nextBoundary(from time.Time) time.Time {
	if ctrl.IsActive() {
		return nextOccurrence(from, ctrl.windowEnd, ctrl.timezone)
	}

	return nextOccurrence(from, ctrl.windowStart, ctrl.timezone)
}

// isInWindow reports whether tod (time-of-day as duration from midnight) falls
// inside the [start, end) energy-saving window. When start >= end the window
// spans midnight: active from start until midnight and from midnight until end.
func isInWindow(tod, start, end time.Duration) bool {
	if start < end {
		return tod >= start && tod < end
	}

	// Midnight-spanning: active from start until midnight, then until end.
	return tod >= start || tod < end
}

// nextOccurrence returns the next wall-clock time at which the time-of-day
// equals targetTOD. If that moment has already passed today, it returns the
// same TOD tomorrow.
func nextOccurrence(from time.Time, targetTOD time.Duration, loc *time.Location) time.Time {
	localFrom := from.In(loc)
	startOfDay := time.Date(localFrom.Year(), localFrom.Month(), localFrom.Day(), 0, 0, 0, 0, loc)
	candidate := startOfDay.Add(targetTOD)

	if !candidate.After(from) {
		candidate = candidate.AddDate(0, 0, 1)
	}

	return candidate
}

// timeOfDay extracts the elapsed duration since local midnight for time t in
// the given location.
func timeOfDay(t time.Time, loc *time.Location) time.Duration {
	local := t.In(loc)

	return time.Duration(local.Hour())*time.Hour +
		time.Duration(local.Minute())*time.Minute +
		time.Duration(local.Second())*time.Second +
		time.Duration(local.Nanosecond())
}

// parseHHMM parses an "HH:MM" string into a time.Duration from midnight.
func parseHHMM(s string) (time.Duration, error) {
	parsed, err := time.Parse("15:04", s)
	if err != nil {
		return 0, fmt.Errorf("energysaving: invalid HH:MM time %q: %w", s, err)
	}

	return time.Duration(parsed.Hour())*time.Hour +
		time.Duration(parsed.Minute())*time.Minute, nil
}
