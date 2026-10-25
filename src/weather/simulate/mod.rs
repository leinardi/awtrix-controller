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

//! A synthetic forecast for trying the weather features without real weather: every step has
//! the same WMO code and Go's fixed values, chosen to trip most warnings. For local debugging only.

use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use tracing::warn;

use super::controller::FetchFunction;
use super::fetch::ForecastPoint;

/// More than the default heavy-rain threshold (5 mm per 15 minutes).
const PRECIPITATION_MM: f64 = 6.0;
/// More than the heavy-snowfall threshold (0.5 cm per 15 minutes).
const SNOWFALL_CM: f64 = 1.0;
/// At the default frost ceiling (1.5 °C, inclusive).
const TEMPERATURE_C: f64 = 1.5;
/// The dew point, 1.5 °C below the temperature: inside a 2 °C frost spread, outside the 1 °C
/// default.
const DEW_POINT_C: f64 = 0.0;
/// Between the default gust warning (45 km/h) and severe (60 km/h) thresholds.
const WIND_GUSTS_KMH: f64 = 50.0;
/// Below the default severe fog visibility (200 m).
const VISIBILITY_M: f64 = 150.0;
/// The forecast step, 15 minutes.
const STEP_SECONDS: i64 = 15 * 60;
/// 12 hours of steps.
const STEPS: i64 = 48;

/// A fetch that returns 12 hours of steps from now, rounded down to 15 minutes, all with WMO code
/// `code`. It reads the wall clock: the simulation stands in for a live forecast.
#[must_use]
pub fn simulate_fetch(code: i64) -> FetchFunction {
    Arc::new(move |_cancel, latitude, longitude, _timezone| {
        warn!(
            wmo_code = code,
            latitude, longitude, "weather: SIMULATION MODE — returning synthetic forecast"
        );
        Box::pin(std::future::ready(Ok(synthetic_forecast(
            code,
            wall_clock(),
        ))))
    })
}

#[expect(
    clippy::disallowed_methods,
    reason = "the debug simulation stands in for a live forecast, which is anchored to the wall clock"
)]
fn wall_clock() -> Timestamp {
    Timestamp::now()
}

/// The synthetic forecast from `now` rounded down to 15 minutes.
fn synthetic_forecast(code: i64, now: Timestamp) -> Vec<ForecastPoint> {
    let first = now.as_second().div_euclid(STEP_SECONDS) * STEP_SECONDS;
    let first = Timestamp::from_second(first).unwrap_or(now);
    (0..STEPS)
        .map(|step| ForecastPoint {
            time: first
                .saturating_add(SignedDuration::from_secs(step * STEP_SECONDS))
                .unwrap_or(first),
            weather_code: Some(code),
            precipitation: Some(PRECIPITATION_MM),
            snowfall: Some(SNOWFALL_CM),
            temperature_2m: Some(TEMPERATURE_C),
            dew_point_2m: Some(DEW_POINT_C),
            wind_gusts_10m: Some(WIND_GUSTS_KMH),
            visibility: Some(VISIBILITY_M),
        })
        .collect()
}

#[cfg(test)]
mod tests;
