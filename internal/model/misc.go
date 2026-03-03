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

package model

// EffectSettings configures the background effect animation for a notification
// or custom app.
type EffectSettings struct {
	Speed   int           `json:"speed,omitempty"`
	Palette EffectPalette `json:"palette,omitempty"`
	Blend   bool          `json:"blend,omitempty"`
}

// Indicator represents the payload for the side LED indicator topics
// ({clientId}/indicator1, indicator2, indicator3).
// Send an empty payload to clear the indicator.
type Indicator struct {
	Color string `json:"color,omitempty"`
	Blink int    `json:"blink,omitempty"`
	Fade  int    `json:"fade,omitempty"`
}

// MoodLight represents the payload for {clientId}/moodlight.
// Send an empty payload to disable mood-light mode.
type MoodLight struct {
	Brightness int    `json:"brightness,omitempty"`
	Color      string `json:"color,omitempty"`
	Kelvin     int    `json:"kelvin,omitempty"` // color temperature in Kelvin (alternative to Color)
}

// Power represents the payload for {clientId}/power.
// The Power field intentionally omits omitempty: false (power off) is a meaningful value.
type Power struct {
	Power bool `json:"power"`
}

// Sleep represents the payload for {clientId}/sleep.
// The Sleep field intentionally omits omitempty: 0 (sleep indefinitely) is a meaningful value.
type Sleep struct {
	Sleep int `json:"sleep"`
}

// Rtttl represents the payload for {clientId}/rtttl.
type Rtttl struct {
	Rtttl string `json:"rtttl,omitempty"`
}

// Sound represents the payload for {clientId}/sound.
// Send the filename (without extension) of an RTTTL file from the MELODIES folder,
// or the 4-digit number of a DFPlayer MP3.
type Sound struct {
	Sound string `json:"sound,omitempty"`
}

// AppSwitch represents the payload for {clientId}/switch.
// Use BuiltInApp* constants for the built-in apps, or a custom app's name.
type AppSwitch struct {
	Name string `json:"name"`
}
