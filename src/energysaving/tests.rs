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

use std::thread;
use std::time::Duration;

use jiff::SignedDuration;
use jiff::civil::{date, time};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;

use super::*;
use crate::clock::TokioClock;
use crate::test_support::Captured;

fn instant(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn zone(name: &str) -> TimeZone {
    crate::config::load_time_zone(name).expect("zone")
}

fn berlin() -> TimeZone {
    zone("Europe/Berlin")
}

fn new_york() -> TimeZone {
    zone("America/New_York")
}

fn config(start: &str, end: &str) -> EnergySavingConfig {
    EnergySavingConfig {
        start: start.to_owned(),
        end: end.to_owned(),
    }
}

fn window(start: &str, end: &str, zone: TimeZone) -> Window {
    Window::new(
        start.parse().expect("start"),
        end.parse().expect("end"),
        zone,
    )
}

/// A controller started at `now` on a paused clock, its scheduler, and the state changes it
/// reports with the time of each.
fn started(
    start: &str,
    end: &str,
    zone: TimeZone,
    now: Timestamp,
) -> (
    Arc<EnergySaving>,
    Scheduler,
    mpsc::UnboundedReceiver<(bool, Timestamp)>,
) {
    let clock = TokioClock::new(now);
    let scheduler = Scheduler::new(Arc::new(clock), &CancellationToken::new());
    let (changes, received) = mpsc::unbounded_channel();
    let on_change: OnActiveChange = Arc::new(move |active| {
        let _ = changes.send((active, clock.now()));
        Box::pin(std::future::ready(()))
    });
    let controller =
        EnergySaving::new(&config(start, end), zone, Arc::new(clock), on_change).expect("valid");
    controller.start(&scheduler);
    (controller, scheduler, received)
}

/// Go: `TestActivationAndDeactivationTC03TC04`
#[tokio::test(start_paused = true)]
async fn activation_and_deactivation() {
    let now = instant("2024-01-15T01:59:00Z");
    let (controller, scheduler, mut changes) = started("02:00", "06:00", TimeZone::UTC, now);
    assert!(!controller.is_active(), "before the window");

    let activation = changes.recv().await.expect("activation");
    assert_eq!(activation, (true, instant("2024-01-15T02:00:00Z")));
    assert!(controller.is_active());

    let deactivation = changes.recv().await.expect("deactivation");
    assert_eq!(deactivation, (false, instant("2024-01-15T06:00:00Z")));
    assert!(!controller.is_active());
    scheduler.stop().await;
}

/// Go: `TestMidnightSpanningWindowTC09`
#[tokio::test(start_paused = true)]
async fn midnight_spanning_window() {
    let cases = [
        ("2024-01-15T22:59:00Z", false),
        ("2024-01-15T23:30:00Z", true),
        ("2024-01-15T04:59:00Z", true),
        ("2024-01-15T05:01:00Z", false),
    ];
    for (now, want) in cases {
        let (controller, scheduler, _changes) =
            started("23:00", "05:00", TimeZone::UTC, instant(now));
        assert_eq!(controller.is_active(), want, "at {now}");
        scheduler.stop().await;
    }

    let now = instant("2024-01-15T22:59:00Z");
    let (controller, scheduler, mut changes) = started("23:00", "05:00", TimeZone::UTC, now);
    assert!(!controller.is_active());
    let activation = changes.recv().await.expect("activation");
    assert_eq!(activation, (true, instant("2024-01-15T23:00:00Z")));
    let deactivation = changes.recv().await.expect("deactivation");
    assert_eq!(deactivation, (false, instant("2024-01-16T05:00:00Z")));
    scheduler.stop().await;
}

/// Go: `TestTimezoneAwarenessTC21`
#[tokio::test(start_paused = true)]
async fn timezone_awareness() {
    let now = instant("2024-01-15T07:00:00Z"); // 02:00 EST
    let (controller, scheduler, _changes) = started("02:00", "06:00", new_york(), now);
    assert!(controller.is_active());
    scheduler.stop().await;
}

/// Go: `TestConcurrentIsActiveReads`
#[tokio::test]
async fn concurrent_is_active_reads() {
    const READERS: usize = 50;

    let now = instant("2024-06-15T12:00:00Z");
    let (controller, scheduler, _changes) = started("02:00", "06:00", TimeZone::UTC, now);
    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let controller = Arc::clone(&controller);
            thread::spawn(move || controller.is_active())
        })
        .collect();
    for reader in readers {
        assert!(!reader.join().expect("reader"));
    }
    scheduler.stop().await;
}

/// A wall clock that lags the timer (here it never moves at all) still applies each transition:
/// the state is read at the boundary that fired, not at a wall-clock time just before it.
#[tokio::test(start_paused = true)]
async fn lagging_wall_clock_still_applies_transitions() {
    let frozen = instant("2024-01-15T01:59:00Z");
    let clock = Arc::new(crate::clock::FakeClock::new(frozen));
    let scheduler = Scheduler::new(clock.clone(), &CancellationToken::new());
    let (changes, mut received) = mpsc::unbounded_channel();
    let on_change: OnActiveChange = Arc::new(move |active| {
        let _ = changes.send(active);
        Box::pin(std::future::ready(()))
    });
    let controller = EnergySaving::new(&config("02:00", "06:00"), TimeZone::UTC, clock, on_change)
        .expect("valid");
    controller.start(&scheduler);

    // Without the fix the job keeps re-arming without a change, so give up after a day.
    let day = Duration::from_hours(24);
    let activation = tokio::time::timeout(day, received.recv()).await;
    assert_eq!(activation, Ok(Some(true)));
    let deactivation = tokio::time::timeout(day, received.recv()).await;
    assert_eq!(deactivation, Ok(Some(false)));
    scheduler.stop().await;
}

/// Go's `Start` returned an error for a time that is not HH:MM; here the constructor does.
#[test]
fn invalid_time_is_rejected() {
    let on_change: OnActiveChange = Arc::new(|_| Box::pin(std::future::ready(())));
    let clock = Arc::new(crate::clock::FakeClock::new(instant(
        "2024-01-15T00:00:00Z",
    )));
    for (start, end) in [("25:00", "06:00"), ("02:00", "6pm"), ("", "06:00")] {
        let result = EnergySaving::new(
            &config(start, end),
            TimeZone::UTC,
            Arc::clone(&clock) as Arc<dyn Clock>,
            Arc::clone(&on_change),
        );
        assert!(result.is_err(), "{start}-{end} accepted");
    }
}

/// The two daylight-saving transition days of each zone.
fn transition_days() -> [(TimeZone, Date); 4] {
    [
        (berlin(), date(2026, 3, 29)),
        (berlin(), date(2026, 10, 25)),
        (new_york(), date(2026, 3, 8)),
        (new_york(), date(2026, 11, 1)),
    ]
}

/// Q10: a 07:00 start fires at 07:00 local on a transition day (Go's elapsed-since-midnight
/// arithmetic fired it at 08:00 on spring-forward days and 06:00 on fall-back days), and a 09:00
/// end at 09:00.
#[tokio::test(start_paused = true)]
async fn seven_o_clock_fires_at_seven_local_on_transition_days() {
    for (zone, day) in transition_days() {
        let evening_before =
            resolve_local(day.yesterday().expect("date"), time(20, 0, 0, 0), &zone)
                .expect("instant");
        let (_controller, scheduler, mut changes) =
            started("07:00", "09:00", zone.clone(), evening_before);

        let (active, at) = changes.recv().await.expect("activation");
        assert!(active);
        let local = at.to_zoned(zone.clone());
        assert_eq!(
            (local.date(), local.time()),
            (day, time(7, 0, 0, 0)),
            "{zone:?}"
        );

        let (active, at) = changes.recv().await.expect("deactivation");
        assert!(!active);
        let local = at.to_zoned(zone.clone());
        assert_eq!(
            (local.date(), local.time()),
            (day, time(9, 0, 0, 0)),
            "{zone:?}"
        );
        scheduler.stop().await;
    }
}

/// Checks each `(date, start, end, opens, closes)` window of `zone`.
fn assert_windows(zone: &TimeZone, cases: &[(Date, &str, &str, &str, &str)]) {
    for &(day, start, end, opens, closes) in cases {
        assert_eq!(
            window(start, end, zone.clone()).for_date(day),
            Some((instant(opens), instant(closes))),
            "{zone:?} {day} {start}-{end}"
        );
    }
}

/// Q10 in Berlin: a start or end inside the gap moves forward by the gap, one inside the fold
/// takes the earlier instant, and an ordinary time on the transition day is unchanged.
#[test]
fn berlin_boundaries_follow_the_dst_policy() {
    // Spring forward on 2026-03-29, 02:00 CET to 03:00 CEST; fall back on 2026-10-25, 03:00 CEST
    // to 02:00 CET.
    let spring = date(2026, 3, 29);
    let fall = date(2026, 10, 25);
    assert_windows(
        &berlin(),
        &[
            (
                spring,
                "02:30",
                "07:00",
                "2026-03-29T01:30:00Z",
                "2026-03-29T05:00:00Z",
            ),
            (
                spring,
                "01:00",
                "02:30",
                "2026-03-29T00:00:00Z",
                "2026-03-29T01:30:00Z",
            ),
            (
                spring,
                "12:00",
                "13:00",
                "2026-03-29T10:00:00Z",
                "2026-03-29T11:00:00Z",
            ),
            (
                date(2026, 3, 28),
                "23:00",
                "02:30",
                "2026-03-28T22:00:00Z",
                "2026-03-29T01:30:00Z",
            ),
            (
                fall,
                "02:30",
                "07:00",
                "2026-10-25T00:30:00Z",
                "2026-10-25T06:00:00Z",
            ),
            (
                fall,
                "01:00",
                "02:30",
                "2026-10-24T23:00:00Z",
                "2026-10-25T00:30:00Z",
            ),
            (
                fall,
                "12:00",
                "13:00",
                "2026-10-25T11:00:00Z",
                "2026-10-25T12:00:00Z",
            ),
        ],
    );
}

/// Q10 in New York, likewise.
#[test]
fn new_york_boundaries_follow_the_dst_policy() {
    // Spring forward on 2026-03-08, 02:00 EST to 03:00 EDT; fall back on 2026-11-01, 02:00 EDT
    // to 01:00 EST.
    let spring = date(2026, 3, 8);
    let fall = date(2026, 11, 1);
    assert_windows(
        &new_york(),
        &[
            (
                spring,
                "02:30",
                "07:00",
                "2026-03-08T07:30:00Z",
                "2026-03-08T11:00:00Z",
            ),
            (
                spring,
                "01:00",
                "02:30",
                "2026-03-08T06:00:00Z",
                "2026-03-08T07:30:00Z",
            ),
            (
                spring,
                "12:00",
                "13:00",
                "2026-03-08T16:00:00Z",
                "2026-03-08T17:00:00Z",
            ),
            (
                fall,
                "01:30",
                "07:00",
                "2026-11-01T05:30:00Z",
                "2026-11-01T12:00:00Z",
            ),
            (
                fall,
                "00:30",
                "01:30",
                "2026-11-01T04:30:00Z",
                "2026-11-01T05:30:00Z",
            ),
            (
                fall,
                "12:00",
                "13:00",
                "2026-11-01T17:00:00Z",
                "2026-11-01T18:00:00Z",
            ),
        ],
    );
}

/// Q10: a restart is judged against the resolved instants, so on Berlin's spring-forward day
/// 03:15 is before a 02:30 start (03:30), and the second 02:15 on its fall-back day is after a
/// 02:30 end (the first 02:30). New York's equivalents likewise.
#[tokio::test(start_paused = true)]
async fn restarts_inside_shifted_boundaries_are_inactive() {
    let cases = [
        (berlin(), "02:30", "07:00", "2026-03-29T01:15:00Z", false), // 03:15 CEST
        (berlin(), "02:30", "07:00", "2026-03-29T01:30:00Z", true),  // 03:30 CEST
        (berlin(), "01:00", "02:30", "2026-10-25T01:15:00Z", false), // the second 02:15, CET
        (berlin(), "01:00", "02:30", "2026-10-25T00:15:00Z", true),  // the first 02:15, CEST
        (new_york(), "02:30", "07:00", "2026-03-08T07:15:00Z", false), // 03:15 EDT
        (new_york(), "00:30", "01:30", "2026-11-01T06:15:00Z", false), // the second 01:15, EST
        (new_york(), "00:30", "01:30", "2026-11-01T05:15:00Z", true), // the first 01:15, EDT
    ];
    for (zone, start, end, now, want) in cases {
        assert_eq!(
            window(start, end, zone.clone()).is_active(instant(now)),
            want,
            "{start}-{end} at {now}"
        );
        let (controller, scheduler, _changes) = started(start, end, zone, instant(now));
        assert_eq!(
            controller.is_active(),
            want,
            "controller {start}-{end} at {now}"
        );
        scheduler.stop().await;
    }
}

/// Q10: 02:30–03:15 on a spring-forward day resolves to 03:30–03:15, an empty window: energy
/// saving does not engage that day, and that is logged at info.
#[test]
fn empty_window_on_spring_forward_day() {
    for (zone, day) in [
        (berlin(), date(2026, 3, 29)),
        (new_york(), date(2026, 3, 8)),
    ] {
        let window = window("02:30", "03:15", zone.clone());
        assert_eq!(window.for_date(day), None, "{zone:?}");

        let midnight = resolve_local(day, Time::midnight(), &zone).expect("midnight");
        let next_midnight = next_local_midnight(midnight, &zone);
        let mut now = midnight;
        while now < next_midnight {
            assert!(!window.is_active(now), "{zone:?} active at {now}");
            now += SignedDuration::from_mins(1);
        }

        let captured = Captured::default();
        let _guard = captured.install(LevelFilter::INFO);
        let _ = window.next_transition(midnight - SignedDuration::from_hours(1));
        let log = captured.text();
        assert!(log.contains("empties the window"), "{log}");
        assert!(log.contains(&format!(r#""date":"{day}""#)), "{log}");
    }
}

/// Q10 regression: Berlin, 02:30–03:15, running from 2026-03-27. After the 2026-03-28 end the
/// controller re-arms across the empty 2026-03-29 and activates on 2026-03-30 without a restart.
#[tokio::test(start_paused = true)]
async fn empty_window_re_arms_without_restart() {
    let window = window("02:30", "03:15", berlin());
    assert_eq!(
        window.next_transition(instant("2026-03-28T02:15:01Z")), // 03:15:01 CET
        Some(instant("2026-03-30T00:30:00Z")),                   // 02:30 CEST
    );

    let now = instant("2026-03-26T23:00:00Z"); // 2026-03-27 00:00 CET
    let (_controller, scheduler, mut changes) = started("02:30", "03:15", berlin(), now);
    let want = [
        (true, "2026-03-27T01:30:00Z"),
        (false, "2026-03-27T02:15:00Z"),
        (true, "2026-03-28T01:30:00Z"),
        (false, "2026-03-28T02:15:00Z"),
        (true, "2026-03-30T00:30:00Z"),
        (false, "2026-03-30T01:15:00Z"),
    ];
    for (active, at) in want {
        assert_eq!(changes.recv().await.expect("change"), (active, instant(at)));
    }
    scheduler.stop().await;
}

const EQUIVALENCE_WINDOWS: [(&str, &str); 5] = [
    ("02:30", "07:00"),
    ("01:00", "02:30"),
    ("23:00", "02:30"),
    ("02:30", "03:15"),
    ("01:30", "01:45"),
];

/// Runs a controller continuously from two days before `day` to two days after it, applying only
/// its scheduled transitions on the paused clock, and checks at every minute, at every window
/// boundary and 1 s either side that its state is the one a restart at that instant would find.
async fn assert_continuous_matches_restart(zone: TimeZone, day: Date) {
    let first_day = day.checked_sub(jiff::Span::new().days(2)).expect("date");
    let end_day = day.checked_add(jiff::Span::new().days(3)).expect("date");
    let base = resolve_local(first_day, Time::midnight(), &zone).expect("instant");
    let end = resolve_local(end_day, Time::midnight(), &zone).expect("instant");

    for (start, finish) in EQUIVALENCE_WINDOWS {
        let restarted = window(start, finish, zone.clone());
        let mut checks = Vec::new();
        let mut minute = base;
        while minute < end {
            checks.push(minute);
            minute += SignedDuration::from_mins(1);
        }
        let mut date = first_day.yesterday().expect("date");
        while date <= end_day {
            if let Some((opens, closes)) = restarted.resolve(date) {
                for boundary in [opens, closes] {
                    for offset in [-1, 0, 1] {
                        checks.push(boundary + SignedDuration::from_secs(offset));
                    }
                }
            }
            date = date.tomorrow().expect("date");
        }
        checks.retain(|check| (base..end).contains(check));
        checks.sort();
        checks.dedup();

        let started_at = tokio::time::Instant::now();
        let (controller, scheduler, _changes) = started(start, finish, zone.clone(), base);
        for check in checks {
            let offset = Duration::try_from(check.duration_since(base)).expect("after base");
            tokio::time::sleep_until(started_at + offset).await;
            // Let a transition due at this instant run before reading the state.
            for _ in 0..3 {
                tokio::task::yield_now().await;
            }
            assert_eq!(
                controller.is_active(),
                restarted.is_active(check),
                "{zone:?} {start}-{finish} at {check}"
            );
        }
        scheduler.stop().await;
    }
}

/// Q10 equivalence, Berlin spring forward.
#[tokio::test(start_paused = true)]
async fn continuous_matches_restart_berlin_spring() {
    assert_continuous_matches_restart(berlin(), date(2026, 3, 29)).await;
}

/// Q10 equivalence, Berlin fall back.
#[tokio::test(start_paused = true)]
async fn continuous_matches_restart_berlin_fall() {
    assert_continuous_matches_restart(berlin(), date(2026, 10, 25)).await;
}

/// Q10 equivalence, New York spring forward.
#[tokio::test(start_paused = true)]
async fn continuous_matches_restart_new_york_spring() {
    assert_continuous_matches_restart(new_york(), date(2026, 3, 8)).await;
}

/// Q10 equivalence, New York fall back.
#[tokio::test(start_paused = true)]
async fn continuous_matches_restart_new_york_fall() {
    assert_continuous_matches_restart(new_york(), date(2026, 11, 1)).await;
}
