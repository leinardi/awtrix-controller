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

	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/weather"
)

func TestWMOToOverlay(t *testing.T) {
	t.Parallel()

	tests := []struct {
		wmo  int
		want model.OverlayEffect
	}{
		{0, ""},
		{1, ""},
		{2, ""},
		{3, ""},
		{45, ""},
		{48, ""},
		{51, model.OverlayEffectDrizzle},
		{53, model.OverlayEffectDrizzle},
		{55, model.OverlayEffectDrizzle},
		{56, model.OverlayEffectFrost},
		{57, model.OverlayEffectFrost},
		{61, model.OverlayEffectRain},
		{63, model.OverlayEffectRain},
		{65, model.OverlayEffectRain},
		{66, model.OverlayEffectFrost},
		{67, model.OverlayEffectFrost},
		{71, model.OverlayEffectSnow},
		{73, model.OverlayEffectSnow},
		{75, model.OverlayEffectSnow},
		{77, model.OverlayEffectSnow},
		{80, model.OverlayEffectRain},
		{81, model.OverlayEffectRain},
		{82, model.OverlayEffectStorm},
		{85, model.OverlayEffectSnow},
		{86, model.OverlayEffectSnow},
		{95, model.OverlayEffectThunder},
		{96, model.OverlayEffectThunder},
		{99, model.OverlayEffectThunder},
		{999, ""},
	}

	for _, testCase := range tests {
		t.Run("", func(t *testing.T) {
			t.Parallel()

			got := weather.WMOToOverlay(testCase.wmo)
			if got != testCase.want {
				t.Errorf("WMOToOverlay(%d) = %q, want %q", testCase.wmo, got, testCase.want)
			}
		})
	}
}

func TestSelectOverlayPicksHighestPriority(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)

	points := []weather.ForecastPoint{
		{Time: now, WeatherCode: 51, Visibility: math.NaN()},                       // drizzle
		{Time: now.Add(15 * time.Minute), WeatherCode: 95, Visibility: math.NaN()}, // thunder
	}

	got := weather.SelectOverlay(points, now, 60)
	if got != model.OverlayEffectThunder {
		t.Errorf("SelectOverlay = %q, want %q", got, model.OverlayEffectThunder)
	}
}

func TestSelectOverlayWindowBoundary(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)

	points := []weather.ForecastPoint{
		{
			Time:        now.Add(-1 * time.Minute),
			WeatherCode: 95,
			Visibility:  math.NaN(),
		}, // before now → excluded
		{
			Time:        now,
			WeatherCode: 55,
			Visibility:  math.NaN(),
		}, // at now → included
		{
			Time:        now.Add(60 * time.Minute),
			WeatherCode: 95,
			Visibility:  math.NaN(),
		}, // at horizon → excluded
	}

	got := weather.SelectOverlay(points, now, 60)
	if got != model.OverlayEffectDrizzle {
		t.Errorf("SelectOverlay = %q, want %q (horizon-exclusive)", got, model.OverlayEffectDrizzle)
	}
}

func TestSelectOverlayEmpty(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)

	got := weather.SelectOverlay(nil, now, 60)
	if got != "" {
		t.Errorf("SelectOverlay(nil) = %q, want \"\"", got)
	}
}

func TestSelectOverlayNoPointsInWindow(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)

	points := []weather.ForecastPoint{
		{Time: now.Add(-15 * time.Minute), WeatherCode: 95, Visibility: math.NaN()},
	}

	got := weather.SelectOverlay(points, now, 60)
	if got != "" {
		t.Errorf("SelectOverlay = %q, want \"\" (all outside window)", got)
	}
}
