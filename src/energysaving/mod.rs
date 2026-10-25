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

//! The nightly energy-saving window. While it is active the devices dim (the settings layer reads
//! [`EnergySaving::is_active`]); the window may span midnight. Each change of state calls an
//! `on_change` callback so the settings layer can push the new payload.
//!
//! Q10: the start and end are civil wall-clock times, so `07:00` means 07:00 local every day,
//! daylight-saving days included. Each local date has a [`Window`] of two resolved instants, and
//! both the active state and the next transition are derived from those instants only, never by
//! comparing times of day. A controller running continuously and one restarted at any instant
//! therefore agree.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use jiff::Timestamp;
use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use tracing::{error, info};

use crate::clock::{Clock, next_local_midnight, resolve_local};
use crate::config::{EnergySavingConfig, ParseError, TimeOfDay};
use crate::scheduler::Scheduler;

/// How many local days [`Window::next_transition`] looks at before giving up. Only a
/// daylight-saving change can empty a day's window, and those are months apart, so a week and a
/// day is far more than enough.
pub const MAX_WINDOW_SEARCH_DAYS: usize = 8;

/// Called with the new state after each change.
pub type OnActiveChange =
    Arc<dyn Fn(bool) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// The energy-saving window in a time zone: for each local date `D`, from the start time on `D`
/// to the end time on `D`, or on `D + 1` when the end is not after the start (a window that spans
/// midnight). Both are resolved with [`resolve_local`].
#[derive(Clone, Debug)]
pub struct Window {
    start: Time,
    end: Time,
    zone: TimeZone,
}

impl Window {
    /// The window from `start` to `end`, local to `zone`.
    #[must_use]
    pub fn new(start: TimeOfDay, end: TimeOfDay, zone: TimeZone) -> Self {
        Self {
            start: civil_time(start),
            end: civil_time(end),
            zone,
        }
    }

    /// The window that opens on local date `date`, as `[start, end)` instants, or `None` when it
    /// is empty: a daylight-saving gap can resolve the start to an instant at or after the end
    /// (02:30–03:15 on Berlin's spring-forward day starts at 03:30).
    #[must_use]
    pub fn for_date(&self, date: Date) -> Option<(Timestamp, Timestamp)> {
        self.resolve(date).filter(|(start, end)| start < end)
    }

    /// Whether `instant` lies inside the window that opened on its local date or the day before.
    #[must_use]
    pub fn is_active(&self, instant: Timestamp) -> bool {
        let today = self.local_date(instant);
        [today.yesterday().ok(), Some(today)]
            .into_iter()
            .flatten()
            .filter_map(|date| self.for_date(date))
            .any(|(start, end)| start <= instant && instant < end)
    }

    /// The first start or end of a non-empty window that is after `now`, walking forward a day at
    /// a time from the window that opened the day before `now`'s local date. Empty windows are
    /// skipped, and logged at info when they still lie ahead. `None` when no boundary turns up
    /// within [`MAX_WINDOW_SEARCH_DAYS`] days, which only bad zone data could cause.
    #[must_use]
    pub fn next_transition(&self, now: Timestamp) -> Option<Timestamp> {
        let today = self.local_date(now);
        let mut date = today.yesterday().unwrap_or(today);

        for _ in 0..MAX_WINDOW_SEARCH_DAYS {
            match self.resolve(date) {
                Some((start, end)) if start < end => {
                    if start > now {
                        return Some(start);
                    }
                    if end > now {
                        return Some(end);
                    }
                }
                Some((start, _)) if start > now => {
                    info!(
                        date = %date,
                        start = %self.start,
                        end = %self.end,
                        "energysaving: a daylight-saving change empties the window on this day; energy saving stays off"
                    );
                }
                Some(_) | None => {}
            }
            date = date.tomorrow().ok()?;
        }

        None
    }

    /// The window's time zone.
    #[must_use]
    pub const fn zone(&self) -> &TimeZone {
        &self.zone
    }

    /// The start and end instants for local date `date`, empty or not.
    fn resolve(&self, date: Date) -> Option<(Timestamp, Timestamp)> {
        let start = resolve_local(date, self.start, &self.zone).ok()?;
        let end_date = if self.end > self.start {
            date
        } else {
            date.tomorrow().ok()?
        };
        let end = resolve_local(end_date, self.end, &self.zone).ok()?;
        Some((start, end))
    }

    fn local_date(&self, instant: Timestamp) -> Date {
        instant.to_zoned(self.zone.clone()).date()
    }
}

/// Tracks the energy-saving state and schedules its changes.
pub struct EnergySaving {
    window: Window,
    clock: Arc<dyn Clock>,
    on_change: OnActiveChange,
    active: AtomicBool,
    /// The boundary the scheduled job is waiting for.
    due: Mutex<Timestamp>,
}

impl EnergySaving {
    /// A controller for the configured window in `zone`.
    ///
    /// # Errors
    ///
    /// The start or end is not an HH:MM time.
    pub fn new(
        config: &EnergySavingConfig,
        zone: TimeZone,
        clock: Arc<dyn Clock>,
        on_change: OnActiveChange,
    ) -> Result<Arc<Self>, ParseError> {
        let start: TimeOfDay = config.start.parse()?;
        let end: TimeOfDay = config.end.parse()?;
        Ok(Arc::new(Self {
            window: Window::new(start, end, zone),
            clock,
            on_change,
            active: AtomicBool::new(false),
            due: Mutex::new(Timestamp::UNIX_EPOCH),
        }))
    }

    /// Sets the state from the current time and schedules the next boundary.
    pub fn start(self: &Arc<Self>, scheduler: &Scheduler) {
        let now = self.clock.now();
        self.active
            .store(self.window.is_active(now), Ordering::SeqCst);
        let fire_at = self.arm(now);

        let on_transition = Arc::downgrade(self);
        let on_reschedule = Arc::downgrade(self);
        scheduler.schedule(
            "energysaving",
            fire_at,
            move || transition(&on_transition),
            move |fired| {
                on_reschedule
                    .upgrade()
                    .map_or(fired, |controller| controller.arm(fired))
            },
        );
    }

    /// Whether energy saving is active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// The window this controller follows.
    #[must_use]
    pub const fn window(&self) -> &Window {
        &self.window
    }

    /// The next boundary after `after`, remembered as the one the job waits for. When none is
    /// found the state is re-checked at the next local midnight, so the controller never stalls.
    fn arm(&self, after: Timestamp) -> Timestamp {
        let fire_at = self.window.next_transition(after).unwrap_or_else(|| {
            error!(
                after = %after,
                search_days = MAX_WINDOW_SEARCH_DAYS,
                "energysaving: no window boundary found; re-checking at next midnight"
            );
            next_local_midnight(after, self.window.zone())
        });
        *self.due.lock().unwrap_or_else(PoisonError::into_inner) = fire_at;
        fire_at
    }

    /// Re-derives the state at the boundary that fired and reports whether it changed. The state
    /// is read at the boundary or the current time, whichever is later: a wall clock stepped back
    /// a little would otherwise read the old state at the boundary, and the next boundary,
    /// computed from this one, would skip the change entirely.
    fn update(&self) -> Option<bool> {
        let due = *self.due.lock().unwrap_or_else(PoisonError::into_inner);
        let active = self.window.is_active(self.clock.now().max(due));
        let previous = self.active.swap(active, Ordering::SeqCst);
        (previous != active).then_some(active)
    }
}

/// The scheduled action: update the state and report a change. A controller that has been
/// dropped does nothing.
fn transition(controller: &Weak<EnergySaving>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let Some(controller) = controller.upgrade() else {
        return Box::pin(std::future::ready(()));
    };
    match controller.update() {
        Some(active) => (controller.on_change)(active),
        None => Box::pin(std::future::ready(())),
    }
}

/// The civil time for a configured time of day, whose hour and minute are in range by
/// construction.
fn civil_time(time_of_day: TimeOfDay) -> Time {
    let hour = i8::try_from(time_of_day.hour()).unwrap_or_default();
    let minute = i8::try_from(time_of_day.minute()).unwrap_or_default();
    Time::new(hour, minute, 0, 0).unwrap_or_default()
}

#[cfg(test)]
mod tests;
