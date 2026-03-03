/*
 * MIT License
 *
 * Copyright (c) 2025 Roberto Leinardi
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

// Notification represents the payload sent to {clientId}/notify to display a
// transient message on the device. All fields are optional; omit a field to
// use the device's default behavior for that setting.
//
//nolint:tagliatelle // firmware uses mixed-case JSON keys (e.g. progressBC); must match exactly
type Notification struct {
	Text           string          `json:"text,omitempty"`
	TextCase       int             `json:"textCase,omitempty"`
	TopText        bool            `json:"topText,omitempty"`
	TextOffset     int             `json:"textOffset,omitempty"`
	Center         bool            `json:"center,omitempty"`
	Color          string          `json:"color,omitempty"`
	Gradient       []string        `json:"gradient,omitempty"`
	BlinkText      int             `json:"blinkText,omitempty"`
	FadeText       int             `json:"fadeText,omitempty"`
	Background     string          `json:"background,omitempty"`
	Rainbow        bool            `json:"rainbow,omitempty"`
	Icon           string          `json:"icon,omitempty"`
	PushIcon       int             `json:"pushIcon,omitempty"`
	Repeat         int             `json:"repeat,omitempty"`
	Duration       int             `json:"duration,omitempty"`
	Hold           bool            `json:"hold,omitempty"`
	Sound          string          `json:"sound,omitempty"`
	Rtttl          string          `json:"rtttl,omitempty"`
	LoopSound      bool            `json:"loopSound,omitempty"`
	Bar            []int           `json:"bar,omitempty"`
	Line           []int           `json:"line,omitempty"`
	Autoscale      bool            `json:"autoscale,omitempty"`
	Progress       int             `json:"progress,omitempty"`
	ProgressC      string          `json:"progressC,omitempty"`
	ProgressBC     string          `json:"progressBC,omitempty"`
	Draw           []DrawCommand   `json:"draw,omitempty"`
	Stack          bool            `json:"stack,omitempty"`
	Wakeup         bool            `json:"wakeup,omitempty"`
	NoScroll       bool            `json:"noScroll,omitempty"`
	Clients        []string        `json:"clients,omitempty"`
	ScrollSpeed    int             `json:"scrollSpeed,omitempty"`
	Effect         string          `json:"effect,omitempty"`
	EffectSettings *EffectSettings `json:"effectSettings,omitempty"`
	Overlay        string          `json:"overlay,omitempty"`
}

// CustomApp represents the payload sent to {clientId}/custom/{appName} to create
// or update a persistent custom app in the device's app rotation. It embeds all
// Notification fields and adds app-lifecycle fields.
type CustomApp struct {
	Notification

	Pos          int  `json:"pos,omitempty"`
	Lifetime     int  `json:"lifetime,omitempty"`
	LifetimeMode int  `json:"lifetimeMode,omitempty"`
	Save         bool `json:"save,omitempty"`
}
