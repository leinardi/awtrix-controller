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
	"time"

	"github.com/leinardi/awtrix-controller/internal/logger"
)

const (
	simulatePrecipitationMm = 6.0   // > DefaultWeatherHeavyRainMmPer15Min (5.0)
	simulateSnowfallCm      = 1.0   // > snowfallHeavyThresholdCm (0.5)
	simulateTemperatureC    = 1.5   // < DefaultWeatherFrostTempC (2.0)
	simulateDewPointC       = 0.0   // temp-dew delta = 1.5 < DefaultWeatherFrostDewPointDeltaC (2.0)
	simulateWindGustsKmh    = 50.0  // > DefaultWeatherGustWarnKmh (45), < GustSevereKmh (60)
	simulateVisibilityM     = 150.0 // < DefaultWeatherFogVisibilitySevereM (200)
	simulateStepMinutes     = 15
)

// SimulateFetchFunc returns a FetchFunc that always returns forecastSteps
// synthetic ForecastPoints covering the next 12 hours, all with wmoCode.
// Supplementary values (precipitation, snowfall, temperature, wind gusts,
// visibility) are set at warning-threshold levels so that any event type
// applicable to the WMO code will be detected. Intended for local testing only.
func SimulateFetchFunc(wmoCode int) FetchFunc {
	return func(_ context.Context, lat, lon float64, timezone string) ([]ForecastPoint, error) {
		logger.L().Warn("weather: SIMULATION MODE — returning synthetic forecast",
			"wmo_code", wmoCode,
			"lat", lat,
			"lon", lon,
		)

		loc, locErr := time.LoadLocation(timezone)
		if locErr != nil {
			loc = time.UTC
		}

		now := time.Now().In(loc).Truncate(simulateStepMinutes * time.Minute)
		points := make([]ForecastPoint, forecastSteps)

		for idx := range forecastSteps {
			points[idx] = ForecastPoint{
				Time:          now.Add(time.Duration(idx) * simulateStepMinutes * time.Minute),
				WeatherCode:   wmoCode,
				Precipitation: simulatePrecipitationMm,
				Snowfall:      simulateSnowfallCm,
				Temperature2m: simulateTemperatureC,
				DewPoint2m:    simulateDewPointC,
				WindGusts10m:  simulateWindGustsKmh,
				Visibility:    simulateVisibilityM,
			}
		}

		return points, nil
	}
}
