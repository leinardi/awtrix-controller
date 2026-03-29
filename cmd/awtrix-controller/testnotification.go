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

package main

import (
	"flag"
	"strconv"
	"strings"

	"github.com/leinardi/awtrix-controller/internal/model"
)

// testNotificationParams holds the raw flag values for the --test-notification-* group.
// All fields default to their zero values; use the visited map to distinguish
// "flag was set" from "flag was left at its default".
//
// Pointer-typed notification fields (Center *bool, Stack *bool, Repeat *int) are
// represented as strings so that an explicit false / 0 can be distinguished from
// "not set" (empty string).
type testNotificationParams struct {
	text        string
	icon        string
	duration    int
	rainbow     bool
	scrollSpeed int
	noScroll    bool
	color       string
	background  string
	overlay     string // cast to model.OverlayEffect
	effect      string // cast to model.Effect
	blinkText   int
	fadeText    int
	textCase    int // cast to model.TextCaseMode
	topText     bool
	textOffset  int
	pushIcon    int    // cast to model.PushIconBehavior
	center      string // "" | "true" | "false" → *bool
	hold        bool
	sound       string
	rtttl       string
	loopSound   bool
	stack       string // "" | "true" | "false" → *bool
	wakeup      bool
	repeat      string // "" | numeric string   → *int
}

// registerTestNotificationFlags registers all --test-notification-* flags on flagSet,
// storing the parsed values into params.
func registerTestNotificationFlags(flagSet *flag.FlagSet, params *testNotificationParams) {
	flagSet.StringVar(&params.text, "test-notification-text", "",
		"Test notification: text message (debug only)")
	flagSet.StringVar(&params.icon, "test-notification-icon", "",
		"Test notification: LaMetric icon ID (debug only)")
	flagSet.IntVar(&params.duration, "test-notification-duration", 0,
		"Test notification: display duration in seconds (debug only)")
	flagSet.BoolVar(&params.rainbow, "test-notification-rainbow", false,
		"Test notification: enable rainbow text colors (debug only)")
	flagSet.IntVar(&params.scrollSpeed, "test-notification-scroll-speed", 0,
		"Test notification: scroll speed in pixels per frame (debug only)")
	flagSet.BoolVar(&params.noScroll, "test-notification-no-scroll", false,
		"Test notification: disable text scrolling (debug only)")
	flagSet.StringVar(&params.color, "test-notification-color", "",
		"Test notification: text color as hex string e.g. #FF0000 (debug only)")
	flagSet.StringVar(&params.background, "test-notification-background", "",
		"Test notification: background fill color as hex string (debug only)")
	flagSet.StringVar(
		&params.overlay,
		"test-notification-overlay",
		"",
		"Test notification: weather overlay effect (clear|snow|rain|drizzle|storm|thunder|frost) (debug only)",
	)
	flagSet.StringVar(&params.effect, "test-notification-effect", "",
		"Test notification: background animation effect name e.g. Plasma (debug only)")
	flagSet.IntVar(&params.blinkText, "test-notification-blink-text", 0,
		"Test notification: blink rate in milliseconds (debug only)")
	flagSet.IntVar(&params.fadeText, "test-notification-fade-text", 0,
		"Test notification: fade rate in milliseconds (debug only)")
	flagSet.IntVar(&params.textCase, "test-notification-text-case", 0,
		"Test notification: text case mode (0=global 1=upper 2=as-is) (debug only)")
	flagSet.BoolVar(&params.topText, "test-notification-top-text", false,
		"Test notification: display text at the top of the matrix (debug only)")
	flagSet.IntVar(&params.textOffset, "test-notification-text-offset", 0,
		"Test notification: horizontal text offset in pixels (debug only)")
	flagSet.IntVar(&params.pushIcon, "test-notification-push-icon", 0,
		"Test notification: icon behavior (0=static 1=scroll 2=fixed) (debug only)")
	flagSet.StringVar(
		&params.center,
		"test-notification-center",
		"",
		"Test notification: center text horizontally (true|false; omit to use device default) (debug only)",
	)
	flagSet.BoolVar(&params.hold, "test-notification-hold", false,
		"Test notification: keep notification visible until dismissed (debug only)")
	flagSet.StringVar(&params.sound, "test-notification-sound", "",
		"Test notification: sound file name on the device filesystem (debug only)")
	flagSet.StringVar(&params.rtttl, "test-notification-rtttl", "",
		"Test notification: RTTTL melody string to play (debug only)")
	flagSet.BoolVar(&params.loopSound, "test-notification-loop-sound", false,
		"Test notification: loop sound while notification is displayed (debug only)")
	flagSet.StringVar(
		&params.stack,
		"test-notification-stack",
		"",
		"Test notification: queue behind current notification (true|false; omit to use device default) (debug only)",
	)
	flagSet.BoolVar(&params.wakeup, "test-notification-wakeup", false,
		"Test notification: wake display from sleep before showing (debug only)")
	flagSet.StringVar(
		&params.repeat,
		"test-notification-repeat",
		"",
		"Test notification: number of times to scroll content (-1=infinite; omit to use device default) (debug only)",
	)
}

// buildTestNotification returns a *model.Notification assembled from the given
// params if at least one --test-notification-* flag appears in visited, or nil
// if no such flag was set.
//
//nolint:cyclop,funlen,gocyclo // each branch maps one CLI flag to one notification field; extraction would not reduce real complexity
func buildTestNotification(
	visited map[string]bool,
	params *testNotificationParams,
) *model.Notification {
	if !hasTestNotificationFlag(visited) {
		return nil
	}

	notif := &model.Notification{}

	if visited["test-notification-text"] {
		notif.Text = model.NewPlainText(params.text)
	}

	if visited["test-notification-icon"] {
		notif.Icon = params.icon
	}

	if visited["test-notification-duration"] {
		notif.Duration = params.duration
	}

	if visited["test-notification-rainbow"] {
		notif.Rainbow = params.rainbow
	}

	if visited["test-notification-scroll-speed"] {
		notif.ScrollSpeed = params.scrollSpeed
	}

	if visited["test-notification-no-scroll"] {
		notif.NoScroll = params.noScroll
	}

	if visited["test-notification-color"] {
		notif.Color = params.color
	}

	if visited["test-notification-background"] {
		notif.Background = params.background
	}

	if visited["test-notification-overlay"] {
		notif.Overlay = model.OverlayEffect(params.overlay)
	}

	if visited["test-notification-effect"] {
		notif.Effect = model.Effect(params.effect)
	}

	if visited["test-notification-blink-text"] {
		notif.BlinkText = params.blinkText
	}

	if visited["test-notification-fade-text"] {
		notif.FadeText = params.fadeText
	}

	if visited["test-notification-text-case"] {
		notif.TextCase = model.TextCaseMode(params.textCase)
	}

	if visited["test-notification-top-text"] {
		notif.TopText = params.topText
	}

	if visited["test-notification-text-offset"] {
		notif.TextOffset = params.textOffset
	}

	if visited["test-notification-push-icon"] {
		notif.PushIcon = model.PushIconBehavior(params.pushIcon)
	}

	if visited["test-notification-center"] {
		notif.Center = parseBoolPtr(params.center)
	}

	if visited["test-notification-hold"] {
		notif.Hold = params.hold
	}

	if visited["test-notification-sound"] {
		notif.Sound = params.sound
	}

	if visited["test-notification-rtttl"] {
		notif.Rtttl = params.rtttl
	}

	if visited["test-notification-loop-sound"] {
		notif.LoopSound = params.loopSound
	}

	if visited["test-notification-stack"] {
		notif.Stack = parseBoolPtr(params.stack)
	}

	if visited["test-notification-wakeup"] {
		notif.Wakeup = params.wakeup
	}

	if visited["test-notification-repeat"] {
		notif.Repeat = parseIntPtr(params.repeat)
	}

	return notif
}

// hasTestNotificationFlag reports whether any key in visited starts with the
// "test-notification-" prefix.
func hasTestNotificationFlag(visited map[string]bool) bool {
	for key := range visited {
		if strings.HasPrefix(key, "test-notification-") {
			return true
		}
	}

	return false
}

// parseBoolPtr converts a flag string value to a *bool pointer.
// "true" → &true, "false" → &false, anything else → nil.
func parseBoolPtr(value string) *bool {
	switch value {
	case "true":
		result := true

		return &result
	case "false":
		result := false

		return &result
	default:
		return nil
	}
}

// parseIntPtr converts a flag string value to a *int pointer.
// A valid integer string → &int, empty or invalid → nil.
func parseIntPtr(value string) *int {
	if value == "" {
		return nil
	}

	parsed, parseErr := strconv.Atoi(value)
	if parseErr != nil {
		return nil
	}

	return &parsed
}
