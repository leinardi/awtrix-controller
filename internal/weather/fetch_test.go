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
	"math"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/leinardi/awtrix-controller/internal/weather"
)

const validForecastJSON = `{
  "minutely_15": {
    "time":           ["2025-01-01T12:00","2025-01-01T12:15"],
    "weather_code":   [95, 0],
    "precipitation":  [3.5, 0.0],
    "snowfall":       [0.0, 0.0],
    "temperature_2m": [8.1, 7.9],
    "dew_point_2m":   [5.0, 4.8],
    "wind_gusts_10m": [52.0, 48.0],
    "visibility":     [900.0, 5000.0]
  }
}`

func makeServer(t *testing.T, statusCode int, body string) *httptest.Server {
	t.Helper()

	return httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		writer.WriteHeader(statusCode)
		_, _ = writer.Write([]byte(body))
	}))
}

func TestFetchSuccess(t *testing.T) {
	t.Parallel()

	srv := makeServer(t, http.StatusOK, validForecastJSON)
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	points, fetchErr := fetcher.Fetch(context.Background(), 48.0, 11.0, "UTC")
	if fetchErr != nil {
		t.Fatalf("Fetch() unexpected error: %v", fetchErr)
	}

	if len(points) != 2 {
		t.Fatalf("len(points) = %d, want 2", len(points))
	}

	if points[0].WeatherCode != 95 {
		t.Errorf("points[0].WeatherCode = %d, want 95", points[0].WeatherCode)
	}

	if points[0].Precipitation != 3.5 {
		t.Errorf("points[0].Precipitation = %v, want 3.5", points[0].Precipitation)
	}

	if points[0].WindGusts10m != 52.0 {
		t.Errorf("points[0].WindGusts10m = %v, want 52.0", points[0].WindGusts10m)
	}

	if points[0].Visibility != 900.0 {
		t.Errorf("points[0].Visibility = %v, want 900.0", points[0].Visibility)
	}
}

func TestFetchHTTPError(t *testing.T) {
	t.Parallel()

	srv := makeServer(t, http.StatusInternalServerError, "server error")
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	points, fetchErr := fetcher.Fetch(context.Background(), 48.0, 11.0, "UTC")
	if fetchErr == nil {
		t.Fatal("Fetch() expected error, got nil")
	}

	if points != nil {
		t.Errorf("Fetch() points = %v, want nil", points)
	}

	if !strings.Contains(fetchErr.Error(), "500") {
		t.Errorf("error = %q, want it to contain \"500\"", fetchErr.Error())
	}
}

func TestFetchInvalidJSON(t *testing.T) {
	t.Parallel()

	srv := makeServer(t, http.StatusOK, "{not valid json{{")
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	_, fetchErr := fetcher.Fetch(context.Background(), 48.0, 11.0, "UTC")
	if fetchErr == nil {
		t.Fatal("Fetch() expected error for invalid JSON, got nil")
	}
}

func TestFetchMismatchedArrays(t *testing.T) {
	t.Parallel()

	body := `{
  "minutely_15": {
    "time":           ["2025-01-01T12:00"],
    "weather_code":   [95, 0],
    "precipitation":  [3.5],
    "snowfall":       [0.0],
    "temperature_2m": [8.1],
    "dew_point_2m":   [5.0],
    "wind_gusts_10m": [52.0],
    "visibility":     [900.0]
  }
}`

	srv := makeServer(t, http.StatusOK, body)
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	_, fetchErr := fetcher.Fetch(context.Background(), 48.0, 11.0, "UTC")
	if fetchErr == nil {
		t.Fatal("Fetch() expected error for mismatched arrays, got nil")
	}
}

func TestFetchEmptyArrays(t *testing.T) {
	t.Parallel()

	body := `{
  "minutely_15": {
    "time": [],
    "weather_code": [],
    "precipitation": [],
    "snowfall": [],
    "temperature_2m": [],
    "dew_point_2m": [],
    "wind_gusts_10m": [],
    "visibility": []
  }
}`

	srv := makeServer(t, http.StatusOK, body)
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	_, fetchErr := fetcher.Fetch(context.Background(), 48.0, 11.0, "UTC")
	if fetchErr == nil {
		t.Fatal("Fetch() expected error for empty arrays, got nil")
	}
}

func TestFetchContextCanceled(t *testing.T) {
	t.Parallel()

	srv := makeServer(t, http.StatusOK, validForecastJSON)
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	cancelledCtx, cancel := context.WithCancel(context.Background())
	cancel()

	_, fetchErr := fetcher.Fetch(cancelledCtx, 48.0, 11.0, "UTC")
	if fetchErr == nil {
		t.Fatal("Fetch() expected error for canceled context, got nil")
	}
}

func TestFetchURLEncodesTimezone(t *testing.T) {
	t.Parallel()

	var capturedURL string

	srv := httptest.NewServer(
		http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
			capturedURL = request.URL.RawQuery

			writer.WriteHeader(http.StatusOK)
			_, _ = writer.Write([]byte(validForecastJSON))
		}),
	)
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	_, fetchErr := fetcher.Fetch(context.Background(), 48.0, 11.0, "Europe/Berlin")
	if fetchErr != nil {
		t.Fatalf("Fetch() unexpected error: %v", fetchErr)
	}

	if !strings.Contains(capturedURL, "Europe%2FBerlin") {
		t.Errorf("URL query = %q, want it to contain \"Europe%%2FBerlin\"", capturedURL)
	}
}

func TestFetchVisibilityAbsentWhenMissing(t *testing.T) {
	t.Parallel()

	body := `{
  "minutely_15": {
    "time":           ["2025-01-01T12:00"],
    "weather_code":   [0],
    "precipitation":  [0.0],
    "snowfall":       [0.0],
    "temperature_2m": [10.0],
    "dew_point_2m":   [5.0],
    "wind_gusts_10m": [20.0]
  }
}`

	srv := makeServer(t, http.StatusOK, body)
	defer srv.Close()

	fetcher := weather.NewFetcherWithClient(srv.Client(), srv.URL)

	points, fetchErr := fetcher.Fetch(context.Background(), 48.0, 11.0, "UTC")
	if fetchErr != nil {
		t.Fatalf("Fetch() unexpected error: %v", fetchErr)
	}

	if !math.IsNaN(points[0].Visibility) {
		t.Errorf("Visibility = %v, want NaN when absent", points[0].Visibility)
	}
}
