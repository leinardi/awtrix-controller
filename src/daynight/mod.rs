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

//! Whether the configured location is in day or night, from astronomical sunrise and sunset, and
//! the scheduled transitions between the two. Each transition calls an `on_change` callback so the
//! settings layer can push the matching theme.
//!
//! Q3: the sun is computed for the **local date in the configured time zone**. The Go
//! implementation used the UTC date of the instant, so just after local midnight east of UTC it
//! computed the previous day's sun.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Weak};

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use sunrise::{Coordinates, SolarDay, SolarEvent};
use tracing::warn;

use crate::clock::{Clock, next_local_midnight};
use crate::config::LocationConfig;
use crate::scheduler::Scheduler;

/// Day or night.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    /// The sun is above the horizon.
    Day = 0,
    /// The sun is below the horizon.
    Night = 1,
}

impl Mode {
    /// "day" or "night".
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Night => "night",
        }
    }

    const fn from_u8(value: u8) -> Self {
        if value == Self::Day as u8 {
            Self::Day
        } else {
            Self::Night
        }
    }
}

/// The read-only view of the current mode, for the settings builder.
pub trait ModeProvider: Send + Sync {
    /// The current mode.
    fn current_mode(&self) -> Mode;
}

/// Sunrise and sunset for a latitude, a longitude and a local date, or `None` when either does not
/// happen that day (polar night, midnight sun) or cannot be computed.
pub type SunFunction = Arc<dyn Fn(f64, f64, Date) -> Option<(Timestamp, Timestamp)> + Send + Sync>;

/// Called with the new mode after each transition.
pub type OnModeChange = Arc<dyn Fn(Mode) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Tracks day and night and schedules the transitions.
pub struct DayNight {
    latitude: f64,
    longitude: f64,
    zone: TimeZone,
    clock: Arc<dyn Clock>,
    on_change: OnModeChange,
    sun: SunFunction,
    mode: AtomicU8,
}

impl DayNight {
    /// A controller using the `sunrise` crate.
    #[must_use]
    pub fn new(
        location: LocationConfig,
        zone: TimeZone,
        clock: Arc<dyn Clock>,
        on_change: OnModeChange,
    ) -> Arc<Self> {
        Self::with_sun_function(location, zone, clock, on_change, Arc::new(sunrise_sunset))
    }

    /// A controller with an injected sunrise and sunset function, for tests.
    #[must_use]
    pub fn with_sun_function(
        location: LocationConfig,
        zone: TimeZone,
        clock: Arc<dyn Clock>,
        on_change: OnModeChange,
        sun: SunFunction,
    ) -> Arc<Self> {
        Arc::new(Self {
            latitude: location.latitude,
            longitude: location.longitude,
            zone,
            clock,
            on_change,
            sun,
            mode: AtomicU8::new(Mode::Night as u8),
        })
    }

    /// Sets the mode from the current time and schedules the next sunrise or sunset. When the sun
    /// cannot be computed for today (polar night, midnight sun) the mode is night and a retry is
    /// scheduled at the next local midnight. Retries keep night until sun events are available.
    pub fn start(self: &Arc<Self>, scheduler: &Scheduler) {
        let now = self.clock.now();
        let today = self.local_date(now);

        let fire_at = if let Some((rise, set)) = self.sun_on(today) {
            let mode = if now < rise || now >= set {
                Mode::Night
            } else {
                Mode::Day
            };
            self.set_mode(mode);
            self.next_event_after(now, rise, set)
        } else {
            self.set_mode(Mode::Night);
            warn!(
                date = %today,
                latitude = self.latitude,
                longitude = self.longitude,
                "daynight: polar night or midnight sun — cannot compute sunrise/sunset; staying Night and retrying at next midnight"
            );
            self.next_midnight(now)
        };

        let on_transition = Arc::downgrade(self);
        let on_reschedule = Arc::downgrade(self);
        scheduler.schedule_with_due(
            "daynight",
            fire_at,
            move |due| transition(&on_transition, due),
            move |fired| {
                on_reschedule
                    .upgrade()
                    .map_or(fired, |controller| controller.reschedule(fired))
            },
        );
    }

    /// Derives the state at the due instant or current time, whichever is later. A polar retry
    /// cannot manufacture daylight, and a lagging wall clock cannot skip a real transition.
    fn update(&self, due: Timestamp) -> Option<Mode> {
        let now = self.clock.now().max(due);
        let mode = match self.sun_on(self.local_date(now)) {
            Some((rise, set)) if rise <= now && now < set => Mode::Day,
            Some(_) | None => Mode::Night,
        };
        let previous = self.mode.swap(mode as u8, Ordering::SeqCst);
        (Mode::from_u8(previous) != mode).then_some(mode)
    }

    fn set_mode(&self, mode: Mode) {
        self.mode.store(mode as u8, Ordering::SeqCst);
    }

    /// The next sun event after the due instant. Catching up re-evaluates the mode even if an
    /// earlier callback ran past another boundary. A polar retry can resume at today's sunrise.
    fn reschedule(&self, fired: Timestamp) -> Timestamp {
        let fired_date = self.local_date(fired);

        let Some((rise, set)) = self.sun_on(fired_date) else {
            warn!(
                date = %fired_date,
                latitude = self.latitude,
                longitude = self.longitude,
                "daynight: polar night or midnight sun in reschedule; retrying at next midnight"
            );
            return self.next_midnight(fired);
        };

        self.next_event_after(fired, rise, set)
    }

    /// Whichever of today's sunrise and sunset comes next, or tomorrow's sunrise when both have
    /// passed.
    fn next_event_after(&self, now: Timestamp, rise: Timestamp, set: Timestamp) -> Timestamp {
        if now < rise {
            return rise;
        }
        if now < set {
            return set;
        }

        let tomorrow = self.local_date(now).tomorrow().ok();
        tomorrow
            .and_then(|tomorrow| self.sun_on(tomorrow))
            .map_or_else(
                || self.next_midnight(now),
                |(tomorrow_rise, _)| tomorrow_rise,
            )
    }

    fn sun_on(&self, date: Date) -> Option<(Timestamp, Timestamp)> {
        (self.sun)(self.latitude, self.longitude, date)
    }

    fn local_date(&self, instant: Timestamp) -> Date {
        instant.to_zoned(self.zone.clone()).date()
    }

    /// The start of the local day after `instant`.
    fn next_midnight(&self, instant: Timestamp) -> Timestamp {
        next_local_midnight(instant, &self.zone)
    }
}

impl ModeProvider for DayNight {
    fn current_mode(&self) -> Mode {
        Mode::from_u8(self.mode.load(Ordering::SeqCst))
    }
}

/// The scheduled action: re-evaluate the mode and report a change. A dropped controller does
/// nothing.
fn transition(
    controller: &Weak<DayNight>,
    due: Timestamp,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let Some(controller) = controller.upgrade() else {
        return Box::pin(std::future::ready(()));
    };
    match controller.update(due) {
        Some(mode) => (controller.on_change)(mode),
        None => Box::pin(std::future::ready(())),
    }
}

/// Sunrise and sunset from the `sunrise` crate, which matches the Go implementation's go-solar
/// within a second for the same date.
#[must_use]
pub fn sunrise_sunset(latitude: f64, longitude: f64, date: Date) -> Option<(Timestamp, Timestamp)> {
    let naive = chrono::NaiveDate::from_ymd_opt(
        i32::from(date.year()),
        u32::try_from(date.month()).ok()?,
        u32::try_from(date.day()).ok()?,
    )?;
    let solar_day = SolarDay::new(Coordinates::new(latitude, longitude)?, naive);
    let rise = solar_day.event_time(SolarEvent::Sunrise)?;
    let set = solar_day.event_time(SolarEvent::Sunset)?;
    Some((
        Timestamp::from_second(rise.timestamp()).ok()?,
        Timestamp::from_second(set.timestamp()).ok()?,
    ))
}

#[cfg(test)]
mod tests;
