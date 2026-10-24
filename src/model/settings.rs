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

//! The partial settings payload, published retained to `{clientID}/settings`.

use serde::{Deserialize, Serialize};

use super::enums::is_unset_name;
use super::{ClockMode, DateFormat, OverlayEffect, TimeFormat, TransitionEffect, is_zero};

/// The settings the controller manages, plus the rest of the firmware's settings for
/// completeness. Every field but `overlay` is left out when unset, so the device keeps whatever it
/// has for the settings the controller does not send.
///
/// A setting whose zero value is meaningful (`BRI` 0, `ABRI` false, `TEFF` 0, and the boolean
/// settings whose firmware default is true) is an `Option`, so that the zero value can be sent
/// explicitly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // --- Fields managed by the controller ---
    /// The calendar header colour (`#RRGGBB`), from the day/night theme. Default "#FF0000".
    #[serde(rename = "CHCOL", skip_serializing_if = "is_zero")]
    pub ch_col: String,
    /// The calendar body colour (`#RRGGBB`), from the day/night theme. Default "#FFFFFF".
    #[serde(rename = "CBCOL", skip_serializing_if = "is_zero")]
    pub cb_col: String,
    /// The active weekday indicator colour, from the day/night theme.
    #[serde(rename = "WDCA", skip_serializing_if = "is_zero")]
    pub wdca: String,
    /// The inactive weekday indicator colour, from the day/night theme.
    #[serde(rename = "WDCI", skip_serializing_if = "is_zero")]
    pub wdci: String,
    /// The time app's text colour, from the day/night theme; "#000000" inherits the global one.
    #[serde(rename = "TIME_COL", skip_serializing_if = "is_zero")]
    pub time_col: String,
    /// The date app's text colour, from the day/night theme; "#000000" inherits the global one.
    #[serde(rename = "DATE_COL", skip_serializing_if = "is_zero")]
    pub date_col: String,
    /// The matrix brightness, 0 to 255: 1 while energy saving is active, unset otherwise.
    #[serde(rename = "BRI", skip_serializing_if = "Option::is_none")]
    pub bri: Option<i64>,
    /// Automatic brightness: false while energy saving is active, true otherwise.
    #[serde(rename = "ABRI", skip_serializing_if = "Option::is_none")]
    pub abri: Option<bool>,

    // --- Other firmware settings, not managed by the controller ---
    /// How long each app is shown, in seconds. Default 7.
    #[serde(rename = "ATIME", skip_serializing_if = "is_zero")]
    pub atime: i64,
    /// The app transition animation. Default `Slide`.
    #[serde(rename = "TEFF", skip_serializing_if = "Option::is_none")]
    pub teff: Option<TransitionEffect>,
    /// The transition duration in milliseconds. Default 500.
    #[serde(rename = "TSPEED", skip_serializing_if = "is_zero")]
    pub tspeed: i64,
    /// The global text colour.
    #[serde(rename = "TCOL", skip_serializing_if = "is_zero")]
    pub tcol: String,
    /// The time app's clock face. Default `Calendar`.
    #[serde(rename = "TMODE", skip_serializing_if = "Option::is_none")]
    pub tmode: Option<ClockMode>,
    /// The calendar text colour in the time app. Default "#000000".
    #[serde(rename = "CTCOL", skip_serializing_if = "is_zero")]
    pub ctcol: String,
    /// Shows the weekday. Default true.
    #[serde(rename = "WD", skip_serializing_if = "Option::is_none")]
    pub wd: Option<bool>,
    /// Switches to the next app automatically.
    #[serde(rename = "ATRANS", skip_serializing_if = "Option::is_none")]
    pub atrans: Option<bool>,
    /// A colour correction as an RGB triple.
    #[serde(rename = "CCORRECTION", skip_serializing_if = "Vec::is_empty")]
    pub ccorrection: Vec<i64>,
    /// A colour temperature adjustment as an RGB triple.
    #[serde(rename = "CTEMP", skip_serializing_if = "Vec::is_empty")]
    pub ctemp: Vec<i64>,
    /// The time app's format.
    #[serde(rename = "TFORMAT", skip_serializing_if = "is_unset_name")]
    pub tformat: Option<TimeFormat>,
    /// The date app's format.
    #[serde(rename = "DFORMAT", skip_serializing_if = "is_unset_name")]
    pub dformat: Option<DateFormat>,
    /// Starts the week on Monday. Default true.
    #[serde(rename = "SOM", skip_serializing_if = "Option::is_none")]
    pub som: Option<bool>,
    /// Shows temperatures in Celsius. Default true.
    #[serde(rename = "CEL", skip_serializing_if = "Option::is_none")]
    pub cel: Option<bool>,
    /// Blocks the physical keys (they still send MQTT events). Default false.
    #[serde(rename = "BLOCKN", skip_serializing_if = "is_zero")]
    pub blockn: bool,
    /// Shows all text in uppercase. Default true.
    #[serde(rename = "UPPERCASE", skip_serializing_if = "Option::is_none")]
    pub uppercase: Option<bool>,
    /// The temperature app's text colour; "#000000" inherits the global one.
    #[serde(rename = "TEMP_COL", skip_serializing_if = "is_zero")]
    pub temp_col: String,
    /// The humidity app's text colour; "#000000" inherits the global one.
    #[serde(rename = "HUM_COL", skip_serializing_if = "is_zero")]
    pub hum_col: String,
    /// The battery app's text colour; "#000000" inherits the global one.
    #[serde(rename = "BAT_COL", skip_serializing_if = "is_zero")]
    pub bat_col: String,
    /// The scroll speed as a percentage of the original. Default 100.
    #[serde(rename = "SSPEED", skip_serializing_if = "is_zero")]
    pub sspeed: i64,
    /// Enables the native time app (takes effect after a reboot). Default true.
    #[serde(rename = "TIM", skip_serializing_if = "Option::is_none")]
    pub tim: Option<bool>,
    /// Enables the native date app (takes effect after a reboot). Default true.
    #[serde(rename = "DAT", skip_serializing_if = "Option::is_none")]
    pub dat: Option<bool>,
    /// Enables the native humidity app (takes effect after a reboot). Default true.
    #[serde(rename = "HUM", skip_serializing_if = "Option::is_none")]
    pub show_hum: Option<bool>,
    /// Enables the native temperature app (takes effect after a reboot). Default true.
    #[serde(rename = "TEMP", skip_serializing_if = "Option::is_none")]
    pub show_temp: Option<bool>,
    /// Enables the native battery app (takes effect after a reboot). Default true.
    #[serde(rename = "BAT", skip_serializing_if = "Option::is_none")]
    pub show_bat: Option<bool>,
    /// Turns the matrix on or off, without the power endpoint's animation. Default true.
    #[serde(rename = "MATP", skip_serializing_if = "Option::is_none")]
    pub matp: Option<bool>,
    /// The buzzer and `DFPlayer` volume, 0 to 30. Default 0.
    #[serde(rename = "VOL", skip_serializing_if = "is_zero")]
    pub vol: i64,
    /// The global weather overlay. Always written: `null` clears any overlay on the device.
    #[serde(rename = "OVERLAY")]
    pub overlay: Option<OverlayEffect>,
}
