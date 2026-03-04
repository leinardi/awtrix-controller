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

// Package notification schedules and delivers birthday and New Year
// notifications to all connected Awtrix3 displays.
package notification

import (
	"fmt"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

// BirthdayNotifier schedules yearly birthday notifications and fans them out
// to all currently connected clients when the alarm fires.
type BirthdayNotifier struct {
	birthdays []config.BirthdayConfig
	timezone  *time.Location
	clk       clock.Clock
	sched     *scheduler.Scheduler
	publish   func(clientID string, notification *model.Notification) error
}

// NewBirthdayNotifier returns a BirthdayNotifier that will schedule one alarm
// per birthday entry. Call Start to arm the alarms.
func NewBirthdayNotifier(
	birthdays []config.BirthdayConfig,
	timezone *time.Location,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	publish func(clientID string, notification *model.Notification) error,
) *BirthdayNotifier {
	return &BirthdayNotifier{
		birthdays: birthdays,
		timezone:  timezone,
		clk:       clk,
		sched:     sched,
		publish:   publish,
	}
}

// Start schedules a yearly alarm for each configured birthday. connectedIDs is
// called at alarm fire time to enumerate which clients should receive the
// notification.
func (notif *BirthdayNotifier) Start(connectedIDs func() []string) {
	now := notif.clk.Now()

	for _, birthdayCfg := range notif.birthdays {
		dob, err := time.Parse("2006-01-02", birthdayCfg.DateOfBirth)
		if err != nil {
			// Entries with invalid dates are already filtered by config.Load;
			// this guard protects against direct construction in tests.
			logger.L().Warn("notification: skipping birthday with invalid date_of_birth",
				"name", birthdayCfg.Name,
				"date_of_birth", birthdayCfg.DateOfBirth,
			)

			continue
		}

		month := dob.Month()
		day := dob.Day()
		birthYear := dob.Year()
		cfg := birthdayCfg // capture loop variable

		fireAt := nextAnnualOccurrence(now, month, day, notif.timezone)

		notif.sched.Schedule(
			"birthday:"+cfg.Name,
			fireAt,
			func() {
				notif.onBirthday(&cfg, birthYear, connectedIDs)
			},
			func(fired time.Time) time.Time {
				return nextAnnualOccurrence(fired.In(notif.timezone), month, day, notif.timezone)
			},
		)
	}
}

// onBirthday is called by the scheduler when a birthday alarm fires. It
// computes the person's age, builds the notification, and publishes to every
// currently connected client.
func (notif *BirthdayNotifier) onBirthday(
	cfg *config.BirthdayConfig,
	birthYear int,
	connectedIDs func() []string,
) {
	firedLocal := notif.clk.Now().In(notif.timezone)
	age := firedLocal.Year() - birthYear

	text := cfg.Message
	if text == "" {
		text = fmt.Sprintf("Happy %d Birthday %s!", age, cfg.Name)
	}

	birthdayNotification := &model.Notification{
		Text:        text,
		Icon:        cfg.Icon,
		Duration:    cfg.Duration,
		Rainbow:     *cfg.Rainbow,
		Rtttl:       cfg.RTTTL,
		LoopSound:   false,
		ScrollSpeed: cfg.ScrollSpeed,
		Wakeup:      *cfg.Wakeup,
	}

	for _, clientID := range connectedIDs() {
		publishErr := notif.publish(clientID, birthdayNotification)
		if publishErr != nil {
			logger.L().Warn("notification: birthday publish failed",
				"client_id", clientID,
				"name", cfg.Name,
				"err", publishErr,
			)
		}
	}
}

// nextAnnualOccurrence returns the next occurrence of the given month/day at
// 00:00:00 in loc that is strictly after from.
func nextAnnualOccurrence(from time.Time, month time.Month, day int, loc *time.Location) time.Time {
	localFrom := from.In(loc)
	candidate := time.Date(localFrom.Year(), month, day, 0, 0, 0, 0, loc)

	if !candidate.After(from) {
		candidate = time.Date(localFrom.Year()+1, month, day, 0, 0, 0, 0, loc)
	}

	return candidate
}
