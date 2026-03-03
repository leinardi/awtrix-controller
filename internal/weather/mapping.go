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
	"time"

	"github.com/leinardi/awtrix-controller/internal/model"
)

// EventType identifies a warning notification category.
type EventType int

const (
	// EventTypeThunderstorm covers WMO codes 95/96/99.
	EventTypeThunderstorm EventType = iota
	// EventTypeFreezingPrecip covers freezing drizzle/rain (WMO 56/57/66/67).
	EventTypeFreezingPrecip
	// EventTypeFrostRisk covers conditions where frost is likely.
	EventTypeFrostRisk
	// EventTypeHeavyRain covers heavy precipitation events.
	EventTypeHeavyRain
	// EventTypeStrongGusts covers high wind gust conditions.
	EventTypeStrongGusts
	// EventTypeSnow covers snowfall events (WMO 71/73/75/77/85/86).
	EventTypeSnow
	// EventTypeFog covers fog/depositing rime fog (WMO 45/48).
	EventTypeFog
)

// allEventTypes lists every EventType for iteration.
var allEventTypes = []EventType{
	EventTypeThunderstorm,
	EventTypeFreezingPrecip,
	EventTypeFrostRisk,
	EventTypeHeavyRain,
	EventTypeStrongGusts,
	EventTypeSnow,
	EventTypeFog,
}

// overlayPriority maps an OverlayEffect to a numeric priority (higher wins).
var overlayPriority = map[model.OverlayEffect]int{
	"":                         0,
	model.OverlayEffectDrizzle: 1,
	model.OverlayEffectRain:    2,
	model.OverlayEffectSnow:    3,
	model.OverlayEffectStorm:   4,
	model.OverlayEffectFrost:   5,
	model.OverlayEffectThunder: 6,
}

// WMOToOverlay maps a WMO weather code to its OverlayEffect.
// Returns "" (no overlay) for codes that have no associated effect.
func WMOToOverlay(wmoCode int) model.OverlayEffect {
	switch wmoCode {
	case wmoClearSky, wmoMainlyClear, wmoPartlyCloudy, wmoOvercast:
		return ""
	case wmoFogLight, wmoFogRime:
		return ""
	case wmoDrizzleLight, wmoDrizzleModerate, wmoDrizzleHeavy:
		return model.OverlayEffectDrizzle
	case wmoFreezingDrizzleLight, wmoFreezingDrizzleHeavy:
		return model.OverlayEffectFrost
	case wmoRainLight, wmoRainModerate, wmoRainHeavy:
		return model.OverlayEffectRain
	case wmoFreezingRainLight, wmoFreezingRainHeavy:
		return model.OverlayEffectFrost
	case wmoSnowLight, wmoSnowMod, wmoSnowHeavy, wmoSnowGrains:
		return model.OverlayEffectSnow
	case wmoRainShowerSlight, wmoRainShowerModerate:
		return model.OverlayEffectRain
	case wmoStormHeavyRain:
		return model.OverlayEffectStorm
	case wmoSnowShowerLight, wmoSnowShowerHeavy:
		return model.OverlayEffectSnow
	case wmoThunderstorm, wmoThunderstormSev2, wmoThunderstormSev3:
		return model.OverlayEffectThunder
	default:
		return ""
	}
}

// SelectOverlay returns the highest-priority overlay among ForecastPoints in
// [now, now+horizonMinutes). Returns "" when the window is empty or all codes
// map to no overlay.
func SelectOverlay(points []ForecastPoint, now time.Time, horizonMinutes int) model.OverlayEffect {
	horizon := now.Add(time.Duration(horizonMinutes) * time.Minute)

	var best model.OverlayEffect

	for pointIdx := range points {
		timestamp := points[pointIdx].Time
		if timestamp.Before(now) || !timestamp.Before(horizon) {
			continue
		}

		candidate := WMOToOverlay(points[pointIdx].WeatherCode)
		if overlayPriority[candidate] > overlayPriority[best] {
			best = candidate
		}
	}

	return best
}
