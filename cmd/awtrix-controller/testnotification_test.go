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
	"testing"

	"github.com/leinardi/awtrix-controller/internal/model"
)

// TestBuildTestNotification_TC_NoFlags_ReturnsNil verifies that buildTestNotification
// returns nil when no test-notification-* flag appears in visited.
func TestBuildTestNotification_TC_NoFlags_ReturnsNil(t *testing.T) {
	t.Parallel()

	result := buildTestNotification(map[string]bool{"config": true}, &testNotificationParams{})
	if result != nil {
		t.Errorf("expected nil when no test-notification-* flags visited, got %+v", result)
	}
}

// TestBuildTestNotification_TC_EmptyVisited_ReturnsNil verifies that buildTestNotification
// returns nil when the visited map is empty.
func TestBuildTestNotification_TC_EmptyVisited_ReturnsNil(t *testing.T) {
	t.Parallel()

	result := buildTestNotification(map[string]bool{}, &testNotificationParams{})
	if result != nil {
		t.Errorf("expected nil for empty visited map, got %+v", result)
	}
}

// TestBuildTestNotification_TC_OverlayOnly verifies that only Overlay is populated
// when only the overlay flag is visited.
func TestBuildTestNotification_TC_OverlayOnly(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{"test-notification-overlay": true}
	params := testNotificationParams{overlay: "snow"}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Overlay != model.OverlayEffectSnow {
		t.Errorf("expected Overlay=%q, got %q", model.OverlayEffectSnow, result.Overlay)
	}

	if result.Text != nil {
		t.Errorf("expected Text to be nil, got %+v", result.Text)
	}

	if result.Rtttl != "" {
		t.Errorf("expected Rtttl to be empty, got %q", result.Rtttl)
	}
}

// TestBuildTestNotification_TC_RtttlOnly verifies that only Rtttl is populated
// when only the rtttl flag is visited.
func TestBuildTestNotification_TC_RtttlOnly(t *testing.T) {
	t.Parallel()

	const melody = "Batman:d=16,o=5,b=180:8p,8b,8b,8b,2e."

	visited := map[string]bool{"test-notification-rtttl": true}
	params := testNotificationParams{rtttl: melody}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Rtttl != melody {
		t.Errorf("expected Rtttl=%q, got %q", melody, result.Rtttl)
	}

	if result.Overlay != "" {
		t.Errorf("expected Overlay to be empty, got %q", result.Overlay)
	}
}

// TestBuildTestNotification_TC_TextAndIcon verifies that Text and Icon are set
// correctly when both flags are visited.
func TestBuildTestNotification_TC_TextAndIcon(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{
		"test-notification-text": true,
		"test-notification-icon": true,
	}
	params := testNotificationParams{text: "Hello", icon: "1234"}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Text == nil {
		t.Fatal("expected Text to be non-nil")

		return
	}

	if result.Icon != "1234" {
		t.Errorf("expected Icon=%q, got %q", "1234", result.Icon)
	}
}

// TestBuildTestNotification_TC_BoolPtrCenter_True verifies that Center is set to
// &true when center="true" is visited.
func TestBuildTestNotification_TC_BoolPtrCenter_True(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{"test-notification-center": true}
	params := testNotificationParams{center: "true"}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Center == nil {
		t.Fatal("expected Center to be non-nil")

		return
	}

	if !*result.Center {
		t.Errorf("expected Center=true, got false")
	}
}

// TestBuildTestNotification_TC_BoolPtrCenter_False verifies that Center is set to
// &false when center="false" is visited.
func TestBuildTestNotification_TC_BoolPtrCenter_False(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{"test-notification-center": true}
	params := testNotificationParams{center: "false"}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Center == nil {
		t.Fatal("expected Center to be non-nil")

		return
	}

	if *result.Center {
		t.Errorf("expected Center=false, got true")
	}
}

// TestBuildTestNotification_TC_BoolPtrCenter_Empty verifies that Center is nil
// when center="" is visited (empty string = omit from payload).
func TestBuildTestNotification_TC_BoolPtrCenter_Empty(t *testing.T) {
	t.Parallel()

	// Another flag triggers test-notification mode; center is visited but empty.
	visited := map[string]bool{
		"test-notification-overlay": true,
		"test-notification-center":  true,
	}
	params := testNotificationParams{overlay: "rain", center: ""}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Center != nil {
		t.Errorf("expected Center=nil for empty string, got %v", *result.Center)
	}
}

// TestBuildTestNotification_TC_RepeatPtr verifies that Repeat is set to &3
// when repeat="3" is visited.
func TestBuildTestNotification_TC_RepeatPtr(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{"test-notification-repeat": true}
	params := testNotificationParams{repeat: "3"}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Repeat == nil {
		t.Fatal("expected Repeat to be non-nil")

		return
	}

	if *result.Repeat != 3 {
		t.Errorf("expected Repeat=3, got %d", *result.Repeat)
	}
}

// TestBuildTestNotification_TC_RepeatPtr_Empty verifies that Repeat is nil
// when repeat="" is visited (empty string = omit from payload).
func TestBuildTestNotification_TC_RepeatPtr_Empty(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{
		"test-notification-overlay": true,
		"test-notification-repeat":  true,
	}
	params := testNotificationParams{overlay: "frost", repeat: ""}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Repeat != nil {
		t.Errorf("expected Repeat=nil for empty string, got %d", *result.Repeat)
	}
}

// TestBuildTestNotification_TC_StackPtr verifies that Stack is set correctly
// when the stack flag is visited.
func TestBuildTestNotification_TC_StackPtr(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{"test-notification-stack": true}
	params := testNotificationParams{stack: "false"}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Stack == nil {
		t.Fatal("expected Stack to be non-nil")

		return
	}

	if *result.Stack {
		t.Errorf("expected Stack=false, got true")
	}
}

// TestBuildTestNotification_TC_FullNotification verifies that all simple fields are
// populated correctly when all flags are visited.
//
//nolint:cyclop,gocyclo // each assertion checks one notification field; extraction would not reduce real complexity
func TestBuildTestNotification_TC_FullNotification(t *testing.T) {
	t.Parallel()

	visited := map[string]bool{
		"test-notification-text":         true,
		"test-notification-icon":         true,
		"test-notification-duration":     true,
		"test-notification-rainbow":      true,
		"test-notification-scroll-speed": true,
		"test-notification-no-scroll":    true,
		"test-notification-color":        true,
		"test-notification-background":   true,
		"test-notification-overlay":      true,
		"test-notification-effect":       true,
		"test-notification-blink-text":   true,
		"test-notification-fade-text":    true,
		"test-notification-text-case":    true,
		"test-notification-top-text":     true,
		"test-notification-text-offset":  true,
		"test-notification-push-icon":    true,
		"test-notification-hold":         true,
		"test-notification-sound":        true,
		"test-notification-rtttl":        true,
		"test-notification-loop-sound":   true,
		"test-notification-wakeup":       true,
	}

	params := testNotificationParams{
		text:        "Full test",
		icon:        "42",
		duration:    30,
		rainbow:     true,
		scrollSpeed: 75,
		noScroll:    true,
		color:       "#FF0000",
		background:  "#000000",
		overlay:     "thunder",
		effect:      "Plasma",
		blinkText:   500,
		fadeText:    200,
		textCase:    1,
		topText:     true,
		textOffset:  5,
		pushIcon:    2,
		hold:        true,
		sound:       "alarm",
		rtttl:       "scale:d=4,o=5,b=100:c,d,e,f",
		loopSound:   true,
		wakeup:      true,
	}

	result := buildTestNotification(visited, &params)

	if result == nil {
		t.Fatal("expected non-nil notification")

		return
	}

	if result.Text == nil {
		t.Error("expected Text to be non-nil")
	}

	if result.Icon != "42" {
		t.Errorf("Icon: expected %q, got %q", "42", result.Icon)
	}

	if result.Duration != 30 {
		t.Errorf("Duration: expected 30, got %d", result.Duration)
	}

	if !result.Rainbow {
		t.Error("Rainbow: expected true")
	}

	if result.ScrollSpeed != 75 {
		t.Errorf("ScrollSpeed: expected 75, got %d", result.ScrollSpeed)
	}

	if !result.NoScroll {
		t.Error("NoScroll: expected true")
	}

	if result.Color != "#FF0000" {
		t.Errorf("Color: expected %q, got %q", "#FF0000", result.Color)
	}

	if result.Background != "#000000" {
		t.Errorf("Background: expected %q, got %q", "#000000", result.Background)
	}

	if result.Overlay != model.OverlayEffectThunder {
		t.Errorf("Overlay: expected %q, got %q", model.OverlayEffectThunder, result.Overlay)
	}

	if result.Effect != model.EffectPlasma {
		t.Errorf("Effect: expected %q, got %q", model.EffectPlasma, result.Effect)
	}

	if result.BlinkText != 500 {
		t.Errorf("BlinkText: expected 500, got %d", result.BlinkText)
	}

	if result.FadeText != 200 {
		t.Errorf("FadeText: expected 200, got %d", result.FadeText)
	}

	if result.TextCase != model.TextCaseUppercase {
		t.Errorf("TextCase: expected %d, got %d", model.TextCaseUppercase, result.TextCase)
	}

	if !result.TopText {
		t.Error("TopText: expected true")
	}

	if result.TextOffset != 5 {
		t.Errorf("TextOffset: expected 5, got %d", result.TextOffset)
	}

	if result.PushIcon != model.PushIconFixed {
		t.Errorf("PushIcon: expected %d, got %d", model.PushIconFixed, result.PushIcon)
	}

	if !result.Hold {
		t.Error("Hold: expected true")
	}

	if result.Sound != "alarm" {
		t.Errorf("Sound: expected %q, got %q", "alarm", result.Sound)
	}

	if result.Rtttl != "scale:d=4,o=5,b=100:c,d,e,f" {
		t.Errorf("Rtttl: expected %q, got %q", "scale:d=4,o=5,b=100:c,d,e,f", result.Rtttl)
	}

	if !result.LoopSound {
		t.Error("LoopSound: expected true")
	}

	if !result.Wakeup {
		t.Error("Wakeup: expected true")
	}
}
