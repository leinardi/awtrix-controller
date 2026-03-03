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

// Package notification schedules and delivers configurable recurring notifications
// to all connected Awtrix3 displays.
package notification

import (
	"errors"
	"strconv"
	"strings"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

// errUnknownWeekday is returned by parseWeekday for unrecognized names.
var errUnknownWeekday = errors.New("unknown weekday")

// daysInWeek is the number of days in a week, used for weekly recurrence.
const daysInWeek = 7

// ScheduledNotifier schedules and delivers configurable recurring notifications
// to all currently connected clients when each alarm fires.
type ScheduledNotifier struct {
	notifications []config.ScheduledNotificationConfig
	timezone      *time.Location
	clk           clock.Clock
	sched         *scheduler.Scheduler
	publish       func(clientID string, notification *model.Notification) error
}

// NewScheduledNotifier returns a ScheduledNotifier that will schedule one alarm
// per notification entry. Call Start to arm the alarms.
func NewScheduledNotifier(
	notifications []config.ScheduledNotificationConfig,
	timezone *time.Location,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	publish func(clientID string, notification *model.Notification) error,
) *ScheduledNotifier {
	return &ScheduledNotifier{
		notifications: notifications,
		timezone:      timezone,
		clk:           clk,
		sched:         sched,
		publish:       publish,
	}
}

// Start schedules a recurring alarm for each configured notification entry.
// Entries with enabled: false are skipped entirely. Weekly entries produce one
// alarm per weekday in Weekdays.
// connectedIDs is called at alarm fire time to enumerate which clients should
// receive the notification.
func (notif *ScheduledNotifier) Start(connectedIDs func() []string) {
	now := notif.clk.Now()

	for i := range notif.notifications {
		if notif.notifications[i].Enabled != nil && !*notif.notifications[i].Enabled {
			continue
		}

		cfg := notif.notifications[i] // capture loop variable

		if cfg.Repeat == "weekly" {
			notif.scheduleWeeklyAlarms(&cfg, now, connectedIDs)

			continue
		}

		fireAt, nextFn, ok := notif.resolveSchedule(&cfg, now)
		if !ok {
			continue
		}

		notif.sched.Schedule(
			"scheduled:"+cfg.Name,
			fireAt,
			func() {
				notif.onScheduled(&cfg, connectedIDs)
			},
			func(fired time.Time) time.Time {
				return nextFn(fired.In(notif.timezone))
			},
		)
	}
}

// scheduleWeeklyAlarms registers one alarm per entry in cfg.Weekdays, each
// repeating weekly on that day at the configured time.
func (notif *ScheduledNotifier) scheduleWeeklyAlarms(
	cfg *config.ScheduledNotificationConfig,
	now time.Time,
	connectedIDs func() []string,
) {
	fireTimeStr := cfg.Time
	if fireTimeStr == "" {
		fireTimeStr = "00:00"
	}

	parsedTime, parseErr := time.Parse("15:04", fireTimeStr)
	if parseErr != nil {
		logger.L().Warn("notification: skipping weekly entry with invalid time",
			"name", cfg.Name,
			"time", cfg.Time,
		)

		return
	}

	hour := parsedTime.Hour()
	minute := parsedTime.Minute()

	for _, weekdayStr := range cfg.Weekdays {
		weekday, weekdayErr := parseWeekday(weekdayStr)
		if weekdayErr != nil {
			continue // already validated by config; defensive guard
		}

		fireAt := nextWeeklyOccurrence(now, weekday, hour, minute, notif.timezone)

		notif.sched.Schedule(
			"scheduled:"+cfg.Name+":"+weekdayStr,
			fireAt,
			func() {
				notif.onScheduled(cfg, connectedIDs)
			},
			func(fired time.Time) time.Time {
				return nextWeeklyOccurrence(
					fired.In(notif.timezone),
					weekday,
					hour,
					minute,
					notif.timezone,
				)
			},
		)
	}
}

// onScheduled is called by the scheduler when an alarm fires. It builds the
// notification and publishes it to every currently connected client.
func (notif *ScheduledNotifier) onScheduled(
	cfg *config.ScheduledNotificationConfig,
	connectedIDs func() []string,
) {
	firedLocal := notif.clk.Now().In(notif.timezone)
	text := formatMessage(cfg, firedLocal)

	scheduledNotification := &model.Notification{
		AppContent: model.AppContent{
			Text:        model.NewPlainText(text),
			Icon:        cfg.Icon,
			Duration:    cfg.Duration,
			Rainbow:     *cfg.Rainbow,
			ScrollSpeed: cfg.ScrollSpeed,
		},
		Rtttl:  cfg.RTTTL,
		Wakeup: *cfg.Wakeup,
	}

	for _, clientID := range connectedIDs() {
		publishErr := notif.publish(clientID, scheduledNotification)
		if publishErr != nil {
			logger.L().Warn("notification: scheduled publish failed",
				"client_id", clientID,
				"name", cfg.Name,
				"err", publishErr,
			)
		}
	}
}

// resolveSchedule parses the notification config and returns the first fire
// time, a recurrence function, and whether parsing succeeded.
func (notif *ScheduledNotifier) resolveSchedule(
	cfg *config.ScheduledNotificationConfig,
	now time.Time,
) (time.Time, func(time.Time) time.Time, bool) {
	fireTimeStr := cfg.Time
	if fireTimeStr == "" {
		fireTimeStr = "00:00"
	}

	parsedTime, parseTimeErr := time.Parse("15:04", fireTimeStr)
	if parseTimeErr != nil {
		logger.L().Warn("notification: skipping entry with invalid time",
			"name", cfg.Name,
			"time", cfg.Time,
		)

		return time.Time{}, nil, false
	}

	hour := parsedTime.Hour()
	minute := parsedTime.Minute()

	switch cfg.Repeat {
	case "yearly":
		parsedDate, dateErr := time.Parse("01-02", cfg.Date)
		if dateErr != nil {
			logger.L().Warn("notification: skipping yearly entry with invalid date",
				"name", cfg.Name,
				"date", cfg.Date,
			)

			return time.Time{}, nil, false
		}

		month := parsedDate.Month()
		day := parsedDate.Day()
		fireAt := nextYearlyOccurrence(now, month, day, hour, minute, notif.timezone)

		return fireAt, func(from time.Time) time.Time {
			return nextYearlyOccurrence(from, month, day, hour, minute, notif.timezone)
		}, true

	case "monthly":
		day := cfg.Day
		if day < 1 || day > 31 {
			logger.L().Warn("notification: skipping monthly entry with invalid day",
				"name", cfg.Name,
				"day", day,
			)

			return time.Time{}, nil, false
		}

		fireAt := nextMonthlyOccurrence(now, day, hour, minute, notif.timezone)

		return fireAt, func(from time.Time) time.Time {
			return nextMonthlyOccurrence(from, day, hour, minute, notif.timezone)
		}, true

	case "daily":
		fireAt := nextDailyOccurrence(now, hour, minute, notif.timezone)

		return fireAt, func(from time.Time) time.Time {
			return nextDailyOccurrence(from, hour, minute, notif.timezone)
		}, true

	default:
		logger.L().Warn("notification: skipping entry with unknown repeat",
			"name", cfg.Name,
			"repeat", cfg.Repeat,
		)

		return time.Time{}, nil, false
	}
}

// formatMessage expands template tokens in cfg.Message and returns the result.
// Supported tokens: {name} → cfg.Name, {year} → fire-time year.
// Falls back to cfg.Name when Message is empty.
func formatMessage(cfg *config.ScheduledNotificationConfig, fireTime time.Time) string {
	text := cfg.Message
	if text == "" {
		text = cfg.Name
	}

	text = strings.ReplaceAll(text, "{name}", cfg.Name)
	text = strings.ReplaceAll(text, "{year}", strconv.Itoa(fireTime.Year()))

	return text
}

// nextYearlyOccurrence returns the next occurrence of month/day at HH:MM in loc
// that is strictly after from.
func nextYearlyOccurrence(
	from time.Time,
	month time.Month,
	day, hour, minute int,
	loc *time.Location,
) time.Time {
	localFrom := from.In(loc)
	candidate := time.Date(localFrom.Year(), month, day, hour, minute, 0, 0, loc)

	if !candidate.After(from) {
		candidate = time.Date(localFrom.Year()+1, month, day, hour, minute, 0, 0, loc)
	}

	return candidate
}

// nextMonthlyOccurrence returns the next occurrence of day-of-month at HH:MM
// in loc that is strictly after from.
func nextMonthlyOccurrence(from time.Time, day, hour, minute int, loc *time.Location) time.Time {
	localFrom := from.In(loc)
	candidate := time.Date(localFrom.Year(), localFrom.Month(), day, hour, minute, 0, 0, loc)

	if !candidate.After(from) {
		candidate = time.Date(localFrom.Year(), localFrom.Month()+1, day, hour, minute, 0, 0, loc)
	}

	return candidate
}

// nextWeeklyOccurrence returns the next occurrence of weekday at HH:MM in loc
// that is strictly after from.
func nextWeeklyOccurrence(
	from time.Time,
	weekday time.Weekday,
	hour, minute int,
	loc *time.Location,
) time.Time {
	localFrom := from.In(loc)
	daysUntil := int(weekday) - int(localFrom.Weekday())

	if daysUntil < 0 {
		daysUntil += daysInWeek
	}

	candidate := time.Date(
		localFrom.Year(), localFrom.Month(), localFrom.Day()+daysUntil,
		hour, minute, 0, 0, loc,
	)

	if !candidate.After(from) {
		candidate = candidate.AddDate(0, 0, daysInWeek)
	}

	return candidate
}

// nextDailyOccurrence returns the next occurrence of HH:MM in loc that is
// strictly after from.
func nextDailyOccurrence(from time.Time, hour, minute int, loc *time.Location) time.Time {
	localFrom := from.In(loc)
	candidate := time.Date(
		localFrom.Year(),
		localFrom.Month(),
		localFrom.Day(),
		hour,
		minute,
		0,
		0,
		loc,
	)

	if !candidate.After(from) {
		candidate = candidate.AddDate(0, 0, 1)
	}

	return candidate
}

// parseWeekday converts a weekday string (monday–sunday) to time.Weekday.
func parseWeekday(weekdayStr string) (time.Weekday, error) {
	switch strings.ToLower(weekdayStr) {
	case "sunday":
		return time.Sunday, nil
	case "monday":
		return time.Monday, nil
	case "tuesday":
		return time.Tuesday, nil
	case "wednesday":
		return time.Wednesday, nil
	case "thursday":
		return time.Thursday, nil
	case "friday":
		return time.Friday, nil
	case "saturday":
		return time.Saturday, nil
	default:
		return time.Sunday, errUnknownWeekday
	}
}
