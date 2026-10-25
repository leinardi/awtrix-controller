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

async fn simulated(code: i64) -> Vec<ForecastPoint> {
    let fetch = simulate_fetch(code);
    fetch(
        tokio_util::sync::CancellationToken::new(),
        52.52,
        13.405,
        "UTC".to_owned(),
    )
    .await
    .expect("synthetic forecast")
}

/// Go: `TestSimulateFetchFuncReturnsCorrectCount`
#[tokio::test]
async fn returns_48_steps() {
    assert_eq!(simulated(95).await.len(), 48);
}

/// Go: `TestSimulateFetchFuncWMOCode`
#[tokio::test]
async fn every_step_has_the_code() {
    assert!(
        simulated(71)
            .await
            .iter()
            .all(|point| point.weather_code == Some(71))
    );
}

/// Go: `TestSimulateFetchFuncTimeProgression`
#[tokio::test]
async fn steps_are_15_minutes_apart() {
    let points = simulated(95).await;
    for pair in points.windows(2) {
        assert_eq!(
            pair[1].time.duration_since(pair[0].time),
            SignedDuration::from_mins(15)
        );
    }
}

/// The forecast starts at `now` rounded down to 15 minutes, with every value present.
#[test]
fn starts_at_the_quarter_hour() {
    let points = synthetic_forecast(95, instant("2025-01-01T12:22:41Z"));
    assert_eq!(points[0].time, instant("2025-01-01T12:15:00Z"));
    assert_eq!(points[47].time, instant("2025-01-02T00:00:00Z"));
    assert_eq!(
        points[0],
        ForecastPoint {
            time: instant("2025-01-01T12:15:00Z"),
            weather_code: Some(95),
            precipitation: Some(6.0),
            snowfall: Some(1.0),
            temperature_2m: Some(1.5),
            dew_point_2m: Some(0.0),
            wind_gusts_10m: Some(50.0),
            visibility: Some(150.0),
        }
    );
}
