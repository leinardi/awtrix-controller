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

package weather_test

import (
	"context"
	"errors"
	"math"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
	"github.com/leinardi/awtrix-controller/internal/weather"
)

// errNetworkFail is a sentinel error for simulating fetch failures in tests.
var errNetworkFail = errors.New("simulated network failure")

// ── timer factories used by controller tests ──────────────────────────────────

type controllerTimerHandle struct {
	mu      sync.Mutex
	fired   bool
	stopped bool
}

func (handle *controllerTimerHandle) Stop() bool {
	handle.mu.Lock()
	defer handle.mu.Unlock()

	if handle.fired {
		return false
	}

	handle.stopped = true

	return true
}

// immediateFactory fires all timers immediately (for testing the first poll).
func immediateFactory() scheduler.TimerFactory {
	return func(delay time.Duration, callback func()) scheduler.TimerHandle {
		handle := &controllerTimerHandle{}

		if delay <= 0 {
			handle.fired = true

			go callback()
		}

		return handle
	}
}

type capturingEntry struct {
	delay    time.Duration
	callback func()
}

// capturingFactory records each timer without firing it.
type capturingFactory struct {
	mu      sync.Mutex
	entries []capturingEntry
}

func (cf *capturingFactory) factory() scheduler.TimerFactory {
	return func(delay time.Duration, callback func()) scheduler.TimerHandle {
		cf.mu.Lock()
		cf.entries = append(cf.entries, capturingEntry{delay: delay, callback: callback})
		cf.mu.Unlock()

		return &controllerTimerHandle{}
	}
}

func (cf *capturingFactory) lastDelay() time.Duration {
	cf.mu.Lock()
	defer cf.mu.Unlock()

	if len(cf.entries) == 0 {
		return -1
	}

	return cf.entries[len(cf.entries)-1].delay
}

// ── helpers ───────────────────────────────────────────────────────────────────

func defaultCtrlCfg() config.WeatherConfig {
	return config.WeatherConfig{
		PollIntervalMinutes:       15,
		OverlayHorizonMinutes:     60,
		NotificationHorizonHours:  8,
		NotificationRepeatMinutes: 60,
		InactiveAfterMissingPolls: 2,
		NotificationTextRepeat:    3,
		GustWarnKmh:               45.0,
		GustSevereKmh:             60.0,
		HeavyRainMmPer15Min:       5.0,
		FogVisibilityWarnM:        1000.0,
		FogVisibilitySevereM:      200.0,
		FrostTempC:                2.0,
		FrostDewPointDeltaC:       2.0,
		NotifyThunderstorm:        boolPtr(true),
		NotifyFreezingPrecip:      boolPtr(true),
		NotifyFrostRisk:           boolPtr(true),
		NotifyHeavyRain:           boolPtr(true),
		NotifyStrongGusts:         boolPtr(true),
		NotifySnow:                boolPtr(true),
		NotifyFog:                 boolPtr(true),
	}
}

func defaultLoc() config.LocationConfig {
	lat := 48.0
	lon := 11.0

	return config.LocationConfig{Latitude: &lat, Longitude: &lon}
}

func makeThunderstormPoints(now time.Time) []weather.ForecastPoint {
	return []weather.ForecastPoint{
		{
			Time:          now,
			WeatherCode:   95,
			Precipitation: 2.0,
			Snowfall:      0,
			Temperature2m: 15,
			DewPoint2m:    10,
			WindGusts10m:  30,
			Visibility:    math.NaN(),
		},
	}
}

const conditionTimeout = 2 * time.Second

func waitForCondition(t *testing.T, condition func() bool) {
	t.Helper()

	deadline := time.Now().Add(conditionTimeout)

	for time.Now().Before(deadline) {
		if condition() {
			return
		}

		time.Sleep(time.Millisecond)
	}

	t.Fatal("condition not met within timeout")
}

// ── tests ─────────────────────────────────────────────────────────────────────

func TestControllerPollSuccess(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	sched := scheduler.NewWithFactory(ctx, fakeClock, immediateFactory())
	defer sched.Stop()

	var (
		overlayMu       sync.Mutex
		overlayReceived model.OverlayEffect
	)

	var notifCount atomic.Int32

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		return makeThunderstormPoints(now), nil
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(overlay model.OverlayEffect) {
			overlayMu.Lock()
			overlayReceived = overlay
			overlayMu.Unlock()
		},
		func(_ string, _ *model.Notification) error {
			notifCount.Add(1)

			return nil
		},
		func() []string { return []string{"device1"} },
	)

	ctrl.Start()

	waitForCondition(t, func() bool {
		overlayMu.Lock()
		defer overlayMu.Unlock()

		return overlayReceived == model.OverlayEffectThunder
	})

	overlayMu.Lock()
	gotOverlay := overlayReceived
	overlayMu.Unlock()

	if gotOverlay != model.OverlayEffectThunder {
		t.Errorf("overlay = %q, want %q", gotOverlay, model.OverlayEffectThunder)
	}

	waitForCondition(t, func() bool { return notifCount.Load() >= 1 })
}

func TestControllerPollFetchError(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	sched := scheduler.NewWithFactory(ctx, fakeClock, immediateFactory())
	defer sched.Stop()

	var (
		fetchCalled atomic.Bool
		notifCount  atomic.Int32
	)

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		fetchCalled.Store(true)

		return nil, errNetworkFail
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(_ model.OverlayEffect) {},
		func(_ string, _ *model.Notification) error {
			notifCount.Add(1)

			return nil
		},
		func() []string { return []string{"device1"} },
	)

	ctrl.Start()

	// Wait for the poll to have run (fetch was called).
	waitForCondition(t, fetchCalled.Load)

	// Give the goroutine time to complete.
	time.Sleep(10 * time.Millisecond)

	if notifCount.Load() != 0 {
		t.Errorf("notifCount = %d, want 0 on fetch error", notifCount.Load())
	}
}

func TestControllerOverlayDedup(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	var callCount atomic.Int32

	// Controllable factory: fire the first call immediately, queue subsequent.
	type timerEntry struct {
		handle   *controllerTimerHandle
		callback func()
	}

	var (
		timersMu   sync.Mutex
		timers     []timerEntry
		firstFired atomic.Bool
	)

	factory := func(delay time.Duration, callback func()) scheduler.TimerHandle {
		handle := &controllerTimerHandle{}

		timersMu.Lock()

		timers = append(timers, timerEntry{handle: handle, callback: callback})
		timersMu.Unlock()

		if !firstFired.Load() && delay <= 0 {
			firstFired.Store(true)

			handle.fired = true

			go callback()
		}

		return handle
	}

	sched := scheduler.NewWithFactory(ctx, fakeClock, factory)
	defer sched.Stop()

	var (
		overlayCalls atomic.Int32
		lastOverlay  model.OverlayEffect
		overlayMu    sync.Mutex
	)

	var pollCount atomic.Int32

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		pollCount.Add(1)

		return makeThunderstormPoints(now), nil
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(overlay model.OverlayEffect) {
			overlayMu.Lock()
			lastOverlay = overlay
			overlayMu.Unlock()
			overlayCalls.Add(1)
		},
		func(_ string, _ *model.Notification) error {
			callCount.Add(1)

			return nil
		},
		func() []string { return []string{"device1"} },
	)

	ctrl.Start()

	// Wait for first poll.
	waitForCondition(t, func() bool { return pollCount.Load() >= 1 })

	// Trigger a second poll manually.
	timersMu.Lock()

	var secondCallback func()

	if len(timers) >= 2 {
		secondCallback = timers[1].callback
	}

	timersMu.Unlock()

	if secondCallback != nil {
		secondCallback()
		waitForCondition(t, func() bool { return pollCount.Load() >= 2 })
	}

	// onOverlay should have been called exactly once (same overlay both times).
	overlayMu.Lock()
	gotOverlay := lastOverlay
	gotCalls := overlayCalls.Load()
	overlayMu.Unlock()

	if gotOverlay != model.OverlayEffectThunder {
		t.Errorf("overlay = %q, want %q", gotOverlay, model.OverlayEffectThunder)
	}

	if gotCalls != 1 {
		t.Errorf("onOverlay call count = %d, want 1 (dedup)", gotCalls)
	}
}

func TestControllerNotifySentToAllClients(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	sched := scheduler.NewWithFactory(ctx, fakeClock, immediateFactory())
	defer sched.Stop()

	var notifMu sync.Mutex

	notifiedClients := make(map[string]int)

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		return makeThunderstormPoints(now), nil
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(_ model.OverlayEffect) {},
		func(clientID string, _ *model.Notification) error {
			notifMu.Lock()
			notifiedClients[clientID]++
			notifMu.Unlock()

			return nil
		},
		func() []string { return []string{"device1", "device2"} },
	)

	ctrl.Start()

	waitForCondition(t, func() bool {
		notifMu.Lock()
		defer notifMu.Unlock()

		return notifiedClients["device1"] >= 1 && notifiedClients["device2"] >= 1
	})

	notifMu.Lock()
	defer notifMu.Unlock()

	if notifiedClients["device1"] < 1 {
		t.Error("device1 should have received notification")
	}

	if notifiedClients["device2"] < 1 {
		t.Error("device2 should have received notification")
	}
}

func TestControllerNoNotifyWhenDisabled(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	sched := scheduler.NewWithFactory(ctx, fakeClock, immediateFactory())
	defer sched.Stop()

	cfg := defaultCtrlCfg()
	cfg.NotifyThunderstorm = boolPtr(false)

	var (
		overlayCalled atomic.Bool
		notifCount    atomic.Int32
	)

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		return makeThunderstormPoints(now), nil
	}

	ctrl := weather.NewWithFetchFunc(
		cfg, defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(_ model.OverlayEffect) { overlayCalled.Store(true) },
		func(_ string, _ *model.Notification) error {
			notifCount.Add(1)

			return nil
		},
		func() []string { return []string{"device1"} },
	)

	ctrl.Start()

	// Wait for poll to complete via overlay callback.
	waitForCondition(t, overlayCalled.Load)

	if notifCount.Load() != 0 {
		t.Errorf("notifCount = %d, want 0 (thunderstorm notify disabled)", notifCount.Load())
	}
}

func TestControllerOnDeviceConnectedDelivers(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	sched := scheduler.NewWithFactory(ctx, fakeClock, immediateFactory())
	defer sched.Stop()

	var overlayChanged atomic.Bool

	var notifMu sync.Mutex

	notifiedClients := make(map[string]int)

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		return makeThunderstormPoints(now), nil
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(_ model.OverlayEffect) { overlayChanged.Store(true) },
		func(clientID string, _ *model.Notification) error {
			notifMu.Lock()
			notifiedClients[clientID]++
			notifMu.Unlock()

			return nil
		},
		func() []string { return nil }, // no clients connected during poll
	)

	ctrl.Start()

	// Wait for the poll to complete (overlay changes to thunder).
	waitForCondition(t, overlayChanged.Load)

	// No clients were connected during the poll — nothing published yet.
	notifMu.Lock()
	countBeforeConnect := len(notifiedClients)
	notifMu.Unlock()

	if countBeforeConnect != 0 {
		t.Fatalf("expected no notifications before connect, got %d", countBeforeConnect)
	}

	// Device connects within the repeat interval.
	ctrl.OnDeviceConnected("device1")

	waitForCondition(t, func() bool {
		notifMu.Lock()
		defer notifMu.Unlock()

		return notifiedClients["device1"] >= 1
	})

	notifMu.Lock()
	got := notifiedClients["device1"]
	notifMu.Unlock()

	if got < 1 {
		t.Errorf("device1 notifications = %d, want >= 1", got)
	}
}

func TestControllerOnDeviceConnectedStale(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	sched := scheduler.NewWithFactory(ctx, fakeClock, immediateFactory())
	defer sched.Stop()

	var overlayChanged atomic.Bool

	var notifCount atomic.Int32

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		return makeThunderstormPoints(now), nil
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(_ model.OverlayEffect) { overlayChanged.Store(true) },
		func(_ string, _ *model.Notification) error {
			notifCount.Add(1)

			return nil
		},
		func() []string { return nil }, // no clients during poll
	)

	ctrl.Start()

	// Wait for the poll to complete.
	waitForCondition(t, overlayChanged.Load)

	// Advance clock past the repeat interval (60 min).
	fakeClock.Advance(61 * time.Minute)

	// Device connects after cache is stale — should not publish.
	ctrl.OnDeviceConnected("device1")

	// Allow time for any spurious goroutine activity.
	time.Sleep(20 * time.Millisecond)

	if notifCount.Load() != 0 {
		t.Errorf("notifCount = %d, want 0 (cache is stale)", notifCount.Load())
	}
}

func TestControllerOnDeviceConnectedNoPendingEvents(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	sched := scheduler.NewWithFactory(ctx, fakeClock, immediateFactory())
	defer sched.Stop()

	var fetchCalled atomic.Bool

	var notifCount atomic.Int32

	// Return clear-sky points — no event candidates.
	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		fetchCalled.Store(true)

		return []weather.ForecastPoint{
			{
				Time:          now,
				WeatherCode:   0,
				Precipitation: 0,
				Snowfall:      0,
				Temperature2m: 20,
				DewPoint2m:    5,
				WindGusts10m:  10,
				Visibility:    10000,
			},
		}, nil
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(_ model.OverlayEffect) {},
		func(_ string, _ *model.Notification) error {
			notifCount.Add(1)

			return nil
		},
		func() []string { return nil },
	)

	ctrl.Start()

	// Wait for fetch to complete, then let the goroutine finish.
	waitForCondition(t, fetchCalled.Load)
	time.Sleep(20 * time.Millisecond)

	// No events detected — cache is empty.
	ctrl.OnDeviceConnected("device1")

	time.Sleep(20 * time.Millisecond)

	if notifCount.Load() != 0 {
		t.Errorf("notifCount = %d, want 0 (no pending events)", notifCount.Load())
	}
}

func TestControllerRescheduleInterval(t *testing.T) {
	t.Parallel()

	ctx := t.Context()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	fakeClock := clock.NewFakeClock(now)

	capturing := &capturingFactory{}

	sched := scheduler.NewWithFactory(ctx, fakeClock, capturing.factory())
	defer sched.Stop()

	fetchFn := func(_ context.Context, _, _ float64, _ string) ([]weather.ForecastPoint, error) {
		return nil, nil
	}

	ctrl := weather.NewWithFetchFunc(
		defaultCtrlCfg(), defaultLoc(), "UTC", fakeClock, sched,
		fetchFn,
		func(_ model.OverlayEffect) {},
		func(_ string, _ *model.Notification) error { return nil },
		func() []string { return nil },
	)

	ctrl.Start()

	// The first timer should be scheduled with delay = 0 (fire immediately).
	// The second timer (reschedule) should be ~15 min from the fired time.
	// We can't easily trigger the second schedule without a more complex setup,
	// but we can verify the first schedule fires at now (delay=0).
	wantDelay := time.Duration(0)
	gotDelay := capturing.lastDelay()

	if gotDelay != wantDelay {
		t.Errorf("first timer delay = %v, want %v", gotDelay, wantDelay)
	}
}
