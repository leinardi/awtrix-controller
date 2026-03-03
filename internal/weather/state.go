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

package weather

import "time"

// EventState tracks the notification lifecycle for one EventType.
type EventState struct {
	Active       bool
	Fingerprint  string
	Severity     int
	LastSentAt   time.Time
	LastSeenAt   time.Time
	MissingPolls int
}

// StateManager manages per-type EventState. Not goroutine-safe; caller serializes.
type StateManager struct {
	states map[EventType]*EventState
}

// NewStateManager returns a StateManager pre-populated for all 7 event types.
func NewStateManager() *StateManager {
	states := make(map[EventType]*EventState, len(allEventTypes))

	for _, eventType := range allEventTypes {
		states[eventType] = &EventState{}
	}

	return &StateManager{states: states}
}

// Process evaluates candidates from one successful poll against current state.
// Returns the subset of candidates that should trigger notifications.
// Updates Active/MissingPolls/Fingerprint/Severity/LastSentAt for all types.
func (sm *StateManager) Process(
	candidates []EventCandidate,
	now time.Time,
	repeatInterval time.Duration,
	inactiveAfter int,
) []EventCandidate {
	// Build a lookup set of candidates by type.
	byType := make(map[EventType]EventCandidate, len(candidates))

	for _, candidate := range candidates {
		byType[candidate.Type] = candidate
	}

	var toNotify []EventCandidate

	for _, eventType := range allEventTypes {
		state := sm.states[eventType]
		candidate, found := byType[eventType]

		if found {
			state.MissingPolls = 0
			state.LastSeenAt = now

			shouldNotify := !state.Active ||
				candidate.Severity > state.Severity ||
				candidate.Fingerprint != state.Fingerprint ||
				now.Sub(state.LastSentAt) >= repeatInterval

			if shouldNotify {
				toNotify = append(toNotify, candidate)
				state.LastSentAt = now
			}

			state.Active = true
			state.Fingerprint = candidate.Fingerprint
			state.Severity = candidate.Severity
		} else if state.Active {
			state.MissingPolls++

			if state.MissingPolls >= inactiveAfter {
				state.Active = false
				state.MissingPolls = 0
			}
		}
	}

	return toNotify
}

// OnFetchFailure is a no-op: preserves all state on fetch errors (fail-safe).
func (*StateManager) OnFetchFailure() {}

// IsActive reports whether the given event type is currently active.
func (sm *StateManager) IsActive(eventType EventType) bool {
	state, found := sm.states[eventType]
	if !found {
		return false
	}

	return state.Active
}
