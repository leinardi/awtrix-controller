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

use std::time::Duration;

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

fn entry(name: &str, message: &str, repeat: Repeat) -> ScheduledNotificationConfig {
    ScheduledNotificationConfig {
        name: name.to_owned(),
        message: message.to_owned(),
        repeat,
        date: String::new(),
        day: 0,
        weekdays: Vec::new(),
        time: "00:00".to_owned(),
        enabled: true,
        duration: 600,
        icon: "14004".to_owned(),
        rainbow: true,
        scroll_speed: 50,
        wakeup: true,
        rtttl: String::new(),
    }
}

fn birthday(date: &str, name: &str) -> ScheduledNotificationConfig {
    ScheduledNotificationConfig {
        date: date.to_owned(),
        ..entry(name, "Happy Birthday {name}!", Repeat::Yearly)
    }
}

fn new_year(enabled: bool) -> ScheduledNotificationConfig {
    ScheduledNotificationConfig {
        date: "01-01".to_owned(),
        icon: "5855".to_owned(),
        enabled,
        ..entry("New Year", "Happy New Year {year}!", Repeat::Yearly)
    }
}

fn weekly(name: &str, weekdays: &[&str]) -> ScheduledNotificationConfig {
    ScheduledNotificationConfig {
        weekdays: weekdays.iter().map(|&weekday| weekday.to_owned()).collect(),
        time: "10:00".to_owned(),
        duration: 60,
        icon: "609".to_owned(),
        rainbow: false,
        ..entry(name, name, Repeat::Weekly)
    }
}

/// One publish: the device, the notification and when it was published.
type Published = (String, Arc<Notification>, Timestamp);

/// A notifier started at `now` on a paused clock for devices `clients`, its scheduler, and what it
/// publishes. Publishing to a device named `unreachable` fails.
fn started(
    entries: Vec<ScheduledNotificationConfig>,
    zone: TimeZone,
    now: Timestamp,
    clients: &[&str],
) -> (Scheduler, mpsc::UnboundedReceiver<Published>) {
    let clock = TokioClock::new(now);
    let scheduler = Scheduler::new(Arc::new(clock), &CancellationToken::new());
    let (sender, received) = mpsc::unbounded_channel();
    let publish: Publish = Arc::new(move |client_id, notification| {
        let sender = sender.clone();
        Box::pin(async move {
            if client_id == "unreachable" {
                return Err("device unreachable".into());
            }
            let _ = sender.send((client_id, notification, clock.now()));
            Ok(())
        })
    });
    let clients: Vec<String> = clients.iter().map(|&client| client.to_owned()).collect();
    let connected_ids: ConnectedIds = Arc::new(move || clients.clone());

    let notifier = ScheduledNotifier::new(entries, zone, Arc::new(clock), publish);
    notifier.start(&scheduler, &connected_ids);
    (scheduler, received)
}

fn text(notification: &Notification) -> Option<&TextContent> {
    notification.content.text.as_ref()
}

fn plain(text: &str) -> TextContent {
    TextContent::plain(text)
}

/// Go: `TestBirthdayFiresTC05`
#[tokio::test(start_paused = true)]
async fn birthday_fires() {
    let now = instant("2024-04-15T23:59:59Z");
    let (scheduler, mut published) = started(
        vec![birthday("04-16", "Alice")],
        TimeZone::UTC,
        now,
        &["client1", "client2"],
    );

    for client in ["client1", "client2"] {
        let (client_id, notification, at) = published.recv().await.expect("published");
        assert_eq!(client_id, client);
        assert_eq!(at, instant("2024-04-16T00:00:00Z"));
        assert_eq!(text(&notification), Some(&plain("Happy Birthday Alice!")));
        assert_eq!(notification.content.icon, "14004");
        assert_eq!(notification.content.duration, 600);
        assert!(notification.content.rainbow);
        assert_eq!(notification.content.scroll_speed, 50);
        assert!(notification.wakeup);
    }
    scheduler.stop().await;
}

/// Go: `TestBirthdayFiresInNextCalendarYearTC06`
#[tokio::test(start_paused = true)]
async fn birthday_fires_in_next_calendar_year() {
    let now = instant("2024-12-01T00:00:00Z");
    let (scheduler, mut published) = started(
        vec![birthday("03-03", "Bob")],
        TimeZone::UTC,
        now,
        &["device1"],
    );

    let (_, notification, at) = published.recv().await.expect("published");
    assert_eq!(at, instant("2025-03-03T00:00:00Z"));
    assert_eq!(text(&notification), Some(&plain("Happy Birthday Bob!")));
    scheduler.stop().await;
}

/// Go: `TestNewYearNotificationTC11TC12`
#[tokio::test(start_paused = true)]
async fn new_year_notification() {
    let now = instant("2024-12-31T23:59:59Z");
    let (scheduler, mut published) =
        started(vec![new_year(true)], TimeZone::UTC, now, &["device1"]);

    let (_, notification, at) = published.recv().await.expect("published");
    assert_eq!(at, instant("2025-01-01T00:00:00Z"));
    assert_eq!(text(&notification), Some(&plain("Happy New Year 2025!")));
    scheduler.stop().await;
}

/// Go: `TestScheduledNotificationDisabledTC13`
#[tokio::test(start_paused = true)]
async fn disabled_entry_never_fires() {
    let now = instant("2024-12-31T23:59:59Z");
    let (scheduler, mut published) =
        started(vec![new_year(false)], TimeZone::UTC, now, &["device1"]);

    // A negative assertion: nothing arrives within more than a year of paused time. With no job
    // scheduled nothing holds the publisher, so the channel closes instead.
    let waited = tokio::time::timeout(Duration::from_hours(400 * 24), published.recv()).await;
    assert!(
        !matches!(waited, Ok(Some(_))),
        "a disabled entry published {waited:?}"
    );
    scheduler.stop().await;
}

/// Go: `TestCustomMessageAndIconTC14`
#[tokio::test(start_paused = true)]
async fn custom_message_and_icon() {
    let custom = ScheduledNotificationConfig {
        message: "Bonne Année!".to_owned(),
        icon: "9999".to_owned(),
        ..new_year(true)
    };
    let now = instant("2024-12-31T23:59:59Z");
    let (scheduler, mut published) = started(vec![custom], TimeZone::UTC, now, &["device1"]);

    let (_, notification, _) = published.recv().await.expect("published");
    assert_eq!(text(&notification), Some(&plain("Bonne Année!")));
    assert_eq!(notification.content.icon, "9999");
    scheduler.stop().await;
}

/// Go: `TestTwoEntriesInOneNotifierTC15`
#[tokio::test(start_paused = true)]
async fn two_entries_in_one_notifier() {
    let now = instant("2024-12-31T23:59:59Z");
    let entries = vec![birthday("01-01", "Alice"), new_year(true)];
    let (scheduler, mut published) = started(entries, TimeZone::UTC, now, &["device1"]);

    let mut texts = Vec::new();
    for _ in 0..2 {
        let (_, notification, at) = published.recv().await.expect("published");
        assert_eq!(at, instant("2025-01-01T00:00:00Z"));
        texts.push(text(&notification).cloned());
    }
    texts.sort_by_key(|text| format!("{text:?}"));
    assert_eq!(
        texts,
        [
            Some(plain("Happy Birthday Alice!")),
            Some(plain("Happy New Year 2025!")),
        ]
    );
    scheduler.stop().await;
}

/// Go: `TestWeeklyMultipleWeekdaysFireOnEachDay`
#[tokio::test(start_paused = true)]
async fn weekly_multiple_weekdays_fire_on_each_day() {
    let now = instant("2024-12-29T09:00:00Z"); // a Sunday
    let (scheduler, mut published) = started(
        vec![weekly("Standup", &["monday", "friday"])],
        TimeZone::UTC,
        now,
        &["device1"],
    );

    let (_, _, monday) = published.recv().await.expect("monday");
    assert_eq!(monday, instant("2024-12-30T10:00:00Z"));
    let (_, _, friday) = published.recv().await.expect("friday");
    assert_eq!(friday, instant("2025-01-03T10:00:00Z"));
    let (_, _, next_monday) = published.recv().await.expect("next monday");
    assert_eq!(next_monday, instant("2025-01-06T10:00:00Z"));
    scheduler.stop().await;
}

/// Go: `TestWeeklySingleWeekdayStillWorks`
#[tokio::test(start_paused = true)]
async fn weekly_single_weekday() {
    let now = instant("2024-12-28T09:00:00Z"); // a Saturday
    let (scheduler, mut published) = started(
        vec![weekly("Standup", &["monday"])],
        TimeZone::UTC,
        now,
        &["device1"],
    );

    let (_, notification, at) = published.recv().await.expect("published");
    assert_eq!(at, instant("2024-12-30T10:00:00Z"));
    assert_eq!(text(&notification), Some(&plain("Standup")));
    scheduler.stop().await;
}

/// A device that cannot be reached is logged and skipped; the others still get the notification.
#[tokio::test(start_paused = true)]
async fn publish_failure_is_logged_and_skipped() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);

    let now = instant("2024-12-31T23:59:59Z");
    let (scheduler, mut published) = started(
        vec![new_year(true)],
        TimeZone::UTC,
        now,
        &["unreachable", "device1"],
    );

    let (client_id, _, _) = published.recv().await.expect("published");
    assert_eq!(client_id, "device1");
    let log = captured.text();
    assert!(log.contains("scheduled publish failed"), "{log}");
    assert!(log.contains(r#""client_id":"unreachable""#), "{log}");
    scheduler.stop().await;
}

/// `{name}` and `{year}` are replaced wherever they appear; an empty message is the name.
#[test]
fn message_formatting() {
    let mut formatted = entry("Alice", "{name}, {name}! {year}", Repeat::Daily);
    assert_eq!(format_message(&formatted, 2025), "Alice, Alice! 2025");
    formatted.message = String::new();
    assert_eq!(format_message(&formatted, 2025), "Alice");
    formatted.name = "Year {year}".to_owned();
    assert_eq!(format_message(&formatted, 2025), "Year 2025");
    formatted.message = "<b>&</b>".to_owned();
    assert_eq!(format_message(&formatted, 2025), "<b>&</b>");
}

/// Weekday names are matched in any case, as Go's `strings.ToLower` did.
#[test]
fn weekday_parsing() {
    assert_eq!(parse_weekday("monday"), Ok(Weekday::Monday));
    assert_eq!(parse_weekday("Sunday"), Ok(Weekday::Sunday));
    assert_eq!(parse_weekday("FRIDAY"), Ok(Weekday::Friday));
    assert!(parse_weekday("mon").is_err());
    assert!(parse_weekday("").is_err());
}

fn nine() -> Time {
    time(9, 0, 0, 0)
}

/// Q4: a monthly day past the end of the month fires on its last day. Go's `time.Date` rolled it
/// into the next month instead (day 31 from April 15 fired on May 1).
#[test]
fn monthly_day_clamps_to_the_last_day_of_the_month() {
    let utc = TimeZone::UTC;
    let cases = [
        ("2026-04-15T12:00:00Z", 31, "2026-04-30T09:00:00Z"),
        ("2026-02-01T12:00:00Z", 30, "2026-02-28T09:00:00Z"),
        ("2026-02-01T12:00:00Z", 29, "2026-02-28T09:00:00Z"),
        ("2026-01-31T10:00:00Z", 31, "2026-02-28T09:00:00Z"),
        ("2028-02-01T12:00:00Z", 29, "2028-02-29T09:00:00Z"),
        ("2026-04-15T12:00:00Z", 15, "2026-05-15T09:00:00Z"),
        ("2026-04-15T08:00:00Z", 15, "2026-04-15T09:00:00Z"),
    ];
    for (from, day, want) in cases {
        assert_eq!(
            next_monthly_occurrence(instant(from), day, nine(), &utc),
            instant(want),
            "{from} day {day}"
        );
    }

    // Recurring from each fire time: every month's last day, then back to the 31st.
    let mut fired = instant("2026-01-01T00:00:00Z");
    for want in [
        "2026-01-31",
        "2026-02-28",
        "2026-03-31",
        "2026-04-30",
        "2026-05-31",
    ] {
        fired = next_monthly_occurrence(fired, 31, nine(), &utc);
        assert_eq!(fired, instant(&format!("{want}T09:00:00Z")));
    }
}

/// Q5: a yearly 02-29 fires on February 28 outside leap years. Go's `time.Date` moved it to
/// March 1.
#[test]
fn yearly_february_29_fires_on_february_28_outside_leap_years() {
    let utc = TimeZone::UTC;
    let leap_day: MonthDay = "02-29".parse().expect("date");
    let cases = [
        ("2026-01-01T12:00:00Z", "2026-02-28T09:00:00Z"),
        ("2026-02-28T09:00:00Z", "2027-02-28T09:00:00Z"),
        ("2027-03-01T12:00:00Z", "2028-02-29T09:00:00Z"),
        ("2028-01-01T12:00:00Z", "2028-02-29T09:00:00Z"),
        ("2028-02-29T09:00:00Z", "2029-02-28T09:00:00Z"),
    ];
    for (from, want) in cases {
        assert_eq!(
            next_yearly_occurrence(instant(from), leap_day, nine(), &utc),
            instant(want),
            "{from}"
        );
    }
}

/// Checks that every occurrence helper, asked from the day before (daily from 23:00, the others
/// from noon), lands each `(day, time)` on `want`.
fn assert_occurrences(name: &str, cases: &[(Date, Time, &str)]) {
    let zone = zone(name);
    for &(day, at, want) in cases {
        let want = instant(want);
        let day_before = day.yesterday().expect("date");
        let evening = resolve_local(day_before, time(23, 0, 0, 0), &zone).expect("instant");
        let noon = resolve_local(day_before, time(12, 0, 0, 0), &zone).expect("instant");
        let month_day: MonthDay = format!("{:02}-{:02}", day.month(), day.day())
            .parse()
            .expect("date");

        let weekly = next_weekly_occurrence(noon, day.weekday(), at, &zone);
        let monthly = next_monthly_occurrence(noon, day.day(), at, &zone);
        let yearly = next_yearly_occurrence(noon, month_day, at, &zone);
        assert_eq!(
            next_daily_occurrence(evening, at, &zone),
            want,
            "daily {name} {day} {at}"
        );
        assert_eq!(weekly, want, "weekly {name} {day} {at}");
        assert_eq!(monthly, want, "monthly {name} {day} {at}");
        assert_eq!(yearly, want, "yearly {name} {day} {at}");
    }
}

/// Q6 golden test in Berlin: a time in the gap moves forward by the gap, a time in the fold
/// takes the earlier instant, an ordinary time is unchanged, for all four repeats.
#[test]
fn berlin_occurrences_follow_the_dst_policy() {
    // Spring forward on 2026-03-29, 02:00 CET to 03:00 CEST; fall back on 2026-10-25, 03:00 CEST
    // to 02:00 CET.
    let spring = date(2026, 3, 29);
    let fall = date(2026, 10, 25);
    assert_occurrences(
        "Europe/Berlin",
        &[
            (spring, time(1, 30, 0, 0), "2026-03-29T00:30:00Z"),
            (spring, time(2, 30, 0, 0), "2026-03-29T01:30:00Z"), // gap: 03:30 CEST
            (spring, time(12, 0, 0, 0), "2026-03-29T10:00:00Z"),
            (fall, time(1, 30, 0, 0), "2026-10-24T23:30:00Z"),
            (fall, time(2, 30, 0, 0), "2026-10-25T00:30:00Z"), // fold: the first, CEST
            (fall, time(12, 0, 0, 0), "2026-10-25T11:00:00Z"),
        ],
    );
}

/// Q6 golden test in New York, likewise.
#[test]
fn new_york_occurrences_follow_the_dst_policy() {
    // Spring forward on 2026-03-08, 02:00 EST to 03:00 EDT; fall back on 2026-11-01, 02:00 EDT
    // to 01:00 EST.
    let spring = date(2026, 3, 8);
    let fall = date(2026, 11, 1);
    assert_occurrences(
        "America/New_York",
        &[
            (spring, time(1, 30, 0, 0), "2026-03-08T06:30:00Z"),
            (spring, time(2, 30, 0, 0), "2026-03-08T07:30:00Z"), // gap: 03:30 EDT
            (spring, time(12, 0, 0, 0), "2026-03-08T16:00:00Z"),
            (fall, time(1, 30, 0, 0), "2026-11-01T05:30:00Z"), // fold: the first, EDT
            (fall, time(2, 30, 0, 0), "2026-11-01T07:30:00Z"),
            (fall, time(12, 0, 0, 0), "2026-11-01T17:00:00Z"),
        ],
    );
}

/// Q6: an occurrence in the fold fires once, and one in the gap stays at its wall-clock time on
/// the following days. (Go's `AddDate` from a gap-resolved time kept the shifted wall clock, so a
/// New York 02:30 fired at 01:30 from then on.)
#[test]
fn recurring_occurrences_across_transitions() {
    let berlin = zone("Europe/Berlin");
    let new_york = zone("America/New_York");
    let cases = [
        (
            &berlin,
            time(2, 30, 0, 0),
            "2026-10-25T00:30:00Z",
            "2026-10-26T01:30:00Z",
        ),
        (
            &new_york,
            time(1, 30, 0, 0),
            "2026-11-01T05:30:00Z",
            "2026-11-02T06:30:00Z",
        ),
        (
            &berlin,
            time(2, 30, 0, 0),
            "2026-03-29T01:30:00Z",
            "2026-03-30T00:30:00Z",
        ),
        (
            &new_york,
            time(2, 30, 0, 0),
            "2026-03-08T07:30:00Z",
            "2026-03-09T06:30:00Z",
        ),
    ];
    for (zone, at, fired, want) in cases {
        assert_eq!(
            next_daily_occurrence(instant(fired), at, zone),
            instant(want),
            "after {fired}"
        );
        let weekday = instant(fired).to_zoned(zone.clone()).weekday();
        let week_later = instant(want) + jiff::SignedDuration::from_hours(6 * 24);
        let weekly = next_weekly_occurrence(instant(fired), weekday, at, zone);
        assert_eq!(
            weekly.to_zoned(zone.clone()).time(),
            at,
            "weekly after {fired}"
        );
        assert!(weekly >= week_later, "weekly after {fired}: {weekly}");
    }
}

/// Entries the configuration would have rejected are skipped, not scheduled.
#[tokio::test(start_paused = true)]
async fn invalid_entries_are_skipped() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);

    let bad_time = ScheduledNotificationConfig {
        time: "25:00".to_owned(),
        ..entry("bad time", "", Repeat::Daily)
    };
    let bad_day = ScheduledNotificationConfig {
        day: 32,
        ..entry("bad day", "", Repeat::Monthly)
    };
    let bad_date = ScheduledNotificationConfig {
        date: "02-30".to_owned(),
        ..entry("bad date", "", Repeat::Yearly)
    };
    let now = instant("2024-12-31T23:59:59Z");
    let (scheduler, mut published) = started(
        vec![bad_time, bad_day, bad_date],
        TimeZone::UTC,
        now,
        &["device1"],
    );

    // A negative assertion: nothing arrives within more than a year of paused time. With no job
    // scheduled nothing holds the publisher, so the channel closes instead.
    let waited = tokio::time::timeout(Duration::from_hours(400 * 24), published.recv()).await;
    assert!(
        !matches!(waited, Ok(Some(_))),
        "an invalid entry published {waited:?}"
    );
    let log = captured.text();
    for message in ["invalid time", "invalid day", "invalid date"] {
        assert!(log.contains(message), "{message} not in {log}");
    }
    scheduler.stop().await;
}

#[test]
fn empty_time_is_midnight() {
    let midnight = ScheduledNotificationConfig {
        time: String::new(),
        ..entry("midnight", "", Repeat::Daily)
    };
    assert_eq!(fire_time(&midnight), Some(Time::midnight()));
}
