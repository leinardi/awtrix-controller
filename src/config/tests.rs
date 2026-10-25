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

use std::path::Path;

use super::*;

const MINIMAL_VALID: &str = "
mqtt:
  username: \"awtrix\"
  password: \"changeme\"
location:
  latitude: 48.137154
  longitude: 11.576124
";

fn parse_ok(text: &str) -> Config {
    parse(text).expect("valid configuration")
}

fn with_minimal(extra: &str) -> String {
    format!("{MINIMAL_VALID}{extra}")
}

fn sample_weather() -> WeatherConfig {
    WeatherConfig {
        enabled: true,
        poll_interval_minutes: 15,
        overlay_horizon_minutes: 60,
        notification_horizon_hours: 8,
        notification_repeat_minutes: 60,
        inactive_after_missing_polls: 2,
        data_stale_ttl_minutes: 45,
        notification_text_repeat: 3,
        gust_warn_kmh: 45.0,
        gust_severe_kmh: 60.0,
        heavy_rain_mm_per_15_min: 5.0,
        fog_visibility_warn_m: 1000.0,
        fog_visibility_severe_m: 200.0,
        frost_temp_c: 2.0,
        frost_dew_point_delta_c: 2.0,
        frost_warn_precip_window_h: 2.0,
        frost_warn_precip_mm: 0.2,
        notify_thunderstorm: true,
        notify_freezing_precip: true,
        notify_frost_risk: true,
        notify_heavy_rain: true,
        notify_strong_gusts: true,
        notify_snow: true,
        notify_fog: false,
        rtttl: String::new(),
    }
}

fn notification(name: &str) -> ScheduledNotificationConfig {
    ScheduledNotificationConfig {
        name: name.into(),
        message: String::new(),
        repeat: Repeat::Daily,
        date: String::new(),
        day: 0,
        weekdays: Vec::new(),
        time: "00:00".into(),
        enabled: true,
        duration: 60,
        icon: "9597".into(),
        rainbow: false,
        scroll_speed: 50,
        wakeup: true,
        rtttl: String::new(),
    }
}

/// Go: `TestNewConfigDebugView`
#[test]
fn debug_view_omits_the_password() {
    let config = Config {
        mqtt: MqttConfig {
            port: 1883,
            ws_port: None,
            username: "user".into(),
            password: "secret".into(),
        },
        location: LocationConfig {
            latitude: 52.52,
            longitude: 13.405,
        },
        timezone: "Europe/Berlin".into(),
        energy_saving: EnergySavingConfig {
            start: "00:30".into(),
            end: "06:00".into(),
        },
        theme: ThemeConfig {
            day: ThemeColors {
                calendar_accent: "#FF0000".into(),
                content: "#FFFFFF".into(),
            },
            night: ThemeColors {
                calendar_accent: "#FF0000".into(),
                content: "#474747".into(),
            },
        },
        scheduled_notifications: vec![notification("one"), notification("two")],
        weather: sample_weather(),
        log_format: LogFormat::Text,
    };

    let view = config.debug_view();
    assert_eq!(view.mqtt_port, 1883);
    assert_eq!(view.mqtt_username, "user");
    assert!((view.latitude - 52.52).abs() < f64::EPSILON);
    assert!((view.longitude - 13.405).abs() < f64::EPSILON);
    assert_eq!(view.timezone, "Europe/Berlin");
    assert_eq!(view.scheduled_notifications_count, 2);
    assert!(view.weather_enabled);
    assert!(!view.weather_notify_fog);
    assert!(view.weather_notify_thunderstorm);
    assert!(!format!("{view:?}").contains("secret"));
}

/// Go: `TestLoadFileErrors` (file not found)
#[test]
fn missing_file_is_a_read_error() {
    let error = load(Path::new("/nonexistent/path/to/config.yaml")).expect_err("missing file");
    assert!(matches!(error, ConfigError::Read { .. }));
    assert!(error.to_string().contains("read config file"), "{error}");
}

/// Go: `TestLoadFileErrors` (invalid YAML)
#[test]
fn invalid_yaml_is_a_parse_error() {
    let error = parse("mqtt:\n  username: [unclosed\n").expect_err("invalid YAML");
    assert!(error.to_string().contains("parse config"), "{error}");
}

/// A type error names the field path, which yaml.v3 did not, and keeps the line number, which
/// it did.
#[test]
fn type_errors_name_the_field_path_and_line() {
    let text = with_minimal("weather:\n  poll_interval_minutes: often\n");
    let error = parse(&text).expect_err("type error");
    let message = error.to_string();
    assert!(
        message.contains("weather.poll_interval_minutes"),
        "{message}"
    );
    assert!(message.contains("line 9"), "{message}");
}

/// Go: `TestLoadRequiredFields`
#[test]
fn missing_required_fields() {
    let location = "location:\n  latitude: 48.0\n  longitude: 11.0\n";
    let cases = [
        (
            format!("mqtt:\n  password: \"changeme\"\n{location}"),
            "mqtt.username is required",
        ),
        (
            format!("mqtt:\n  username: \"awtrix\"\n{location}"),
            "mqtt.password is required",
        ),
        (
            "mqtt:\n  username: a\n  password: b\nlocation:\n  longitude: 11.576124\n".into(),
            "location.latitude is required",
        ),
        (
            "mqtt:\n  username: a\n  password: b\nlocation:\n  latitude: 48.137154\n".into(),
            "location.longitude is required",
        ),
    ];

    for (text, want) in cases {
        let error = parse(&text).expect_err(want);
        assert_eq!(error.to_string(), want);
    }
    assert!(matches!(
        parse(&format!("mqtt:\n  password: x\n{location}")),
        Err(ConfigError::MqttUsernameRequired)
    ));
}

/// Go: `TestLoadTimezone`
#[test]
fn timezone() {
    assert_eq!(parse_ok(MINIMAL_VALID).timezone, "");
    assert_eq!(
        parse_ok(&with_minimal("timezone: \"Europe/Berlin\"\n")).timezone,
        "Europe/Berlin"
    );

    let error = parse(&with_minimal("timezone: \"Not/AReal/Timezone\"\n")).expect_err("bad zone");
    assert!(error.to_string().contains("invalid timezone"), "{error}");
}

/// Go: `TestLoadThemeColors`
#[test]
fn invalid_hex_color() {
    let error = parse(&with_minimal(
        "theme:\n  day:\n    calendar_accent: \"red\"\n",
    ))
    .expect_err("red");
    assert!(matches!(
        error,
        ConfigError::InvalidHexColor {
            theme: "day",
            field: "calendar_accent",
            ..
        }
    ));
    assert_eq!(
        error.to_string(),
        "theme.day: calendar_accent \"red\": not a valid #RRGGBB color"
    );

    for color in ["#12345", "#1234567", "123456#", "#GGGGGG", "#ÿÿÿ"] {
        let text = with_minimal(&format!("theme:\n  night:\n    content: \"{color}\"\n"));
        assert!(parse(&text).is_err(), "{color} accepted");
    }
    let text = with_minimal("theme:\n  night:\n    content: \"#a1B2c3\"\n");
    assert_eq!(parse_ok(&text).theme.night.content, "#a1B2c3");
}

/// Go: `TestLoadDefaults`
#[test]
fn defaults() {
    let config = parse_ok(MINIMAL_VALID);
    assert_eq!(config.mqtt.port, DEFAULT_MQTT_PORT);
    assert_eq!(config.mqtt.ws_port, None);
    assert_eq!(config.energy_saving.start, DEFAULT_ENERGY_SAVING_START);
    assert_eq!(config.energy_saving.end, DEFAULT_ENERGY_SAVING_END);
    assert_eq!(config.theme.day.calendar_accent, DEFAULT_THEME_DAY_ACCENT);
    assert_eq!(config.theme.day.content, DEFAULT_THEME_DAY_CONTENT);
    assert_eq!(
        config.theme.night.calendar_accent,
        DEFAULT_THEME_NIGHT_ACCENT
    );
    assert_eq!(config.theme.night.content, DEFAULT_THEME_NIGHT_CONTENT);
    assert!(config.scheduled_notifications.is_empty());
    assert_eq!(config.log_format, LogFormat::Text);
    assert!(!config.weather.enabled);
}

/// The fields Q2 leaves alone keep Go's "0 means unset": an explicit 0 takes the default.
#[test]
fn zero_still_means_default_outside_the_thresholds() {
    let text = concat!(
        "mqtt:\n  username: a\n  password: b\n  port: 0\n",
        "location:\n  latitude: 1\n  longitude: 2\n",
        "weather:\n  poll_interval_minutes: 0\n",
        "scheduled_notifications:\n",
        "  - name: x\n    repeat: daily\n    duration: 0\n    scroll_speed: 0\n",
    );
    let config = parse_ok(text);
    assert_eq!(config.mqtt.port, DEFAULT_MQTT_PORT);
    assert_eq!(
        config.weather.poll_interval_minutes,
        DEFAULT_WEATHER_POLL_INTERVAL_MINUTES
    );
    assert_eq!(
        config.scheduled_notifications[0].duration,
        DEFAULT_SCHEDULED_NOTIFICATION_DURATION
    );
    assert_eq!(
        config.scheduled_notifications[0].scroll_speed,
        DEFAULT_SCHEDULED_NOTIFICATION_SCROLL_SPEED
    );
}

/// Go: `TestLoadScheduledNotificationEnabled`
#[test]
fn scheduled_notification_enabled() {
    let entry = "scheduled_notifications:\n  - name: \"New Year\"\n    repeat: yearly\n    date: \"01-01\"\n";
    let config = parse_ok(&with_minimal(entry));
    assert_eq!(config.scheduled_notifications.len(), 1);
    assert!(config.scheduled_notifications[0].enabled);

    let config = parse_ok(&with_minimal(&format!("{entry}    enabled: false\n")));
    assert_eq!(config.scheduled_notifications.len(), 1);
    assert!(!config.scheduled_notifications[0].enabled);
}

/// Go: `TestLoadScheduledNotificationsWeekly`
#[test]
fn scheduled_notifications_weekly() {
    let empty =
        "scheduled_notifications:\n  - name: \"Standup\"\n    repeat: weekly\n    weekdays: []\n";
    assert!(
        parse_ok(&with_minimal(empty))
            .scheduled_notifications
            .is_empty()
    );

    let invalid = "scheduled_notifications:\n  - name: \"Bad\"\n    repeat: weekly\n    weekdays:\n      - monday\n      - funday\n";
    assert!(
        parse_ok(&with_minimal(invalid))
            .scheduled_notifications
            .is_empty()
    );

    let valid = concat!(
        "scheduled_notifications:\n  - name: \"Standup\"\n    repeat: weekly\n",
        "    weekdays:\n      - monday\n      - tuesday\n      - wednesday\n",
        "      - thursday\n      - friday\n    time: \"09:45\"\n",
    );
    let config = parse_ok(&with_minimal(valid));
    assert_eq!(config.scheduled_notifications.len(), 1);
    let entry = &config.scheduled_notifications[0];
    assert_eq!(entry.repeat, Repeat::Weekly);
    assert_eq!(
        entry.weekdays,
        ["monday", "tuesday", "wednesday", "thursday", "friday"]
    );
    assert_eq!(entry.time, "09:45");
}

/// Go: `TestWeatherConfigDefaultsApplied`
#[test]
fn weather_defaults_applied() {
    let weather = parse_ok(MINIMAL_VALID).weather;
    assert_eq!(
        weather.poll_interval_minutes,
        DEFAULT_WEATHER_POLL_INTERVAL_MINUTES
    );
    assert_eq!(
        weather.overlay_horizon_minutes,
        DEFAULT_WEATHER_OVERLAY_HORIZON_MINUTES
    );
    assert_eq!(
        weather.notification_horizon_hours,
        DEFAULT_WEATHER_NOTIFICATION_HORIZON_HOURS
    );
    assert_eq!(
        weather.notification_repeat_minutes,
        DEFAULT_WEATHER_NOTIFICATION_REPEAT_MINUTES
    );
    assert_eq!(
        weather.inactive_after_missing_polls,
        DEFAULT_WEATHER_INACTIVE_AFTER_MISSING_POLLS
    );
    assert_eq!(
        weather.data_stale_ttl_minutes,
        DEFAULT_WEATHER_DATA_STALE_TTL_MINUTES
    );
    assert_eq!(
        weather.notification_text_repeat,
        DEFAULT_WEATHER_NOTIFICATION_TEXT_REPEAT
    );
    let thresholds = [
        (weather.gust_warn_kmh, DEFAULT_WEATHER_GUST_WARN_KMH),
        (weather.gust_severe_kmh, DEFAULT_WEATHER_GUST_SEVERE_KMH),
        (
            weather.heavy_rain_mm_per_15_min,
            DEFAULT_WEATHER_HEAVY_RAIN_MM_PER_15MIN,
        ),
        (
            weather.fog_visibility_warn_m,
            DEFAULT_WEATHER_FOG_VISIBILITY_WARN_M,
        ),
        (
            weather.fog_visibility_severe_m,
            DEFAULT_WEATHER_FOG_VISIBILITY_SEVERE_M,
        ),
        (weather.frost_temp_c, DEFAULT_WEATHER_FROST_TEMP_C),
        (
            weather.frost_dew_point_delta_c,
            DEFAULT_WEATHER_FROST_DEW_POINT_DELTA_C,
        ),
        (
            weather.frost_warn_precip_window_h,
            DEFAULT_WEATHER_FROST_WARN_PRECIP_WINDOW_H,
        ),
        (
            weather.frost_warn_precip_mm,
            DEFAULT_WEATHER_FROST_WARN_PRECIP_MM,
        ),
    ];
    for (value, default) in thresholds {
        assert!(
            (value - default).abs() < f64::EPSILON,
            "{value} != {default}"
        );
    }
    assert!(weather.rtttl.is_empty());
}

/// Go: `TestWeatherConfigUserValuesPreserved`
#[test]
fn weather_user_values_preserved() {
    let text = with_minimal(
        "weather:\n  enabled: true\n  poll_interval_minutes: 30\n  gust_warn_kmh: 55.0\n",
    );
    let weather = parse_ok(&text).weather;
    assert!(weather.enabled);
    assert_eq!(weather.poll_interval_minutes, 30);
    assert!((weather.gust_warn_kmh - 55.0).abs() < f64::EPSILON);
}

/// Q2: an explicit 0 threshold is honoured, where Go replaced it by the default.
#[test]
fn explicit_zero_threshold_is_honoured() {
    let text = with_minimal(concat!(
        "weather:\n",
        "  frost_temp_c: 0\n",
        "  gust_warn_kmh: 0\n",
        "  frost_warn_precip_mm: 0.0\n",
        "  frost_dew_point_delta_c: ~\n",
    ));
    let weather = parse_ok(&text).weather;
    assert!(weather.frost_temp_c.abs() < f64::EPSILON);
    assert!(weather.gust_warn_kmh.abs() < f64::EPSILON);
    assert!(weather.frost_warn_precip_mm.abs() < f64::EPSILON);
    // null still means "not given".
    assert!(
        (weather.frost_dew_point_delta_c - DEFAULT_WEATHER_FROST_DEW_POINT_DELTA_C).abs()
            < f64::EPSILON
    );
}

/// Q2: validation checks each threshold's range instead.
#[test]
fn threshold_ranges_are_validated() {
    let rejected = [
        ("gust_warn_kmh", "-1"),
        ("gust_severe_kmh", "-0.5"),
        ("heavy_rain_mm_per_15min", "-5"),
        ("fog_visibility_warn_m", "-1"),
        ("fog_visibility_severe_m", ".inf"),
        ("frost_temp_c", ".nan"),
        ("frost_dew_point_delta_c", "-1"),
        ("frost_warn_precip_window_h", "-2"),
        ("frost_warn_precip_mm", "-0.1"),
    ];
    for (field, value) in rejected {
        let text = with_minimal(&format!("weather:\n  {field}: {value}\n"));
        let error = parse(&text).expect_err(field);
        assert!(
            matches!(error, ConfigError::OutOfRange { field: reported, .. } if reported == field),
            "{field}: {error}"
        );
    }

    let text = with_minimal("weather:\n  frost_temp_c: -3.5\n");
    assert!((parse_ok(&text).weather.frost_temp_c + 3.5).abs() < f64::EPSILON);
}

/// Go: `TestWeatherConfigFogDefaultsFalse`
#[test]
fn weather_fog_defaults_false() {
    assert!(!parse_ok(MINIMAL_VALID).weather.notify_fog);
}

/// Go: `TestWeatherConfigNotifyDefaultsTrue`
#[test]
fn weather_notify_defaults_true() {
    let weather = parse_ok(MINIMAL_VALID).weather;
    assert!(weather.notify_thunderstorm);
    assert!(weather.notify_freezing_precip);
    assert!(weather.notify_frost_risk);
    assert!(weather.notify_heavy_rain);
    assert!(weather.notify_strong_gusts);
    assert!(weather.notify_snow);
}

/// Go: `TestLoadScheduledNotifications` (bad repeat skipped, bad yearly date skipped, defaults)
#[test]
fn scheduled_notifications_filtering_and_defaults() {
    let bad_repeat = concat!(
        "scheduled_notifications:\n  - name: \"Bad\"\n    repeat: \"invalid\"\n",
        "  - name: \"Alice\"\n    repeat: yearly\n    date: \"04-16\"\n",
    );
    let config = parse_ok(&with_minimal(bad_repeat));
    assert_eq!(config.scheduled_notifications.len(), 1);
    assert_eq!(config.scheduled_notifications[0].name, "Alice");

    let bad_date = concat!(
        "scheduled_notifications:\n  - name: \"BadDate\"\n    repeat: yearly\n    date: \"not-a-date\"\n",
        "  - name: \"Alice\"\n    repeat: yearly\n    date: \"04-16\"\n",
    );
    let config = parse_ok(&with_minimal(bad_date));
    assert_eq!(config.scheduled_notifications.len(), 1);
    assert_eq!(config.scheduled_notifications[0].name, "Alice");

    let defaults =
        "scheduled_notifications:\n  - name: \"Alice\"\n    repeat: yearly\n    date: \"04-16\"\n";
    let config = parse_ok(&with_minimal(defaults));
    let entry = &config.scheduled_notifications[0];
    assert_eq!(entry.repeat, Repeat::Yearly);
    assert_eq!(entry.duration, DEFAULT_SCHEDULED_NOTIFICATION_DURATION);
    assert_eq!(entry.icon, DEFAULT_SCHEDULED_NOTIFICATION_ICON);
    assert!(!entry.rainbow);
    assert!(entry.wakeup);
    assert_eq!(
        entry.scroll_speed,
        DEFAULT_SCHEDULED_NOTIFICATION_SCROLL_SPEED
    );
    assert_eq!(entry.time, DEFAULT_SCHEDULED_NOTIFICATION_TIME);
}

/// The monthly day range and the time format, which the Go tests did not cover: Go's checks are
/// mirrored (day 1–31, HH:MM with a one-digit hour allowed).
#[test]
fn scheduled_notifications_monthly_day_and_time() {
    let entry = |extra: &str| {
        with_minimal(&format!(
            "scheduled_notifications:\n  - name: x\n    repeat: monthly\n{extra}"
        ))
    };
    assert!(
        parse_ok(&entry("    day: 0\n"))
            .scheduled_notifications
            .is_empty()
    );
    assert!(
        parse_ok(&entry("    day: 32\n"))
            .scheduled_notifications
            .is_empty()
    );
    assert_eq!(
        parse_ok(&entry("    day: 31\n")).scheduled_notifications[0].day,
        31
    );
    assert_eq!(
        parse_ok(&entry("    day: 1\n    time: \"7:00\"\n")).scheduled_notifications[0].time,
        "7:00"
    );
    assert!(
        parse_ok(&entry("    day: 1\n    time: \"7:5\"\n"))
            .scheduled_notifications
            .is_empty()
    );
}

/// `log_format` (Q8): text by default, json on request, anything else refused.
#[test]
fn log_format_values() {
    assert_eq!(parse_ok(MINIMAL_VALID).log_format, LogFormat::Text);
    assert_eq!(
        parse_ok(&with_minimal("log_format: text\n")).log_format,
        LogFormat::Text
    );
    assert_eq!(
        parse_ok(&with_minimal("log_format: json\n")).log_format,
        LogFormat::Json
    );
    let error = parse(&with_minimal("log_format: xml\n")).expect_err("xml");
    assert!(matches!(error, ConfigError::InvalidLogFormat { .. }));
}

/// The yaml.v3 leniencies a hand-written file can rely on, captured from the Go baseline with a
/// throwaway program: unknown keys are ignored, an unquoted number is a string where a string is
/// expected, YAML 1.1 booleans are booleans, a float in an integer field is truncated, and `null`
/// leaves the zero value.
#[test]
fn yaml_reads_like_yaml_v3() {
    let text = concat!(
        "unknown_top_level: 1\n",
        "mqtt:\n  username: 1234\n  password: true\n  port: 1_883\n  bogus: x\n",
        "location:\n  latitude: 48\n  longitude: 11.5\n",
        "theme: ~\n",
        "energy_saving:\n  start: ~\n",
        "scheduled_notifications:\n",
        "  - name: x\n    repeat: daily\n    icon: 14004\n    duration: 60.9\n",
        "    rainbow: yes\n    wakeup: off\n    enabled: ~\n",
        "  - ~\n",
        "weather:\n  enabled: on\n  notify_fog: Y\n",
    );
    let config = parse_ok(text);
    assert_eq!(config.mqtt.username, "1234");
    assert_eq!(config.mqtt.password, "true");
    assert_eq!(config.mqtt.port, 1883);
    assert!((config.location.latitude - 48.0).abs() < f64::EPSILON);
    assert_eq!(config.theme.day.content, DEFAULT_THEME_DAY_CONTENT);
    assert_eq!(config.energy_saving.start, DEFAULT_ENERGY_SAVING_START);
    // The null entry has no repeat, so it is skipped like any invalid entry.
    assert_eq!(config.scheduled_notifications.len(), 1);
    let entry = &config.scheduled_notifications[0];
    assert_eq!(entry.icon, "14004");
    assert_eq!(entry.duration, 60);
    assert!(entry.rainbow);
    assert!(!entry.wakeup);
    assert!(entry.enabled);
    assert!(config.weather.enabled);
    assert!(config.weather.notify_fog);

    for refused in [
        "weather:\n  enabled: maybe\n",
        "weather:\n  poll_interval_minutes: true\n",
        "mqtt:\n  username: [a]\n",
        "weather:\n  poll_interval_minutes: 1\n  poll_interval_minutes: 2\n",
    ] {
        assert!(
            parse(&with_minimal(refused)).is_err(),
            "{refused:?} accepted"
        );
    }
}

/// The shipped sample configuration parses, and says what it claims.
#[test]
fn sample_configuration_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("deployments/config.sample.yaml");
    let config = load(&path).expect("the sample configuration is valid");
    assert_eq!(config.mqtt.port, 1883);
    assert_eq!(config.timezone, "Europe/Berlin");
    assert_eq!(config.scheduled_notifications.len(), 2);
    assert!(config.weather.enabled);
    assert_eq!(config.log_format, LogFormat::Text);
}

/// The format is read on its own, before the full load, and anything unreadable is `None`.
#[test]
fn peek_log_format_reads_only_the_format() {
    let path = std::env::temp_dir().join(format!(
        "awtrix-controller-peek-{}.yaml",
        std::process::id()
    ));
    let cases = [
        ("log_format: json\nmqtt: {}\n", Some(LogFormat::Json)),
        ("log_format: text\n", Some(LogFormat::Text)),
        ("mqtt:\n  port: 1\n", Some(LogFormat::Text)),
        ("log_format: xml\n", None),
        ("log_format: [json\n", None),
    ];
    for (text, expected) in cases {
        std::fs::write(&path, text).expect("write");
        assert_eq!(peek_log_format(&path), expected, "{text:?}");
    }
    std::fs::remove_file(&path).expect("remove");
    assert_eq!(peek_log_format(&path), None);
}
