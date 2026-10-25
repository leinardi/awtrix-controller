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

//! The configuration file as written: every field optional, decoded leniently the way yaml.v3
//! decoded it (see [`lenient`](super::lenient)). Unknown keys are ignored.

use serde::Deserialize;

use super::lenient;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileConfig {
    #[serde(deserialize_with = "lenient::or_default")]
    pub mqtt: FileMqtt,
    #[serde(deserialize_with = "lenient::or_default")]
    pub location: FileLocation,
    #[serde(deserialize_with = "lenient::or_default")]
    pub timezone: String,
    #[serde(deserialize_with = "lenient::or_default")]
    pub energy_saving: FileEnergySaving,
    #[serde(deserialize_with = "lenient::or_default")]
    pub theme: FileTheme,
    #[serde(deserialize_with = "lenient::list")]
    pub scheduled_notifications: Vec<FileScheduledNotification>,
    #[serde(deserialize_with = "lenient::or_default")]
    pub weather: FileWeather,
    #[serde(deserialize_with = "lenient::or_default")]
    pub log_format: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileMqtt {
    #[serde(deserialize_with = "lenient::int")]
    pub port: Option<i64>,
    #[serde(deserialize_with = "lenient::int")]
    pub ws_port: Option<i64>,
    #[serde(deserialize_with = "lenient::or_default")]
    pub username: String,
    #[serde(deserialize_with = "lenient::or_default")]
    pub password: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileLocation {
    #[serde(deserialize_with = "lenient::float")]
    pub latitude: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub longitude: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileEnergySaving {
    #[serde(deserialize_with = "lenient::or_default")]
    pub start: String,
    #[serde(deserialize_with = "lenient::or_default")]
    pub end: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileTheme {
    #[serde(deserialize_with = "lenient::or_default")]
    pub day: FileThemeColors,
    #[serde(deserialize_with = "lenient::or_default")]
    pub night: FileThemeColors,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileThemeColors {
    #[serde(deserialize_with = "lenient::or_default")]
    pub calendar_accent: String,
    #[serde(deserialize_with = "lenient::or_default")]
    pub content: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileScheduledNotification {
    #[serde(deserialize_with = "lenient::or_default")]
    pub name: String,
    #[serde(deserialize_with = "lenient::or_default")]
    pub message: String,
    #[serde(deserialize_with = "lenient::or_default")]
    pub repeat: String,
    #[serde(deserialize_with = "lenient::or_default")]
    pub date: String,
    #[serde(deserialize_with = "lenient::int")]
    pub day: Option<i64>,
    #[serde(deserialize_with = "lenient::list")]
    pub weekdays: Vec<String>,
    #[serde(deserialize_with = "lenient::or_default")]
    pub time: String,
    #[serde(deserialize_with = "lenient::boolean")]
    pub enabled: Option<bool>,
    #[serde(deserialize_with = "lenient::int")]
    pub duration: Option<i64>,
    #[serde(deserialize_with = "lenient::or_default")]
    pub icon: String,
    #[serde(deserialize_with = "lenient::boolean")]
    pub rainbow: Option<bool>,
    #[serde(deserialize_with = "lenient::int")]
    pub scroll_speed: Option<i64>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub wakeup: Option<bool>,
    #[serde(deserialize_with = "lenient::or_default")]
    pub rtttl: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct FileWeather {
    #[serde(deserialize_with = "lenient::boolean")]
    pub enabled: Option<bool>,
    #[serde(deserialize_with = "lenient::int")]
    pub poll_interval_minutes: Option<i64>,
    #[serde(deserialize_with = "lenient::int")]
    pub overlay_horizon_minutes: Option<i64>,
    #[serde(deserialize_with = "lenient::int")]
    pub notification_horizon_hours: Option<i64>,
    #[serde(deserialize_with = "lenient::int")]
    pub notification_repeat_minutes: Option<i64>,
    #[serde(deserialize_with = "lenient::int")]
    pub inactive_after_missing_polls: Option<i64>,
    #[serde(deserialize_with = "lenient::int")]
    pub data_stale_ttl_minutes: Option<i64>,
    #[serde(deserialize_with = "lenient::int")]
    pub notification_text_repeat: Option<i64>,
    #[serde(deserialize_with = "lenient::float")]
    pub gust_warn_kmh: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub gust_severe_kmh: Option<f64>,
    #[serde(
        rename = "heavy_rain_mm_per_15min",
        deserialize_with = "lenient::float"
    )]
    pub heavy_rain_mm_per_15_min: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub fog_visibility_warn_m: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub fog_visibility_severe_m: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub frost_temp_c: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub frost_dew_point_delta_c: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub frost_warn_precip_window_h: Option<f64>,
    #[serde(deserialize_with = "lenient::float")]
    pub frost_warn_precip_mm: Option<f64>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub notify_thunderstorm: Option<bool>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub notify_freezing_precip: Option<bool>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub notify_frost_risk: Option<bool>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub notify_heavy_rain: Option<bool>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub notify_strong_gusts: Option<bool>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub notify_snow: Option<bool>,
    #[serde(deserialize_with = "lenient::boolean")]
    pub notify_fog: Option<bool>,
    #[serde(deserialize_with = "lenient::or_default")]
    pub rtttl: String,
}
