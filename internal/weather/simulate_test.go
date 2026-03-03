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
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/weather"
)

func TestSimulateFetchFuncReturnsCorrectCount(t *testing.T) {
	t.Parallel()

	fetchFn := weather.SimulateFetchFunc(95)

	points, fetchErr := fetchFn(context.Background(), 52.52, 13.405, "UTC")
	if fetchErr != nil {
		t.Fatalf("SimulateFetchFunc returned error: %v", fetchErr)
	}

	const wantPoints = 48 // forecastSteps

	if len(points) != wantPoints {
		t.Errorf("len(points) = %d, want %d", len(points), wantPoints)
	}
}

func TestSimulateFetchFuncWMOCode(t *testing.T) {
	t.Parallel()

	const wmoCode = 71

	fetchFn := weather.SimulateFetchFunc(wmoCode)

	points, fetchErr := fetchFn(context.Background(), 52.52, 13.405, "UTC")
	if fetchErr != nil {
		t.Fatalf("SimulateFetchFunc returned error: %v", fetchErr)
	}

	for idx, point := range points {
		if point.WeatherCode != wmoCode {
			t.Errorf("points[%d].WeatherCode = %d, want %d", idx, point.WeatherCode, wmoCode)
		}
	}
}

func TestSimulateFetchFuncTimeProgression(t *testing.T) {
	t.Parallel()

	fetchFn := weather.SimulateFetchFunc(95)

	points, fetchErr := fetchFn(context.Background(), 52.52, 13.405, "UTC")
	if fetchErr != nil {
		t.Fatalf("SimulateFetchFunc returned error: %v", fetchErr)
	}

	const wantStep = 15 * time.Minute

	for idx := 1; idx < len(points); idx++ {
		step := points[idx].Time.Sub(points[idx-1].Time)
		if step != wantStep {
			t.Errorf("points[%d] time gap = %v, want %v", idx, step, wantStep)
		}
	}
}
