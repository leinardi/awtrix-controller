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
	"math"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/weather"
)

func boolPtr(b bool) *bool { return &b }

func defaultWeatherCfg() config.WeatherConfig {
	return config.WeatherConfig{
		NotificationHorizonHours: 8,
		GustWarnKmh:              45.0,
		GustSevereKmh:            60.0,
		HeavyRainMmPer15Min:      5.0,
		FogVisibilityWarnM:       1000.0,
		FogVisibilitySevereM:     200.0,
		FrostTempC:               2.0,
		FrostDewPointDeltaC:      2.0,
		NotifyThunderstorm:       boolPtr(true),
		NotifyFreezingPrecip:     boolPtr(true),
		NotifyFrostRisk:          boolPtr(true),
		NotifyHeavyRain:          boolPtr(true),
		NotifyStrongGusts:        boolPtr(true),
		NotifySnow:               boolPtr(true),
		NotifyFog:                boolPtr(true),
	}
}

func makePoint(
	now time.Time,
	offsetMin int,
	wmo int,
	precip, snowfall, temp, dewPoint, gusts, visibility float64,
) weather.ForecastPoint {
	return weather.ForecastPoint{
		Time:          now.Add(time.Duration(offsetMin) * time.Minute),
		WeatherCode:   wmo,
		Precipitation: precip,
		Snowfall:      snowfall,
		Temperature2m: temp,
		DewPoint2m:    dewPoint,
		WindGusts10m:  gusts,
		Visibility:    visibility,
	}
}

func findEvent(
	candidates []weather.EventCandidate,
	eventType weather.EventType,
) (weather.EventCandidate, bool) {
	for _, candidate := range candidates {
		if candidate.Type == eventType {
			return candidate, true
		}
	}

	return weather.EventCandidate{}, false
}

func TestDetectEventsThunderstorm(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	t.Run("severity 1 for WMO 95", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 95, 0, 0, 10, 5, 20, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeThunderstorm)

		if !found {
			t.Fatal("expected EventTypeThunderstorm")
		}

		if candidate.Severity != 1 {
			t.Errorf("Severity = %d, want 1", candidate.Severity)
		}
	})

	t.Run("severity 2 for WMO 96", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 96, 0, 0, 10, 5, 20, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeThunderstorm)

		if !found {
			t.Fatal("expected EventTypeThunderstorm")
		}

		if candidate.Severity != 2 {
			t.Errorf("Severity = %d, want 2", candidate.Severity)
		}
	})
}

func TestDetectEventsFreezingPrecip(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	points := []weather.ForecastPoint{makePoint(now, 0, 56, 1.0, 0, -1, -3, 10, math.NaN())}

	candidates := weather.DetectEvents(points, now, cfg)
	candidate, found := findEvent(candidates, weather.EventTypeFreezingPrecip)

	if !found {
		t.Fatal("expected EventTypeFreezingPrecip")
	}

	if candidate.Severity != 2 {
		t.Errorf("Severity = %d, want 2", candidate.Severity)
	}
}

func TestDetectEventsFrostRisk(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	t.Run("sev2 when temp<=0 and precip>0", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 0, 0.5, 0, -0.5, -2, 5, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeFrostRisk)

		if !found {
			t.Fatal("expected EventTypeFrostRisk")
		}

		if candidate.Severity != 2 {
			t.Errorf("Severity = %d, want 2", candidate.Severity)
		}
	})

	t.Run("sev1 when temp<=FrostTempC and small dewpoint delta", func(t *testing.T) {
		t.Parallel()

		// temp=1.5 <= FrostTempC=2.0; delta = 1.5 - 0.5 = 1.0 <= FrostDewPointDeltaC=2.0
		points := []weather.ForecastPoint{makePoint(now, 0, 0, 0, 0, 1.5, 0.5, 5, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeFrostRisk)

		if !found {
			t.Fatal("expected EventTypeFrostRisk")
		}

		if candidate.Severity != 1 {
			t.Errorf("Severity = %d, want 1", candidate.Severity)
		}
	})
}

func TestDetectEventsHeavyRain(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	t.Run("sev2 for WMO 82", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 82, 8.0, 0, 15, 10, 30, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeHeavyRain)

		if !found {
			t.Fatal("expected EventTypeHeavyRain")
		}

		if candidate.Severity != 2 {
			t.Errorf("Severity = %d, want 2", candidate.Severity)
		}
	})

	t.Run("sev1 for precip above threshold", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 61, 6.0, 0, 15, 10, 30, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeHeavyRain)

		if !found {
			t.Fatal("expected EventTypeHeavyRain")
		}

		if candidate.Severity != 1 {
			t.Errorf("Severity = %d, want 1", candidate.Severity)
		}
	})
}

func TestDetectEventsStrongGusts(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	t.Run("sev2 above severe threshold", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 0, 0, 0, 10, 5, 65.0, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeStrongGusts)

		if !found {
			t.Fatal("expected EventTypeStrongGusts")
		}

		if candidate.Severity != 2 {
			t.Errorf("Severity = %d, want 2", candidate.Severity)
		}
	})

	t.Run("sev1 between warn and severe", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 0, 0, 0, 10, 5, 50.0, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeStrongGusts)

		if !found {
			t.Fatal("expected EventTypeStrongGusts")
		}

		if candidate.Severity != 1 {
			t.Errorf("Severity = %d, want 1", candidate.Severity)
		}
	})
}

func TestDetectEventsSnow(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	t.Run("sev2 for heavy snowfall", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 75, 0, 1.0, -3, -5, 10, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeSnow)

		if !found {
			t.Fatal("expected EventTypeSnow")
		}

		if candidate.Severity != 2 {
			t.Errorf("Severity = %d, want 2", candidate.Severity)
		}
	})

	t.Run("sev1 for light snowfall", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 71, 0, 0.2, -1, -3, 10, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeSnow)

		if !found {
			t.Fatal("expected EventTypeSnow")
		}

		if candidate.Severity != 1 {
			t.Errorf("Severity = %d, want 1", candidate.Severity)
		}
	})
}

func TestDetectEventsFog(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	t.Run("sev2 when visibility below severe threshold", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 45, 0, 0, 5, 4, 5, 100.0)}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeFog)

		if !found {
			t.Fatal("expected EventTypeFog")
		}

		if candidate.Severity != 2 {
			t.Errorf("Severity = %d, want 2", candidate.Severity)
		}
	})

	t.Run("sev1 when WMO fog with ok visibility", func(t *testing.T) {
		t.Parallel()

		points := []weather.ForecastPoint{makePoint(now, 0, 45, 0, 0, 5, 4, 5, math.NaN())}

		candidates := weather.DetectEvents(points, now, cfg)
		candidate, found := findEvent(candidates, weather.EventTypeFog)

		if !found {
			t.Fatal("expected EventTypeFog")
		}

		if candidate.Severity != 1 {
			t.Errorf("Severity = %d, want 1", candidate.Severity)
		}
	})
}

func TestDetectEventsDisabledType(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()
	cfg.NotifyThunderstorm = boolPtr(false)

	points := []weather.ForecastPoint{makePoint(now, 0, 95, 0, 0, 10, 5, 20, math.NaN())}

	candidates := weather.DetectEvents(points, now, cfg)
	_, found := findEvent(candidates, weather.EventTypeThunderstorm)

	if found {
		t.Error("expected EventTypeThunderstorm to be absent (notify disabled)")
	}
}

func TestDetectEventsOutsideHorizon(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	// Point is 9 hours ahead, beyond the 8h horizon.
	points := []weather.ForecastPoint{makePoint(now, 540, 95, 0, 0, 10, 5, 20, math.NaN())}

	candidates := weather.DetectEvents(points, now, cfg)
	_, found := findEvent(candidates, weather.EventTypeThunderstorm)

	if found {
		t.Error("expected no events beyond horizon")
	}
}

func TestDetectEventsFingerprint(t *testing.T) {
	t.Parallel()

	// Two points 15min apart both round to the same 30-min bucket → same fingerprint.
	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	cfg := defaultWeatherCfg()

	pointA := makePoint(now, 0, 95, 0, 0, 10, 5, 20, math.NaN())
	pointB := makePoint(now, 15, 95, 0, 0, 10, 5, 20, math.NaN())

	candidatesA := weather.DetectEvents([]weather.ForecastPoint{pointA}, now, cfg)
	candidatesB := weather.DetectEvents([]weather.ForecastPoint{pointB}, now, cfg)

	candidateA, foundA := findEvent(candidatesA, weather.EventTypeThunderstorm)
	candidateB, foundB := findEvent(candidatesB, weather.EventTypeThunderstorm)

	if !foundA || !foundB {
		t.Fatal("expected events from both point sets")
	}

	if candidateA.Fingerprint != candidateB.Fingerprint {
		t.Errorf("fingerprints differ: %q vs %q", candidateA.Fingerprint, candidateB.Fingerprint)
	}
}
