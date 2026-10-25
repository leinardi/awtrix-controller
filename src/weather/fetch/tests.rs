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

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

use super::*;

const VALID_FORECAST_JSON: &str = r#"{
  "minutely_15": {
    "time":           ["2025-01-01T12:00","2025-01-01T12:15"],
    "weather_code":   [95, 0],
    "precipitation":  [3.5, 0.0],
    "snowfall":       [0.0, 0.0],
    "temperature_2m": [8.1, 7.9],
    "dew_point_2m":   [5.0, 4.8],
    "wind_gusts_10m": [52.0, 48.0],
    "visibility":     [900.0, 5000.0]
  }
}"#;

/// A loopback HTTP server answering every request with `status` and `body`, its base URL, and
/// the request lines it receives.
fn serve(status: u16, body: &'static str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let (requests, received) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            let _ = reader.read_line(&mut request_line);
            loop {
                let mut header = String::new();
                let read = reader.read_line(&mut header).unwrap_or(0);
                if read == 0 || header == "\r\n" {
                    break;
                }
            }
            let _ = requests.send(request_line.trim_end().to_owned());
            let response = format!(
                "HTTP/1.1 {status} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{address}"), received)
}

async fn fetch(
    status: u16,
    body: &'static str,
    timezone: &str,
) -> Result<Vec<ForecastPoint>, FetchError> {
    let (base_url, _requests) = serve(status, body);
    Fetcher::with_base_url(&base_url)
        .fetch(&CancellationToken::new(), 48.0, 11.0, timezone)
        .await
}

fn instant(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

/// Go: `TestFetchSuccess`
#[tokio::test]
async fn fetch_success() {
    let points = fetch(200, VALID_FORECAST_JSON, "UTC")
        .await
        .expect("points");
    assert_eq!(points.len(), 2);
    assert_eq!(
        points[0],
        ForecastPoint {
            time: instant("2025-01-01T12:00:00Z"),
            weather_code: Some(95),
            precipitation: Some(3.5),
            snowfall: Some(0.0),
            temperature_2m: Some(8.1),
            dew_point_2m: Some(5.0),
            wind_gusts_10m: Some(52.0),
            visibility: Some(900.0),
        }
    );
    assert_eq!(points[1].time, instant("2025-01-01T12:15:00Z"));
}

/// Go: `TestFetchHTTPError`
#[tokio::test]
async fn fetch_http_error() {
    let error = fetch(500, "server error", "UTC").await.expect_err("status");
    assert!(matches!(error, FetchError::HttpStatus(500)), "{error:?}");
    assert!(error.to_string().contains("500"), "{error}");
}

/// Go: `TestFetchInvalidJSON`
#[tokio::test]
async fn fetch_invalid_json() {
    let error = fetch(200, "{not valid json{{", "UTC")
        .await
        .expect_err("JSON");
    assert!(matches!(error, FetchError::Decode(_)), "{error:?}");
}

/// Go: `TestFetchMismatchedArrays`
#[tokio::test]
async fn fetch_mismatched_arrays() {
    let body = r#"{"minutely_15": {
        "time": ["2025-01-01T12:00"], "weather_code": [95, 0], "precipitation": [3.5],
        "snowfall": [0.0], "temperature_2m": [8.1], "dew_point_2m": [5.0],
        "wind_gusts_10m": [52.0], "visibility": [900.0]}}"#;
    let error = fetch(200, body, "UTC").await.expect_err("mismatch");
    assert!(matches!(error, FetchError::MismatchedArrays), "{error:?}");
}

/// Go: `TestFetchEmptyArrays`
#[tokio::test]
async fn fetch_empty_arrays() {
    let body = r#"{"minutely_15": {
        "time": [], "weather_code": [], "precipitation": [], "snowfall": [],
        "temperature_2m": [], "dew_point_2m": [], "wind_gusts_10m": [], "visibility": []}}"#;
    let error = fetch(200, body, "UTC").await.expect_err("empty");
    assert!(matches!(error, FetchError::EmptyArrays), "{error:?}");

    // A missing or null block or array reads as empty, as Go's decoder left it.
    for body in [
        r"{}",
        r#"{"minutely_15": null}"#,
        r#"{"minutely_15": {"time": null}}"#,
    ] {
        let error = parse_raw(serde_json::from_str(body).expect("JSON"), "UTC").expect_err(body);
        assert!(
            matches!(error, FetchError::EmptyArrays),
            "{body}: {error:?}"
        );
    }
}

/// Go: `TestFetchContextCanceled`
#[tokio::test]
async fn fetch_cancelled() {
    let (base_url, _requests) = serve(200, VALID_FORECAST_JSON);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = Fetcher::with_base_url(&base_url)
        .fetch(&cancel, 48.0, 11.0, "UTC")
        .await
        .expect_err("cancelled");
    assert!(matches!(error, FetchError::Cancelled), "{error:?}");
}

/// Cancelling a fetch whose server never answers returns at once, without waiting for the
/// request's timeout. The runtime is built by hand and shut down in the background: dropping a
/// `#[tokio::test]` runtime would wait for the blocked request on the blocking pool.
#[test]
fn cancel_returns_while_the_request_hangs() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    thread::spawn(move || {
        // Accept and hold every connection without answering.
        let mut held = Vec::new();
        for stream in listener.incoming() {
            held.push(stream);
        }
    });

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let fetched = runtime.block_on(async {
        let cancel = CancellationToken::new();
        let canceller = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            canceller.cancel();
        });

        let stalled = Fetcher::with_base_url(&format!("http://{address}"));
        tokio::time::timeout(FETCH_TIMEOUT / 2, stalled.fetch(&cancel, 48.0, 11.0, "UTC")).await
    });
    runtime.shutdown_background();

    let fetched = fetched.expect("returned before the request timed out");
    assert!(matches!(fetched, Err(FetchError::Cancelled)), "{fetched:?}");
}

/// Go: `TestFetchURLEncodesTimezone`. The whole query matches Go's `url.Values.Encode`.
#[tokio::test]
async fn fetch_url_encodes_the_query_like_go() {
    let (base_url, requests) = serve(200, VALID_FORECAST_JSON);
    Fetcher::with_base_url(&base_url)
        .fetch(&CancellationToken::new(), 48.0, 11.0, "Europe/Berlin")
        .await
        .expect("points");
    let request_line = requests.recv().expect("request");
    assert_eq!(
        request_line,
        "GET /v1/forecast?forecast_minutely_15=48&latitude=48&longitude=11&minutely_15=weather_code%2Cprecipitation%2Csnowfall%2Ctemperature_2m%2Cdew_point_2m%2Cwind_gusts_10m%2Cvisibility&timeformat=iso8601&timezone=Europe%2FBerlin HTTP/1.1"
    );
}

/// Coordinates are written as Go's `strconv.FormatFloat(value, 'f', -1, 64)` wrote them.
#[test]
fn coordinates_are_formatted_like_go() {
    let fetcher = Fetcher::with_base_url("http://localhost");
    let url = fetcher.url(52.520_008, -0.000_000_1, "UTC");
    assert!(
        url.contains("&latitude=52.520008&longitude=-0.0000001&"),
        "{url}"
    );
    assert_eq!(query_escape("a b+c/é"), "a+b%2Bc%2F%C3%A9");
}

/// Go: `TestFetchVisibilityAbsentWhenMissing`
#[tokio::test]
async fn visibility_is_none_when_missing() {
    let body = r#"{"minutely_15": {
        "time": ["2025-01-01T12:00"], "weather_code": [0], "precipitation": [0.0],
        "snowfall": [0.0], "temperature_2m": [10.0], "dew_point_2m": [5.0],
        "wind_gusts_10m": [20.0]}}"#;
    let points = fetch(200, body, "UTC").await.expect("points");
    assert_eq!(points[0].visibility, None);
}

/// Open-Meteo's -999 visibility, and a visibility array of the wrong length, are missing values.
#[test]
fn visibility_sentinel_and_wrong_length_are_none() {
    let body = r#"{"minutely_15": {
        "time": ["2025-01-01T12:00", "2025-01-01T12:15"], "weather_code": [0, 0],
        "precipitation": [0.0, 0.0], "snowfall": [0.0, 0.0], "temperature_2m": [10.0, 10.0],
        "dew_point_2m": [5.0, 5.0], "wind_gusts_10m": [20.0, 20.0], "visibility": [-999.0, 800.0]}}"#;
    let points = parse_raw(serde_json::from_str(body).expect("JSON"), "UTC").expect("points");
    assert_eq!(points[0].visibility, None);
    assert_eq!(points[1].visibility, Some(800.0));

    let body = body.replace("[-999.0, 800.0]", "[800.0]");
    let points = parse_raw(serde_json::from_str(&body).expect("JSON"), "UTC").expect("points");
    assert_eq!(points[1].visibility, None);
}

/// Q1: a `null` in any array is a missing value for that step, not 0. (Go decoded it as 0.)
#[test]
fn null_values_are_missing() {
    let body = r#"{"minutely_15": {
        "time": ["2025-01-01T12:00"], "weather_code": [null], "precipitation": [null],
        "snowfall": [null], "temperature_2m": [null], "dew_point_2m": [null],
        "wind_gusts_10m": [null], "visibility": [null]}}"#;
    let points = parse_raw(serde_json::from_str(body).expect("JSON"), "UTC").expect("points");
    assert_eq!(
        points,
        [ForecastPoint {
            time: instant("2025-01-01T12:00:00Z"),
            weather_code: None,
            precipitation: None,
            snowfall: None,
            temperature_2m: None,
            dew_point_2m: None,
            wind_gusts_10m: None,
            visibility: None,
        }]
    );
}

/// Step times are local to the requested time zone; an unknown zone reads them as UTC, and a
/// time that is not `YYYY-MM-DDTHH:MM` fails the fetch.
#[test]
fn times_are_local_to_the_time_zone() {
    let body = r#"{"minutely_15": {
        "time": ["2025-01-01T12:00"], "weather_code": [0], "precipitation": [0.0],
        "snowfall": [0.0], "temperature_2m": [10.0], "dew_point_2m": [5.0],
        "wind_gusts_10m": [20.0]}}"#;
    let parse = |timezone: &str| parse_raw(serde_json::from_str(body).expect("JSON"), timezone);
    assert_eq!(
        parse("Europe/Berlin").expect("points")[0].time,
        instant("2025-01-01T11:00:00Z")
    );
    assert_eq!(
        parse("Not/AZone").expect("points")[0].time,
        instant("2025-01-01T12:00:00Z")
    );

    let bad = body.replace("2025-01-01T12:00", "2025-01-01 12:00");
    let error = parse_raw(serde_json::from_str(&bad).expect("JSON"), "UTC").expect_err("time");
    assert!(matches!(error, FetchError::Time { .. }), "{error:?}");
}

/// Like Go's `json.Decoder.Decode`, decoding stops after the first JSON value; an empty body is
/// a decode error.
#[test]
fn decoding_ignores_trailing_data() {
    let body = format!("{VALID_FORECAST_JSON}\n{{\"more\": true}} trailing");
    let raw = decode(body.as_bytes()).expect("first value");
    assert_eq!(parse_raw(raw, "UTC").expect("points").len(), 2);
    assert!(matches!(decode(&b""[..]), Err(FetchError::Decode(_))));
    assert!(matches!(decode(&b"[1, 2]"[..]), Err(FetchError::Decode(_))));
}
