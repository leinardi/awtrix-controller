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

//! The current time, behind a trait so that every time-dependent module can be tested without
//! reading the wall clock, and the one conversion from a local date and wall-clock time to an
//! instant.
//!
//! This module is the only one allowed to read the wall clock (`clippy.toml` bans
//! `Timestamp::now` and its kin everywhere else).

use std::sync::{Arc, Mutex, PoisonError};

use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

/// A source of the current time.
pub trait Clock: Send + Sync {
    /// The current instant.
    fn now(&self) -> Timestamp;
}

/// The wall clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    #[expect(
        clippy::disallowed_methods,
        reason = "the production clock is the one place that reads the wall clock"
    )]
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// A clock that follows tokio's time: a base instant plus the time tokio says has elapsed since
/// the clock was made. Under a paused runtime (`start_paused`) wall time and timers then move
/// together, which `Timestamp::now` would not.
#[derive(Clone, Copy, Debug)]
pub struct TokioClock {
    base: Timestamp,
    started: tokio::time::Instant,
}

impl TokioClock {
    /// A clock that reads `base` now and moves with tokio's time from here on.
    #[must_use]
    pub fn new(base: Timestamp) -> Self {
        Self {
            base,
            started: tokio::time::Instant::now(),
        }
    }
}

impl Clock for TokioClock {
    fn now(&self) -> Timestamp {
        let elapsed = tokio::time::Instant::now().duration_since(self.started);
        let elapsed = SignedDuration::try_from(elapsed).unwrap_or(SignedDuration::MAX);
        self.base.saturating_add(elapsed).unwrap_or(Timestamp::MAX)
    }
}

/// A clock the caller sets and advances by hand. Safe to share between threads.
#[derive(Clone, Debug)]
pub struct FakeClock {
    now: Arc<Mutex<Timestamp>>,
}

impl FakeClock {
    /// A clock reading `now`.
    #[must_use]
    pub fn new(now: Timestamp) -> Self {
        Self {
            now: Arc::new(Mutex::new(now)),
        }
    }

    /// Sets the current instant.
    pub fn set(&self, now: Timestamp) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = now;
    }

    /// Moves the clock by `duration` (backwards when negative).
    pub fn advance(&self, duration: SignedDuration) {
        let mut now = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        *now = now.saturating_add(duration).unwrap_or(*now);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Converts a local date and wall-clock time in `zone` to an instant. Every such conversion goes
/// through here, so the daylight-saving policy is decided once:
///
/// - **Gap** (the wall time does not exist, the clocks jump over it): the wall time shifted
///   forward by the gap's length, so 02:30 on a spring-forward day with a one-hour gap is 03:30.
/// - **Fold** (the wall time happens twice, the clocks fall back over it): the earlier instant.
///
/// That is jiff's `Disambiguation::Compatible`, which is also RFC 5545's rule.
///
/// # Errors
///
/// The date and time lie outside the range jiff represents.
pub fn resolve_local(date: Date, time: Time, zone: &TimeZone) -> Result<Timestamp, jiff::Error> {
    let zoned = zone
        .to_ambiguous_zoned(date.to_datetime(time))
        .compatible()?;
    Ok(zoned.timestamp())
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use jiff::civil::{date, time};

    use super::*;

    fn instant(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn zone(name: &str) -> TimeZone {
        crate::config::load_time_zone(name).expect("zone")
    }

    /// Go: `TestRealClockNowIsRecent`
    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "checking the production clock against the wall clock"
    )]
    fn system_clock_now_is_recent() {
        let before = Timestamp::now();
        let now = SystemClock.now();
        let after = Timestamp::now();
        let tolerance = SignedDuration::from_secs(1);
        assert!(now >= before - tolerance && now <= after + tolerance);
    }

    /// Go: `TestFakeClockNowReturnsSetValue`
    #[test]
    fn fake_clock_now_returns_set_value() {
        let fixed = instant("2024-06-15T12:00:00Z");
        let clock = FakeClock::new(fixed);
        assert_eq!(clock.now(), fixed);

        let later = fixed + SignedDuration::from_hours(1);
        clock.set(later);
        assert_eq!(clock.now(), later);
    }

    /// Go: `TestFakeClockAdvance`
    #[test]
    fn fake_clock_advance() {
        let start = instant("2024-01-01T00:00:00Z");
        let clock = FakeClock::new(start);
        clock.advance(SignedDuration::from_mins(30));
        assert_eq!(clock.now(), start + SignedDuration::from_mins(30));
    }

    /// Go: `TestFakeClockConcurrentSafety`
    #[test]
    fn fake_clock_concurrent_safety() {
        const THREADS: usize = 10;
        const ITERATIONS: i64 = 100;

        let start = instant("2024-01-01T00:00:00Z");
        let clock = FakeClock::new(start);
        let mut handles = Vec::new();
        for _ in 0..THREADS {
            let writer = clock.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    writer.advance(SignedDuration::from_millis(1));
                }
            }));
            let reader = clock.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    let _ = reader.now();
                }
            }));
        }
        for handle in handles {
            handle.join().expect("thread");
        }

        let threads = i64::try_from(THREADS).expect("small");
        assert_eq!(
            clock.now(),
            start + SignedDuration::from_millis(threads * ITERATIONS)
        );
    }

    /// The test clock moves with tokio's paused time, so a timer and the wall time agree.
    #[tokio::test(start_paused = true)]
    async fn tokio_clock_follows_paused_time() {
        let base = instant("2026-03-29T00:00:00Z");
        let clock = TokioClock::new(base);
        assert_eq!(clock.now(), base);

        tokio::time::sleep(Duration::from_mins(90)).await;
        assert_eq!(clock.now(), base + SignedDuration::from_mins(90));

        tokio::time::advance(Duration::from_millis(250)).await;
        assert_eq!(
            clock.now(),
            base + SignedDuration::from_mins(90) + SignedDuration::from_millis(250)
        );
    }

    /// Berlin's spring-forward day (2026-03-29, 02:00 → 03:00 CET → CEST): a time in the gap
    /// moves forward by the gap, an ordinary time is unchanged.
    #[test]
    fn resolve_local_berlin_spring_forward() {
        let berlin = zone("Europe/Berlin");
        let day = date(2026, 3, 29);
        let gap = resolve_local(day, time(2, 30, 0, 0), &berlin).expect("gap");
        assert_eq!(gap, instant("2026-03-29T01:30:00Z")); // 03:30 CEST
        let gap_start = resolve_local(day, time(2, 0, 0, 0), &berlin).expect("gap start");
        assert_eq!(gap_start, instant("2026-03-29T01:00:00Z")); // 03:00 CEST
        let ordinary = resolve_local(day, time(12, 0, 0, 0), &berlin).expect("ordinary");
        assert_eq!(ordinary, instant("2026-03-29T10:00:00Z"));
        let before = resolve_local(day, time(1, 59, 0, 0), &berlin).expect("before");
        assert_eq!(before, instant("2026-03-29T00:59:00Z")); // 01:59 CET
    }

    /// Berlin's fall-back day (2026-10-25, 03:00 CEST → 02:00 CET): a time in the fold takes the
    /// earlier instant.
    #[test]
    fn resolve_local_berlin_fall_back() {
        let berlin = zone("Europe/Berlin");
        let day = date(2026, 10, 25);
        let fold = resolve_local(day, time(2, 30, 0, 0), &berlin).expect("fold");
        assert_eq!(fold, instant("2026-10-25T00:30:00Z")); // the first 02:30, CEST
        let ordinary = resolve_local(day, time(12, 0, 0, 0), &berlin).expect("ordinary");
        assert_eq!(ordinary, instant("2026-10-25T11:00:00Z")); // CET
        let after = resolve_local(day, time(3, 0, 0, 0), &berlin).expect("after");
        assert_eq!(after, instant("2026-10-25T02:00:00Z")); // 03:00 CET
    }

    /// New York's spring-forward day (2026-03-08, 02:00 → 03:00 EST → EDT).
    #[test]
    fn resolve_local_new_york_spring_forward() {
        let new_york = zone("America/New_York");
        let day = date(2026, 3, 8);
        let gap = resolve_local(day, time(2, 30, 0, 0), &new_york).expect("gap");
        assert_eq!(gap, instant("2026-03-08T07:30:00Z")); // 03:30 EDT
        let ordinary = resolve_local(day, time(12, 0, 0, 0), &new_york).expect("ordinary");
        assert_eq!(ordinary, instant("2026-03-08T16:00:00Z")); // EDT
    }

    /// New York's fall-back day (2026-11-01, 02:00 EDT → 01:00 EST).
    #[test]
    fn resolve_local_new_york_fall_back() {
        let new_york = zone("America/New_York");
        let day = date(2026, 11, 1);
        let fold = resolve_local(day, time(1, 30, 0, 0), &new_york).expect("fold");
        assert_eq!(fold, instant("2026-11-01T05:30:00Z")); // the first 01:30, EDT
        let ordinary = resolve_local(day, time(12, 0, 0, 0), &new_york).expect("ordinary");
        assert_eq!(ordinary, instant("2026-11-01T17:00:00Z")); // EST
    }
}
