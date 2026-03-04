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

func newBirthdayConfig(dateOfBirth, name string) config.BirthdayConfig {
	return config.BirthdayConfig{
		DateOfBirth: dateOfBirth,
		Name:        name,
		Duration:    600,
		Icon:        "14004",
		Rainbow:     boolPtr(true),
		ScrollSpeed: 50,
		Wakeup:      boolPtr(true),
	}
}

func newNewYearConfig(enabled bool) config.NewYearConfig {
	return config.NewYearConfig{
		Enabled:     boolPtr(enabled),
		Icon:        "5855",
		Duration:    600,
		Rainbow:     boolPtr(true),
		ScrollSpeed: 50,
		Wakeup:      boolPtr(true),
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

// ── tests ─────────────────────────────────────────────────────────────────────

// TestBirthdayFiresWithCorrectAgeTC05 covers TC-05: the birthday alarm fires
// with the correct age. Alice (born 1990-04-16) turns 34 in 2024.
// Two connected clients both receive the notification.
func TestBirthdayFiresWithCorrectAgeTC05(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	// One second before Alice's 34th birthday.
	fakeClock := clock.NewFakeClock(time.Date(2024, time.April, 15, 23, 59, 59, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	clients := []string{"client1", "client2"}

	var waitGroup sync.WaitGroup
	waitGroup.Add(len(clients))

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())
	alice := newBirthdayConfig("1990-04-16", "Alice")

	notif := notification.NewBirthdayNotifier(
		[]config.BirthdayConfig{alice},
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

	wantText := "Happy 34 Birthday Alice!"

	for _, record := range records {
		if record.notification.Text != wantText {
			t.Errorf("notification.Text = %q, want %q", record.notification.Text, wantText)
		}
	}
}

// TestBirthdayAgeComputedAtFireTimeTC06 covers TC-06: when the application
// starts in December of year N and the birthday is in March of year N+1, the
// age is computed at fire time (N+1 − birth_year).
// Bob (born 1990-03-03) starts in December 2024 → fires 2025-03-03 → age 35.
func TestBirthdayAgeComputedAtFireTimeTC06(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 1, 0, 0, 0, 0, timezone))
	ctx := t.Context()

	factory := makeControllableFactory()

	var waitGroup sync.WaitGroup
	waitGroup.Add(1)

	captured := &capturePublisher{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory.Factory())
	bob := newBirthdayConfig("1990-03-03", "Bob")

	notif := notification.NewBirthdayNotifier(
		[]config.BirthdayConfig{bob},
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

	// Advance clock to Bob's 35th birthday in 2025.
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

	wantText := "Happy 35 Birthday Bob!"
	if records[0].notification.Text != wantText {
		t.Errorf("notification.Text = %q, want %q", records[0].notification.Text, wantText)
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

	notif := notification.NewNewYearNotifier(
		newNewYearConfig(true),
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
	if records[0].notification.Text != wantText {
		t.Errorf("notification.Text = %q, want %q", records[0].notification.Text, wantText)
	}
}

// TestNewYearDisabledTC13 covers TC-13: when new_year.enabled is false no
// alarm is scheduled and publish is never called.
func TestNewYearDisabledTC13(t *testing.T) {
	t.Parallel()

	timezone := time.UTC
	fakeClock := clock.NewFakeClock(time.Date(2024, time.December, 31, 23, 59, 59, 0, timezone))
	ctx := t.Context()

	sched := scheduler.NewWithFactory(ctx, fakeClock, makeFakeFactory())

	captured := &capturePublisher{}

	notif := notification.NewNewYearNotifier(
		newNewYearConfig(false),
		timezone,
		fakeClock,
		sched,
		captured.publish,
	)

	notif.Start(func() []string { return []string{"device1"} })

	sched.Stop()

	if records := captured.all(); len(records) != 0 {
		t.Errorf("publish called %d times, want 0 (new_year disabled)", len(records))
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

	newYearCfg := newNewYearConfig(true)
	newYearCfg.Message = "Bonne Année!"
	newYearCfg.Icon = "9999"

	notif := notification.NewNewYearNotifier(
		newYearCfg,
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

	if records[0].notification.Text != "Bonne Année!" {
		t.Errorf("Text = %q, want %q", records[0].notification.Text, "Bonne Année!")
	}

	if records[0].notification.Icon != "9999" {
		t.Errorf("Icon = %q, want %q", records[0].notification.Icon, "9999")
	}
}

// TestJan1BirthdayAndNewYearTC15 covers TC-15: when a birthday falls on
// January 1st both the birthday alarm and the New Year alarm fire
// independently as two separate publish calls.
func TestJan1BirthdayAndNewYearTC15(t *testing.T) {
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

	// Alice born January 1, 1990 — turns 35 in 2025.
	alice := newBirthdayConfig("1990-01-01", "Alice")

	birthdayNotif := notification.NewBirthdayNotifier(
		[]config.BirthdayConfig{alice},
		timezone,
		fakeClock,
		sched,
		publishFn,
	)

	newYearNotif := notification.NewNewYearNotifier(
		newNewYearConfig(true),
		timezone,
		fakeClock,
		sched,
		publishFn,
	)

	birthdayNotif.Start(connectedIDs)
	newYearNotif.Start(connectedIDs)

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
		seenTexts[record.notification.Text] = true
	}

	if !seenTexts["Happy 35 Birthday Alice!"] {
		t.Errorf("missing birthday notification; got texts: %v", seenTexts)
	}

	if !seenTexts["Happy New Year 2025!"] {
		t.Errorf("missing new year notification; got texts: %v", seenTexts)
	}
}
