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

//! Scheduled notifications: each configured entry fires daily, weekly, monthly or yearly at a
//! local wall-clock time and is published to every connected device.
//!
//! Every occurrence is a local date and time resolved with
//! [`resolve_local`](crate::clock::resolve_local) (Q6), so a time in a daylight-saving gap fires
//! shifted forward by the gap and a time in a fold fires once, at the earlier instant. A day of the
//! month past the month's end fires on its last day (Q4), and so does a yearly `02-29` outside leap
//! years (Q5).

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use jiff::Timestamp;
use jiff::civil::{Date, Time, Weekday};
use jiff::tz::TimeZone;
use thiserror::Error;
use tracing::warn;

use crate::clock::{Clock, resolve_local};
use crate::config::{MonthDay, Repeat, ScheduledNotificationConfig, TimeOfDay};
use crate::model::{AppContent, Notification, TextContent};
use crate::scheduler::Scheduler;

/// The number of days in a week.
const DAYS_IN_WEEK: i8 = 7;

/// Publishes a notification to one device.
pub type Publish = Arc<
    dyn Fn(
            String,
            Arc<Notification>,
        )
            -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>
        + Send
        + Sync,
>;

/// The IDs of the devices connected now.
pub type ConnectedIds = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// A weekday name that is not `monday` to `sunday`.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("unknown weekday {0:?}")]
pub struct UnknownWeekday(String);

/// Schedules the configured notifications and publishes each one when it fires.
pub struct ScheduledNotifier {
    notifications: Vec<ScheduledNotificationConfig>,
    zone: TimeZone,
    clock: Arc<dyn Clock>,
    publish: Publish,
}

/// When an entry fires.
#[derive(Clone, Copy, Debug)]
enum Schedule {
    Daily,
    Weekly(Weekday),
    Monthly(i8),
    Yearly(MonthDay),
}

impl ScheduledNotifier {
    /// A notifier for `notifications`, local to `zone`.
    #[must_use]
    pub fn new(
        notifications: Vec<ScheduledNotificationConfig>,
        zone: TimeZone,
        clock: Arc<dyn Clock>,
        publish: Publish,
    ) -> Self {
        Self {
            notifications,
            zone,
            clock,
            publish,
        }
    }

    /// Schedules every enabled entry, a weekly one once per listed weekday. `connected_ids` is
    /// asked for the devices each time a notification fires. Entries that do not parse are
    /// skipped with a warning; the configuration has already rejected them, so this is only a
    /// guard.
    pub fn start(&self, scheduler: &Scheduler, connected_ids: &ConnectedIds) {
        let now = self.clock.now();

        for entry in &self.notifications {
            if !entry.enabled {
                continue;
            }
            let Some(time) = fire_time(entry) else {
                warn!(
                    name = entry.name,
                    time = entry.time,
                    "notification: skipping entry with invalid time"
                );
                continue;
            };
            let entry = Arc::new(entry.clone());

            for (job, schedule) in schedules(&entry) {
                let zone = self.zone.clone();
                let fire_at = next_occurrence(schedule, now, time, &zone);
                let action = self.action(&entry, connected_ids);
                scheduler.schedule(&job, fire_at, action, move |fired| {
                    next_occurrence(schedule, fired, time, &zone)
                });
            }
        }
    }

    /// The scheduled action for `entry`: build the notification at the time it fires and publish
    /// it to every connected device, logging each failure.
    fn action(
        &self,
        entry: &Arc<ScheduledNotificationConfig>,
        connected_ids: &ConnectedIds,
    ) -> impl FnMut() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static {
        let entry = Arc::clone(entry);
        let connected_ids = Arc::clone(connected_ids);
        let clock = Arc::clone(&self.clock);
        let zone = self.zone.clone();
        let publish = Arc::clone(&self.publish);

        move || {
            let year = clock.now().to_zoned(zone.clone()).year();
            let notification = Arc::new(build_notification(&entry, year));
            let entry = Arc::clone(&entry);
            let clients = connected_ids();
            let publish = Arc::clone(&publish);
            Box::pin(async move {
                for client_id in clients {
                    let published = publish(client_id.clone(), Arc::clone(&notification)).await;
                    if let Err(error) = published {
                        warn!(
                            client_id,
                            name = entry.name,
                            error = %error,
                            "notification: scheduled publish failed"
                        );
                    }
                }
            })
        }
    }
}

/// The jobs for one entry, each with its scheduler name: one per weekday for a weekly entry.
fn schedules(entry: &ScheduledNotificationConfig) -> Vec<(String, Schedule)> {
    let job = format!("scheduled:{}", entry.name);
    match entry.repeat {
        Repeat::Daily => vec![(job, Schedule::Daily)],
        Repeat::Weekly => entry
            .weekdays
            .iter()
            .filter_map(|name| match parse_weekday(name) {
                Ok(weekday) => Some((format!("{job}:{name}"), Schedule::Weekly(weekday))),
                Err(error) => {
                    warn!(name = entry.name, error = %error, "notification: skipping weekday");
                    None
                }
            })
            .collect(),
        Repeat::Monthly => match i8::try_from(entry.day) {
            Ok(day) if (1..=31).contains(&day) => vec![(job, Schedule::Monthly(day))],
            Ok(_) | Err(_) => {
                warn!(
                    name = entry.name,
                    day = entry.day,
                    "notification: skipping monthly entry with invalid day"
                );
                Vec::new()
            }
        },
        Repeat::Yearly => {
            if let Ok(date) = entry.date.parse::<MonthDay>() {
                vec![(job, Schedule::Yearly(date))]
            } else {
                warn!(
                    name = entry.name,
                    date = entry.date,
                    "notification: skipping yearly entry with invalid date"
                );
                Vec::new()
            }
        }
    }
}

/// The entry's fire time; an empty time is midnight, as in Go.
fn fire_time(entry: &ScheduledNotificationConfig) -> Option<Time> {
    if entry.time.is_empty() {
        return Some(Time::midnight());
    }
    let time_of_day: TimeOfDay = entry.time.parse().ok()?;
    let hour = i8::try_from(time_of_day.hour()).ok()?;
    let minute = i8::try_from(time_of_day.minute()).ok()?;
    Time::new(hour, minute, 0, 0).ok()
}

fn next_occurrence(schedule: Schedule, from: Timestamp, time: Time, zone: &TimeZone) -> Timestamp {
    match schedule {
        Schedule::Daily => next_daily_occurrence(from, time, zone),
        Schedule::Weekly(weekday) => next_weekly_occurrence(from, weekday, time, zone),
        Schedule::Monthly(day) => next_monthly_occurrence(from, day, time, zone),
        Schedule::Yearly(date) => next_yearly_occurrence(from, date, time, zone),
    }
}

/// The notification an entry publishes, its message formatted for `year`.
fn build_notification(entry: &ScheduledNotificationConfig, year: i16) -> Notification {
    Notification {
        content: AppContent {
            text: Some(TextContent::plain(format_message(entry, year))),
            icon: entry.icon.clone(),
            duration: entry.duration,
            rainbow: entry.rainbow,
            scroll_speed: entry.scroll_speed,
            ..AppContent::default()
        },
        rtttl: entry.rtttl.clone(),
        wakeup: entry.wakeup,
        ..Notification::default()
    }
}

/// The entry's message, or its name when the message is empty, with `{name}` replaced by the
/// name and `{year}` by the year it fires in.
#[must_use]
pub fn format_message(entry: &ScheduledNotificationConfig, year: i16) -> String {
    let template = if entry.message.is_empty() {
        &entry.name
    } else {
        &entry.message
    };
    template
        .replace("{name}", &entry.name)
        .replace("{year}", &year.to_string())
}

/// A weekday from its English name, in any case.
///
/// # Errors
///
/// The name is not `monday` to `sunday`.
pub fn parse_weekday(name: &str) -> Result<Weekday, UnknownWeekday> {
    match name.to_lowercase().as_str() {
        "monday" => Ok(Weekday::Monday),
        "tuesday" => Ok(Weekday::Tuesday),
        "wednesday" => Ok(Weekday::Wednesday),
        "thursday" => Ok(Weekday::Thursday),
        "friday" => Ok(Weekday::Friday),
        "saturday" => Ok(Weekday::Saturday),
        "sunday" => Ok(Weekday::Sunday),
        _ => Err(UnknownWeekday(name.to_owned())),
    }
}

/// The first of `dates` whose `time` resolves to an instant after `from`.
fn first_after(
    from: Timestamp,
    time: Time,
    zone: &TimeZone,
    dates: impl IntoIterator<Item = Option<Date>>,
) -> Option<Timestamp> {
    dates
        .into_iter()
        .flatten()
        .filter_map(|date| resolve_local(date, time, zone).ok())
        .find(|candidate| *candidate > from)
}

fn local_date(instant: Timestamp, zone: &TimeZone) -> Date {
    instant.to_zoned(zone.clone()).date()
}

/// The next `time` after `from`: today's, else tomorrow's.
#[must_use]
pub fn next_daily_occurrence(from: Timestamp, time: Time, zone: &TimeZone) -> Timestamp {
    let today = local_date(from, zone);
    let dates = [Some(today), today.tomorrow().ok()];
    first_after(from, time, zone, dates).unwrap_or(Timestamp::MAX)
}

/// The next `time` on `weekday` after `from`: this week's, else next week's.
#[must_use]
pub fn next_weekly_occurrence(
    from: Timestamp,
    weekday: Weekday,
    time: Time,
    zone: &TimeZone,
) -> Timestamp {
    let today = local_date(from, zone);
    let days_until = (weekday.to_monday_zero_offset() - today.weekday().to_monday_zero_offset())
        .rem_euclid(DAYS_IN_WEEK);
    let this_week = today.checked_add(jiff::Span::new().days(days_until)).ok();
    let next_week =
        this_week.and_then(|date| date.checked_add(jiff::Span::new().days(DAYS_IN_WEEK)).ok());
    first_after(from, time, zone, [this_week, next_week]).unwrap_or(Timestamp::MAX)
}

/// The next `time` on day `day` of a month after `from`, this month's, else next month's. A day
/// past the end of a month means its last day (Q4).
#[must_use]
pub fn next_monthly_occurrence(from: Timestamp, day: i8, time: Time, zone: &TimeZone) -> Timestamp {
    let first_of_month = local_date(from, zone).first_of_month();
    let next_month = first_of_month.checked_add(jiff::Span::new().months(1)).ok();
    let dates =
        [Some(first_of_month), next_month].map(|month| month.map(|month| clamped(month, day)));
    first_after(from, time, zone, dates).unwrap_or(Timestamp::MAX)
}

/// The next `time` on `date` after `from`, this year's, else next year's. `02-29` means
/// February 28 outside leap years (Q5).
#[must_use]
pub fn next_yearly_occurrence(
    from: Timestamp,
    date: MonthDay,
    time: Time,
    zone: &TimeZone,
) -> Timestamp {
    let year = local_date(from, zone).year();
    let in_year = |year: Option<i16>| -> Option<Date> {
        let month = i8::try_from(date.month()).ok()?;
        let day = i8::try_from(date.day()).ok()?;
        let first_of_month = Date::new(year?, month, 1).ok()?;
        Some(clamped(first_of_month, day))
    };
    let dates = [in_year(Some(year)), in_year(year.checked_add(1))];
    first_after(from, time, zone, dates).unwrap_or(Timestamp::MAX)
}

/// Day `day` of the month starting at `first_of_month`, or its last day when the month is
/// shorter.
fn clamped(first_of_month: Date, day: i8) -> Date {
    let day = day.clamp(1, first_of_month.days_in_month());
    first_of_month
        .with()
        .day(day)
        .build()
        .unwrap_or(first_of_month)
}

#[cfg(test)]
mod tests;
