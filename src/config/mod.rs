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

//! Loading and validating the YAML configuration file. The file is read once at startup.
//!
//! The file is decoded into an all-optional layer ([`file`]) that reads YAML the way the Go
//! implementation's yaml.v3 did, then turned into a [`Config`] whose every field is concrete:
//! required fields are checked, defaults are applied (all here, from [`defaults`]), and value
//! constraints are validated.
//!
//! The weather thresholds are the one place this differs from Go (Q2): Go replaced an explicit
//! `0` by the default, so `frost_temp_c: 0` could not be configured. Here only an absent (or
//! `null`) threshold takes the default; any given value is used, and validation checks its range
//! instead.

mod defaults;
mod file;
mod lenient;
mod time_of_day;
mod timezone;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::{fs, io};

use thiserror::Error;
use tracing::warn;

pub use defaults::*;
pub use time_of_day::{MonthDay, ParseError, TimeOfDay};
pub use timezone::{UnknownTimeZone, load_time_zone};

use file::{FileConfig, FileScheduledNotification, FileThemeColors, FileWeather};

/// The validated application configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The embedded MQTT broker.
    pub mqtt: MqttConfig,
    /// The coordinates for sunrise and sunset.
    pub location: LocationConfig,
    /// The IANA time zone name; empty means the system time zone.
    pub timezone: String,
    /// The nightly energy-saving window.
    pub energy_saving: EnergySavingConfig,
    /// The day and night colours.
    pub theme: ThemeConfig,
    /// The valid scheduled notifications; invalid entries are dropped with a warning.
    pub scheduled_notifications: Vec<ScheduledNotificationConfig>,
    /// The weather overlay and warnings.
    pub weather: WeatherConfig,
    /// The log output format.
    pub log_format: LogFormat,
}

/// The embedded MQTT broker's settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MqttConfig {
    /// The TCP listen port.
    pub port: i64,
    /// The WebSocket listen port; `None` disables WebSocket.
    pub ws_port: Option<i64>,
    /// The username every device must use.
    pub username: String,
    /// The password every device must use.
    pub password: String,
}

/// The coordinates sunrise and sunset are computed for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocationConfig {
    /// Degrees north.
    pub latitude: f64,
    /// Degrees east.
    pub longitude: f64,
}

/// The energy-saving window, as HH:MM wall-clock times in the configured time zone. The window
/// may span midnight (start "23:00", end "05:00").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnergySavingConfig {
    /// The start, HH:MM.
    pub start: String,
    /// The end, HH:MM.
    pub end: String,
}

/// The colours of the day and night themes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeConfig {
    /// The day theme.
    pub day: ThemeColors,
    /// The night theme.
    pub night: ThemeColors,
}

/// The two colours a theme manages, as `#RRGGBB`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeColors {
    /// The calendar header and active-weekday colour.
    pub calendar_accent: String,
    /// The text, date and inactive-weekday colour.
    pub content: String,
}

/// How often a scheduled notification repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// Every day.
    Daily,
    /// On the listed weekdays.
    Weekly,
    /// On a day of every month.
    Monthly,
    /// On a date of every year.
    Yearly,
}

/// One recurring notification. Every field is filled once validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledNotificationConfig {
    /// The name, `{name}` in the message.
    pub name: String,
    /// The message template, with `{name}` and `{year}` tokens.
    pub message: String,
    /// How often it repeats.
    pub repeat: Repeat,
    /// MM-DD, for a yearly notification.
    pub date: String,
    /// The day of the month (1–31), for a monthly notification.
    pub day: i64,
    /// The weekdays (monday–sunday), for a weekly notification.
    pub weekdays: Vec<String>,
    /// The fire time, HH:MM.
    pub time: String,
    /// Whether it fires.
    pub enabled: bool,
    /// How long it is shown, in seconds.
    pub duration: i64,
    /// The `LaMetric` icon ID.
    pub icon: String,
    /// Rainbow text.
    pub rainbow: bool,
    /// The scroll speed, in pixels per frame.
    pub scroll_speed: i64,
    /// Wakes the display from sleep first.
    pub wakeup: bool,
    /// An optional RTTTL melody.
    pub rtttl: String,
}

/// The weather overlay and warning settings, every default applied.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent configuration key (enabled and one per event type)"
)]
pub struct WeatherConfig {
    /// Whether any weather feature runs.
    pub enabled: bool,
    /// How often the forecast is polled, in minutes.
    pub poll_interval_minutes: i64,
    /// The look-ahead window for the overlay, in minutes.
    pub overlay_horizon_minutes: i64,
    /// The look-ahead window for events, in hours.
    pub notification_horizon_hours: i64,
    /// The minimum interval between repeats of one event's notification, in minutes.
    pub notification_repeat_minutes: i64,
    /// How many consecutive polls without an event make it inactive.
    pub inactive_after_missing_polls: i64,
    /// The maximum age of forecast data, in minutes.
    pub data_stale_ttl_minutes: i64,
    /// How many times a notification's text scrolls.
    pub notification_text_repeat: i64,
    /// The wind gust warning threshold, km/h.
    pub gust_warn_kmh: f64,
    /// The wind gust severe threshold, km/h.
    pub gust_severe_kmh: f64,
    /// The heavy-rain threshold, mm per 15 minutes.
    pub heavy_rain_mm_per_15_min: f64,
    /// The fog warning visibility, metres.
    pub fog_visibility_warn_m: f64,
    /// The fog severe visibility, metres.
    pub fog_visibility_severe_m: f64,
    /// The air temperature ceiling for frost risk, °C.
    pub frost_temp_c: f64,
    /// The largest temperature to dew point spread for frost risk, °C.
    pub frost_dew_point_delta_c: f64,
    /// The look-forward window of the frost-risk wet-surface gate, hours.
    pub frost_warn_precip_window_h: f64,
    /// The precipitation the frost-risk gate needs within its window, mm.
    pub frost_warn_precip_mm: f64,
    /// Notify thunderstorms.
    pub notify_thunderstorm: bool,
    /// Notify freezing precipitation.
    pub notify_freezing_precip: bool,
    /// Notify frost risk.
    pub notify_frost_risk: bool,
    /// Notify heavy rain.
    pub notify_heavy_rain: bool,
    /// Notify strong gusts.
    pub notify_strong_gusts: bool,
    /// Notify snow.
    pub notify_snow: bool,
    /// Notify fog.
    pub notify_fog: bool,
    /// An optional RTTTL melody played with each weather notification.
    pub rtttl: String,
}

/// The log output format (`log_format`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// tracing-subscriber's human-readable text (the default).
    #[default]
    Text,
    /// One JSON object per line.
    Json,
}

/// Why the configuration could not be loaded.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The file could not be read.
    #[error("read config file {}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// The file is not valid YAML for the configuration; the message names the field path and the
    /// line.
    #[error("parse config: {0}")]
    Parse(#[from] serde_yaml_ng::Error),
    /// `mqtt.username` is absent.
    #[error("mqtt.username is required")]
    MqttUsernameRequired,
    /// `mqtt.password` is absent.
    #[error("mqtt.password is required")]
    MqttPasswordRequired,
    /// `location.latitude` is absent.
    #[error("location.latitude is required")]
    LatitudeRequired,
    /// `location.longitude` is absent.
    #[error("location.longitude is required")]
    LongitudeRequired,
    /// `timezone` names no time zone.
    #[error("invalid timezone {name:?}: {source}")]
    InvalidTimezone {
        /// The name given.
        name: String,
        /// The cause.
        source: UnknownTimeZone,
    },
    /// A theme colour is not `#RRGGBB`.
    #[error("theme.{theme}: {field} {value:?}: not a valid #RRGGBB color")]
    InvalidHexColor {
        /// `day` or `night`.
        theme: &'static str,
        /// The colour's key.
        field: &'static str,
        /// The value given.
        value: String,
    },
    /// A weather threshold is outside its range.
    #[error("weather.{field} = {value}: must be {expected}")]
    OutOfRange {
        /// The threshold's key.
        field: &'static str,
        /// The value given.
        value: f64,
        /// The range, in words.
        expected: &'static str,
    },
    /// `log_format` is neither `text` nor `json`.
    #[error("log_format {value:?}: must be \"text\" or \"json\"")]
    InvalidLogFormat {
        /// The value given.
        value: String,
    },
}

/// Reads, parses and validates the configuration file at `path`.
///
/// # Errors
///
/// The file cannot be read, is not valid YAML for the configuration, or fails validation; the
/// first violation is reported.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })?;
    parse(&text)
}

/// The file's `log_format` alone, read before [`load`] so that the logger is up when `load`
/// logs the entries it skips. `None` when the file cannot be read or parsed, or the value is not
/// a format; [`load`] then reports why.
#[must_use]
pub fn peek_log_format(path: &Path) -> Option<LogFormat> {
    #[derive(serde::Deserialize)]
    struct LogFormatOnly {
        #[serde(default)]
        log_format: String,
    }

    let text = fs::read_to_string(path).ok()?;
    let file: LogFormatOnly = serde_yaml_ng::from_str(&text).ok()?;
    log_format(file.log_format).ok()
}

/// Parses and validates configuration YAML.
///
/// # Errors
///
/// The text is not valid YAML for the configuration, or fails validation.
pub fn parse(text: &str) -> Result<Config, ConfigError> {
    let file: FileConfig = serde_yaml_ng::from_str(text)?;
    validate(file)
}

/// Checks the required fields, applies the defaults and validates the values, in Go's order, so
/// the first violation reported is the one Go reported.
fn validate(file: FileConfig) -> Result<Config, ConfigError> {
    if file.mqtt.username.is_empty() {
        return Err(ConfigError::MqttUsernameRequired);
    }
    if file.mqtt.password.is_empty() {
        return Err(ConfigError::MqttPasswordRequired);
    }
    let mqtt = MqttConfig {
        port: non_zero_or(file.mqtt.port, DEFAULT_MQTT_PORT),
        ws_port: file.mqtt.ws_port,
        username: file.mqtt.username,
        password: file.mqtt.password,
    };

    let latitude = file
        .location
        .latitude
        .ok_or(ConfigError::LatitudeRequired)?;
    let longitude = file
        .location
        .longitude
        .ok_or(ConfigError::LongitudeRequired)?;

    if !file.timezone.is_empty() {
        load_time_zone(&file.timezone).map_err(|source| ConfigError::InvalidTimezone {
            name: file.timezone.clone(),
            source,
        })?;
    }

    let energy_saving = EnergySavingConfig {
        start: non_empty_or(file.energy_saving.start, DEFAULT_ENERGY_SAVING_START),
        end: non_empty_or(file.energy_saving.end, DEFAULT_ENERGY_SAVING_END),
    };

    let theme = ThemeConfig {
        day: theme_colors(
            file.theme.day,
            "day",
            DEFAULT_THEME_DAY_ACCENT,
            DEFAULT_THEME_DAY_CONTENT,
        )?,
        night: theme_colors(
            file.theme.night,
            "night",
            DEFAULT_THEME_NIGHT_ACCENT,
            DEFAULT_THEME_NIGHT_CONTENT,
        )?,
    };

    let scheduled_notifications = file
        .scheduled_notifications
        .into_iter()
        .filter_map(scheduled_notification)
        .collect();

    let weather = weather(file.weather)?;
    let log_format = log_format(file.log_format)?;

    Ok(Config {
        mqtt,
        location: LocationConfig {
            latitude,
            longitude,
        },
        timezone: file.timezone,
        energy_saving,
        theme,
        scheduled_notifications,
        weather,
        log_format,
    })
}

/// Go's "zero means unset" default for the fields Q2 leaves alone: absent, `null` and `0` all take
/// the default.
fn non_zero_or(value: Option<i64>, default: i64) -> i64 {
    value.filter(|given| *given != 0).unwrap_or(default)
}

fn non_empty_or(value: String, default: &str) -> String {
    if value.is_empty() {
        default.to_owned()
    } else {
        value
    }
}

fn theme_colors(
    colors: FileThemeColors,
    theme: &'static str,
    default_accent: &str,
    default_content: &str,
) -> Result<ThemeColors, ConfigError> {
    let check = |field: &'static str, value: String, default: &str| {
        if value.is_empty() {
            Ok(default.to_owned())
        } else if is_valid_hex_color(&value) {
            Ok(value)
        } else {
            Err(ConfigError::InvalidHexColor {
                theme,
                field,
                value,
            })
        }
    };

    Ok(ThemeColors {
        calendar_accent: check("calendar_accent", colors.calendar_accent, default_accent)?,
        content: check("content", colors.content, default_content)?,
    })
}

/// Reports whether a value is a `#RRGGBB` colour.
fn is_valid_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

/// Validates one entry and applies its defaults, or logs why it is skipped and drops it.
fn scheduled_notification(entry: FileScheduledNotification) -> Option<ScheduledNotificationConfig> {
    let name = entry.name.as_str();
    let day = entry.day.unwrap_or(0);

    let repeat = match entry.repeat.as_str() {
        "daily" => Repeat::Daily,
        "weekly" => {
            if entry.weekdays.is_empty() {
                warn!(
                    name,
                    "skipping scheduled_notification: weekdays list is empty"
                );
                return None;
            }
            if let Some(weekday) = entry
                .weekdays
                .iter()
                .find(|weekday| !WEEKDAYS.contains(&weekday.as_str()))
            {
                warn!(
                    name,
                    weekday, "skipping scheduled_notification: invalid weekday"
                );
                return None;
            }
            Repeat::Weekly
        }
        "monthly" => {
            if !(1..=31).contains(&day) {
                warn!(
                    name,
                    day, "skipping scheduled_notification: day out of range"
                );
                return None;
            }
            Repeat::Monthly
        }
        "yearly" => {
            if entry.date.parse::<MonthDay>().is_err() {
                warn!(
                    name,
                    date = entry.date,
                    "skipping scheduled_notification: invalid date"
                );
                return None;
            }
            Repeat::Yearly
        }
        repeat => {
            warn!(
                name,
                repeat, "skipping scheduled_notification: unknown repeat"
            );
            return None;
        }
    };

    if !entry.time.is_empty() && entry.time.parse::<TimeOfDay>().is_err() {
        warn!(
            name,
            time = entry.time,
            "skipping scheduled_notification: invalid time"
        );
        return None;
    }

    Some(ScheduledNotificationConfig {
        repeat,
        day,
        time: non_empty_or(entry.time, DEFAULT_SCHEDULED_NOTIFICATION_TIME),
        enabled: entry.enabled.unwrap_or(true),
        duration: non_zero_or(entry.duration, DEFAULT_SCHEDULED_NOTIFICATION_DURATION),
        icon: non_empty_or(entry.icon, DEFAULT_SCHEDULED_NOTIFICATION_ICON),
        rainbow: entry.rainbow.unwrap_or(false),
        scroll_speed: non_zero_or(
            entry.scroll_speed,
            DEFAULT_SCHEDULED_NOTIFICATION_SCROLL_SPEED,
        ),
        wakeup: entry.wakeup.unwrap_or(true),
        name: entry.name,
        message: entry.message,
        date: entry.date,
        weekdays: entry.weekdays,
        rtttl: entry.rtttl,
    })
}

const NON_NEGATIVE: &str = "a finite number >= 0";
const FINITE: &str = "a finite number";

/// A weather threshold (Q2): absent or `null` takes the default, any given value is kept and must
/// lie in its range.
fn threshold(
    value: Option<f64>,
    default: f64,
    field: &'static str,
    expected: &'static str,
) -> Result<f64, ConfigError> {
    let Some(value) = value else {
        return Ok(default);
    };

    let in_range = value.is_finite() && (expected == FINITE || value >= 0.0);
    if in_range {
        Ok(value)
    } else {
        Err(ConfigError::OutOfRange {
            field,
            value,
            expected,
        })
    }
}

fn weather(file: FileWeather) -> Result<WeatherConfig, ConfigError> {
    Ok(WeatherConfig {
        enabled: file.enabled.unwrap_or(false),
        poll_interval_minutes: non_zero_or(
            file.poll_interval_minutes,
            DEFAULT_WEATHER_POLL_INTERVAL_MINUTES,
        ),
        overlay_horizon_minutes: non_zero_or(
            file.overlay_horizon_minutes,
            DEFAULT_WEATHER_OVERLAY_HORIZON_MINUTES,
        ),
        notification_horizon_hours: non_zero_or(
            file.notification_horizon_hours,
            DEFAULT_WEATHER_NOTIFICATION_HORIZON_HOURS,
        ),
        notification_repeat_minutes: non_zero_or(
            file.notification_repeat_minutes,
            DEFAULT_WEATHER_NOTIFICATION_REPEAT_MINUTES,
        ),
        inactive_after_missing_polls: non_zero_or(
            file.inactive_after_missing_polls,
            DEFAULT_WEATHER_INACTIVE_AFTER_MISSING_POLLS,
        ),
        data_stale_ttl_minutes: non_zero_or(
            file.data_stale_ttl_minutes,
            DEFAULT_WEATHER_DATA_STALE_TTL_MINUTES,
        ),
        notification_text_repeat: non_zero_or(
            file.notification_text_repeat,
            DEFAULT_WEATHER_NOTIFICATION_TEXT_REPEAT,
        ),
        gust_warn_kmh: threshold(
            file.gust_warn_kmh,
            DEFAULT_WEATHER_GUST_WARN_KMH,
            "gust_warn_kmh",
            NON_NEGATIVE,
        )?,
        gust_severe_kmh: threshold(
            file.gust_severe_kmh,
            DEFAULT_WEATHER_GUST_SEVERE_KMH,
            "gust_severe_kmh",
            NON_NEGATIVE,
        )?,
        heavy_rain_mm_per_15_min: threshold(
            file.heavy_rain_mm_per_15_min,
            DEFAULT_WEATHER_HEAVY_RAIN_MM_PER_15MIN,
            "heavy_rain_mm_per_15min",
            NON_NEGATIVE,
        )?,
        fog_visibility_warn_m: threshold(
            file.fog_visibility_warn_m,
            DEFAULT_WEATHER_FOG_VISIBILITY_WARN_M,
            "fog_visibility_warn_m",
            NON_NEGATIVE,
        )?,
        fog_visibility_severe_m: threshold(
            file.fog_visibility_severe_m,
            DEFAULT_WEATHER_FOG_VISIBILITY_SEVERE_M,
            "fog_visibility_severe_m",
            NON_NEGATIVE,
        )?,
        frost_temp_c: threshold(
            file.frost_temp_c,
            DEFAULT_WEATHER_FROST_TEMP_C,
            "frost_temp_c",
            FINITE,
        )?,
        frost_dew_point_delta_c: threshold(
            file.frost_dew_point_delta_c,
            DEFAULT_WEATHER_FROST_DEW_POINT_DELTA_C,
            "frost_dew_point_delta_c",
            NON_NEGATIVE,
        )?,
        frost_warn_precip_window_h: threshold(
            file.frost_warn_precip_window_h,
            DEFAULT_WEATHER_FROST_WARN_PRECIP_WINDOW_H,
            "frost_warn_precip_window_h",
            NON_NEGATIVE,
        )?,
        frost_warn_precip_mm: threshold(
            file.frost_warn_precip_mm,
            DEFAULT_WEATHER_FROST_WARN_PRECIP_MM,
            "frost_warn_precip_mm",
            NON_NEGATIVE,
        )?,
        notify_thunderstorm: file.notify_thunderstorm.unwrap_or(true),
        notify_freezing_precip: file.notify_freezing_precip.unwrap_or(true),
        notify_frost_risk: file.notify_frost_risk.unwrap_or(true),
        notify_heavy_rain: file.notify_heavy_rain.unwrap_or(true),
        notify_strong_gusts: file.notify_strong_gusts.unwrap_or(true),
        notify_snow: file.notify_snow.unwrap_or(true),
        // Fog is noisier, so it is off unless asked for.
        notify_fog: file.notify_fog.unwrap_or(false),
        rtttl: file.rtttl,
    })
}

fn log_format(value: String) -> Result<LogFormat, ConfigError> {
    match value.as_str() {
        "" | "text" => Ok(LogFormat::Text),
        "json" => Ok(LogFormat::Json),
        _ => Err(ConfigError::InvalidLogFormat { value }),
    }
}

/// A flat snapshot of the configuration for the startup debug log, with Go's fields. The MQTT
/// password is left out on purpose.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a flat copy of the configuration's independent flags, for logging"
)]
pub struct ConfigDebugView<'config> {
    /// `mqtt.port`.
    pub mqtt_port: i64,
    /// `mqtt.username`.
    pub mqtt_username: &'config str,
    /// `location.latitude`.
    pub latitude: f64,
    /// `location.longitude`.
    pub longitude: f64,
    /// `timezone`.
    pub timezone: &'config str,
    /// `energy_saving.start`.
    pub energy_saving_start: &'config str,
    /// `energy_saving.end`.
    pub energy_saving_end: &'config str,
    /// `theme.day.calendar_accent`.
    pub theme_day_accent: &'config str,
    /// `theme.day.content`.
    pub theme_day_content: &'config str,
    /// `theme.night.calendar_accent`.
    pub theme_night_accent: &'config str,
    /// `theme.night.content`.
    pub theme_night_content: &'config str,
    /// How many scheduled notifications are valid.
    pub scheduled_notifications_count: usize,
    /// `weather.enabled`.
    pub weather_enabled: bool,
    /// `weather.poll_interval_minutes`.
    pub weather_poll_interval_minutes: i64,
    /// `weather.overlay_horizon_minutes`.
    pub weather_overlay_horizon_minutes: i64,
    /// `weather.notification_horizon_hours`.
    pub weather_notification_horizon_hours: i64,
    /// `weather.notification_repeat_minutes`.
    pub weather_notification_repeat_minutes: i64,
    /// `weather.inactive_after_missing_polls`.
    pub weather_inactive_after_missing_polls: i64,
    /// `weather.notification_text_repeat`.
    pub weather_notification_text_repeat: i64,
    /// `weather.gust_warn_kmh`.
    pub weather_gust_warn_kmh: f64,
    /// `weather.gust_severe_kmh`.
    pub weather_gust_severe_kmh: f64,
    /// `weather.heavy_rain_mm_per_15min`.
    pub weather_heavy_rain_mm_per_15_min: f64,
    /// `weather.fog_visibility_warn_m`.
    pub weather_fog_visibility_warn_m: f64,
    /// `weather.fog_visibility_severe_m`.
    pub weather_fog_visibility_severe_m: f64,
    /// `weather.frost_temp_c`.
    pub weather_frost_temp_c: f64,
    /// `weather.frost_dew_point_delta_c`.
    pub weather_frost_dew_point_delta_c: f64,
    /// `weather.frost_warn_precip_window_h`.
    pub weather_frost_warn_precip_window_h: f64,
    /// `weather.frost_warn_precip_mm`.
    pub weather_frost_warn_precip_mm: f64,
    /// `weather.notify_thunderstorm`.
    pub weather_notify_thunderstorm: bool,
    /// `weather.notify_freezing_precip`.
    pub weather_notify_freezing_precip: bool,
    /// `weather.notify_frost_risk`.
    pub weather_notify_frost_risk: bool,
    /// `weather.notify_heavy_rain`.
    pub weather_notify_heavy_rain: bool,
    /// `weather.notify_strong_gusts`.
    pub weather_notify_strong_gusts: bool,
    /// `weather.notify_snow`.
    pub weather_notify_snow: bool,
    /// `weather.notify_fog`.
    pub weather_notify_fog: bool,
}

impl Config {
    /// The configuration as logged at startup, without the MQTT password.
    #[must_use]
    pub fn debug_view(&self) -> ConfigDebugView<'_> {
        ConfigDebugView {
            mqtt_port: self.mqtt.port,
            mqtt_username: &self.mqtt.username,
            latitude: self.location.latitude,
            longitude: self.location.longitude,
            timezone: &self.timezone,
            energy_saving_start: &self.energy_saving.start,
            energy_saving_end: &self.energy_saving.end,
            theme_day_accent: &self.theme.day.calendar_accent,
            theme_day_content: &self.theme.day.content,
            theme_night_accent: &self.theme.night.calendar_accent,
            theme_night_content: &self.theme.night.content,
            scheduled_notifications_count: self.scheduled_notifications.len(),
            weather_enabled: self.weather.enabled,
            weather_poll_interval_minutes: self.weather.poll_interval_minutes,
            weather_overlay_horizon_minutes: self.weather.overlay_horizon_minutes,
            weather_notification_horizon_hours: self.weather.notification_horizon_hours,
            weather_notification_repeat_minutes: self.weather.notification_repeat_minutes,
            weather_inactive_after_missing_polls: self.weather.inactive_after_missing_polls,
            weather_notification_text_repeat: self.weather.notification_text_repeat,
            weather_gust_warn_kmh: self.weather.gust_warn_kmh,
            weather_gust_severe_kmh: self.weather.gust_severe_kmh,
            weather_heavy_rain_mm_per_15_min: self.weather.heavy_rain_mm_per_15_min,
            weather_fog_visibility_warn_m: self.weather.fog_visibility_warn_m,
            weather_fog_visibility_severe_m: self.weather.fog_visibility_severe_m,
            weather_frost_temp_c: self.weather.frost_temp_c,
            weather_frost_dew_point_delta_c: self.weather.frost_dew_point_delta_c,
            weather_frost_warn_precip_window_h: self.weather.frost_warn_precip_window_h,
            weather_frost_warn_precip_mm: self.weather.frost_warn_precip_mm,
            weather_notify_thunderstorm: self.weather.notify_thunderstorm,
            weather_notify_freezing_precip: self.weather.notify_freezing_precip,
            weather_notify_frost_risk: self.weather.notify_frost_risk,
            weather_notify_heavy_rain: self.weather.notify_heavy_rain,
            weather_notify_strong_gusts: self.weather.notify_strong_gusts,
            weather_notify_snow: self.weather.notify_snow,
            weather_notify_fog: self.weather.notify_fog,
        }
    }
}
