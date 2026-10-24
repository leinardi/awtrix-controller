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

use super::*;

fn instant(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn now() -> Timestamp {
    instant("2025-01-01T12:00:00Z")
}

/// The thresholds of the Go tests, every notification enabled.
fn config() -> WeatherConfig {
    WeatherConfig {
        enabled: true,
        poll_interval_minutes: 15,
        overlay_horizon_minutes: 60,
        notification_horizon_hours: 8,
        notification_repeat_minutes: 60,
        inactive_after_missing_polls: 2,
        data_stale_ttl_minutes: 45,
        notification_text_repeat: 3,
        gust_warn_kmh: 45.0,
        gust_severe_kmh: 60.0,
        heavy_rain_mm_per_15_min: 5.0,
        fog_visibility_warn_m: 1000.0,
        fog_visibility_severe_m: 200.0,
        frost_temp_c: 2.0,
        frost_dew_point_delta_c: 2.0,
        frost_warn_precip_window_h: 2.0,
        frost_warn_precip_mm: 0.2,
        notify_thunderstorm: true,
        notify_freezing_precip: true,
        notify_frost_risk: true,
        notify_heavy_rain: true,
        notify_strong_gusts: true,
        notify_snow: true,
        notify_fog: true,
        rtttl: String::new(),
    }
}

/// A step `offset` minutes after [`now`] with every measurement present but visibility.
#[expect(clippy::too_many_arguments, reason = "mirrors the Go tests' makePoint")]
fn point(
    offset: i64,
    code: i64,
    precipitation: f64,
    snowfall: f64,
    temperature: f64,
    dew_point: f64,
    gusts: f64,
    visibility: Option<f64>,
) -> ForecastPoint {
    ForecastPoint {
        time: now() + SignedDuration::from_mins(offset),
        weather_code: Some(code),
        precipitation: Some(precipitation),
        snowfall: Some(snowfall),
        temperature_2m: Some(temperature),
        dew_point_2m: Some(dew_point),
        wind_gusts_10m: Some(gusts),
        visibility,
    }
}

/// The severity detected for `event_type`, if any.
fn severity_of(points: &[ForecastPoint], event_type: EventType) -> Option<Severity> {
    detect_events(points, now(), &config())
        .into_iter()
        .find(|candidate| candidate.event_type == event_type)
        .map(|candidate| candidate.severity)
}

/// Go: `TestDetectEventsThunderstorm`
#[test]
fn thunderstorm() {
    let warning = [point(0, 95, 0.0, 0.0, 10.0, 5.0, 20.0, None)];
    assert_eq!(
        severity_of(&warning, EventType::Thunderstorm),
        Some(Severity::Warning)
    );
    let severe = [point(0, 96, 0.0, 0.0, 10.0, 5.0, 20.0, None)];
    assert_eq!(
        severity_of(&severe, EventType::Thunderstorm),
        Some(Severity::Severe)
    );
}

/// Go: `TestDetectEventsFreezingPrecip`
#[test]
fn freezing_precipitation() {
    let points = [point(0, 56, 1.0, 0.0, -1.0, -3.0, 10.0, None)];
    assert_eq!(
        severity_of(&points, EventType::FreezingPrecip),
        Some(Severity::Severe)
    );
}

/// Go: `TestDetectEventsFrostRisk`
#[test]
fn frost_risk() {
    // Severe when the temperature is at or below freezing with precipitation.
    let freezing_rain = [point(0, 0, 0.5, 0.0, -0.5, -2.0, 5.0, None)];
    assert_eq!(
        severity_of(&freezing_rain, EventType::FrostRisk),
        Some(Severity::Severe)
    );

    // A warning at 1.5 °C, 1 °C above the dew point, with 0.3 mm an hour later.
    let wet_cold = [
        point(0, 0, 0.0, 0.0, 1.5, 0.5, 5.0, None),
        point(60, 0, 0.3, 0.0, 1.5, 0.5, 5.0, None),
    ];
    assert_eq!(
        severity_of(&wet_cold, EventType::FrostRisk),
        Some(Severity::Warning)
    );

    // No warning on a dry cold night.
    let dry_cold = [point(0, 0, 0.0, 0.0, 1.0, 0.5, 5.0, None)];
    assert_eq!(severity_of(&dry_cold, EventType::FrostRisk), None);

    // Severe for freezing drizzle and freezing rain codes even with no precipitation.
    let drizzle = [point(0, 56, 0.0, 0.0, -1.0, -3.0, 5.0, None)];
    assert_eq!(
        severity_of(&drizzle, EventType::FrostRisk),
        Some(Severity::Severe)
    );
    let rain = [point(0, 66, 0.0, 0.0, 0.0, -2.0, 5.0, None)];
    assert_eq!(
        severity_of(&rain, EventType::FrostRisk),
        Some(Severity::Severe)
    );

    // Severe at 0.3 °C with 0.15 mm.
    let near_freezing = [point(0, 0, 0.15, 0.0, 0.3, -1.0, 5.0, None)];
    assert_eq!(
        severity_of(&near_freezing, EventType::FrostRisk),
        Some(Severity::Severe)
    );
}

/// Go: `TestDetectEventsHeavyRain`
#[test]
fn heavy_rain() {
    let storm = [point(0, 82, 8.0, 0.0, 15.0, 10.0, 30.0, None)];
    assert_eq!(
        severity_of(&storm, EventType::HeavyRain),
        Some(Severity::Severe)
    );
    let heavy = [point(0, 61, 6.0, 0.0, 15.0, 10.0, 30.0, None)];
    assert_eq!(
        severity_of(&heavy, EventType::HeavyRain),
        Some(Severity::Warning)
    );
}

/// Go: `TestDetectEventsStrongGusts`
#[test]
fn strong_gusts() {
    let severe = [point(0, 0, 0.0, 0.0, 10.0, 5.0, 65.0, None)];
    assert_eq!(
        severity_of(&severe, EventType::StrongGusts),
        Some(Severity::Severe)
    );
    let warning = [point(0, 0, 0.0, 0.0, 10.0, 5.0, 50.0, None)];
    assert_eq!(
        severity_of(&warning, EventType::StrongGusts),
        Some(Severity::Warning)
    );
}

/// Go: `TestDetectEventsSnow`
#[test]
fn snow() {
    let heavy = [point(0, 75, 0.0, 1.0, -3.0, -5.0, 10.0, None)];
    assert_eq!(severity_of(&heavy, EventType::Snow), Some(Severity::Severe));
    let light = [point(0, 71, 0.0, 0.2, -1.0, -3.0, 10.0, None)];
    assert_eq!(
        severity_of(&light, EventType::Snow),
        Some(Severity::Warning)
    );
}

/// Go: `TestDetectEventsFog`
#[test]
fn fog() {
    let dense = [point(0, 45, 0.0, 0.0, 5.0, 4.0, 5.0, Some(100.0))];
    assert_eq!(severity_of(&dense, EventType::Fog), Some(Severity::Severe));
    let unknown_visibility = [point(0, 45, 0.0, 0.0, 5.0, 4.0, 5.0, None)];
    assert_eq!(
        severity_of(&unknown_visibility, EventType::Fog),
        Some(Severity::Warning)
    );
}

/// Go: `TestDetectEventsDisabledType`
#[test]
fn disabled_type_is_not_reported() {
    let points = [point(0, 95, 0.0, 0.0, 10.0, 5.0, 20.0, None)];
    let disabled = WeatherConfig {
        notify_thunderstorm: false,
        ..config()
    };
    let candidates = detect_events(&points, now(), &disabled);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.event_type != EventType::Thunderstorm)
    );
}

/// Go: `TestDetectEventsOutsideHorizon`
#[test]
fn outside_horizon() {
    let points = [point(9 * 60, 95, 0.0, 0.0, 10.0, 5.0, 20.0, None)];
    assert_eq!(severity_of(&points, EventType::Thunderstorm), None);
    // The horizon itself is inside, as in Go.
    let at_horizon = [point(8 * 60, 95, 0.0, 0.0, 10.0, 5.0, 20.0, None)];
    assert_eq!(
        severity_of(&at_horizon, EventType::Thunderstorm),
        Some(Severity::Warning)
    );
}

/// Go: `TestDetectEventsFingerprint`
#[test]
fn fingerprint_rounds_to_30_minutes() {
    let detect = |offset| {
        detect_events(
            &[point(offset, 95, 0.0, 0.0, 10.0, 5.0, 20.0, None)],
            now(),
            &config(),
        )
        .remove(0)
    };
    let first = detect(0);
    let second = detect(15);
    assert_eq!(first.fingerprint, second.fingerprint);
    // type 0, 2025-01-01T12:00:00Z, severity 1: the format Go wrote.
    assert_eq!(first.fingerprint, "0:1735732800:1");
    assert_ne!(detect(30).fingerprint, first.fingerprint);
}

/// The earliest qualifying step is the start, and the highest severity across steps wins.
#[test]
fn earliest_start_and_highest_severity() {
    let points = [
        point(30, 96, 0.0, 0.0, 10.0, 5.0, 20.0, None),
        point(15, 95, 0.0, 0.0, 10.0, 5.0, 20.0, None),
    ];
    let candidate = detect_events(&points, now(), &config()).remove(0);
    assert_eq!(candidate.event_type, EventType::Thunderstorm);
    assert_eq!(candidate.start_time, now() + SignedDuration::from_mins(15));
    assert_eq!(candidate.severity, Severity::Severe);
}

/// Q1, reading "a null visibility gives no fog alert" as the false alert Go raised: Go read a
/// `null` visibility as 0 m, below every threshold, and raised a severe fog alert. A missing
/// visibility now raises no visibility-based alert; a fog weather code is still the warning it
/// was in Go when the visibility was absent.
#[test]
fn null_visibility_gives_no_severe_fog_alert() {
    let zero_metres = [point(0, 45, 0.0, 0.0, 5.0, 4.0, 5.0, Some(0.0))];
    assert_eq!(
        severity_of(&zero_metres, EventType::Fog),
        Some(Severity::Severe)
    );
    let null = [point(0, 45, 0.0, 0.0, 5.0, 4.0, 5.0, None)];
    assert_eq!(severity_of(&null, EventType::Fog), Some(Severity::Warning));
}

/// Q1: a `null` temperature skips the frost checks that need it. Go read it as 0 °C, which with
/// precipitation is a severe frost alert.
#[test]
fn null_temperature_gives_no_frost_alert() {
    let mut points = [
        point(0, 61, 0.5, 0.0, 0.0, 0.0, 5.0, None),
        point(15, 61, 0.5, 0.0, 0.0, 0.0, 5.0, None),
    ];
    for step in &mut points {
        step.temperature_2m = None;
    }
    assert_eq!(severity_of(&points, EventType::FrostRisk), None);

    // A freezing-precipitation code needs no temperature.
    points[0].weather_code = Some(66);
    assert_eq!(
        severity_of(&points, EventType::FrostRisk),
        Some(Severity::Severe)
    );
}

/// Q1: every other missing value skips its check: gusts, precipitation, snowfall, dew point and
/// the weather code.
#[test]
fn missing_values_skip_their_checks() {
    let mut step = point(0, 75, 9.0, 2.0, 1.0, 0.5, 70.0, None);
    step.wind_gusts_10m = None;
    step.precipitation = None;
    step.snowfall = None;
    let points = [step.clone()];
    assert_eq!(severity_of(&points, EventType::StrongGusts), None);
    assert_eq!(severity_of(&points, EventType::HeavyRain), None);
    assert_eq!(
        severity_of(&points, EventType::Snow),
        Some(Severity::Warning)
    );

    // No dew point: no frost warning, though the next step is wet.
    let mut cold = point(0, 0, 0.0, 0.0, 1.0, 0.5, 5.0, None);
    cold.dew_point_2m = None;
    let wet = point(15, 0, 0.3, 0.0, 5.0, 4.0, 5.0, None);
    assert_eq!(severity_of(&[cold, wet], EventType::FrostRisk), None);

    // No weather code: none of the code-based events.
    step.weather_code = None;
    assert!(detect_events(&[step], now(), &config()).is_empty());
}

/// A huge horizon saturates instead of overflowing.
#[test]
fn huge_horizon_does_not_overflow() {
    let points = [point(24 * 60, 95, 0.0, 0.0, 10.0, 5.0, 20.0, None)];
    let unbounded = WeatherConfig {
        notification_horizon_hours: i64::MAX,
        ..config()
    };
    let candidates = detect_events(&points, now(), &unbounded);
    assert_eq!(candidates.len(), 1);
}
