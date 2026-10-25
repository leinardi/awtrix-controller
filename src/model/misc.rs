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

//! The small single-purpose payloads.

use serde::{Deserialize, Serialize};

use super::enums::is_unset_name;
use super::{EffectPalette, is_zero};

/// The background effect of a notification or custom app.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectSettings {
    /// The animation speed.
    #[serde(skip_serializing_if = "is_zero")]
    pub speed: i64,
    /// The colour palette.
    #[serde(skip_serializing_if = "is_unset_name")]
    pub palette: Option<EffectPalette>,
    /// Blends the palette colours.
    #[serde(skip_serializing_if = "is_zero")]
    pub blend: bool,
}

/// The payload of the side LED indicator topics (`{clientID}/indicator1` to `indicator3`). An
/// empty payload clears the indicator.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Indicator {
    /// The colour.
    #[serde(skip_serializing_if = "is_zero")]
    pub color: String,
    /// The blink interval in milliseconds.
    #[serde(skip_serializing_if = "is_zero")]
    pub blink: i64,
    /// The fade interval in milliseconds.
    #[serde(skip_serializing_if = "is_zero")]
    pub fade: i64,
}

/// The payload of `{clientID}/moodlight`. An empty payload turns mood-light mode off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoodLight {
    /// The brightness.
    #[serde(skip_serializing_if = "is_zero")]
    pub brightness: i64,
    /// The colour.
    #[serde(skip_serializing_if = "is_zero")]
    pub color: String,
    /// The colour temperature in kelvin, an alternative to `color`.
    #[serde(skip_serializing_if = "is_zero")]
    pub kelvin: i64,
}

/// The payload of `{clientID}/power`. Always written: `false` (power off) is meaningful.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Power {
    /// Whether the matrix is on.
    pub power: bool,
}

/// The payload of `{clientID}/sleep`. Always written: `0` (sleep indefinitely) is meaningful.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sleep {
    /// The sleep duration in seconds.
    pub sleep: i64,
}

/// The payload of `{clientID}/rtttl`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rtttl {
    /// The RTTTL melody.
    #[serde(skip_serializing_if = "is_zero")]
    pub rtttl: String,
}

/// The payload of `{clientID}/sound`: an RTTTL file name from the MELODIES folder (without
/// extension), or the 4-digit number of a `DFPlayer` MP3.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sound {
    /// The sound to play.
    #[serde(skip_serializing_if = "is_zero")]
    pub sound: String,
}

/// The payload of `{clientID}/switch`: a [`BuiltInApp`](super::BuiltInApp) name or a custom
/// app's name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSwitch {
    /// The app to switch to.
    pub name: String,
}
