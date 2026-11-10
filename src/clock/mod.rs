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
//! The clock itself ([`Clock`], [`TokioClock`] and the tests' `FakeClock`) is rust-common's
//! `clock`, re-exported here so the call sites name `crate::clock`. `clock::SystemClock` is
//! deliberately not re-exported: only the production wiring in `run` builds it, and the debug
//! weather simulation reads the wall clock directly (`clippy.toml` bans `Timestamp::now` and its
//! kin, and `clock::SystemClock` itself, in this crate).

use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

#[cfg(test)]
pub(crate) use clock::FakeClock;
pub use clock::{Clock, TokioClock};

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

/// The start of the local day after `instant` in `zone`, resolved through [`resolve_local`] so a
/// zone whose clocks change at midnight follows the same policy.
#[must_use]
pub fn next_local_midnight(instant: Timestamp, zone: &TimeZone) -> Timestamp {
    let today = instant.to_zoned(zone.clone()).date();
    today
        .tomorrow()
        .ok()
        .and_then(|tomorrow| resolve_local(tomorrow, Time::midnight(), zone).ok())
        .unwrap_or_else(|| instant.saturating_add(1.day()).unwrap_or(Timestamp::MAX))
}

#[cfg(test)]
mod tests {
    use jiff::civil::{date, time};

    use super::*;

    fn instant(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn zone(name: &str) -> TimeZone {
        crate::config::load_time_zone(name).expect("zone")
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

    /// The next local midnight is the start of the following local day, not 24 hours on, even
    /// across a daylight-saving change.
    #[test]
    fn next_local_midnight_is_the_next_local_day() {
        let berlin = zone("Europe/Berlin");
        assert_eq!(
            next_local_midnight(instant("2026-03-28T22:59:59Z"), &berlin), // 23:59:59 CET
            instant("2026-03-28T23:00:00Z")
        );
        assert_eq!(
            next_local_midnight(instant("2026-03-28T23:00:00Z"), &berlin), // 00:00 CET
            instant("2026-03-29T22:00:00Z")                                // 00:00 CEST, 23 hours on
        );
        assert_eq!(
            next_local_midnight(instant("2026-10-25T12:00:00Z"), &berlin),
            instant("2026-10-25T23:00:00Z")
        );
        assert_eq!(
            next_local_midnight(instant("2024-01-15T14:00:00Z"), &TimeZone::UTC),
            instant("2024-01-16T00:00:00Z")
        );
        // Santiago's clocks spring forward at midnight: 00:00 does not exist on 2026-09-06, so
        // the day starts at 01:00.
        let santiago = zone("America/Santiago");
        assert_eq!(
            next_local_midnight(instant("2026-09-05T12:00:00Z"), &santiago),
            instant("2026-09-06T04:00:00Z")
        );
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
