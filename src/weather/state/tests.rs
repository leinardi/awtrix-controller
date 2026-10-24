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

use super::*;

const INACTIVE_AFTER: i64 = 2;

fn repeat_interval() -> SignedDuration {
    SignedDuration::from_mins(60)
}

fn now() -> Timestamp {
    "2025-01-01T12:00:00Z".parse().expect("timestamp")
}

fn later(minutes: i64) -> Timestamp {
    now() + SignedDuration::from_mins(minutes)
}

fn candidate(severity: Severity, fingerprint: &str) -> EventCandidate {
    EventCandidate {
        event_type: EventType::Thunderstorm,
        start_time: now(),
        severity,
        fingerprint: fingerprint.to_owned(),
    }
}

fn process(manager: &mut StateManager, candidates: &[EventCandidate], at: Timestamp) -> usize {
    manager
        .process(candidates, at, repeat_interval(), INACTIVE_AFTER)
        .len()
}

/// Go: `TestStateManagerNewEvent`
#[test]
fn new_event_is_notified() {
    let mut manager = StateManager::new();
    assert_eq!(
        process(&mut manager, &[candidate(Severity::Warning, "fp1")], now()),
        1
    );
    assert!(manager.is_active(EventType::Thunderstorm));
}

/// Go: `TestStateManagerRepeatWithinInterval`
#[test]
fn no_repeat_within_interval() {
    let mut manager = StateManager::new();
    let candidates = [candidate(Severity::Warning, "fp1")];
    process(&mut manager, &candidates, now());
    assert_eq!(process(&mut manager, &candidates, later(30)), 0);
}

/// Go: `TestStateManagerRepeatAfterInterval`
#[test]
fn repeat_after_interval() {
    let mut manager = StateManager::new();
    let candidates = [candidate(Severity::Warning, "fp1")];
    process(&mut manager, &candidates, now());
    assert_eq!(process(&mut manager, &candidates, later(60)), 1);
}

/// Go: `TestStateManagerEscalation`
#[test]
fn escalation_is_notified_at_once() {
    let mut manager = StateManager::new();
    process(&mut manager, &[candidate(Severity::Warning, "fp1")], now());
    assert_eq!(
        process(
            &mut manager,
            &[candidate(Severity::Severe, "fp1")],
            later(5)
        ),
        1
    );
}

/// Go: `TestStateManagerFingerprintChange`
#[test]
fn fingerprint_change_is_notified() {
    let mut manager = StateManager::new();
    process(&mut manager, &[candidate(Severity::Warning, "fp1")], now());
    assert_eq!(
        process(
            &mut manager,
            &[candidate(Severity::Warning, "fp2")],
            later(5)
        ),
        1
    );
}

/// Go: `TestStateManagerInactiveAfterN`
#[test]
fn inactive_after_n_missing_polls() {
    let mut manager = StateManager::new();
    process(&mut manager, &[candidate(Severity::Warning, "fp1")], now());
    process(&mut manager, &[], later(15));
    process(&mut manager, &[], later(30));
    assert!(!manager.is_active(EventType::Thunderstorm));
}

/// Go: `TestStateManagerInactiveAfterNMinus1`
#[test]
fn still_active_after_n_minus_1_missing_polls() {
    let mut manager = StateManager::new();
    process(&mut manager, &[candidate(Severity::Warning, "fp1")], now());
    process(&mut manager, &[], later(15));
    assert!(manager.is_active(EventType::Thunderstorm));
}

/// Go: `TestStateManagerFetchFailureNoMutation`
#[test]
fn fetch_failure_changes_nothing() {
    let mut manager = StateManager::new();
    process(&mut manager, &[candidate(Severity::Warning, "fp1")], now());
    let before = manager.state(EventType::Thunderstorm).clone();
    manager.on_fetch_failure();
    assert!(manager.is_active(EventType::Thunderstorm));
    assert_eq!(manager.state(EventType::Thunderstorm), &before);
}

/// Go: `TestStateManagerRecoveryAfterGap`
#[test]
fn recovery_after_gap_is_not_resent() {
    let mut manager = StateManager::new();
    let candidates = [candidate(Severity::Warning, "fp1")];
    process(&mut manager, &candidates, now());
    process(&mut manager, &[], later(15));
    assert_eq!(process(&mut manager, &candidates, later(30)), 0);
    assert!(manager.is_active(EventType::Thunderstorm));
}

/// An event that went inactive is notified again when it returns, whatever the interval.
#[test]
fn returning_event_is_notified() {
    let mut manager = StateManager::new();
    let candidates = [candidate(Severity::Warning, "fp1")];
    process(&mut manager, &candidates, now());
    process(&mut manager, &[], later(15));
    process(&mut manager, &[], later(30));
    assert_eq!(process(&mut manager, &candidates, later(45)), 1);
    let state = manager.state(EventType::Thunderstorm);
    assert_eq!(state.last_sent_at, Some(later(45)));
    assert_eq!(state.missing_polls, 0);
}
