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

// Package daynight computes and tracks whether the configured location is
// currently in day or night mode based on astronomical sunrise/sunset times.
// Mode transitions are scheduled via the Scheduler and trigger an onChange
// callback so downstream components (e.g. the settings builder) can react.
package daynight

import (
	"sync"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
	solar "github.com/mstephenholl/go-solar"
)

// Mode represents the current day/night state.
type Mode int

const (
	// Day indicates that the sun is above the horizon at the configured location.
	Day Mode = iota
	// Night indicates that the sun is below the horizon.
	Night
)

// String returns a human-readable label for the mode.
func (m Mode) String() string {
	if m == Day {
		return "day"
	}

	return "night"
}

// ModeProvider is the read-only view of the Controller consumed by the settings
// package to select the correct theme when building a Settings payload.
type ModeProvider interface {
	CurrentMode() Mode
}

// SunriseFunc computes the sunrise and sunset times for the given geographic
// coordinates and calendar date. It returns ok=false when either event cannot
// be determined (polar night, midnight sun, or any other calculation error);
// in that case rise and set are zero values.
type SunriseFunc func(lat, lon float64, date time.Time) (rise, set time.Time, ok bool)

// Controller manages the day/night mode state and schedules transitions.
// All exported methods are safe for concurrent use.
type Controller struct {
	latitude  float64
	longitude float64
	timezone  *time.Location
	clk       clock.Clock
	sched     *scheduler.Scheduler
	onChange  func(Mode)
	sunFunc   SunriseFunc

	mu   sync.RWMutex
	mode Mode // Night is the zero value; overwritten by Start
}

// New returns a Controller that uses the production go-solar sunrise/sunset
// implementation. Call Start to determine the initial mode and schedule
// transitions.
func New(
	loc config.LocationConfig,
	timezone *time.Location,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	onChange func(Mode),
) *Controller {
	return NewWithSunriseFunc(loc, timezone, clk, sched, onChange, defaultSunriseFunc)
}

// NewWithSunriseFunc returns a Controller with an injectable SunriseFunc. This
// constructor is intended for tests that need deterministic sunrise/sunset
// values without real geographic computation.
func NewWithSunriseFunc(
	loc config.LocationConfig,
	timezone *time.Location,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	onChange func(Mode),
	sunFunc SunriseFunc,
) *Controller {
	return &Controller{
		latitude:  *loc.Latitude,
		longitude: *loc.Longitude,
		timezone:  timezone,
		clk:       clk,
		sched:     sched,
		onChange:  onChange,
		sunFunc:   sunFunc,
	}
}

// Start determines the current day/night mode from the clock and schedules
// the next sunrise or sunset transition. It must be called exactly once before
// any other method. It never blocks; the transition callback runs in the
// Scheduler's goroutine.
func (ctrl *Controller) Start() error {
	now := ctrl.clk.Now().In(ctrl.timezone)

	riseTime, sunsetTime, ok := ctrl.sunFunc(ctrl.latitude, ctrl.longitude, now)
	if !ok {
		ctrl.mu.Lock()
		ctrl.mode = Night
		ctrl.mu.Unlock()

		logger.L().
			Warn("daynight: polar night or midnight sun — cannot compute sunrise/sunset; staying Night and retrying at next midnight",
				"date", now.Format("2006-01-02"),
				"latitude", ctrl.latitude,
				"longitude", ctrl.longitude,
			)

		ctrl.sched.Schedule(
			"daynight",
			nextMidnight(now, ctrl.timezone),
			ctrl.onTransition,
			ctrl.reschedule,
		)

		return nil
	}

	ctrl.mu.Lock()

	if now.Before(riseTime) || !now.Before(sunsetTime) {
		ctrl.mode = Night
	} else {
		ctrl.mode = Day
	}

	ctrl.mu.Unlock()

	nextFireAt := ctrl.nextEventAfter(now, riseTime, sunsetTime)
	ctrl.sched.Schedule("daynight", nextFireAt, ctrl.onTransition, ctrl.reschedule)

	return nil
}

// CurrentMode returns the current day/night mode. It is safe for concurrent use
// and satisfies the ModeProvider interface.
func (ctrl *Controller) CurrentMode() Mode {
	ctrl.mu.RLock()
	defer ctrl.mu.RUnlock()

	return ctrl.mode
}

// onTransition is called by the Scheduler when a sunrise or sunset timer fires.
// It toggles the mode and invokes the onChange callback.
func (ctrl *Controller) onTransition() {
	ctrl.mu.Lock()

	if ctrl.mode == Day {
		ctrl.mode = Night
	} else {
		ctrl.mode = Day
	}

	newMode := ctrl.mode
	ctrl.mu.Unlock()

	ctrl.onChange(newMode)
}

// reschedule is the Scheduler reschedule function called after each transition.
// It returns the time of the next opposite event (sunset after sunrise, or
// tomorrow's sunrise after sunset). On polar fallback it returns next midnight.
func (ctrl *Controller) reschedule(fired time.Time) time.Time {
	firedLocal := fired.In(ctrl.timezone)

	_, sunsetTime, ok := ctrl.sunFunc(ctrl.latitude, ctrl.longitude, firedLocal)
	if !ok {
		logger.L().
			Warn("daynight: polar night or midnight sun in reschedule; retrying at next midnight",
				"date", firedLocal.Format("2006-01-02"),
				"latitude", ctrl.latitude,
				"longitude", ctrl.longitude,
			)

		return nextMidnight(firedLocal, ctrl.timezone)
	}

	currentMode := ctrl.CurrentMode()

	switch currentMode {
	case Day:
		// Just transitioned to Day at sunrise; next event is today's sunset.
		return sunsetTime
	case Night:
		// Just transitioned to Night at sunset; next event is tomorrow's sunrise.
		tomorrow := firedLocal.AddDate(0, 0, 1)

		tomorrowRise, _, tomorrowOK := ctrl.sunFunc(ctrl.latitude, ctrl.longitude, tomorrow)
		if !tomorrowOK {
			logger.L().
				Warn("daynight: polar night or midnight sun for next day in reschedule; retrying at next midnight",
					"date", tomorrow.Format("2006-01-02"),
					"latitude", ctrl.latitude,
					"longitude", ctrl.longitude,
				)

			return nextMidnight(firedLocal, ctrl.timezone)
		}

		return tomorrowRise
	}

	// Unreachable: Mode is either Day or Night.
	return nextMidnight(firedLocal, ctrl.timezone)
}

// nextEventAfter returns whichever of riseTime or sunsetTime comes next after
// now. If both are already past, it fetches tomorrow's sunrise.
func (ctrl *Controller) nextEventAfter(now, riseTime, sunsetTime time.Time) time.Time {
	if now.Before(riseTime) {
		return riseTime
	}

	if now.Before(sunsetTime) {
		return sunsetTime
	}

	// Both events already passed today; schedule tomorrow's sunrise.
	tomorrow := now.AddDate(0, 0, 1)

	tomorrowRise, _, tomorrowOK := ctrl.sunFunc(ctrl.latitude, ctrl.longitude, tomorrow)
	if tomorrowOK {
		return tomorrowRise
	}

	return nextMidnight(now, ctrl.timezone)
}

// defaultSunriseFunc wraps the go-solar library for production use.
func defaultSunriseFunc(lat, lon float64, date time.Time) (rise, set time.Time, ok bool) {
	location := solar.NewLocation(lat, lon)
	solarTime := solar.NewTimeFromDateTime(date)

	rise, set, err := solar.SunriseSunset(location, solarTime)

	return rise, set, err == nil
}

// nextMidnight returns the start of the calendar day following t in the given
// timezone (i.e. 00:00:00 of the next day).
func nextMidnight(t time.Time, loc *time.Location) time.Time {
	localTime := t.In(loc)
	startOfDay := time.Date(localTime.Year(), localTime.Month(), localTime.Day(), 0, 0, 0, 0, loc)

	return startOfDay.AddDate(0, 0, 1)
}
