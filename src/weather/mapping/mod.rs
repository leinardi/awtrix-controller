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

//! WMO weather codes: the overlay each one shows, and the event types the controller notifies.

use jiff::{SignedDuration, Timestamp};

use super::ForecastPoint;
use crate::model::OverlayEffect;

/// WMO weather codes.
pub(super) mod wmo {
    pub const FOG_LIGHT: i64 = 45;
    pub const FOG_RIME: i64 = 48;
    pub const DRIZZLE_LIGHT: i64 = 51;
    pub const DRIZZLE_MODERATE: i64 = 53;
    pub const DRIZZLE_HEAVY: i64 = 55;
    pub const FREEZING_DRIZZLE_LIGHT: i64 = 56;
    pub const FREEZING_DRIZZLE_HEAVY: i64 = 57;
    pub const RAIN_LIGHT: i64 = 61;
    pub const RAIN_MODERATE: i64 = 63;
    pub const RAIN_HEAVY: i64 = 65;
    pub const FREEZING_RAIN_LIGHT: i64 = 66;
    pub const FREEZING_RAIN_HEAVY: i64 = 67;
    pub const SNOW_LIGHT: i64 = 71;
    pub const SNOW_MODERATE: i64 = 73;
    pub const SNOW_HEAVY: i64 = 75;
    pub const SNOW_GRAINS: i64 = 77;
    pub const RAIN_SHOWER_SLIGHT: i64 = 80;
    pub const RAIN_SHOWER_MODERATE: i64 = 81;
    pub const STORM_HEAVY_RAIN: i64 = 82;
    pub const SNOW_SHOWER_LIGHT: i64 = 85;
    pub const SNOW_SHOWER_HEAVY: i64 = 86;
    pub const THUNDERSTORM: i64 = 95;
    pub const THUNDERSTORM_HAIL: i64 = 96;
    pub const THUNDERSTORM_HEAVY_HAIL: i64 = 99;
}

/// Seconds in a minute. The horizon saturates rather than overflow on a huge configured value.
const SECONDS_PER_MINUTE: i64 = 60;

/// A category of severe-weather notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventType {
    /// WMO 95, 96 and 99.
    Thunderstorm = 0,
    /// Freezing drizzle and rain, WMO 56, 57, 66 and 67.
    FreezingPrecip = 1,
    /// Conditions where frost on a wet surface is likely.
    FrostRisk = 2,
    /// Heavy precipitation.
    HeavyRain = 3,
    /// Strong wind gusts.
    StrongGusts = 4,
    /// Snowfall, WMO 71, 73, 75, 77, 85 and 86.
    Snow = 5,
    /// Fog and depositing rime fog, WMO 45 and 48.
    Fog = 6,
}

impl EventType {
    /// Every event type, in the order they are reported.
    pub const ALL: [Self; 7] = [
        Self::Thunderstorm,
        Self::FreezingPrecip,
        Self::FrostRisk,
        Self::HeavyRain,
        Self::StrongGusts,
        Self::Snow,
        Self::Fog,
    ];

    /// The number Go gave the type, used in fingerprints.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The overlay a WMO code shows, or `None` for codes without one (clear, cloudy, fog, unknown).
#[must_use]
pub fn wmo_to_overlay(code: i64) -> Option<OverlayEffect> {
    match code {
        wmo::DRIZZLE_LIGHT | wmo::DRIZZLE_MODERATE | wmo::DRIZZLE_HEAVY => {
            Some(OverlayEffect::Drizzle)
        }
        wmo::FREEZING_DRIZZLE_LIGHT
        | wmo::FREEZING_DRIZZLE_HEAVY
        | wmo::FREEZING_RAIN_LIGHT
        | wmo::FREEZING_RAIN_HEAVY => Some(OverlayEffect::Frost),
        wmo::RAIN_LIGHT
        | wmo::RAIN_MODERATE
        | wmo::RAIN_HEAVY
        | wmo::RAIN_SHOWER_SLIGHT
        | wmo::RAIN_SHOWER_MODERATE => Some(OverlayEffect::Rain),
        wmo::SNOW_LIGHT
        | wmo::SNOW_MODERATE
        | wmo::SNOW_HEAVY
        | wmo::SNOW_GRAINS
        | wmo::SNOW_SHOWER_LIGHT
        | wmo::SNOW_SHOWER_HEAVY => Some(OverlayEffect::Snow),
        wmo::STORM_HEAVY_RAIN => Some(OverlayEffect::Storm),
        wmo::THUNDERSTORM | wmo::THUNDERSTORM_HAIL | wmo::THUNDERSTORM_HEAVY_HAIL => {
            Some(OverlayEffect::Thunder)
        }
        // Clear sky and cloud cover (0 to 3), fog (45, 48) and unknown codes.
        _ => None,
    }
}

/// How strongly an overlay wins over another; no overlay is the weakest.
fn priority(overlay: Option<&OverlayEffect>) -> u8 {
    match overlay {
        None | Some(OverlayEffect::Clear | OverlayEffect::Other(_)) => 0,
        Some(OverlayEffect::Drizzle) => 1,
        Some(OverlayEffect::Rain) => 2,
        Some(OverlayEffect::Snow) => 3,
        Some(OverlayEffect::Storm) => 4,
        Some(OverlayEffect::Frost) => 5,
        Some(OverlayEffect::Thunder) => 6,
    }
}

/// The highest-priority overlay among the steps in `[now, now + horizon_minutes)`, or `None`
/// when no step there has one. A step without a weather code has no overlay.
#[must_use]
pub fn select_overlay(
    points: &[ForecastPoint],
    now: Timestamp,
    horizon_minutes: i64,
) -> Option<OverlayEffect> {
    let horizon = now
        .saturating_add(SignedDuration::from_secs(
            horizon_minutes.saturating_mul(SECONDS_PER_MINUTE),
        ))
        .unwrap_or(now);
    points
        .iter()
        .filter(|point| point.time >= now && point.time < horizon)
        .filter_map(|point| point.weather_code.and_then(wmo_to_overlay))
        .fold(None, |best, candidate| {
            if priority(Some(&candidate)) > priority(best.as_ref()) {
                Some(candidate)
            } else {
                best
            }
        })
}

#[cfg(test)]
mod tests;
