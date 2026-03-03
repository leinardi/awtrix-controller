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

package settings_test

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/daynight"
	"github.com/leinardi/awtrix-controller/internal/energysaving"
	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
	"github.com/leinardi/awtrix-controller/internal/settings"
)

// fakeModeProvider implements daynight.ModeProvider with a fixed mode for tests.
type fakeModeProvider struct {
	mode daynight.Mode
}

func (f *fakeModeProvider) CurrentMode() daynight.Mode {
	return f.mode
}

// makeEnergyController creates an energysaving.Controller pre-initialized with
// the given active state. It uses a FakeClock fixed at noon UTC so the
// scheduler timer will not fire during the test.
func makeEnergyController(t *testing.T, active bool) *energysaving.Controller {
	t.Helper()

	fakeClock := clock.NewFakeClock(time.Date(2024, 1, 1, 12, 0, 0, 0, time.UTC))

	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)

	sched := scheduler.New(ctx, fakeClock)

	t.Cleanup(func() { sched.Stop() })

	// Window "00:00"–"23:59": noon is inside → IsActive() returns true.
	// Window "01:00"–"02:00": noon is outside → IsActive() returns false.
	var cfg config.EnergySavingConfig
	if active {
		cfg = config.EnergySavingConfig{Start: "00:00", End: "23:59"}
	} else {
		cfg = config.EnergySavingConfig{Start: "01:00", End: "02:00"}
	}

	ctrl := energysaving.New(cfg, time.UTC, fakeClock, sched, func(bool) {})

	err := ctrl.Start()
	if err != nil {
		t.Fatal("makeEnergyController: Start:", err)
	}

	return ctrl
}

func makeTheme() config.ThemeConfig {
	return config.ThemeConfig{
		Day: config.ThemeColors{
			CalendarAccent: "#FFFFFF",
			Content:        "#AAAAAA",
		},
		Night: config.ThemeColors{
			CalendarAccent: "#000000",
			Content:        "#555555",
		},
	}
}

// unmarshalSettings marshals the settings to JSON and unmarshals into a
// map[string]any so callers can check key presence and values.
func unmarshalSettings(t *testing.T, b *settings.Builder) map[string]any {
	t.Helper()

	built := b.Build()

	data, err := json.Marshal(built)
	if err != nil {
		t.Fatal("json.Marshal:", err)
	}

	var result map[string]any

	unmarshalErr := json.Unmarshal(data, &result)
	if unmarshalErr != nil {
		t.Fatal("json.Unmarshal:", unmarshalErr)
	}

	return result
}

func TestBuildDayEnergyInactive(t *testing.T) {
	t.Parallel()

	// TC-01: Day mode, energy inactive → day colors, ABRI=true, no BRI key.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Day}
	energySaver := makeEnergyController(t, false)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)
	result := unmarshalSettings(t, builder)

	if got, ok := result["CHCOL"].(string); !ok || got != "#FFFFFF" {
		t.Errorf("CHCOL: want %q, got %v", "#FFFFFF", result["CHCOL"])
	}

	if got, ok := result["WDCA"].(string); !ok || got != "#FFFFFF" {
		t.Errorf("WDCA: want %q, got %v", "#FFFFFF", result["WDCA"])
	}

	if got, ok := result["CBCOL"].(string); !ok || got != "#AAAAAA" {
		t.Errorf("CBCOL: want %q, got %v", "#AAAAAA", result["CBCOL"])
	}

	if got, ok := result["ABRI"].(bool); !ok || !got {
		t.Errorf("ABRI: want true, got %v", result["ABRI"])
	}

	if _, hasBRI := result["BRI"]; hasBRI {
		t.Error("BRI key must be absent when energy saving is inactive")
	}
}

func TestBuildNightEnergyInactive(t *testing.T) {
	t.Parallel()

	// Night mode, energy inactive → night colors, ABRI=true, no BRI key.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Night}
	energySaver := makeEnergyController(t, false)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)
	result := unmarshalSettings(t, builder)

	if got, ok := result["CHCOL"].(string); !ok || got != "#000000" {
		t.Errorf("CHCOL: want %q, got %v", "#000000", result["CHCOL"])
	}

	if got, ok := result["CBCOL"].(string); !ok || got != "#555555" {
		t.Errorf("CBCOL: want %q, got %v", "#555555", result["CBCOL"])
	}

	if got, ok := result["ABRI"].(bool); !ok || !got {
		t.Errorf("ABRI: want true, got %v", result["ABRI"])
	}

	if _, hasBRI := result["BRI"]; hasBRI {
		t.Error("BRI key must be absent when energy saving is inactive")
	}
}

func TestBuildDayEnergyActive(t *testing.T) {
	t.Parallel()

	// TC-03: Day mode, energy active → BRI=1, ABRI=false.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Day}
	energySaver := makeEnergyController(t, true)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)
	result := unmarshalSettings(t, builder)

	if got, ok := result["BRI"].(float64); !ok || int(got) != 1 {
		t.Errorf("BRI: want 1, got %v", result["BRI"])
	}

	if got, ok := result["ABRI"].(bool); !ok || got {
		t.Errorf("ABRI: want false, got %v", result["ABRI"])
	}

	// Day theme colors still present.
	if got, ok := result["CHCOL"].(string); !ok || got != "#FFFFFF" {
		t.Errorf("CHCOL: want %q, got %v", "#FFFFFF", result["CHCOL"])
	}
}

func TestBuildNightEnergyActive(t *testing.T) {
	t.Parallel()

	// Night mode, energy active → night colors, BRI=1, ABRI=false.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Night}
	energySaver := makeEnergyController(t, true)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)
	result := unmarshalSettings(t, builder)

	if got, ok := result["CHCOL"].(string); !ok || got != "#000000" {
		t.Errorf("CHCOL: want %q, got %v", "#000000", result["CHCOL"])
	}

	if got, ok := result["BRI"].(float64); !ok || int(got) != 1 {
		t.Errorf("BRI: want 1, got %v", result["BRI"])
	}

	if got, ok := result["ABRI"].(bool); !ok || got {
		t.Errorf("ABRI: want false, got %v", result["ABRI"])
	}
}

func TestBuildOverlayPresent(t *testing.T) {
	t.Parallel()

	// SetOverlay(OverlayEffectSnow) → Build() JSON has "OVERLAY":"snow".
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Day}
	energySaver := makeEnergyController(t, false)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)
	builder.SetOverlay(model.OverlayEffectSnow)

	result := unmarshalSettings(t, builder)

	if got, ok := result["OVERLAY"].(string); !ok || got != string(model.OverlayEffectSnow) {
		t.Errorf("OVERLAY: want %q, got %v", model.OverlayEffectSnow, result["OVERLAY"])
	}
}

func TestBuildOverlayNullByDefault(t *testing.T) {
	t.Parallel()

	// Fresh builder → JSON has "OVERLAY":null.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Day}
	energySaver := makeEnergyController(t, false)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)

	result := unmarshalSettings(t, builder)

	val, hasOverlay := result["OVERLAY"]
	if !hasOverlay {
		t.Error("OVERLAY key must be present")
	}

	if val != nil {
		t.Errorf("OVERLAY: want null, got %v", val)
	}
}

func TestBuildOverlayClearedAfterReset(t *testing.T) {
	t.Parallel()

	// Set overlay then clear → "OVERLAY":null in JSON.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Day}
	energySaver := makeEnergyController(t, false)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)
	builder.SetOverlay(model.OverlayEffectSnow)
	builder.SetOverlay("")

	result := unmarshalSettings(t, builder)

	val, hasOverlay := result["OVERLAY"]
	if !hasOverlay {
		t.Error("OVERLAY key must be present after reset")
	}

	if val != nil {
		t.Errorf("OVERLAY: want null after reset, got %v", val)
	}
}

func TestBuildOverlayConcurrentAccess(t *testing.T) {
	t.Parallel()

	// Goroutine calls SetOverlay while main calls Build — run with -race.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Day}
	energySaver := makeEnergyController(t, false)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)

	done := make(chan struct{})

	go func() {
		defer close(done)

		for range 50 {
			builder.SetOverlay(model.OverlayEffectSnow)
			builder.SetOverlay("")
		}
	}()

	for range 50 {
		_ = builder.Build()
	}

	<-done
}

func TestBuildAllColorFieldsPopulated(t *testing.T) {
	t.Parallel()

	// Verify all 6 color fields are present and mapped correctly.
	theme := makeTheme()
	modeProvider := &fakeModeProvider{mode: daynight.Day}
	energySaver := makeEnergyController(t, false)

	builder := settings.NewBuilder(theme, modeProvider, energySaver)
	result := unmarshalSettings(t, builder)

	accentFields := []string{"CHCOL", "WDCA"}
	contentFields := []string{"CBCOL", "WDCI", "TIME_COL", "DATE_COL"}

	for _, field := range accentFields {
		if got, ok := result[field].(string); !ok || got != "#FFFFFF" {
			t.Errorf("%s: want %q, got %v", field, "#FFFFFF", result[field])
		}
	}

	for _, field := range contentFields {
		if got, ok := result[field].(string); !ok || got != "#AAAAAA" {
			t.Errorf("%s: want %q, got %v", field, "#AAAAAA", result[field])
		}
	}
}
