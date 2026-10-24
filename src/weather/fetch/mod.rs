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

//! Fetching the `minutely_15` forecast from Open-Meteo.
//!
//! The HTTP call is blocking (ureq, rustls with ring and the bundled webpki roots) and runs on
//! tokio's blocking pool. Q1: a `null` in a forecast array means the value is missing for that
//! step, not zero.

use std::fmt::Write as _;
use std::time::Duration;

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::clock::resolve_local;
use crate::config::load_time_zone;

/// The production Open-Meteo endpoint.
pub const DEFAULT_BASE_URL: &str = "https://api.open-meteo.com";

/// How long one request may take, connection and body included.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// 12 hours of `minutely_15` steps.
const FORECAST_STEPS: u32 = 48;

/// The variables requested for each step.
const VARIABLES: &str =
    "weather_code,precipitation,snowfall,temperature_2m,dew_point_2m,wind_gusts_10m,visibility";

/// Open-Meteo's marker for a visibility it does not have.
#[expect(
    clippy::float_cmp,
    reason = "compared exactly against the literal sentinel the API sends"
)]
fn is_visibility_absent(value: f64) -> bool {
    value == -999.0
}

/// A failed fetch.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The forecast has no steps.
    #[error("weather: empty forecast arrays")]
    EmptyArrays,
    /// The forecast's arrays have different lengths.
    #[error("weather: mismatched array lengths")]
    MismatchedArrays,
    /// The server answered with a status other than 200.
    #[error("weather: HTTP {0}: non-OK HTTP status")]
    HttpStatus(u16),
    /// The request could not be made or its body not read.
    #[error("weather: execute request: {0}")]
    Request(#[source] Box<ureq::Error>),
    /// The body is not the expected JSON.
    #[error("weather: decode response: {0}")]
    Decode(#[source] serde_json::Error),
    /// A step's time is not `YYYY-MM-DDTHH:MM`.
    #[error("weather: parse time {value:?}: {source}")]
    Time {
        /// The time as sent.
        value: String,
        /// Why it did not parse.
        #[source]
        source: jiff::Error,
    },
    /// The fetch was cancelled before it finished.
    #[error("weather: fetch cancelled")]
    Cancelled,
    /// The blocking task running the request failed.
    #[error("weather: fetch task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

/// One `minutely_15` step. Every measurement is `None` when the forecast has no value for it.
#[derive(Clone, Debug, PartialEq)]
pub struct ForecastPoint {
    /// The start of the step.
    pub time: Timestamp,
    /// The WMO weather code.
    pub weather_code: Option<i64>,
    /// Precipitation, mm per 15 minutes.
    pub precipitation: Option<f64>,
    /// Snowfall, cm per 15 minutes.
    pub snowfall: Option<f64>,
    /// Air temperature at 2 m, °C.
    pub temperature_2m: Option<f64>,
    /// Dew point at 2 m, °C.
    pub dew_point_2m: Option<f64>,
    /// Wind gusts at 10 m, km/h.
    pub wind_gusts_10m: Option<f64>,
    /// Visibility, m.
    pub visibility: Option<f64>,
}

/// Fetches forecasts from Open-Meteo.
#[derive(Clone, Debug)]
pub struct Fetcher {
    agent: ureq::Agent,
    base_url: String,
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher {
    /// A fetcher for the production endpoint.
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// A fetcher for another endpoint, for tests.
    #[must_use]
    pub fn with_base_url(base_url: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(FETCH_TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            agent,
            base_url: base_url.to_owned(),
        }
    }

    /// The 12-hour `minutely_15` forecast for a location, its times read in `timezone`.
    ///
    /// Cancelling `cancel` returns at once; the request itself, on the blocking pool, is left to
    /// finish within its timeout.
    ///
    /// # Errors
    ///
    /// Any HTTP, decoding or validation failure, or cancellation.
    pub async fn fetch(
        &self,
        cancel: &CancellationToken,
        latitude: f64,
        longitude: f64,
        timezone: &str,
    ) -> Result<Vec<ForecastPoint>, FetchError> {
        let url = self.url(latitude, longitude, timezone);
        debug!(url, latitude, longitude, timezone, "weather: fetch request");
        if cancel.is_cancelled() {
            return Err(FetchError::Cancelled);
        }

        let agent = self.agent.clone();
        let timezone = timezone.to_owned();
        let request = tokio::task::spawn_blocking(move || fetch_blocking(&agent, &url, &timezone));
        tokio::select! {
            biased;
            () = cancel.cancelled() => Err(FetchError::Cancelled),
            joined = request => joined?,
        }
    }

    /// The request URL, with the query encoded as Go's `url.Values.Encode` did: keys sorted, and
    /// everything but letters, digits and `-_.~` percent-encoded.
    fn url(&self, latitude: f64, longitude: f64, timezone: &str) -> String {
        let steps = FORECAST_STEPS.to_string();
        let latitude = latitude.to_string();
        let longitude = longitude.to_string();
        let query = [
            ("forecast_minutely_15", steps.as_str()),
            ("latitude", latitude.as_str()),
            ("longitude", longitude.as_str()),
            ("minutely_15", VARIABLES),
            ("timeformat", "iso8601"),
            ("timezone", timezone),
        ];

        let mut url = format!("{}/v1/forecast?", self.base_url);
        for (index, (key, value)) in query.into_iter().enumerate() {
            if index > 0 {
                url.push('&');
            }
            url.push_str(key);
            url.push('=');
            url.push_str(&query_escape(value));
        }
        url
    }
}

/// Go's `url.QueryEscape`.
fn query_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                escaped.push(char::from(byte));
            }
            b' ' => escaped.push('+'),
            _ => {
                let _ = write!(escaped, "%{byte:02X}");
            }
        }
    }
    escaped
}

fn fetch_blocking(
    agent: &ureq::Agent,
    url: &str,
    timezone: &str,
) -> Result<Vec<ForecastPoint>, FetchError> {
    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| FetchError::Request(Box::new(error)))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(FetchError::HttpStatus(status));
    }

    let raw = decode(response.body_mut().as_reader())?;
    let points = parse_raw(raw, timezone)?;
    debug!(status, points = points.len(), "weather: fetch response");
    Ok(points)
}

/// Decodes the first JSON value of the body and ignores anything after it, as Go's
/// `json.Decoder.Decode` did.
fn decode(body: impl std::io::Read) -> Result<RawResponse, FetchError> {
    serde_json::Deserializer::from_reader(body)
        .into_iter::<RawResponse>()
        .next()
        .unwrap_or_else(|| serde_json::from_str(""))
        .map_err(FetchError::Decode)
}

/// The JSON Open-Meteo returns. A missing or `null` array reads as empty, as Go's decoder left it.
#[derive(Debug, Default, Deserialize)]
pub(super) struct RawResponse {
    #[serde(default)]
    minutely_15: Option<RawMinutely>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMinutely {
    #[serde(default)]
    time: Option<Vec<Option<String>>>,
    #[serde(default)]
    weather_code: Option<Vec<Option<i64>>>,
    #[serde(default)]
    precipitation: Option<Vec<Option<f64>>>,
    #[serde(default)]
    snowfall: Option<Vec<Option<f64>>>,
    #[serde(default)]
    temperature_2m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    dew_point_2m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    wind_gusts_10m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    visibility: Option<Vec<Option<f64>>>,
}

/// Checks the arrays and builds the points. Every array but `visibility` must have one value per
/// step; a `visibility` array of another length is ignored, and its `-999` marks a missing value.
pub(super) fn parse_raw(
    raw: RawResponse,
    timezone: &str,
) -> Result<Vec<ForecastPoint>, FetchError> {
    let minutely = raw.minutely_15.unwrap_or_default();
    let times = minutely.time.unwrap_or_default();
    let count = times.len();
    if count == 0 {
        return Err(FetchError::EmptyArrays);
    }

    let weather_code = minutely.weather_code.unwrap_or_default();
    let precipitation = minutely.precipitation.unwrap_or_default();
    let snowfall = minutely.snowfall.unwrap_or_default();
    let temperature = minutely.temperature_2m.unwrap_or_default();
    let dew_point = minutely.dew_point_2m.unwrap_or_default();
    let wind_gusts = minutely.wind_gusts_10m.unwrap_or_default();
    if [
        weather_code.len(),
        precipitation.len(),
        snowfall.len(),
        temperature.len(),
        dew_point.len(),
        wind_gusts.len(),
    ]
    .iter()
    .any(|&length| length != count)
    {
        return Err(FetchError::MismatchedArrays);
    }
    let visibility = minutely
        .visibility
        .filter(|values| values.len() == count)
        .unwrap_or_else(|| vec![None; count]);

    let zone = load_time_zone(timezone).unwrap_or(TimeZone::UTC);
    times
        .into_iter()
        .enumerate()
        .map(|(index, time)| {
            Ok(ForecastPoint {
                time: parse_time(time.unwrap_or_default(), &zone)?,
                weather_code: weather_code[index],
                precipitation: precipitation[index],
                snowfall: snowfall[index],
                temperature_2m: temperature[index],
                dew_point_2m: dew_point[index],
                wind_gusts_10m: wind_gusts[index],
                visibility: visibility[index].filter(|&value| !is_visibility_absent(value)),
            })
        })
        .collect()
}

/// A step's local `YYYY-MM-DDTHH:MM` time in `zone`.
fn parse_time(value: String, zone: &TimeZone) -> Result<Timestamp, FetchError> {
    DateTime::strptime("%Y-%m-%dT%H:%M", &value)
        .and_then(|local| resolve_local(local.date(), local.time(), zone))
        .map_err(|source| FetchError::Time { value, source })
}

#[cfg(test)]
mod tests;
