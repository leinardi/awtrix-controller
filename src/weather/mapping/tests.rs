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

fn point(time: Timestamp, weather_code: Option<i64>) -> ForecastPoint {
    ForecastPoint {
        time,
        weather_code,
        precipitation: None,
        snowfall: None,
        temperature_2m: None,
        dew_point_2m: None,
        wind_gusts_10m: None,
        visibility: None,
    }
}

/// Go: `TestWMOToOverlay`
#[test]
fn wmo_codes_map_to_overlays() {
    let cases = [
        (0, None),
        (1, None),
        (2, None),
        (3, None),
        (45, None),
        (48, None),
        (51, Some(OverlayEffect::Drizzle)),
        (53, Some(OverlayEffect::Drizzle)),
        (55, Some(OverlayEffect::Drizzle)),
        (56, Some(OverlayEffect::Frost)),
        (57, Some(OverlayEffect::Frost)),
        (61, Some(OverlayEffect::Rain)),
        (63, Some(OverlayEffect::Rain)),
        (65, Some(OverlayEffect::Rain)),
        (66, Some(OverlayEffect::Frost)),
        (67, Some(OverlayEffect::Frost)),
        (71, Some(OverlayEffect::Snow)),
        (73, Some(OverlayEffect::Snow)),
        (75, Some(OverlayEffect::Snow)),
        (77, Some(OverlayEffect::Snow)),
        (80, Some(OverlayEffect::Rain)),
        (81, Some(OverlayEffect::Rain)),
        (82, Some(OverlayEffect::Storm)),
        (85, Some(OverlayEffect::Snow)),
        (86, Some(OverlayEffect::Snow)),
        (95, Some(OverlayEffect::Thunder)),
        (96, Some(OverlayEffect::Thunder)),
        (99, Some(OverlayEffect::Thunder)),
        (999, None),
    ];
    for (code, want) in cases {
        assert_eq!(wmo_to_overlay(code), want, "WMO {code}");
    }
}

/// Go: `TestSelectOverlayPicksHighestPriority`
#[test]
fn select_overlay_picks_highest_priority() {
    let now = instant("2025-01-01T12:00:00Z");
    let points = [
        point(now, Some(51)),
        point(now + SignedDuration::from_mins(15), Some(95)),
    ];
    assert_eq!(
        select_overlay(&points, now, 60),
        Some(OverlayEffect::Thunder)
    );
}

/// Go: `TestSelectOverlayWindowBoundary`
#[test]
fn select_overlay_window_is_start_inclusive_and_horizon_exclusive() {
    let now = instant("2025-01-01T12:00:00Z");
    let points = [
        point(now - SignedDuration::from_mins(1), Some(95)),
        point(now, Some(55)),
        point(now + SignedDuration::from_mins(60), Some(95)),
    ];
    assert_eq!(
        select_overlay(&points, now, 60),
        Some(OverlayEffect::Drizzle)
    );
}

/// Go: `TestSelectOverlayEmpty`
#[test]
fn select_overlay_empty() {
    assert_eq!(
        select_overlay(&[], instant("2025-01-01T12:00:00Z"), 60),
        None
    );
}

/// Go: `TestSelectOverlayNoPointsInWindow`
#[test]
fn select_overlay_no_points_in_window() {
    let now = instant("2025-01-01T12:00:00Z");
    let points = [point(now - SignedDuration::from_mins(15), Some(95))];
    assert_eq!(select_overlay(&points, now, 60), None);
}

/// Every overlay outranks the ones below it, and a step without a weather code has none.
#[test]
fn overlay_priorities() {
    let now = instant("2025-01-01T12:00:00Z");
    let ranked = [51, 61, 71, 82, 56, 95];
    for (index, &weaker) in ranked.iter().enumerate() {
        for &stronger in &ranked[index + 1..] {
            let points = [
                point(now, Some(stronger)),
                point(now, Some(weaker)),
                point(now, None),
            ];
            assert_eq!(
                select_overlay(&points, now, 60),
                wmo_to_overlay(stronger),
                "{weaker} vs {stronger}"
            );
        }
    }
    assert_eq!(select_overlay(&[point(now, None)], now, 60), None);
}

/// A huge horizon saturates instead of overflowing.
#[test]
fn huge_horizon_does_not_overflow() {
    let now = instant("2025-01-01T12:00:00Z");
    let points = [point(now + SignedDuration::from_hours(24), Some(95))];
    assert_eq!(
        select_overlay(&points, now, i64::MAX),
        Some(OverlayEffect::Thunder)
    );
}
