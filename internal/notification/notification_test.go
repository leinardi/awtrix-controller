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

package notification_test

import (
	"sync"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/notification"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

// ── timer factories ──────────────────────────────────────────────────────────

type immediateFakeHandle struct {
	mu    sync.Mutex
	fired bool
}

func (handle *immediateFakeHandle) Stop() bool {
	handle.mu.Lock()
	defer handle.mu.Unlock()

	return !handle.fired
}

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

type controllableHandle struct {
	mu      sync.Mutex
	fired   bool
	stopped bool
}

func (handle *controllableHandle) Stop() bool {
	handle.mu.Lock()
	defer handle.mu.Unlock()

	if handle.fired {
		return false
	}

	handle.stopped = true

	return true
}

type controllableEntry struct {
	handle   *controllableHandle
	callback func()
}

type controllableFactory struct {
	mu     sync.Mutex
	timers []*controllableEntry
}

func makeControllableFactory() *controllableFactory {
	return &controllableFactory{}
}

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

// ── config helpers ────────────────────────────────────────────────────────────

func boolPtr(value bool) *bool { return &value }

// newBirthdayNotifConfig returns a yearly scheduled notification for a birthday.
// date is "MM-DD" format (e.g. "04-16").
func newBirthdayNotifConfig(date, name string) config.ScheduledNotificationConfig {
	return config.ScheduledNotificationConfig{
		Name:        name,
		Message:     "Happy Birthday {name}!",
		Repeat:      "yearly",
		Date:        date,
		Time:        "00:00",
		Duration:    600,
		Icon:        "14004",
		Rainbow:     boolPtr(true),
		ScrollSpeed: 50,
		Wakeup:      boolPtr(true),
		Enabled:     boolPtr(true),
	}
}

// newNewYearNotifConfig returns a yearly scheduled notification for New Year.
func newNewYearNotifConfig(enabled bool) config.ScheduledNotificationConfig {
	return config.ScheduledNotificationConfig{
		Name:        "New Year",
		Message:     "Happy New Year {year}!",
		Repeat:      "yearly",
		Date:        "01-01",
		Time:        "00:00",
		Duration:    600,
		Icon:        "5855",
		Rainbow:     boolPtr(true),
		ScrollSpeed: 50,
		Wakeup:      boolPtr(true),
		Enabled:     boolPtr(enabled),
	}
}

// ── publish capture helper ────────────────────────────────────────────────────

type publishRecord struct {
	clientID     string
	notification *model.Notification
}

type capturePublisher struct {
	mu      sync.Mutex
	records []publishRecord
}

func (pub *capturePublisher) publish(clientID string, notificationArg *model.Notification) error {
	pub.mu.Lock()
	defer pub.mu.Unlock()

	pub.records = append(pub.records, publishRecord{
		clientID:     clientID,
		notification: notificationArg,
	})

	return nil
}

func (pub *capturePublisher) all() []publishRecord {
	pub.mu.Lock()
	defer pub.mu.Unlock()

	result := make([]publishRecord, len(pub.records))
	copy(result, pub.records)

	return result
}

// newWeeklyNotifConfig returns a weekly scheduled notification for the given weekdays.
func newWeeklyNotifConfig(name string, weekdays []string) config.ScheduledNotificationConfig {
	return config.ScheduledNotificationConfig{
		Name:        name,
		Message:     name,
		Repeat:      "weekly",
		Weekdays:    weekdays,
		Time:        "10:00",
		Duration:    60,
		Icon:        "609",
		Rainbow:     boolPtr(false),
		ScrollSpeed: 50,
		Wakeup:      boolPtr(true),
		Enabled:     boolPtr(true),
	}
}

// ── tests ─────────────────────────────────────────────────────────────────────

// TestBirthdayFiresTC05 covers TC-05: the birthday alarm fires with the
// correct message. Alice (date 04-16) fires at midnight on 2024-04-16.
// Two connected clients both receive the notification.
func TestBirthdayFiresTC05(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	// One second before Alice's birthday.
	fakeClock := clock.NewFakeClock(time.Date(2024, time.April, 15, 23, 59, 59, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	clients := []string{"client1", "client2"}

	var waitGroup sync.WaitGroup
	waitGroup.Add(len(clients))

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())
	alice := newBirthdayNotifConfig("04-16", "Alice")

	notif := notification.NewScheduledNotifier(
		[]config.ScheduledNotificationConfig{alice},
		timezone,
		fakeClock,
		sched,
		func(clientID string, notificationArg *model.Notification) error {
			err := captured.publish(clientID, notificationArg)

			waitGroup.Done()

			return err
		},
	)

	notif.Start(func() []string { return clients })

	// Advance clock to birthday midnight and fire the alarm.
	fakeClock.Set(time.Date(2024, time.April, 16, 0, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no pending timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	records := captured.all()
	if len(records) != len(clients) {
		t.Fatalf("publish called %d times, want %d", len(records), len(clients))
	}

	wantText := "Happy Birthday Alice!"

	for _, record := range records {
		if record.notification.Text.Plain() != wantText {
			t.Errorf("notification.Text = %q, want %q", record.notification.Text.Plain(), wantText)
		}
	}
}

// TestBirthdayFiresInNextCalendarYearTC06 covers TC-06: when the application
// starts in December and the birthday is in March of the following year, the
// alarm fires in the next calendar year.
// Bob (date 03-03) starts in December 2024 → fires 2025-03-03.
func TestBirthdayFiresInNextCalendarYearTC06(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 1, 0, 0, 0, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	var waitGroup sync.WaitGroup
	waitGroup.Add(1)

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())
	bob := newBirthdayNotifConfig("03-03", "Bob")

	notif := notification.NewScheduledNotifier(
		[]config.ScheduledNotificationConfig{bob},
		timezone,
		fakeClock,
		sched,
		func(clientID string, notificationArg *model.Notification) error {
			err := captured.publish(clientID, notificationArg)

			waitGroup.Done()

			return err
		},
	)

	notif.Start(func() []string { return []string{"device1"} })

	// Advance clock to Bob's birthday in 2025.
	fakeClock.Set(time.Date(2025, time.March, 3, 0, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no pending timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	records := captured.all()
	if len(records) != 1 {
		t.Fatalf("publish called %d times, want 1", len(records))
	}

	wantText := "Happy Birthday Bob!"
	if records[0].notification.Text.Plain() != wantText {
		t.Errorf("notification.Text = %q, want %q", records[0].notification.Text.Plain(), wantText)
	}
}

// TestNewYearNotificationTC11TC12 covers TC-11 (New Year fires at midnight
// January 1) and TC-12 (the year is computed at fire time, not startup time).
// Clock at 2024-12-31 23:59:59 → fires 2025-01-01 00:00:00 →
// text "Happy New Year 2025!".
func TestNewYearNotificationTC11TC12(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 31, 23, 59, 59, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	var waitGroup sync.WaitGroup
	waitGroup.Add(1)

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	notif := notification.NewScheduledNotifier(
		[]config.ScheduledNotificationConfig{newNewYearNotifConfig(true)},
		timezone,
		fakeClock,
		sched,
		func(clientID string, notificationArg *model.Notification) error {
			err := captured.publish(clientID, notificationArg)

			waitGroup.Done()

			return err
		},
	)

	notif.Start(func() []string { return []string{"device1"} })

	// Advance clock to the New Year moment.
	fakeClock.Set(time.Date(2025, time.January, 1, 0, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no pending timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	records := captured.all()
	if len(records) != 1 {
		t.Fatalf("publish called %d times, want 1", len(records))
	}

	wantText := "Happy New Year 2025!"
	if records[0].notification.Text.Plain() != wantText {
		t.Errorf("notification.Text = %q, want %q", records[0].notification.Text.Plain(), wantText)
	}
}

// TestScheduledNotificationDisabledTC13 covers TC-13: when enabled is false no
// alarm is scheduled and publish is never called.
func TestScheduledNotificationDisabledTC13(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 31, 23, 59, 59, 0, timezone))
	ctx := t.Context()

	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	captured := &capturePublisher{}

	notif := notification.NewScheduledNotifier(
		[]config.ScheduledNotificationConfig{newNewYearNotifConfig(false)},
		timezone,
		fakeClock,
		sched,
		captured.publish,
	)

	notif.Start(func() []string { return []string{"device1"} })

	sched.Stop()

	if records := captured.all(); len(records) != 0 {
		t.Errorf("publish called %d times, want 0 (entry disabled)", len(records))
	}
}

// TestCustomMessageAndIconTC14 covers TC-14: a configured message and icon are
// used verbatim instead of the auto-generated text.
func TestCustomMessageAndIconTC14(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 31, 23, 59, 59, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	var waitGroup sync.WaitGroup
	waitGroup.Add(1)

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	customCfg := newNewYearNotifConfig(true)
	customCfg.Message = "Bonne Année!"
	customCfg.Icon = "9999"

	notif := notification.NewScheduledNotifier(
		[]config.ScheduledNotificationConfig{customCfg},
		timezone,
		fakeClock,
		sched,
		func(clientID string, notificationArg *model.Notification) error {
			err := captured.publish(clientID, notificationArg)

			waitGroup.Done()

			return err
		},
	)

	notif.Start(func() []string { return []string{"device1"} })

	fakeClock.Set(time.Date(2025, time.January, 1, 0, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no pending timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	records := captured.all()
	if len(records) != 1 {
		t.Fatalf("publish called %d times, want 1", len(records))
	}

	if records[0].notification.Text.Plain() != "Bonne Année!" {
		t.Errorf("Text = %q, want %q", records[0].notification.Text.Plain(), "Bonne Année!")
	}

	if records[0].notification.Icon != "9999" {
		t.Errorf("Icon = %q, want %q", records[0].notification.Icon, "9999")
	}
}

// TestTwoEntriesInOneNotifierTC15 covers TC-15: when two entries share a
// single ScheduledNotifier both alarms fire independently as two separate
// publish calls.
func TestTwoEntriesInOneNotifierTC15(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 31, 23, 59, 59, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	// 2 alarms × 1 client = 2 publish calls.
	var waitGroup sync.WaitGroup
	waitGroup.Add(2)

	captured := &capturePublisher{}
	publishFn := func(clientID string, notificationArg *model.Notification) error {
		err := captured.publish(clientID, notificationArg)

		waitGroup.Done()

		return err
	}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())
	connectedIDs := func() []string { return []string{"device1"} }

	// Alice born January 1 — birthday fires at the same time as New Year.
	alice := newBirthdayNotifConfig("01-01", "Alice")

	entries := []config.ScheduledNotificationConfig{
		alice,
		newNewYearNotifConfig(true),
	}

	notif := notification.NewScheduledNotifier(entries, timezone, fakeClock, sched, publishFn)
	notif.Start(connectedIDs)

	// Advance to Jan 1 midnight.
	fakeClock.Set(time.Date(2025, time.January, 1, 0, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no first timer to trigger")
	}

	if !factory.TriggerNext() {
		t.Fatal("no second timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	records := captured.all()
	if len(records) != 2 {
		t.Fatalf("publish called %d times, want 2 (birthday + new year)", len(records))
	}

	seenTexts := map[string]bool{}
	for _, record := range records {
		seenTexts[record.notification.Text.Plain()] = true
	}

	if !seenTexts["Happy Birthday Alice!"] {
		t.Errorf("missing birthday notification; got texts: %v", seenTexts)
	}

	if !seenTexts["Happy New Year 2025!"] {
		t.Errorf("missing new year notification; got texts: %v", seenTexts)
	}
}

// TestWeeklyMultipleWeekdaysFireOnEachDay verifies that a weekly entry with
// multiple weekdays schedules one independent alarm per weekday.
// Clock is set to Sunday 2024-12-29 so Monday=+1d, Friday=+5d are both in the future.
func TestWeeklyMultipleWeekdaysFireOnEachDay(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	// Sunday 2024-12-29 09:00 — before the 10:00 fire time.
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 29, 9, 0, 0, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	// Two weekdays → two alarms → two publish calls (one client).
	var waitGroup sync.WaitGroup
	waitGroup.Add(2)

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	standup := newWeeklyNotifConfig("Standup", []string{"monday", "friday"})

	notif := notification.NewScheduledNotifier(
		[]config.ScheduledNotificationConfig{standup},
		timezone,
		fakeClock,
		sched,
		func(clientID string, notificationArg *model.Notification) error {
			err := captured.publish(clientID, notificationArg)

			waitGroup.Done()

			return err
		},
	)

	notif.Start(func() []string { return []string{"device1"} })

	// Fire Monday alarm (2024-12-30 10:00).
	fakeClock.Set(time.Date(2024, time.December, 30, 10, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no Monday timer to trigger")
	}

	// Fire Friday alarm (2025-01-03 10:00).
	fakeClock.Set(time.Date(2025, time.January, 3, 10, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no Friday timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	records := captured.all()
	if len(records) != 2 {
		t.Fatalf("publish called %d times, want 2 (monday + friday)", len(records))
	}
}

// TestWeeklySingleWeekdayStillWorks verifies that a single-element weekdays
// slice works identically to the old single-weekday case.
func TestWeeklySingleWeekdayStillWorks(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	// Saturday 2024-12-28, so next Monday is 2024-12-30.
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 28, 9, 0, 0, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	var waitGroup sync.WaitGroup
	waitGroup.Add(1)

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())

	standup := newWeeklyNotifConfig("Standup", []string{"monday"})

	notif := notification.NewScheduledNotifier(
		[]config.ScheduledNotificationConfig{standup},
		timezone,
		fakeClock,
		sched,
		func(clientID string, notificationArg *model.Notification) error {
			err := captured.publish(clientID, notificationArg)

			waitGroup.Done()

			return err
		},
	)

	notif.Start(func() []string { return []string{"device1"} })

	fakeClock.Set(time.Date(2024, time.December, 30, 10, 0, 0, 0, timezone))

	if !factory.TriggerNext() {
		t.Fatal("no Monday timer to trigger")
	}

	waitGroup.Wait()
	sched.Stop()

	records := captured.all()
	if len(records) != 1 {
		t.Fatalf("publish called %d times, want 1", len(records))
	}

	if records[0].notification.Text.Plain() != "Standup" {
		t.Errorf("Text = %q, want %q", records[0].notification.Text.Plain(), "Standup")
	}
}
