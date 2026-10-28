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

//! Severe-weather events in the forecast: for each event type, the first step that qualifies
//! within the notification horizon and the highest severity seen.
//!
//! Q1: a check that needs a value the forecast does not have is skipped for that step.

use std::fmt;

use jiff::{SignedDuration, Timestamp};

use super::ForecastPoint;
use super::mapping::{EventType, wmo};
use crate::config::WeatherConfig;

/// Snowfall above this, cm per 15 minutes, makes snow severe.
const SNOWFALL_HEAVY_THRESHOLD_CM: f64 = 0.5;

/// Event start times are rounded down to this in fingerprints.
const FINGERPRINT_ROUNDING_SECONDS: i64 = 30 * 60;

/// The temperature ceiling (°C) of the numeric severe frost gate, slightly above 0 because road
/// surfaces are colder than the air at 2 m.
const FREEZING_PRECIP_TEMP: f64 = 0.5;

/// The precipitation (mm per 15 minutes) the numeric severe frost gate needs alongside a low
/// temperature; it filters out the API's near-zero noise.
const FROST_SEVERE_PRECIP_MM: f64 = 0.1;

/// Seconds in an hour. The horizon saturates rather than overflow on a huge configured value.
const SECONDS_PER_HOUR: i64 = 3600;

/// How serious an event is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// A warning.
    Warning = 1,
    /// A severe warning.
    Severe = 2,
}

impl Severity {
    /// 1 for a warning, 2 for a severe warning, as in fingerprints.
    #[must_use]
    pub const fn level(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.level())
    }
}

/// One detected event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventCandidate {
    /// The event type.
    pub event_type: EventType,
    /// The first step it was detected in.
    pub start_time: Timestamp,
    /// The highest severity detected.
    pub severity: Severity,
    /// `type:start:severity`, the start rounded down to 30 minutes in Unix seconds: the same
    /// event in consecutive forecasts keeps its fingerprint.
    pub fingerprint: String,
}

/// The first qualifying step and the highest severity of one event type.
#[derive(Clone, Copy, Debug)]
struct Detection {
    first: Timestamp,
    severity: Severity,
}

/// The events per type, indexed by [`EventType::index`].
type Detections = [Option<Detection>; EventType::ALL.len()];

/// One candidate per event type found in `[now, now + notification_horizon_hours]` whose
/// notifications are enabled.
#[must_use]
pub fn detect_events(
    points: &[ForecastPoint],
    now: Timestamp,
    config: &WeatherConfig,
) -> Vec<EventCandidate> {
    let horizon = now
        .saturating_add(SignedDuration::from_secs(
            config
                .notification_horizon_hours
                .saturating_mul(SECONDS_PER_HOUR),
        ))
        .unwrap_or(now);

    let mut detections: Detections = [None; EventType::ALL.len()];
    for (index, point) in points.iter().enumerate() {
        if point.time < now || point.time > horizon {
            continue;
        }
        check_point(index, points, config, &mut detections);
    }

    EventType::ALL
        .into_iter()
        .filter(|&event_type| is_notify_enabled(event_type, config))
        .filter_map(|event_type| {
            let detection = detections[event_type.index()]?;
            let rounded = detection
                .first
                .as_second()
                .div_euclid(FINGERPRINT_ROUNDING_SECONDS)
                * FINGERPRINT_ROUNDING_SECONDS;
            Some(EventCandidate {
                event_type,
                start_time: detection.first,
                severity: detection.severity,
                fingerprint: format!("{}:{rounded}:{}", event_type.index(), detection.severity),
            })
        })
        .collect()
}

fn check_point(
    index: usize,
    points: &[ForecastPoint],
    config: &WeatherConfig,
    detections: &mut Detections,
) {
    let point = &points[index];
    let mut record = |event_type: EventType, severity: Severity| {
        record_event(detections, event_type, point.time, severity);
    };

    if let Some(severity) = thunderstorm(point) {
        record(EventType::Thunderstorm, severity);
    }
    if point.weather_code.is_some_and(is_freezing_precip_code) {
        record(EventType::FreezingPrecip, Severity::Severe);
    }
    if let Some(severity) = frost_risk(index, points, config) {
        record(EventType::FrostRisk, severity);
    }
    if let Some(severity) = heavy_rain(point, config) {
        record(EventType::HeavyRain, severity);
    }
    if let Some(severity) = strong_gusts(point, config) {
        record(EventType::StrongGusts, severity);
    }
    if let Some(severity) = snow(point) {
        record(EventType::Snow, severity);
    }
    if let Some(severity) = fog(point, config) {
        record(EventType::Fog, severity);
    }
}

/// Keeps the earliest start and the highest severity of each type.
fn record_event(
    detections: &mut Detections,
    event_type: EventType,
    time: Timestamp,
    severity: Severity,
) {
    let slot = &mut detections[event_type.index()];
    *slot = Some(match *slot {
        None => Detection {
            first: time,
            severity,
        },
        Some(existing) => Detection {
            first: existing.first.min(time),
            severity: existing.severity.max(severity),
        },
    });
}

fn thunderstorm(point: &ForecastPoint) -> Option<Severity> {
    match point.weather_code? {
        wmo::THUNDERSTORM => Some(Severity::Warning),
        wmo::THUNDERSTORM_HAIL | wmo::THUNDERSTORM_HEAVY_HAIL => Some(Severity::Severe),
        _ => None,
    }
}

const fn is_freezing_precip_code(code: i64) -> bool {
    matches!(
        code,
        wmo::FREEZING_DRIZZLE_LIGHT
            | wmo::FREEZING_DRIZZLE_HEAVY
            | wmo::FREEZING_RAIN_LIGHT
            | wmo::FREEZING_RAIN_HEAVY
    )
}

/// Severe on a freezing-precipitation code (whose precipitation the API may bin to 0), or on
/// precipitation at a near-freezing temperature. A warning when the temperature and its spread to
/// the dew point allow frost and precipitation within the look-forward window makes a wet surface
/// plausible.
fn frost_risk(index: usize, points: &[ForecastPoint], config: &WeatherConfig) -> Option<Severity> {
    let point = &points[index];

    let freezing_code = point.weather_code.is_some_and(is_freezing_precip_code);
    let freezing_precipitation = point.temperature_2m.zip(point.precipitation).is_some_and(
        |(temperature, precipitation)| {
            temperature <= FREEZING_PRECIP_TEMP && precipitation >= FROST_SEVERE_PRECIP_MM
        },
    );
    if freezing_code || freezing_precipitation {
        return Some(Severity::Severe);
    }

    // A negative spread is a data quirk that still passes the test.
    let (temperature, dew_point) = point.temperature_2m.zip(point.dew_point_2m)?;
    let frost_possible = temperature <= config.frost_temp_c
        && temperature - dew_point <= config.frost_dew_point_delta_c;
    let window = SignedDuration::try_from_secs_f64(config.frost_warn_precip_window_h * 3600.0)
        .unwrap_or(SignedDuration::MAX);
    (frost_possible
        && has_precipitation_in_window(index, points, window, config.frost_warn_precip_mm))
    .then_some(Severity::Warning)
}

/// Whether any step in `[points[index].time, points[index].time + window]` has at least
/// `minimum` mm of precipitation.
fn has_precipitation_in_window(
    index: usize,
    points: &[ForecastPoint],
    window: SignedDuration,
    minimum: f64,
) -> bool {
    let origin = points[index].time;
    let cutoff = origin.saturating_add(window).unwrap_or(origin);
    points
        .iter()
        .filter(|point| point.time >= origin && point.time <= cutoff)
        .any(|point| {
            point
                .precipitation
                .is_some_and(|precipitation| precipitation >= minimum)
        })
}

fn heavy_rain(point: &ForecastPoint, config: &WeatherConfig) -> Option<Severity> {
    if point.weather_code == Some(wmo::STORM_HEAVY_RAIN) {
        return Some(Severity::Severe);
    }
    point
        .precipitation
        .is_some_and(|precipitation| precipitation > config.heavy_rain_mm_per_15_min)
        .then_some(Severity::Warning)
}

fn strong_gusts(point: &ForecastPoint, config: &WeatherConfig) -> Option<Severity> {
    let gusts = point.wind_gusts_10m?;
    if gusts > config.gust_severe_kmh {
        Some(Severity::Severe)
    } else if gusts > config.gust_warn_kmh {
        Some(Severity::Warning)
    } else {
        None
    }
}

/// Snow codes; severe when the snowfall is heavy, a warning when it is light or unknown.
fn snow(point: &ForecastPoint) -> Option<Severity> {
    match point.weather_code? {
        wmo::SNOW_LIGHT
        | wmo::SNOW_MODERATE
        | wmo::SNOW_HEAVY
        | wmo::SNOW_GRAINS
        | wmo::SNOW_SHOWER_LIGHT
        | wmo::SNOW_SHOWER_HEAVY => {
            let heavy = point
                .snowfall
                .is_some_and(|snowfall| snowfall > SNOWFALL_HEAVY_THRESHOLD_CM);
            Some(if heavy {
                Severity::Severe
            } else {
                Severity::Warning
            })
        }
        _ => None,
    }
}

/// Known visibility uses the numeric thresholds, even without a fog code. When visibility is
/// missing, a fog code alone still warrants a warning.
fn fog(point: &ForecastPoint, config: &WeatherConfig) -> Option<Severity> {
    if let Some(visibility) = point.visibility {
        if visibility < config.fog_visibility_severe_m {
            return Some(Severity::Severe);
        }
        return (visibility < config.fog_visibility_warn_m).then_some(Severity::Warning);
    }
    point
        .weather_code
        .filter(|code| matches!(*code, wmo::FOG_LIGHT | wmo::FOG_RIME))
        .map(|_| Severity::Warning)
}

const fn is_notify_enabled(event_type: EventType, config: &WeatherConfig) -> bool {
    match event_type {
        EventType::Thunderstorm => config.notify_thunderstorm,
        EventType::FreezingPrecip => config.notify_freezing_precip,
        EventType::FrostRisk => config.notify_frost_risk,
        EventType::HeavyRain => config.notify_heavy_rain,
        EventType::StrongGusts => config.notify_strong_gusts,
        EventType::Snow => config.notify_snow,
        EventType::Fog => config.notify_fog,
    }
}

#[cfg(test)]
mod tests;
