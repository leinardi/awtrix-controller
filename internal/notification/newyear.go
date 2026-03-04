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

// NewYearNotifier schedules a yearly New Year notification at midnight on
// January 1st and fans it out to all connected clients when the alarm fires.
// Scheduling is skipped entirely when cfg.Enabled is false.
type NewYearNotifier struct {
	cfg      config.NewYearConfig
	timezone *time.Location
	clk      clock.Clock
	sched    *scheduler.Scheduler
	publish  func(clientID string, notification *model.Notification) error
}

// NewNewYearNotifier returns a NewYearNotifier. Call Start to arm the alarm.
func NewNewYearNotifier(
	cfg config.NewYearConfig,
	timezone *time.Location,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	publish func(clientID string, notification *model.Notification) error,
) *NewYearNotifier {
	return &NewYearNotifier{
		cfg:      cfg,
		timezone: timezone,
		clk:      clk,
		sched:    sched,
		publish:  publish,
	}
}

// Start schedules the New Year alarm. It is a no-op when cfg.Enabled is false.
// connectedIDs is called at alarm fire time to enumerate which clients should
// receive the notification.
func (notif *NewYearNotifier) Start(connectedIDs func() []string) {
	if notif.cfg.Enabled == nil || !*notif.cfg.Enabled {
		return
	}

	now := notif.clk.Now()
	fireAt := nextAnnualOccurrence(now, time.January, 1, notif.timezone)

	notif.sched.Schedule(
		"newyear",
		fireAt,
		func() {
			notif.onNewYear(connectedIDs)
		},
		func(fired time.Time) time.Time {
			return nextAnnualOccurrence(fired.In(notif.timezone), time.January, 1, notif.timezone)
		},
	)
}

// onNewYear is called by the scheduler when the New Year alarm fires. It
// computes the incoming year, builds the notification, and publishes to every
// currently connected client.
func (notif *NewYearNotifier) onNewYear(connectedIDs func() []string) {
	firedLocal := notif.clk.Now().In(notif.timezone)
	year := firedLocal.Year()

	text := notif.cfg.Message
	if text == "" {
		text = fmt.Sprintf("Happy New Year %d!", year)
	}

	newYearNotification := &model.Notification{
		Text:        text,
		Icon:        notif.cfg.Icon,
		Duration:    notif.cfg.Duration,
		Rainbow:     *notif.cfg.Rainbow,
		ScrollSpeed: notif.cfg.ScrollSpeed,
		Wakeup:      *notif.cfg.Wakeup,
	}

	for _, clientID := range connectedIDs() {
		publishErr := notif.publish(clientID, newYearNotification)
		if publishErr != nil {
			logger.L().Warn("notification: new year publish failed",
				"client_id", clientID,
				"err", publishErr,
			)
		}
	}
}
