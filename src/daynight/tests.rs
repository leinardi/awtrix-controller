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

use std::sync::{Mutex, PoisonError};
use std::thread;

use jiff::civil::{date, time};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;

use super::*;
use crate::clock::{TokioClock, resolve_local};
use crate::test_support::Captured;

fn instant(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn zone(name: &str) -> TimeZone {
    crate::config::load_time_zone(name).expect("zone")
}

fn location() -> LocationConfig {
    LocationConfig {
        latitude: 48.0,
        longitude: 11.0,
    }
}

/// Sunrise at 06:00 and sunset at 20:00 local, every day.
fn normal_sun(zone: TimeZone) -> SunFunction {
    Arc::new(move |_, _, date| {
        let rise = resolve_local(date, time(6, 0, 0, 0), &zone).ok()?;
        let set = resolve_local(date, time(20, 0, 0, 0), &zone).ok()?;
        Some((rise, set))
    })
}

/// Polar night: the sun never rises.
fn never_rises() -> SunFunction {
    Arc::new(|_, _, _| None)
}

/// Reports each new mode and the time it was reported at.
fn report_changes(
    changes: mpsc::UnboundedSender<(Mode, Timestamp)>,
    clock: TokioClock,
) -> OnModeChange {
    Arc::new(move |mode| {
        let _ = changes.send((mode, clock.now()));
        Box::pin(std::future::ready(()))
    })
}

/// A controller started at `now` on a paused clock, and its scheduler.
fn started(
    now: Timestamp,
    zone: TimeZone,
    location: LocationConfig,
    sun: Option<SunFunction>,
) -> (
    Arc<DayNight>,
    Scheduler,
    mpsc::UnboundedReceiver<(Mode, Timestamp)>,
) {
    let clock = TokioClock::new(now);
    let scheduler = Scheduler::new(Arc::new(clock), &CancellationToken::new());
    let (changes, received) = mpsc::unbounded_channel();
    let on_change = report_changes(changes, clock);
    let controller = match sun {
        Some(sun) => DayNight::with_sun_function(location, zone, Arc::new(clock), on_change, sun),
        None => DayNight::new(location, zone, Arc::new(clock), on_change),
    };
    controller.start(&scheduler);
    (controller, scheduler, received)
}

/// Go: `TestInitialModeDayAtNoon`
#[tokio::test(start_paused = true)]
async fn initial_mode_day_at_noon() {
    let utc = TimeZone::UTC;
    let now = instant("2024-06-15T12:00:00Z");
    let (controller, scheduler, _changes) =
        started(now, utc.clone(), location(), Some(normal_sun(utc)));
    scheduler.stop().await;
    assert_eq!(controller.current_mode(), Mode::Day);
}

/// Go: `TestInitialModeNightAt2AM`
#[tokio::test(start_paused = true)]
async fn initial_mode_night_at_2am() {
    let utc = TimeZone::UTC;
    let now = instant("2024-06-15T02:00:00Z");
    let (controller, scheduler, _changes) =
        started(now, utc.clone(), location(), Some(normal_sun(utc)));
    scheduler.stop().await;
    assert_eq!(controller.current_mode(), Mode::Night);
}

/// Go: `TestDayToNightTransitionTC02`. Extended to the following sunrise, which the manual timer
/// of the Go original could not reach.
#[tokio::test(start_paused = true)]
async fn day_to_night_transition() {
    let utc = TimeZone::UTC;
    let now = instant("2024-06-15T19:59:00Z");
    let (controller, scheduler, mut changes) =
        started(now, utc.clone(), location(), Some(normal_sun(utc)));
    assert_eq!(
        controller.current_mode(),
        Mode::Day,
        "19:59 is between 06:00 and 20:00"
    );

    let (mode, at) = changes.recv().await.expect("sunset");
    assert_eq!((mode, at), (Mode::Night, instant("2024-06-15T20:00:00Z")));
    assert_eq!(controller.current_mode(), Mode::Night);

    let (mode, at) = changes.recv().await.expect("sunrise");
    assert_eq!((mode, at), (Mode::Day, instant("2024-06-16T06:00:00Z")));
    scheduler.stop().await;
}

/// Go: `TestPolarNightFallbackTC17`. When the sun cannot be computed the mode stays night, a
/// warning is logged, and retries at local midnight never manufacture daylight.
#[tokio::test(start_paused = true)]
async fn polar_night_falls_back_to_night_and_retries_at_midnight() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);

    let now = instant("2024-01-15T14:00:00Z");
    let (controller, scheduler, mut changes) =
        started(now, TimeZone::UTC, location(), Some(never_rises()));
    assert_eq!(controller.current_mode(), Mode::Night);

    let log = captured.text();
    assert!(log.contains(r#""level":"WARN""#), "{log}");
    assert!(log.contains("polar night or midnight sun"), "{log}");
    assert!(log.contains(r#""date":"2024-01-15""#), "{log}");

    for _ in 0..3 {
        tokio::time::advance(std::time::Duration::from_hours(24)).await;
        tokio::task::yield_now().await;
        assert_eq!(controller.current_mode(), Mode::Night);
        assert!(changes.try_recv().is_err(), "polar retry reported daylight");
    }
    scheduler.stop().await;
}

#[tokio::test(start_paused = true)]
async fn polar_recovery_schedules_the_first_available_sunrise() {
    let ordinary = normal_sun(TimeZone::UTC);
    let sun: SunFunction = Arc::new(move |latitude, longitude, day| {
        (day >= date(2024, 1, 17))
            .then(|| ordinary(latitude, longitude, day))
            .flatten()
    });
    let now = instant("2024-01-15T14:00:00Z");
    let (controller, scheduler, mut changes) = started(now, TimeZone::UTC, location(), Some(sun));
    let (mode, at) = changes.recv().await.expect("first sunrise");
    assert_eq!((mode, at), (Mode::Day, instant("2024-01-17T06:00:00Z")));
    assert_eq!(controller.current_mode(), Mode::Day);
    assert_eq!(
        changes.recv().await.expect("sunset"),
        (Mode::Night, instant("2024-01-17T20:00:00Z"))
    );
    scheduler.stop().await;
}

#[tokio::test(start_paused = true)]
async fn lagging_clock_does_not_skip_sun_boundaries() {
    let frozen = Arc::new(crate::clock::FakeClock::new(instant(
        "2024-06-15T19:59:00Z",
    )));
    let scheduler = Scheduler::new(frozen.clone(), &CancellationToken::new());
    let (changes, mut received) = mpsc::unbounded_channel();
    let on_change: OnModeChange = Arc::new(move |mode| {
        let _ = changes.send(mode);
        Box::pin(std::future::ready(()))
    });
    let controller = DayNight::with_sun_function(
        location(),
        TimeZone::UTC,
        frozen,
        on_change,
        normal_sun(TimeZone::UTC),
    );
    controller.start(&scheduler);
    assert_eq!(received.recv().await, Some(Mode::Night));
    assert_eq!(received.recv().await, Some(Mode::Day));
    scheduler.stop().await;
}

#[tokio::test(start_paused = true)]
async fn slow_callback_does_not_skip_an_overdue_sunrise() {
    let clock = TokioClock::new(instant("2024-06-15T19:59:00Z"));
    let scheduler = Scheduler::new(Arc::new(clock), &CancellationToken::new());
    let (changes, mut received) = mpsc::unbounded_channel();
    let on_change: OnModeChange = Arc::new(move |mode| {
        let changes = changes.clone();
        Box::pin(async move {
            let _ = changes.send((mode, clock.now()));
            if mode == Mode::Night {
                tokio::time::sleep(std::time::Duration::from_hours(36)).await;
            }
        })
    });
    let controller = DayNight::with_sun_function(
        location(),
        TimeZone::UTC,
        Arc::new(clock),
        on_change,
        normal_sun(TimeZone::UTC),
    );
    controller.start(&scheduler);
    assert_eq!(
        received.recv().await,
        Some((Mode::Night, instant("2024-06-15T20:00:00Z")))
    );
    assert_eq!(
        received.recv().await,
        Some((Mode::Day, instant("2024-06-17T08:00:00Z")))
    );
    scheduler.stop().await;
}

/// Go: `TestConcurrentCurrentModeReads`
#[tokio::test]
async fn concurrent_current_mode_reads() {
    const READERS: usize = 50;

    let utc = TimeZone::UTC;
    let now = instant("2024-06-15T12:00:00Z");
    let (controller, scheduler, _changes) =
        started(now, utc.clone(), location(), Some(normal_sun(utc)));

    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let controller = Arc::clone(&controller);
            thread::spawn(move || controller.current_mode())
        })
        .collect();
    for reader in readers {
        assert_eq!(reader.join().expect("reader"), Mode::Day);
    }
    scheduler.stop().await;
}

/// Q3: at 00:30 in Berlin the sun is computed for that local date, not for the UTC date, which
/// is still the day before.
#[tokio::test(start_paused = true)]
async fn berlin_after_midnight_uses_the_local_date() {
    let berlin = zone("Europe/Berlin");
    let dates = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&dates);
    let inner = normal_sun(berlin.clone());
    let sun: SunFunction = Arc::new(move |latitude, longitude, date| {
        recorded
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(date);
        inner(latitude, longitude, date)
    });

    let now = instant("2026-06-14T22:30:00Z"); // 00:30 CEST on 2026-06-15
    let (controller, scheduler, mut changes) = started(now, berlin, location(), Some(sun));
    let first = dates
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .first()
        .copied();
    assert_eq!(first, Some(date(2026, 6, 15)));
    assert_eq!(controller.current_mode(), Mode::Night);

    let (mode, at) = changes.recv().await.expect("sunrise");
    assert_eq!((mode, at), (Mode::Day, instant("2026-06-15T04:00:00Z"))); // 06:00 CEST
    scheduler.stop().await;
}

/// Q3 with the real sun: at 20:00 in New York on 2026-06-14 the UTC date is already 06-15, whose
/// sunrise is still ahead, so a UTC-date computation called it night. On the local date the sun
/// sets at 20:30, so it is day.
#[tokio::test(start_paused = true)]
async fn new_york_evening_uses_the_local_date() {
    let new_york = LocationConfig {
        latitude: 40.7128,
        longitude: -74.006,
    };
    let now = instant("2026-06-15T00:00:00Z"); // 20:00 EDT on 2026-06-14
    let (controller, scheduler, _changes) = started(now, zone("America/New_York"), new_york, None);
    assert_eq!(controller.current_mode(), Mode::Day);
    scheduler.stop().await;
}

/// The `sunrise` crate agrees with go-solar, which the Go implementation used, to within a
/// second. The expected values were captured from go-solar v1.0.1 with a throwaway program.
#[test]
fn sunrise_sunset_matches_go_solar() {
    let cases = [
        (
            52.52,
            13.405,
            date(2026, 6, 15),
            "2026-06-15T02:42:52Z",
            "2026-06-15T19:30:53Z",
        ),
        (
            52.52,
            13.405,
            date(2026, 12, 21),
            "2026-12-21T07:14:53Z",
            "2026-12-21T14:53:47Z",
        ),
        (
            40.7128,
            -74.006,
            date(2026, 3, 8),
            "2026-03-08T11:19:13Z",
            "2026-03-08T22:54:39Z",
        ),
        (
            -33.87,
            151.21,
            date(2026, 1, 1),
            "2025-12-31T18:47:22Z",
            "2026-01-01T09:09:12Z",
        ),
    ];
    let tolerance = jiff::SignedDuration::from_secs(1);
    for (latitude, longitude, day, rise, set) in cases {
        let (got_rise, got_set) = sunrise_sunset(latitude, longitude, day).expect("sun");
        for (got, want) in [(got_rise, instant(rise)), (got_set, instant(set))] {
            let difference = got.duration_since(want).abs();
            assert!(
                difference <= tolerance,
                "{latitude},{longitude} {day}: {got} vs {want}"
            );
        }
    }

    // Svalbard: polar night in January, midnight sun in June.
    assert_eq!(sunrise_sunset(78.22, 15.65, date(2026, 1, 15)), None);
    assert_eq!(sunrise_sunset(78.22, 15.65, date(2026, 6, 15)), None);
}
