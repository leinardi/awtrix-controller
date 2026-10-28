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

//! The weather controller: polls the forecast, keeps the display overlay in step with it, and
//! publishes a notification for each new, escalated or repeated severe-weather event.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::events::{EventCandidate, Severity, detect_events};
use super::fetch::{FetchError, Fetcher, ForecastPoint};
use super::mapping::{EventType, select_overlay};
use super::state::StateManager;
use crate::clock::Clock;
use crate::config::{LocationConfig, WeatherConfig, load_time_zone};
use crate::model::{AppContent, Notification, OverlayEffect, TextContent};
use crate::scheduler::Scheduler;
use crate::state::{self, StateError};

/// How long one poll may wait for the forecast.
const POLL_TIMEOUT: Duration = Duration::from_secs(10);

/// Seconds in a minute, for intervals that saturate instead of overflowing.
const SECONDS_PER_MINUTE: i64 = 60;

/// The text colour of a severe warning; a warning keeps the device's default.
const SEVERE_COLOR: &str = "#FFFF00";

/// The result of one fetch.
pub type FetchFuture = Pin<Box<dyn Future<Output = Result<Vec<ForecastPoint>, FetchError>> + Send>>;

/// Fetches the forecast for a cancellation token, a latitude, a longitude and a time zone name.
pub type FetchFunction =
    Arc<dyn Fn(CancellationToken, f64, f64, String) -> FetchFuture + Send + Sync>;

/// The result of publishing one notification.
pub type PublishFuture =
    Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>;

/// Where the controller sends what it decides: the devices, their notifications and the overlay.
/// The broker wiring implements it; tests record the calls.
pub trait Publisher: Send + Sync {
    /// The IDs of the devices connected now.
    fn connected_ids(&self) -> Vec<String>;

    /// Publishes a notification to one device.
    fn publish(&self, client_id: &str, notification: Arc<Notification>) -> PublishFuture;

    /// Applies a new overlay, `None` for none. Called only when it changes.
    fn set_overlay(
        &self,
        overlay: Option<OverlayEffect>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

/// What the controller remembers between polls.
#[derive(Default)]
struct Memory {
    overlay: Option<OverlayEffect>,
    last_notified: Option<NotifiedForecast>,
}

/// The notification batch and the age of the forecast it came from. A later successful poll
/// without new notifications must not refresh this batch's age.
#[derive(Clone)]
struct NotifiedForecast {
    candidates: Vec<EventCandidate>,
    notified_at: Timestamp,
    fetched_at: Timestamp,
}

/// Polls the forecast and publishes overlays and warnings.
pub struct Controller {
    config: WeatherConfig,
    latitude: f64,
    longitude: f64,
    timezone: String,
    zone: TimeZone,
    clock: Arc<dyn Clock>,
    fetch: FetchFunction,
    publisher: Arc<dyn Publisher>,
    cancel: CancellationToken,
    states: Mutex<StateManager>,
    memory: Mutex<Memory>,
}

impl Controller {
    /// A controller fetching from Open-Meteo. Cancelling `cancel` abandons an in-flight fetch.
    #[must_use]
    pub fn new(
        config: WeatherConfig,
        location: &LocationConfig,
        timezone: &str,
        clock: Arc<dyn Clock>,
        publisher: Arc<dyn Publisher>,
        cancel: &CancellationToken,
    ) -> Arc<Self> {
        let fetcher = Fetcher::new();
        let fetch: FetchFunction = Arc::new(move |cancel, latitude, longitude, timezone| {
            let fetcher = fetcher.clone();
            Box::pin(async move { fetcher.fetch(&cancel, latitude, longitude, &timezone).await })
        });
        Self::with_fetch_function(config, location, timezone, clock, fetch, publisher, cancel)
    }

    /// A controller with an injected fetch, for tests and the simulation.
    #[must_use]
    pub fn with_fetch_function(
        config: WeatherConfig,
        location: &LocationConfig,
        timezone: &str,
        clock: Arc<dyn Clock>,
        fetch: FetchFunction,
        publisher: Arc<dyn Publisher>,
        cancel: &CancellationToken,
    ) -> Arc<Self> {
        let timezone = if timezone.is_empty() {
            warn!("weather: no timezone configured; using UTC");
            "UTC"
        } else {
            timezone
        };
        Arc::new(Self {
            config,
            latitude: location.latitude,
            longitude: location.longitude,
            zone: load_time_zone(timezone).unwrap_or(TimeZone::UTC),
            timezone: timezone.to_owned(),
            clock,
            fetch,
            publisher,
            cancel: cancel.child_token(),
            states: Mutex::new(StateManager::new()),
            memory: Mutex::new(Memory::default()),
        })
    }

    /// Schedules the first poll now and the next ones every `poll_interval_minutes`.
    pub fn start(self: &Arc<Self>, scheduler: &Scheduler) {
        info!(
            latitude = self.latitude,
            longitude = self.longitude,
            timezone = self.timezone,
            poll_interval_minutes = self.config.poll_interval_minutes,
            "weather: controller started"
        );

        let interval = minutes(self.config.poll_interval_minutes);
        let controller = Arc::downgrade(self);
        let failures = scheduler.failures();
        scheduler.schedule(
            "weather:poll",
            self.clock.now(),
            move || {
                let controller = controller.clone();
                let failures = failures.clone();
                async move {
                    if let Some(controller) = controller.upgrade()
                        && let Err(error) = controller.poll().await
                    {
                        failures.report(error);
                    }
                }
            },
            move |fired| fired.saturating_add(interval).unwrap_or(fired),
        );
    }

    /// Sends a newly connected device the warnings of the last poll that notified, unless
    /// the repeat interval or the forecast's stale-data TTL has elapsed.
    ///
    /// # Errors
    /// The notification cache is poisoned.
    pub async fn on_device_connected(&self, client_id: &str) -> Result<(), StateError> {
        let cached = {
            let memory = self.memory()?;
            memory.last_notified.clone()
        };
        let Some(cached) = cached else {
            return Ok(());
        };
        let now = self.clock.now();
        let repeat_interval = minutes(self.config.notification_repeat_minutes);
        let ttl = minutes(self.config.data_stale_ttl_minutes);
        if now.duration_since(cached.notified_at) >= repeat_interval
            || now.duration_since(cached.fetched_at) >= ttl
        {
            return Ok(());
        }
        for candidate in &cached.candidates {
            let published = self.publish_to(client_id, candidate).await;
            if let Err(error) = published {
                warn!(
                    client_id,
                    event = ?candidate.event_type,
                    error = %error,
                    "weather: publish notification to new client failed"
                );
            }
        }
        Ok(())
    }

    /// One poll: fetch, update the overlay, detect events and publish those to notify. A failed
    /// fetch clears the overlay and keeps every event's state; a cancelled one does nothing.
    async fn poll(&self) -> Result<(), StateError> {
        let fetch_start = self.clock.now();
        debug!(
            latitude = self.latitude,
            longitude = self.longitude,
            timezone = self.timezone,
            "weather: polling"
        );

        let fetch = (self.fetch)(
            self.cancel.clone(),
            self.latitude,
            self.longitude,
            self.timezone.clone(),
        );
        let points = match tokio::time::timeout(POLL_TIMEOUT, fetch).await {
            Ok(Ok(points)) => points,
            Ok(Err(FetchError::Cancelled)) => return Ok(()),
            Ok(Err(error)) => {
                return self.fetch_failed(&error).await;
            }
            Err(elapsed) => {
                return self.fetch_failed(&elapsed).await;
            }
        };

        let now = self.clock.now();
        let latency = now.duration_since(fetch_start);
        debug!(points = points.len(), latency = %latency, "weather: forecast received");

        let selected = select_overlay(&points, now, self.config.overlay_horizon_minutes);
        debug!(
            overlay = ?selected,
            horizon_minutes = self.config.overlay_horizon_minutes,
            "weather: overlay selected"
        );
        self.apply_overlay(selected.clone()).await?;

        let candidates = detect_events(&points, now, &self.config);
        for candidate in &candidates {
            debug!(
                event = ?candidate.event_type,
                severity = %candidate.severity,
                start_time = %self.local_time(candidate.start_time),
                fingerprint = candidate.fingerprint,
                "weather: event candidate"
            );
        }

        let to_notify = state::checked(self.states.lock(), "weather event state")?.process(
            &candidates,
            now,
            minutes(self.config.notification_repeat_minutes),
            self.config.inactive_after_missing_polls,
        );
        if !to_notify.is_empty() {
            let mut memory = self.memory()?;
            memory.last_notified = Some(NotifiedForecast {
                candidates: to_notify.clone(),
                notified_at: now,
                fetched_at: fetch_start,
            });
        }

        debug!(
            latency = %latency,
            forecast_points = points.len(),
            overlay = ?selected,
            candidates = candidates.len(),
            notifications_queued = to_notify.len(),
            "weather: poll complete"
        );

        for candidate in &to_notify {
            self.publish_candidate(candidate, now).await;
        }
        Ok(())
    }

    async fn fetch_failed(&self, error: &(dyn std::fmt::Display + Sync)) -> Result<(), StateError> {
        warn!(error = %error, "weather: fetch failed; clearing overlay");
        state::checked(self.states.lock(), "weather event state")?.on_fetch_failure();
        self.apply_overlay(None).await
    }

    /// Applies `overlay` only when it differs from the current one.
    async fn apply_overlay(&self, overlay: Option<OverlayEffect>) -> Result<(), StateError> {
        let changed = {
            let mut memory = self.memory()?;
            let changed = memory.overlay != overlay;
            memory.overlay.clone_from(&overlay);
            changed
        };
        debug!(overlay = ?overlay, changed, "weather: overlay update");
        if changed {
            self.publisher.set_overlay(overlay).await;
        }
        Ok(())
    }

    /// Publishes one event to every connected device, logging at info when at least one got it
    /// and at debug when none was connected (a device connecting later gets it then).
    async fn publish_candidate(&self, candidate: &EventCandidate, now: Timestamp) {
        let start_time = self.local_time(candidate.start_time);
        let mut published = 0_usize;
        for client_id in self.publisher.connected_ids() {
            match self.publish_to(&client_id, candidate).await {
                Ok(()) => published += 1,
                Err(error) => warn!(
                    client_id,
                    event = ?candidate.event_type,
                    error = %error,
                    "weather: publish notification failed"
                ),
            }
        }

        if published > 0 {
            info!(
                event = ?candidate.event_type,
                severity = %candidate.severity,
                start_time,
                now = %now,
                clients_notified = published,
                "weather: notification sent"
            );
        } else {
            debug!(
                event = ?candidate.event_type,
                severity = %candidate.severity,
                start_time,
                "weather: notification deferred — no clients connected"
            );
        }
    }

    async fn publish_to(
        &self,
        client_id: &str,
        candidate: &EventCandidate,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let notification = Arc::new(self.notification(candidate));
        self.publisher.publish(client_id, notification).await
    }

    /// The notification for one event: its text and icon, yellow text when severe.
    fn notification(&self, candidate: &EventCandidate) -> Notification {
        let text = format!(
            "{} from {}",
            event_label(candidate.event_type),
            self.local_time(candidate.start_time)
        );
        let color = match candidate.severity {
            Severity::Severe => SEVERE_COLOR.to_owned(),
            Severity::Warning => String::new(),
        };
        Notification {
            content: AppContent {
                text: Some(TextContent::plain(text)),
                icon: event_icon(candidate.event_type).to_owned(),
                color,
                repeat: Some(self.config.notification_text_repeat),
                ..AppContent::default()
            },
            rtttl: self.config.rtttl.clone(),
            wakeup: true,
            ..Notification::default()
        }
    }

    /// `instant` as HH:MM in the configured time zone.
    fn local_time(&self, instant: Timestamp) -> String {
        instant
            .to_zoned(self.zone.clone())
            .strftime("%H:%M")
            .to_string()
    }

    fn memory(&self) -> Result<std::sync::MutexGuard<'_, Memory>, StateError> {
        state::checked(self.memory.lock(), "weather notification cache")
    }
}

/// `count` minutes, saturating instead of overflowing on a huge configured value.
fn minutes(count: i64) -> SignedDuration {
    SignedDuration::from_secs(count.saturating_mul(SECONDS_PER_MINUTE))
}

/// The start of an event's notification text.
const fn event_label(event_type: EventType) -> &'static str {
    match event_type {
        EventType::Thunderstorm => "Thunderstorm",
        EventType::FreezingPrecip => "Freezing rain",
        EventType::FrostRisk => "Frost risk",
        EventType::HeavyRain => "Heavy rain",
        EventType::StrongGusts => "Strong gusts",
        EventType::Snow => "Snow",
        EventType::Fog => "Fog",
    }
}

/// The `LaMetric` icon ID of an event's notification.
const fn event_icon(event_type: EventType) -> &'static str {
    match event_type {
        EventType::Thunderstorm => "49299",
        EventType::FreezingPrecip => "49301",
        EventType::FrostRisk => "4807",
        EventType::HeavyRain => "49300",
        EventType::StrongGusts => "3363",
        EventType::Snow => "2289",
        EventType::Fog => "17056",
    }
}

#[cfg(test)]
mod tests;
