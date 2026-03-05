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
	"fmt"
	"math"
	"time"

	"github.com/leinardi/awtrix-controller/internal/config"
)

const (
	snowfallHeavyThresholdCm    = 0.5
	fingerprintRoundingDuration = 30 * time.Minute

	// WMO code groups.
	wmoThunderstorm     = 95
	wmoThunderstormSev2 = 96
	wmoThunderstormSev3 = 99

	wmoFreezingDrizzleLight = 56
	wmoFreezingDrizzleHeavy = 57
	wmoFreezingRainLight    = 66
	wmoFreezingRainHeavy    = 67

	wmoDrizzleLight    = 51
	wmoDrizzleModerate = 53
	wmoDrizzleHeavy    = 55

	wmoRainLight    = 61
	wmoRainModerate = 63
	wmoRainHeavy    = 65

	wmoRainShowerSlight   = 80
	wmoRainShowerModerate = 81

	wmoSnowLight  = 71
	wmoSnowMod    = 73
	wmoSnowHeavy  = 75
	wmoSnowGrains = 77

	wmoSnowShowerLight = 85
	wmoSnowShowerHeavy = 86

	wmoStormHeavyRain = 82

	wmoFogLight = 45
	wmoFogRime  = 48

	wmoClearSky     = 0
	wmoMainlyClear  = 1
	wmoPartlyCloudy = 2
	wmoOvercast     = 3

	severityWarning = 1
	severitySevere  = 2

	// freezingPrecipTemp is the temperature ceiling (°C) for the numeric severe gate.
	// Set slightly above 0 to account for road surfaces being colder than the 2 m air temp.
	freezingPrecipTemp = 0.5

	// frostSeverePrecipMm is the minimum precipitation (mm/15 min) required alongside a
	// low temperature to trigger a severe frost-risk alert via the numeric gate.
	// It acts as a noise filter for near-zero API values.
	frostSeverePrecipMm = 0.1
)

// EventCandidate describes one detected warning in the forecast horizon.
type EventCandidate struct {
	Type        EventType
	StartTime   time.Time
	Severity    int    // 1=warning, 2=severe
	Fingerprint string // "type:roundedStart30min:severity"
}

// DetectEvents returns one EventCandidate per EventType found in
// [now, now+cfg.NotificationHorizonHours], respecting cfg.Notify* flags.
//
//nolint:gocritic // hugeParam: cfg is a config value type copied once at the API boundary
func DetectEvents(
	points []ForecastPoint,
	now time.Time,
	cfg config.WeatherConfig,
) []EventCandidate {
	horizon := now.Add(time.Duration(cfg.NotificationHorizonHours) * time.Hour)

	// Collect first qualifying point per event type.
	firsts := make(map[EventType]ForecastPoint)
	severities := make(map[EventType]int)

	for pointIdx := range points {
		if points[pointIdx].Time.Before(now) || points[pointIdx].Time.After(horizon) {
			continue
		}

		checkPoint(pointIdx, points, &cfg, firsts, severities)
	}

	candidates := make([]EventCandidate, 0, len(firsts))

	for _, eventType := range allEventTypes {
		if !isNotifyEnabled(eventType, &cfg) {
			continue
		}

		firstPoint, found := firsts[eventType]
		if !found {
			continue
		}

		severity := severities[eventType]
		rounded := firstPoint.Time.Truncate(fingerprintRoundingDuration)
		fingerprint := fmt.Sprintf("%d:%d:%d", int(eventType), rounded.Unix(), severity)

		candidates = append(candidates, EventCandidate{
			Type:        eventType,
			StartTime:   firstPoint.Time,
			Severity:    severity,
			Fingerprint: fingerprint,
		})
	}

	return candidates
}

// checkPoint evaluates a ForecastPoint for all event types and updates firsts/severities.
// pointIdx and points are forwarded to checkers that need look-forward context.
func checkPoint(
	pointIdx int,
	points []ForecastPoint,
	cfg *config.WeatherConfig,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	point := &points[pointIdx]
	checkThunderstorm(point, firsts, severities)
	checkFreezingPrecip(point, firsts, severities)
	checkFrostRisk(pointIdx, points, cfg, firsts, severities)
	checkHeavyRain(point, cfg, firsts, severities)
	checkStrongGusts(point, cfg, firsts, severities)
	checkSnow(point, firsts, severities)
	checkFog(point, cfg, firsts, severities)
}

func recordEvent(
	eventType EventType,
	point *ForecastPoint,
	severity int,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	existing, found := firsts[eventType]
	if !found {
		firsts[eventType] = *point
		severities[eventType] = severity

		return
	}

	// Keep earliest start; escalate severity.
	if point.Time.Before(existing.Time) {
		firsts[eventType] = *point
	}

	if severity > severities[eventType] {
		severities[eventType] = severity
	}
}

func checkThunderstorm(
	point *ForecastPoint,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	switch point.WeatherCode {
	case wmoThunderstorm:
		recordEvent(EventTypeThunderstorm, point, severityWarning, firsts, severities)
	case wmoThunderstormSev2, wmoThunderstormSev3:
		recordEvent(EventTypeThunderstorm, point, severitySevere, firsts, severities)
	}
}

func checkFreezingPrecip(
	point *ForecastPoint,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	switch point.WeatherCode {
	case wmoFreezingDrizzleLight,
		wmoFreezingDrizzleHeavy,
		wmoFreezingRainLight,
		wmoFreezingRainHeavy:
		recordEvent(EventTypeFreezingPrecip, point, severitySevere, firsts, severities)
	}
}

func checkFrostRisk(
	pointIdx int,
	points []ForecastPoint,
	cfg *config.WeatherConfig,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	point := &points[pointIdx]

	// Severe: freezing-precip WMO code (precip field may be 0 due to API binning)
	// OR numeric precip above noise threshold at near-freezing temperature.
	if isFreezingPrecipWMOCode(point.WeatherCode) ||
		(point.Temperature2m <= freezingPrecipTemp && point.Precipitation >= frostSeverePrecipMm) {
		recordEvent(EventTypeFrostRisk, point, severitySevere, firsts, severities)

		return
	}

	// Warning: temperature and dew-point spread indicate frost potential,
	// but only when a wet surface is plausible (precipitation within look-forward window).
	// dewDelta can be negative due to data quirks; that still satisfies the <= test.
	dewDelta := point.Temperature2m - point.DewPoint2m

	if point.Temperature2m <= cfg.FrostTempC && dewDelta <= cfg.FrostDewPointDeltaC {
		windowDur := time.Duration(float64(time.Hour) * cfg.FrostWarnPrecipWindowH)

		if hasPrecipInWindow(pointIdx, points, windowDur, cfg.FrostWarnPrecipMm) {
			recordEvent(EventTypeFrostRisk, point, severityWarning, firsts, severities)
		}
	}
}

// isFreezingPrecipWMOCode reports whether code is a freezing drizzle or freezing rain WMO code.
func isFreezingPrecipWMOCode(code int) bool {
	switch code {
	case wmoFreezingDrizzleLight, wmoFreezingDrizzleHeavy,
		wmoFreezingRainLight, wmoFreezingRainHeavy:
		return true
	default:
		return false
	}
}

// hasPrecipInWindow returns true if any point in [points[pointIdx].Time,
// points[pointIdx].Time+windowDur] has Precipitation >= minPrecipMm.
func hasPrecipInWindow(
	pointIdx int,
	points []ForecastPoint,
	windowDur time.Duration,
	minPrecipMm float64,
) bool {
	origin := points[pointIdx].Time
	cutoff := origin.Add(windowDur)

	for scanIdx := range points {
		scanTime := points[scanIdx].Time
		if scanTime.Before(origin) || scanTime.After(cutoff) {
			continue
		}

		if points[scanIdx].Precipitation >= minPrecipMm {
			return true
		}
	}

	return false
}

func checkHeavyRain(
	point *ForecastPoint,
	cfg *config.WeatherConfig,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	if point.WeatherCode == wmoStormHeavyRain {
		recordEvent(EventTypeHeavyRain, point, severitySevere, firsts, severities)

		return
	}

	if point.Precipitation > cfg.HeavyRainMmPer15Min {
		recordEvent(EventTypeHeavyRain, point, severityWarning, firsts, severities)
	}
}

func checkStrongGusts(
	point *ForecastPoint,
	cfg *config.WeatherConfig,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	if point.WindGusts10m > cfg.GustSevereKmh {
		recordEvent(EventTypeStrongGusts, point, severitySevere, firsts, severities)

		return
	}

	if point.WindGusts10m > cfg.GustWarnKmh {
		recordEvent(EventTypeStrongGusts, point, severityWarning, firsts, severities)
	}
}

func checkSnow(
	point *ForecastPoint,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	switch point.WeatherCode {
	case wmoSnowLight,
		wmoSnowMod,
		wmoSnowHeavy,
		wmoSnowGrains,
		wmoSnowShowerLight,
		wmoSnowShowerHeavy:
		severity := severityWarning
		if point.Snowfall > snowfallHeavyThresholdCm {
			severity = severitySevere
		}

		recordEvent(EventTypeSnow, point, severity, firsts, severities)
	}
}

func checkFog(
	point *ForecastPoint,
	cfg *config.WeatherConfig,
	firsts map[EventType]ForecastPoint,
	severities map[EventType]int,
) {
	switch point.WeatherCode {
	case wmoFogLight, wmoFogRime:
		severity := severityWarning

		if !math.IsNaN(point.Visibility) {
			if point.Visibility < cfg.FogVisibilitySevereM {
				severity = severitySevere
			} else if point.Visibility < cfg.FogVisibilityWarnM {
				severity = severityWarning
			}
		}

		recordEvent(EventTypeFog, point, severity, firsts, severities)
	}
}

// isNotifyEnabled returns whether the cfg flag for the given event type is true.
// A nil pointer is treated as false (safety default).
func isNotifyEnabled(eventType EventType, cfg *config.WeatherConfig) bool {
	switch eventType {
	case EventTypeThunderstorm:
		return cfg.NotifyThunderstorm != nil && *cfg.NotifyThunderstorm
	case EventTypeFreezingPrecip:
		return cfg.NotifyFreezingPrecip != nil && *cfg.NotifyFreezingPrecip
	case EventTypeFrostRisk:
		return cfg.NotifyFrostRisk != nil && *cfg.NotifyFrostRisk
	case EventTypeHeavyRain:
		return cfg.NotifyHeavyRain != nil && *cfg.NotifyHeavyRain
	case EventTypeStrongGusts:
		return cfg.NotifyStrongGusts != nil && *cfg.NotifyStrongGusts
	case EventTypeSnow:
		return cfg.NotifySnow != nil && *cfg.NotifySnow
	case EventTypeFog:
		return cfg.NotifyFog != nil && *cfg.NotifyFog
	}

	return false
}
