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

// AppContent holds the display fields shared by both Notification and CustomApp.
// All fields are optional; omit a field to use the device's default behavior.
//
//nolint:tagliatelle // firmware uses mixed-case JSON keys (e.g. progressBC, barBC); must match exactly
type AppContent struct {
	// Text is the message displayed on the matrix. Use NewPlainText for a simple string,
	// or NewFragmentedText for colored text segments.
	Text *TextContent `json:"text,omitempty"`
	// TextCase controls capitalisation of the text. Default: TextCaseGlobalSetting (0).
	TextCase TextCaseMode `json:"textCase,omitempty"`
	// TopText displays the text at the top of the matrix when true.
	TopText bool `json:"topText,omitempty"`
	// TextOffset is the horizontal text offset in pixels.
	TextOffset int `json:"textOffset,omitempty"`
	// Center centers the text horizontally. Pointer so that false can be sent explicitly
	// (device default is true; omitting means centered).
	Center *bool `json:"center,omitempty"`
	// Color is the text color as a hex string (e.g. "#FF0000") or RGB array.
	Color string `json:"color,omitempty"`
	// Gradient is a list of hex colors used for a gradient text effect.
	Gradient []string `json:"gradient,omitempty"`
	// BlinkText is the blink rate in milliseconds (0 = no blink).
	BlinkText int `json:"blinkText,omitempty"`
	// FadeText is the fade rate in milliseconds (0 = no fade).
	FadeText int `json:"fadeText,omitempty"`
	// Background is the background fill color as a hex string or RGB array.
	Background string `json:"background,omitempty"`
	// Rainbow enables cycling rainbow colors on the text when true.
	Rainbow bool `json:"rainbow,omitempty"`
	// Icon is the LaMetric icon ID displayed alongside the text.
	Icon string `json:"icon,omitempty"`
	// PushIcon controls how the icon moves relative to the scrolling text.
	// Default: PushIconStatic (0). Use PushIconScroll or PushIconFixed to override.
	PushIcon PushIconBehavior `json:"pushIcon,omitempty"`
	// Repeat is the number of times to scroll the content. Pointer so that 0
	// (scroll once) can be sent explicitly; -1 means infinite (device default).
	Repeat *int `json:"repeat,omitempty"`
	// Duration is how long the content is displayed in seconds.
	Duration int `json:"duration,omitempty"`
	// Bar is a list of values to render as a bar chart on the matrix.
	Bar []int `json:"bar,omitempty"`
	// Line is a list of values to render as a line chart on the matrix.
	Line []int `json:"line,omitempty"`
	// Autoscale automatically scales bar/line chart values to fit the matrix.
	// Pointer so that false can be sent explicitly (device default is true).
	Autoscale *bool `json:"autoscale,omitempty"`
	// BarBC is the background color of bar chart bars as a hex string or RGB array.
	BarBC string `json:"barBC,omitempty"`
	// Progress is a progress bar value (0–100). Pointer so that 0 (show 0% bar)
	// can be sent explicitly; -1 hides the bar (device default).
	Progress *int `json:"progress,omitempty"`
	// ProgressC is the progress bar foreground color as a hex string or RGB array.
	ProgressC string `json:"progressC,omitempty"`
	// ProgressBC is the progress bar background color as a hex string or RGB array.
	ProgressBC string `json:"progressBC,omitempty"`
	// Draw is a list of drawing instructions rendered on the matrix (see DrawInstruction).
	Draw DrawList `json:"draw,omitempty"`
	// NoScroll disables text scrolling and displays the text statically when true.
	NoScroll bool `json:"noScroll,omitempty"`
	// ScrollSpeed is the scroll speed in pixels per frame.
	ScrollSpeed int `json:"scrollSpeed,omitempty"`
	// Effect is the background animation to display behind the content text.
	// Use the Effect* constants (e.g. EffectPlasma). See available effects in enums.go.
	Effect Effect `json:"effect,omitempty"`
	// EffectSettings configures the background effect (speed, palette, blend).
	EffectSettings *EffectSettings `json:"effectSettings,omitempty"`
	// Overlay sets a weather effect overlay for this content.
	// Use the OverlayEffect* constants (e.g. OverlayEffectSnow).
	Overlay OverlayEffect `json:"overlay,omitempty"`
}

// Notification represents the payload sent to {clientId}/notify to display a
// transient message on the device. It embeds AppContent for all shared display
// fields and adds notification-specific fields.
type Notification struct {
	AppContent

	// Hold keeps the notification visible until it is explicitly dismissed when true.
	Hold bool `json:"hold,omitempty"`
	// Sound is the name of a sound file on the device filesystem to play.
	Sound string `json:"sound,omitempty"`
	// Rtttl is an RTTTL melody string to play as a notification sound.
	Rtttl string `json:"rtttl,omitempty"`
	// LoopSound loops the notification sound while the notification is displayed when true.
	LoopSound bool `json:"loopSound,omitempty"`
	// Stack queues the notification behind any currently displayed notification.
	// Pointer so that false (don't stack) can be sent explicitly (device default is true).
	Stack *bool `json:"stack,omitempty"`
	// Wakeup wakes the display from sleep before showing the notification when true.
	Wakeup bool `json:"wakeup,omitempty"`
	// Clients restricts delivery to the specified client IDs; empty = all clients.
	Clients []string `json:"clients,omitempty"`
}

// CustomApp represents the payload sent to {clientId}/custom/{appName} to create
// or update a persistent custom app in the device's app rotation. It embeds
// AppContent for all shared display fields and adds app-lifecycle fields.
type CustomApp struct {
	AppContent

	// Pos is the position of the app in the rotation order.
	Pos int `json:"pos,omitempty"`
	// Lifetime is how long the app lives in seconds; 0 = infinite.
	Lifetime int `json:"lifetime,omitempty"`
	// LifetimeMode controls when the lifetime counter resets.
	// LifetimeResetOnMessage (0) resets on each new message; LifetimeResetOnView (1) resets on each display.
	LifetimeMode LifetimeResetMode `json:"lifetimeMode,omitempty"`
	// Save persists the app across device reboots when true.
	Save bool `json:"save,omitempty"`
}
