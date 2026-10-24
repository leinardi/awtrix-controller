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

//! The defaults applied to optional configuration fields, in one place.

/// The MQTT TCP listen port when `mqtt.port` is absent or 0.
pub const DEFAULT_MQTT_PORT: i64 = 1883;

/// The configuration file read when neither `--config` nor `AWTRIX_CONFIG` names one.
pub const DEFAULT_CONFIG_PATH: &str = "/etc/awtrix-controller/config.yaml";

/// The energy-saving window start (HH:MM).
pub const DEFAULT_ENERGY_SAVING_START: &str = "00:30";

/// The energy-saving window end (HH:MM).
pub const DEFAULT_ENERGY_SAVING_END: &str = "06:00";

/// How long a scheduled notification is shown, in seconds.
pub const DEFAULT_SCHEDULED_NOTIFICATION_DURATION: i64 = 60;

/// The `LaMetric` icon ID of a scheduled notification.
pub const DEFAULT_SCHEDULED_NOTIFICATION_ICON: &str = "9597";

/// The scroll speed of a scheduled notification, in pixels per frame.
pub const DEFAULT_SCHEDULED_NOTIFICATION_SCROLL_SPEED: i64 = 50;

/// The fire time of a scheduled notification (HH:MM).
pub const DEFAULT_SCHEDULED_NOTIFICATION_TIME: &str = "00:00";

/// The calendar accent colour of the day theme.
pub const DEFAULT_THEME_DAY_ACCENT: &str = "#FF0000";

/// The content (text and date) colour of the day theme.
pub const DEFAULT_THEME_DAY_CONTENT: &str = "#FFFFFF";

/// The calendar accent colour of the night theme.
pub const DEFAULT_THEME_NIGHT_ACCENT: &str = "#FF0000";

/// The content (text and date) colour of the night theme.
pub const DEFAULT_THEME_NIGHT_CONTENT: &str = "#474747";

/// How often the Open-Meteo forecast is polled, in minutes.
pub const DEFAULT_WEATHER_POLL_INTERVAL_MINUTES: i64 = 15;

/// The look-ahead window for choosing the overlay, in minutes.
pub const DEFAULT_WEATHER_OVERLAY_HORIZON_MINUTES: i64 = 60;

/// The look-ahead window for detecting events, in hours.
pub const DEFAULT_WEATHER_NOTIFICATION_HORIZON_HOURS: i64 = 8;

/// The minimum interval between repeated notifications of one event, in minutes.
pub const DEFAULT_WEATHER_NOTIFICATION_REPEAT_MINUTES: i64 = 60;

/// How many consecutive polls without an event make it inactive.
pub const DEFAULT_WEATHER_INACTIVE_AFTER_MISSING_POLLS: i64 = 2;

/// The maximum age of fetched forecast data, in minutes.
pub const DEFAULT_WEATHER_DATA_STALE_TTL_MINUTES: i64 = 45;

/// How many times a weather notification's text scrolls.
pub const DEFAULT_WEATHER_NOTIFICATION_TEXT_REPEAT: i64 = 3;

/// The wind gust warning threshold, km/h.
pub const DEFAULT_WEATHER_GUST_WARN_KMH: f64 = 45.0;

/// The wind gust severe threshold, km/h.
pub const DEFAULT_WEATHER_GUST_SEVERE_KMH: f64 = 60.0;

/// The heavy-rain threshold, mm per 15 minutes.
pub const DEFAULT_WEATHER_HEAVY_RAIN_MM_PER_15MIN: f64 = 5.0;

/// The fog warning visibility threshold, metres.
pub const DEFAULT_WEATHER_FOG_VISIBILITY_WARN_M: f64 = 1000.0;

/// The fog severe visibility threshold, metres.
pub const DEFAULT_WEATHER_FOG_VISIBILITY_SEVERE_M: f64 = 200.0;

/// The air temperature ceiling for a frost-risk warning, °C.
pub const DEFAULT_WEATHER_FROST_TEMP_C: f64 = 1.5;

/// The largest temperature to dew point spread for a frost-risk warning, °C.
pub const DEFAULT_WEATHER_FROST_DEW_POINT_DELTA_C: f64 = 1.0;

/// The look-forward window of the frost-risk wet-surface gate, hours.
pub const DEFAULT_WEATHER_FROST_WARN_PRECIP_WINDOW_H: f64 = 2.0;

/// The precipitation the frost-risk gate needs within its window, mm.
pub const DEFAULT_WEATHER_FROST_WARN_PRECIP_MM: f64 = 0.2;
