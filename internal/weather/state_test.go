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
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/internal/weather"
)

const (
	repeatInterval = 60 * time.Minute
	inactiveAfter  = 2
)

func makeCandidate(severity int, fp string) weather.EventCandidate {
	return weather.EventCandidate{
		Type:        weather.EventTypeThunderstorm,
		Severity:    severity,
		Fingerprint: fp,
		StartTime:   time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC),
	}
}

func TestStateManagerNewEvent(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}

	toNotify := stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	if len(toNotify) != 1 {
		t.Fatalf("len(toNotify) = %d, want 1", len(toNotify))
	}

	if !stateManager.IsActive(weather.EventTypeThunderstorm) {
		t.Error("EventTypeThunderstorm should be active after first event")
	}
}

func TestStateManagerRepeatWithinInterval(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}

	// First poll — should notify.
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// Second poll 30 min later — same fingerprint, within 60min → no notify.
	later := now.Add(30 * time.Minute)

	toNotify := stateManager.Process(candidates, later, repeatInterval, inactiveAfter)

	if len(toNotify) != 0 {
		t.Errorf("expected no repeat within interval, got %d notifications", len(toNotify))
	}
}

func TestStateManagerRepeatAfterInterval(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}

	// First poll.
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// 60 min later — should repeat.
	later := now.Add(60 * time.Minute)

	toNotify := stateManager.Process(candidates, later, repeatInterval, inactiveAfter)

	if len(toNotify) != 1 {
		t.Errorf("expected repeat after interval, got %d notifications", len(toNotify))
	}
}

func TestStateManagerEscalation(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	// First: severity 1.
	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// 5 min later: severity 2 — should trigger immediately despite < 60min.
	later := now.Add(5 * time.Minute)
	escalated := []weather.EventCandidate{makeCandidate(2, "fp1")}

	toNotify := stateManager.Process(escalated, later, repeatInterval, inactiveAfter)

	if len(toNotify) != 1 {
		t.Errorf("expected escalation notification, got %d", len(toNotify))
	}
}

func TestStateManagerFingerprintChange(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// 5 min later same severity but different fingerprint → should notify.
	later := now.Add(5 * time.Minute)
	changed := []weather.EventCandidate{makeCandidate(1, "fp2")}

	toNotify := stateManager.Process(changed, later, repeatInterval, inactiveAfter)

	if len(toNotify) != 1 {
		t.Errorf("expected notification on fingerprint change, got %d", len(toNotify))
	}
}

func TestStateManagerInactiveAfterN(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// Two consecutive polls with no thunderstorm → should become inactive.
	empty := []weather.EventCandidate{}

	stateManager.Process(empty, now.Add(15*time.Minute), repeatInterval, inactiveAfter)
	stateManager.Process(empty, now.Add(30*time.Minute), repeatInterval, inactiveAfter)

	if stateManager.IsActive(weather.EventTypeThunderstorm) {
		t.Error("EventTypeThunderstorm should be inactive after N missing polls")
	}
}

func TestStateManagerInactiveAfterNMinus1(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// Only one missing poll (N-1=1) → should still be active.
	stateManager.Process(nil, now.Add(15*time.Minute), repeatInterval, inactiveAfter)

	if !stateManager.IsActive(weather.EventTypeThunderstorm) {
		t.Error("EventTypeThunderstorm should still be active after N-1 missing polls")
	}
}

func TestStateManagerFetchFailureNoMutation(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// Fetch failure.
	stateManager.OnFetchFailure()

	// State should be unchanged.
	if !stateManager.IsActive(weather.EventTypeThunderstorm) {
		t.Error("OnFetchFailure should not change active state")
	}
}

func TestStateManagerRecoveryAfterGap(t *testing.T) {
	t.Parallel()

	now := time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC)
	stateManager := weather.NewStateManager()

	candidates := []weather.EventCandidate{makeCandidate(1, "fp1")}
	stateManager.Process(candidates, now, repeatInterval, inactiveAfter)

	// One missing poll (still active).
	stateManager.Process(nil, now.Add(15*time.Minute), repeatInterval, inactiveAfter)

	// Event reappears within 60min → still active, no resend (< repeat interval).
	later := now.Add(30 * time.Minute)

	toNotify := stateManager.Process(candidates, later, repeatInterval, inactiveAfter)

	if !stateManager.IsActive(weather.EventTypeThunderstorm) {
		t.Error("event should still be active after recovery")
	}

	if len(toNotify) != 0 {
		t.Errorf("expected no resend after gap recovery within interval, got %d", len(toNotify))
	}
}
