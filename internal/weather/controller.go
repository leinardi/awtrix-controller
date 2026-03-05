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

package weather

import (
	"context"
	"sync"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
)

const (
	pollTimeout = 10 * time.Second

	severityColor = "#FFFF00" // yellow text for severe-level notifications

	iconThunderstorm   = "63084"
	iconFreezingPrecip = "60934"
	iconFrostRisk      = "43125"
	iconHeavyRain      = "22366"
	iconStrongGusts    = "3363"
	iconSnow           = "63083"
	iconFog            = "17056"
)

// FetchFunc is the fetch abstraction for test injection.
type FetchFunc func(ctx context.Context, lat, lon float64, timezone string) ([]ForecastPoint, error)

// Controller polls Open-Meteo, applies weather overlays, and publishes warning notifications.
type Controller struct {
	cfg          config.WeatherConfig
	latitude     float64
	longitude    float64
	timezone     string
	clk          clock.Clock
	sched        *scheduler.Scheduler
	fetchFn      FetchFunc
	stateManager *StateManager
	onOverlay    func(model.OverlayEffect)
	publishNotif func(clientID string, notif *model.Notification) error
	connectedIDs func() []string

	mu                     sync.Mutex
	lastFetchedAt          time.Time
	currentOverlay         model.OverlayEffect
	lastNotifiedCandidates []EventCandidate
	lastNotifiedAt         time.Time
}

// New wires up a production controller using the real HTTP fetcher.
//
//nolint:gocritic // hugeParam: cfg and loc are config value types copied once at construction
func New(
	cfg config.WeatherConfig,
	loc config.LocationConfig,
	timezone string,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	onOverlay func(model.OverlayEffect),
	publishNotif func(clientID string, notif *model.Notification) error,
	connectedIDs func() []string,
) *Controller {
	fetcher := NewFetcher()

	return NewWithFetchFunc(cfg, loc, timezone, clk, sched, fetcher.Fetch,
		onOverlay, publishNotif, connectedIDs)
}

// NewWithFetchFunc is the test constructor with injectable FetchFunc.
//
//nolint:gocritic // hugeParam: cfg and loc are config value types copied once at construction
func NewWithFetchFunc(
	cfg config.WeatherConfig,
	loc config.LocationConfig,
	timezone string,
	clk clock.Clock,
	sched *scheduler.Scheduler,
	fetchFn FetchFunc,
	onOverlay func(model.OverlayEffect),
	publishNotif func(clientID string, notif *model.Notification) error,
	connectedIDs func() []string,
) *Controller {
	if timezone == "" {
		logger.L().Warn("weather: no timezone configured; using UTC")

		timezone = "UTC"
	}

	return &Controller{
		cfg:          cfg,
		latitude:     *loc.Latitude,
		longitude:    *loc.Longitude,
		timezone:     timezone,
		clk:          clk,
		sched:        sched,
		fetchFn:      fetchFn,
		stateManager: NewStateManager(),
		onOverlay:    onOverlay,
		publishNotif: publishNotif,
		connectedIDs: connectedIDs,
	}
}

// Start schedules the first poll immediately and recurring polls at PollIntervalMinutes.
func (ctrl *Controller) Start() {
	logger.L().Info("weather: controller started",
		"lat", ctrl.latitude,
		"lon", ctrl.longitude,
		"timezone", ctrl.timezone,
		"poll_interval_minutes", ctrl.cfg.PollIntervalMinutes,
	)

	pollInterval := time.Duration(ctrl.cfg.PollIntervalMinutes) * time.Minute
	now := ctrl.clk.Now()

	ctrl.sched.Schedule(
		"weather:poll",
		now,
		ctrl.poll,
		func(fired time.Time) time.Time {
			return fired.Add(pollInterval)
		},
	)
}

// OnDeviceConnected delivers any recently-queued notifications to a newly
// connected device. It is a no-op when no notifications are pending or when
// the last batch is older than NotificationRepeatMinutes (the next scheduled
// poll will re-evaluate via the normal repeat path).
func (ctrl *Controller) OnDeviceConnected(clientID string) {
	repeatInterval := time.Duration(ctrl.cfg.NotificationRepeatMinutes) * time.Minute

	ctrl.mu.Lock()
	candidates := ctrl.lastNotifiedCandidates
	sentAt := ctrl.lastNotifiedAt
	ctrl.mu.Unlock()

	if len(candidates) == 0 {
		return
	}

	if ctrl.clk.Now().Sub(sentAt) >= repeatInterval {
		return
	}

	now := ctrl.clk.Now()

	for _, candidate := range candidates {
		publishErr := ctrl.publishCandidateTo(clientID, candidate, now)
		if publishErr != nil {
			logger.L().Warn("weather: publish notification to new client failed",
				"clientID", clientID,
				"event", candidate.Type,
				"err", publishErr,
			)
		}
	}
}

// poll is the recurring action that fetches forecast data, updates the overlay,
// and publishes any pending notifications.
func (ctrl *Controller) poll() {
	fetchCtx, cancel := context.WithTimeout(context.Background(), pollTimeout)
	defer cancel()

	fetchStart := ctrl.clk.Now()

	logger.L().Debug("weather: polling",
		"lat", ctrl.latitude,
		"lon", ctrl.longitude,
		"timezone", ctrl.timezone,
	)

	points, fetchErr := ctrl.fetchFn(fetchCtx, ctrl.latitude, ctrl.longitude, ctrl.timezone)
	if fetchErr != nil {
		logger.L().Warn("weather: fetch failed; clearing overlay", "err", fetchErr)
		ctrl.stateManager.OnFetchFailure()
		ctrl.applyOverlay("")

		return
	}

	now := ctrl.clk.Now()
	fetchLatency := now.Sub(fetchStart)

	ctrl.mu.Lock()
	ctrl.lastFetchedAt = now
	ctrl.mu.Unlock()

	logger.L().Debug("weather: forecast received", "points", len(points), "latency", fetchLatency)

	selected := SelectOverlay(points, now, ctrl.cfg.OverlayHorizonMinutes)

	logger.L().Debug("weather: overlay selected",
		"overlay", selected,
		"horizon_minutes", ctrl.cfg.OverlayHorizonMinutes,
	)

	ctrl.applyOverlay(selected)

	candidates := DetectEvents(points, now, ctrl.cfg)

	for _, candidate := range candidates {
		logger.L().Debug("weather: event candidate",
			"type", candidate.Type,
			"severity", candidate.Severity,
			"start_time", candidate.StartTime.Format("15:04"),
			"fingerprint", candidate.Fingerprint,
		)
	}

	repeatInterval := time.Duration(ctrl.cfg.NotificationRepeatMinutes) * time.Minute

	toNotify := ctrl.stateManager.Process(
		candidates,
		now,
		repeatInterval,
		ctrl.cfg.InactiveAfterMissingPolls,
	)

	if len(toNotify) > 0 {
		ctrl.mu.Lock()
		ctrl.lastNotifiedCandidates = toNotify
		ctrl.lastNotifiedAt = now
		ctrl.mu.Unlock()
	}

	logger.L().Debug("weather: poll complete",
		"latency", fetchLatency,
		"forecast_points", len(points),
		"overlay", selected,
		"candidates", len(candidates),
		"notifications_queued", len(toNotify),
	)

	for _, candidate := range toNotify {
		ctrl.publishCandidate(candidate, now)
	}
}

// applyOverlay calls onOverlay only when the overlay value changes (dedup).
func (ctrl *Controller) applyOverlay(overlay model.OverlayEffect) {
	ctrl.mu.Lock()
	changed := overlay != ctrl.currentOverlay
	ctrl.currentOverlay = overlay
	ctrl.mu.Unlock()

	logger.L().Debug("weather: overlay update", "overlay", overlay, "changed", changed)

	if changed {
		ctrl.onOverlay(overlay)
	}
}

// publishCandidate publishes a notification for a single event to all connected
// clients. It logs at Info when at least one client was notified, and at Debug
// when no clients were connected yet (deferred until OnDeviceConnected fires).
func (ctrl *Controller) publishCandidate(candidate EventCandidate, now time.Time) {
	loc, locErr := time.LoadLocation(ctrl.timezone)
	if locErr != nil {
		loc = time.UTC
	}

	startStr := candidate.StartTime.In(loc).Format("15:04")
	published := 0

	for _, clientID := range ctrl.connectedIDs() {
		publishErr := ctrl.publishCandidateTo(clientID, candidate, now)
		if publishErr != nil {
			logger.L().Warn("weather: publish notification failed",
				"clientID", clientID,
				"event", candidate.Type,
				"err", publishErr,
			)
		} else {
			published++
		}
	}

	if published > 0 {
		logger.L().Info("weather: notification sent",
			"event", candidate.Type,
			"severity", candidate.Severity,
			"startTime", startStr,
			"now", now,
			"clients_notified", published,
		)
	} else {
		logger.L().Debug("weather: notification deferred — no clients connected",
			"event", candidate.Type,
			"severity", candidate.Severity,
			"startTime", startStr,
		)
	}
}

// publishCandidateTo builds and sends one notification for a single client.
func (ctrl *Controller) publishCandidateTo(
	clientID string,
	candidate EventCandidate,
	_ time.Time,
) error {
	icon := ctrl.iconForEvent(candidate.Type)

	loc, locErr := time.LoadLocation(ctrl.timezone)
	if locErr != nil {
		loc = time.UTC
	}

	startStr := candidate.StartTime.In(loc).Format("15:04")

	var text string

	switch candidate.Type {
	case EventTypeThunderstorm:
		text = "Thunderstorm from " + startStr
	case EventTypeFreezingPrecip:
		text = "Freezing rain from " + startStr
	case EventTypeFrostRisk:
		text = "Frost risk from " + startStr
	case EventTypeHeavyRain:
		text = "Heavy rain from " + startStr
	case EventTypeStrongGusts:
		text = "Strong gusts from " + startStr
	case EventTypeSnow:
		text = "Snow from " + startStr
	case EventTypeFog:
		text = "Fog from " + startStr
	}

	repeatCount := ctrl.cfg.NotificationTextRepeat

	var textColor string
	if candidate.Severity == severitySevere {
		textColor = severityColor
	}

	notif := &model.Notification{
		AppContent: model.AppContent{
			Text:   model.NewPlainText(text),
			Icon:   icon,
			Color:  textColor,
			Repeat: &repeatCount,
		},
		Wakeup: true,
	}

	return ctrl.publishNotif(clientID, notif)
}

// iconForEvent returns the LaMetric icon ID string for the given EventType.
func (*Controller) iconForEvent(eventType EventType) string {
	switch eventType {
	case EventTypeThunderstorm:
		return iconThunderstorm
	case EventTypeFreezingPrecip:
		return iconFreezingPrecip
	case EventTypeFrostRisk:
		return iconFrostRisk
	case EventTypeHeavyRain:
		return iconHeavyRain
	case EventTypeStrongGusts:
		return iconStrongGusts
	case EventTypeSnow:
		return iconSnow
	case EventTypeFog:
		return iconFog
	}

	return ""
}
