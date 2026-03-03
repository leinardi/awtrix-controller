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

// Package settings composes and publishes partial Settings payloads to
// individual Awtrix3 devices based on the current day/night mode and
// energy-saving state.
package settings

import (
	"encoding/json"
	"fmt"
	"sync"

	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/daynight"
	"github.com/leinardi/awtrix-controller/internal/energysaving"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/model"
	mqtt "github.com/wind-c/comqtt/v2/mqtt"
)

// Builder composes a partial model.Settings from the current day/night mode,
// energy-saving state, and weather overlay. It reads all providers at call time.
type Builder struct {
	theme        config.ThemeConfig
	modeProvider daynight.ModeProvider
	energySaver  *energysaving.Controller

	mu             sync.RWMutex
	currentOverlay model.OverlayEffect
}

// NewBuilder returns a Builder that reads from the given providers.
func NewBuilder(
	theme config.ThemeConfig,
	modeProvider daynight.ModeProvider,
	energySaver *energysaving.Controller,
) *Builder {
	return &Builder{
		theme:        theme,
		modeProvider: modeProvider,
		energySaver:  energySaver,
	}
}

// SetOverlay updates the overlay to include in the next Build() result.
// Pass "" to clear the overlay (Build() will send JSON null to the device).
// Safe for concurrent use.
func (b *Builder) SetOverlay(overlay model.OverlayEffect) {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.currentOverlay = overlay
}

// Build returns a partial *model.Settings populated with the 6–8 fields the
// application manages. The returned value must not be modified by the caller.
//
// Color mapping:
//   - CalendarAccent → CHCOL (calendar header), WDCA (weekday active)
//   - Content        → CBCOL (calendar bg), WDCI (weekday inactive),
//     TIME_COL, DATE_COL
func (b *Builder) Build() *model.Settings {
	var colors config.ThemeColors

	if b.modeProvider.CurrentMode() == daynight.Day {
		colors = b.theme.Day
	} else {
		colors = b.theme.Night
	}

	b.mu.RLock()
	overlayEffect := b.currentOverlay
	b.mu.RUnlock()

	var overlayPtr *model.OverlayEffect
	if overlayEffect != "" {
		overlayPtr = &overlayEffect
	}

	result := &model.Settings{
		ChCol:   colors.CalendarAccent,
		CbCol:   colors.Content,
		Wdca:    colors.CalendarAccent,
		Wdci:    colors.Content,
		TimeCol: colors.Content,
		DateCol: colors.Content,
		Overlay: overlayPtr,
	}

	if b.energySaver.IsActive() {
		brightnessValue := 1
		autoBrightness := false
		result.Bri = &brightnessValue
		result.Abri = &autoBrightness
	} else {
		autoBrightness := true
		result.Abri = &autoBrightness
	}

	return result
}

// Push marshals s to JSON and publishes it as a retained QoS-1 message on
// the topic "{clientID}/settings". The retained flag ensures a newly
// connecting device immediately receives the current settings.
func Push(clientID string, srv *mqtt.Server, settings *model.Settings) error {
	payload, err := json.Marshal(settings)
	if err != nil {
		return fmt.Errorf("settings: marshal payload for %s: %w", clientID, err)
	}

	topic := clientID + "/settings"

	logger.L().
		Debug("settings: pushing to client", "clientID", clientID, "topic", topic, "payload", string(payload))

	publishErr := srv.Publish(topic, payload, true, 1)
	if publishErr != nil {
		return fmt.Errorf("settings: publish to %s: %w", topic, publishErr)
	}

	return nil
}
