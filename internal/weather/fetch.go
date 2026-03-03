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

// Package weather fetches minutely_15 forecasts from Open-Meteo, maps WMO codes
// to overlay effects and event types, and manages the notification state machine.
package weather

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"net/http"
	"net/url"
	"strconv"
	"time"

	"github.com/leinardi/awtrix-controller/internal/logger"
)

const (
	defaultBaseURL   = "https://api.open-meteo.com"
	fetchTimeout     = 10 * time.Second
	forecastSteps    = 48 // 12 h of minutely_15 data
	visibilityAbsent = -999.0
)

// Sentinel errors returned by Fetch and parseRaw.
var (
	// errEmptyArrays is returned when the forecast contains zero time steps.
	errEmptyArrays = errors.New("weather: empty forecast arrays")
	// errMismatchedArrays is returned when parallel forecast arrays have different lengths.
	errMismatchedArrays = errors.New("weather: mismatched array lengths")
	// errHTTPError is returned (wrapped with the status code) for non-200 responses.
	errHTTPError = errors.New("weather: non-OK HTTP status")
)

// ForecastPoint is one parsed minutely_15 timestep.
type ForecastPoint struct {
	Time          time.Time
	WeatherCode   int
	Precipitation float64 // mm/15min
	Snowfall      float64 // cm/15min
	Temperature2m float64 // °C
	DewPoint2m    float64 // °C
	WindGusts10m  float64 // km/h
	Visibility    float64 // m; math.NaN() if absent
}

// Fetcher retrieves minutely_15 forecast data from Open-Meteo.
type Fetcher struct {
	httpClient *http.Client
	baseURL    string
}

// NewFetcher returns a production Fetcher with a 10-second timeout.
func NewFetcher() *Fetcher {
	return &Fetcher{
		httpClient: &http.Client{Timeout: fetchTimeout},
		baseURL:    defaultBaseURL,
	}
}

// NewFetcherWithClient returns a Fetcher with a custom HTTP client and base URL.
// Intended for testing.
func NewFetcherWithClient(client *http.Client, baseURL string) *Fetcher {
	return &Fetcher{
		httpClient: client,
		baseURL:    baseURL,
	}
}

// rawResponse is the JSON shape returned by Open-Meteo for minutely_15 requests.
//
//nolint:tagliatelle // Open-Meteo API uses snake_case JSON keys; firmware convention, not configurable
type rawResponse struct {
	Minutely15 struct {
		Time          []string  `json:"time"`
		WeatherCode   []int     `json:"weather_code"`
		Precipitation []float64 `json:"precipitation"`
		Snowfall      []float64 `json:"snowfall"`
		Temperature   []float64 `json:"temperature_2m"`
		DewPoint      []float64 `json:"dew_point_2m"`
		WindGusts     []float64 `json:"wind_gusts_10m"`
		Visibility    []float64 `json:"visibility"`
	} `json:"minutely_15"`
}

// Fetch retrieves a 12-hour minutely_15 forecast for the given coordinates and
// timezone. Returns a non-nil error on any HTTP, parse, or validation failure.
func (f *Fetcher) Fetch(
	ctx context.Context,
	lat, lon float64,
	timezone string,
) ([]ForecastPoint, error) {
	queryParams := url.Values{}
	queryParams.Set("latitude", strconv.FormatFloat(lat, 'f', -1, 64))
	queryParams.Set("longitude", strconv.FormatFloat(lon, 'f', -1, 64))
	queryParams.Set(
		"minutely_15",
		"weather_code,precipitation,snowfall,temperature_2m,dew_point_2m,wind_gusts_10m,visibility",
	)
	queryParams.Set("forecast_minutely_15", strconv.Itoa(forecastSteps))
	queryParams.Set("timezone", timezone)
	queryParams.Set("timeformat", "iso8601")

	rawURL := f.baseURL + "/v1/forecast?" + queryParams.Encode()

	logger.L().Debug("weather: fetch request",
		"url", rawURL,
		"lat", lat,
		"lon", lon,
		"timezone", timezone,
	)

	request, requestErr := http.NewRequestWithContext(ctx, http.MethodGet, rawURL, http.NoBody)
	if requestErr != nil {
		return nil, fmt.Errorf("weather: build request: %w", requestErr)
	}

	response, executeErr := f.httpClient.Do( //nolint:gosec // G704: URL built from config-controlled base URL, not user input
		request,
	)
	if executeErr != nil {
		return nil, fmt.Errorf("weather: execute request: %w", executeErr)
	}

	defer response.Body.Close()

	if response.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("weather: HTTP %d: %w", response.StatusCode, errHTTPError)
	}

	var raw rawResponse

	decodeErr := json.NewDecoder(response.Body).Decode(&raw)
	if decodeErr != nil {
		return nil, fmt.Errorf("weather: decode response: %w", decodeErr)
	}

	points, parseErr := parseRaw(&raw, timezone)
	if parseErr != nil {
		return nil, parseErr
	}

	logger.L().Debug("weather: fetch response",
		"status", response.StatusCode,
		"points", len(points),
	)

	return points, nil
}

// parseRaw validates array lengths and converts the raw response into typed ForecastPoints.
func parseRaw(raw *rawResponse, timezone string) ([]ForecastPoint, error) {
	minutely := raw.Minutely15
	count := len(minutely.Time)

	if count == 0 {
		return nil, errEmptyArrays
	}

	if len(minutely.WeatherCode) != count ||
		len(minutely.Precipitation) != count ||
		len(minutely.Snowfall) != count ||
		len(minutely.Temperature) != count ||
		len(minutely.DewPoint) != count ||
		len(minutely.WindGusts) != count {
		return nil, errMismatchedArrays
	}

	loc, locErr := time.LoadLocation(timezone)
	if locErr != nil {
		loc = time.UTC
	}

	points := make([]ForecastPoint, count)

	for idx := range count {
		parsed, parseErr := time.ParseInLocation("2006-01-02T15:04", minutely.Time[idx], loc)
		if parseErr != nil {
			return nil, fmt.Errorf("weather: parse time %q: %w", minutely.Time[idx], parseErr)
		}

		visibility := math.NaN()

		if len(minutely.Visibility) == count {
			rawVal := minutely.Visibility[idx]
			if rawVal != visibilityAbsent {
				visibility = rawVal
			}
		}

		points[idx] = ForecastPoint{
			Time:          parsed,
			WeatherCode:   minutely.WeatherCode[idx],
			Precipitation: minutely.Precipitation[idx],
			Snowfall:      minutely.Snowfall[idx],
			Temperature2m: minutely.Temperature[idx],
			DewPoint2m:    minutely.DewPoint[idx],
			WindGusts10m:  minutely.WindGusts[idx],
			Visibility:    visibility,
		}
	}

	return points, nil
}
