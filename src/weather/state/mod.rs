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

//! The notification state of each event type across polls: when an event is (re)notified, and
//! when it stops being active.

use jiff::{SignedDuration, Timestamp};

use super::events::{EventCandidate, Severity};
use super::mapping::EventType;

/// The notification state of one event type.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventState {
    /// Whether the event is in the forecast.
    pub active: bool,
    /// The fingerprint last seen.
    pub fingerprint: String,
    /// The severity last seen, `None` before the first.
    pub severity: Option<Severity>,
    /// When it was last notified.
    pub last_sent_at: Option<Timestamp>,
    /// When it was last seen.
    pub last_seen_at: Option<Timestamp>,
    /// Consecutive polls without it while active.
    pub missing_polls: i64,
}

/// The state of every event type. Not shared between threads; the controller owns it.
#[derive(Clone, Debug, Default)]
pub struct StateManager {
    states: [EventState; EventType::ALL.len()],
}

impl StateManager {
    /// Every event type inactive.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies the candidates of one successful poll and returns those to notify: an event that
    /// was not active, has a higher severity, has a new fingerprint, or was last notified at least
    /// `repeat_interval` ago. An active event missing from `inactive_after` consecutive polls
    /// becomes inactive.
    pub fn process(
        &mut self,
        candidates: &[EventCandidate],
        now: Timestamp,
        repeat_interval: SignedDuration,
        inactive_after: i64,
    ) -> Vec<EventCandidate> {
        let mut to_notify = Vec::new();

        for event_type in EventType::ALL {
            let state = &mut self.states[event_type.index()];
            // The last candidate of a type wins, as Go's map did.
            let candidate = candidates
                .iter()
                .rev()
                .find(|candidate| candidate.event_type == event_type);

            if let Some(candidate) = candidate {
                state.missing_polls = 0;
                state.last_seen_at = Some(now);

                let repeat_due = state
                    .last_sent_at
                    .is_none_or(|last_sent| now.duration_since(last_sent) >= repeat_interval);
                let should_notify = !state.active
                    || Some(candidate.severity) > state.severity
                    || candidate.fingerprint != state.fingerprint
                    || repeat_due;
                if should_notify {
                    to_notify.push(candidate.clone());
                    state.last_sent_at = Some(now);
                }

                state.active = true;
                state.fingerprint.clone_from(&candidate.fingerprint);
                state.severity = Some(candidate.severity);
            } else if state.active {
                state.missing_polls += 1;
                if state.missing_polls >= inactive_after {
                    state.active = false;
                    state.missing_polls = 0;
                }
            }
        }

        to_notify
    }

    /// A failed fetch changes nothing: every event keeps its state until a poll succeeds.
    pub const fn on_fetch_failure(&self) {}

    /// Whether `event_type` is active.
    #[must_use]
    pub fn is_active(&self, event_type: EventType) -> bool {
        self.states[event_type.index()].active
    }

    /// The state of `event_type`.
    #[must_use]
    pub fn state(&self, event_type: EventType) -> &EventState {
        &self.states[event_type.index()]
    }
}

#[cfg(test)]
mod tests;
