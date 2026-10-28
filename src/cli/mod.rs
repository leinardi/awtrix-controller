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

//! The command line, parsed with clap. Go's `flag` package also accepted the single-dash long form
//! (`-config`); clap takes `--config` and the short forms only (Q9). Everything else follows Go's
//! flags: a repeated flag keeps its last value, a value may start with `-`, a boolean flag takes no
//! separate value but accepts `=false`, and integers are read as Go literals (`0x10`, `1_000`).

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser};

use crate::config::LogFormat;
use crate::model::{
    Effect, Notification, OverlayEffect, PushIconBehavior, TextCaseMode, TextContent,
};

/// The command-line flags.
#[derive(Debug, Default, Parser)]
#[command(name = "awtrix-controller", args_override_self = true)]
pub struct Cli {
    /// The YAML config file.
    #[arg(
        short,
        long,
        value_name = "PATH",
        allow_hyphen_values = true,
        help = "Path to YAML config file [default: /etc/awtrix-controller/config.yaml, or \
                AWTRIX_CONFIG when set]"
    )]
    pub config: Option<PathBuf>,

    /// The log level.
    #[arg(
        short,
        long,
        value_name = "LEVEL",
        allow_hyphen_values = true,
        help = "Log verbosity (debug|info|warn|error) [default: info, or AWTRIX_LOG_LEVEL when set]"
    )]
    pub log_level: Option<String>,

    /// The log format.
    #[arg(
        long,
        value_name = "FORMAT",
        value_parser = parse_log_format,
        help = "Log format (text|json) [default: the config file's log_format, or text]"
    )]
    pub log_format: Option<LogFormat>,

    /// Print version and exit
    #[arg(short, long, value_name = "BOOL", num_args = 0..=1, require_equals = true,
          default_missing_value = "true", value_parser = parse_go_bool)]
    pub version: Option<bool>,

    /// Simulate all forecast points with this WMO code (0 = disabled; debug only)
    #[arg(long, value_name = "CODE", allow_hyphen_values = true, value_parser = parse_go_int)]
    pub weather_wmo: Option<i64>,

    /// The test notification sent to every device once it is ready (debug only).
    #[command(flatten)]
    pub test_notification: TestNotificationArgs,
}

/// The `--test-notification-*` flags. Setting any of them turns the test-notification mode on;
/// a field that is `None` was not given and stays at the payload's default.
#[derive(Clone, Debug, Default, Args)]
pub struct TestNotificationArgs {
    /// Test notification: text message (debug only)
    #[arg(
        long = "test-notification-text",
        value_name = "TEXT",
        allow_hyphen_values = true
    )]
    pub text: Option<String>,

    /// Test notification: icon ID.
    #[arg(
        long = "test-notification-icon",
        value_name = "ID",
        allow_hyphen_values = true,
        help = "Test notification: LaMetric icon ID (debug only)"
    )]
    pub icon: Option<String>,

    /// Test notification: display duration in seconds (debug only)
    #[arg(long = "test-notification-duration", value_name = "SECONDS",
          allow_hyphen_values = true, value_parser = parse_go_int)]
    pub duration: Option<i64>,

    /// Test notification: enable rainbow text colors (debug only)
    #[arg(long = "test-notification-rainbow", value_name = "BOOL", num_args = 0..=1,
          require_equals = true, default_missing_value = "true", value_parser = parse_go_bool)]
    pub rainbow: Option<bool>,

    /// Test notification: scroll speed in pixels per frame (debug only)
    #[arg(long = "test-notification-scroll-speed", value_name = "SPEED",
          allow_hyphen_values = true, value_parser = parse_go_int)]
    pub scroll_speed: Option<i64>,

    /// Test notification: disable text scrolling (debug only)
    #[arg(long = "test-notification-no-scroll", value_name = "BOOL", num_args = 0..=1,
          require_equals = true, default_missing_value = "true", value_parser = parse_go_bool)]
    pub no_scroll: Option<bool>,

    /// Test notification: text color as hex string e.g. #FF0000 (debug only)
    #[arg(
        long = "test-notification-color",
        value_name = "COLOR",
        allow_hyphen_values = true
    )]
    pub color: Option<String>,

    /// Test notification: background fill color as hex string (debug only)
    #[arg(
        long = "test-notification-background",
        value_name = "COLOR",
        allow_hyphen_values = true
    )]
    pub background: Option<String>,

    /// Test notification: weather overlay effect (clear|snow|rain|drizzle|storm|thunder|frost)
    /// (debug only)
    #[arg(
        long = "test-notification-overlay",
        value_name = "OVERLAY",
        allow_hyphen_values = true
    )]
    pub overlay: Option<String>,

    /// Test notification: background animation effect name e.g. Plasma (debug only)
    #[arg(
        long = "test-notification-effect",
        value_name = "EFFECT",
        allow_hyphen_values = true
    )]
    pub effect: Option<String>,

    /// Test notification: blink rate in milliseconds (debug only)
    #[arg(long = "test-notification-blink-text", value_name = "MILLISECONDS",
          allow_hyphen_values = true, value_parser = parse_go_int)]
    pub blink_text: Option<i64>,

    /// Test notification: fade rate in milliseconds (debug only)
    #[arg(long = "test-notification-fade-text", value_name = "MILLISECONDS",
          allow_hyphen_values = true, value_parser = parse_go_int)]
    pub fade_text: Option<i64>,

    /// Test notification: text case mode (0=global 1=upper 2=as-is) (debug only)
    #[arg(long = "test-notification-text-case", value_name = "MODE",
          allow_hyphen_values = true, value_parser = parse_text_case)]
    pub text_case: Option<TextCaseMode>,

    /// Test notification: display text at the top of the matrix (debug only)
    #[arg(long = "test-notification-top-text", value_name = "BOOL", num_args = 0..=1,
          require_equals = true, default_missing_value = "true", value_parser = parse_go_bool)]
    pub top_text: Option<bool>,

    /// Test notification: horizontal text offset in pixels (debug only)
    #[arg(long = "test-notification-text-offset", value_name = "PIXELS",
          allow_hyphen_values = true, value_parser = parse_go_int)]
    pub text_offset: Option<i64>,

    /// Test notification: icon behavior (0=static 1=scroll 2=fixed) (debug only)
    #[arg(long = "test-notification-push-icon", value_name = "MODE",
          allow_hyphen_values = true, value_parser = parse_push_icon)]
    pub push_icon: Option<PushIconBehavior>,

    /// Test notification: center text horizontally (true|false; omit to use device default)
    /// (debug only)
    #[arg(
        long = "test-notification-center",
        value_name = "BOOL",
        allow_hyphen_values = true
    )]
    pub center: Option<String>,

    /// Test notification: keep notification visible until dismissed (debug only)
    #[arg(long = "test-notification-hold", value_name = "BOOL", num_args = 0..=1,
          require_equals = true, default_missing_value = "true", value_parser = parse_go_bool)]
    pub hold: Option<bool>,

    /// Test notification: sound file name on the device filesystem (debug only)
    #[arg(
        long = "test-notification-sound",
        value_name = "NAME",
        allow_hyphen_values = true
    )]
    pub sound: Option<String>,

    /// Test notification: RTTTL melody string to play (debug only)
    #[arg(
        long = "test-notification-rtttl",
        value_name = "MELODY",
        allow_hyphen_values = true
    )]
    pub rtttl: Option<String>,

    /// Test notification: loop sound while notification is displayed (debug only)
    #[arg(long = "test-notification-loop-sound", value_name = "BOOL", num_args = 0..=1,
          require_equals = true, default_missing_value = "true", value_parser = parse_go_bool)]
    pub loop_sound: Option<bool>,

    /// Test notification: queue behind current notification (true|false; omit to use device
    /// default) (debug only)
    #[arg(
        long = "test-notification-stack",
        value_name = "BOOL",
        allow_hyphen_values = true
    )]
    pub stack: Option<String>,

    /// Test notification: wake display from sleep before showing (debug only)
    #[arg(long = "test-notification-wakeup", value_name = "BOOL", num_args = 0..=1,
          require_equals = true, default_missing_value = "true", value_parser = parse_go_bool)]
    pub wakeup: Option<bool>,

    /// Test notification: number of times to scroll content (-1=infinite; omit to use device
    /// default) (debug only)
    #[arg(
        long = "test-notification-repeat",
        value_name = "COUNT",
        allow_hyphen_values = true
    )]
    pub repeat: Option<String>,
}

impl TestNotificationArgs {
    /// The notification the flags describe, or `None` when no `--test-notification-*` flag was
    /// given. A flag given with an empty value still turns the mode on.
    #[must_use]
    pub fn notification(&self) -> Option<Notification> {
        // Destructured so that a new flag cannot be left out of either list below.
        let Self {
            text,
            icon,
            duration,
            rainbow,
            scroll_speed,
            no_scroll,
            color,
            background,
            overlay,
            effect,
            blink_text,
            fade_text,
            text_case,
            top_text,
            text_offset,
            push_icon,
            center,
            hold,
            sound,
            rtttl,
            loop_sound,
            stack,
            wakeup,
            repeat,
        } = self.clone();

        let any_given = [
            text.is_some(),
            icon.is_some(),
            duration.is_some(),
            rainbow.is_some(),
            scroll_speed.is_some(),
            no_scroll.is_some(),
            color.is_some(),
            background.is_some(),
            overlay.is_some(),
            effect.is_some(),
            blink_text.is_some(),
            fade_text.is_some(),
            text_case.is_some(),
            top_text.is_some(),
            text_offset.is_some(),
            push_icon.is_some(),
            center.is_some(),
            hold.is_some(),
            sound.is_some(),
            rtttl.is_some(),
            loop_sound.is_some(),
            stack.is_some(),
            wakeup.is_some(),
            repeat.is_some(),
        ]
        .contains(&true);
        if !any_given {
            return None;
        }

        let mut notification = Notification {
            hold: hold.unwrap_or_default(),
            sound: sound.unwrap_or_default(),
            rtttl: rtttl.unwrap_or_default(),
            loop_sound: loop_sound.unwrap_or_default(),
            stack: stack.as_deref().and_then(parse_bool_option),
            wakeup: wakeup.unwrap_or_default(),
            ..Notification::default()
        };
        let content = &mut notification.content;
        content.text = text.map(TextContent::plain);
        content.icon = icon.unwrap_or_default();
        content.duration = duration.unwrap_or_default();
        content.rainbow = rainbow.unwrap_or_default();
        content.scroll_speed = scroll_speed.unwrap_or_default();
        content.no_scroll = no_scroll.unwrap_or_default();
        content.color = color.unwrap_or_default();
        content.background = background.unwrap_or_default();
        content.overlay = overlay.map(|name| OverlayEffect::from_name(&name));
        content.effect = effect.map(|name| Effect::from_name(&name));
        content.blink_text = blink_text.unwrap_or_default();
        content.fade_text = fade_text.unwrap_or_default();
        content.text_case = text_case.unwrap_or_default();
        content.top_text = top_text.unwrap_or_default();
        content.text_offset = text_offset.unwrap_or_default();
        content.push_icon = push_icon.unwrap_or_default();
        content.center = center.as_deref().and_then(parse_bool_option);
        content.repeat = repeat.as_deref().and_then(parse_int_option);
        Some(notification)
    }
}

/// The first argument in Go's single-dash long form (`-config`, `-log-level=debug`), with the
/// flag it meant. clap would read it as a cluster of short flags (`-c onfig`), so a Go-era
/// command line could otherwise start with a mangled path or a silently ignored level.
#[must_use]
pub fn single_dash_long_form(args: &[OsString]) -> Option<(String, String)> {
    let mut command = Cli::command();
    command.build();
    let long_names: Vec<&str> = command
        .get_arguments()
        .filter_map(clap::Arg::get_long)
        .chain(["help"])
        .collect();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        let Some(arg) = arg.to_str() else { continue };
        if let Some(long) = arg.strip_prefix("--") {
            if !long.contains('=')
                && command
                    .get_arguments()
                    .any(|option| option.get_long() == Some(long) && takes_separate_value(option))
            {
                args.next();
            }
            continue;
        }
        let Some(short) = arg.strip_prefix('-') else {
            continue;
        };
        let name = short.split_once('=').map_or(short, |(name, _)| name);
        if long_names.contains(&name) {
            return Some((arg.to_owned(), format!("--{name}")));
        }
        // A value-taking short option ends its cluster: its value is the remaining suffix or
        // the next token. Neither is another option, even when it starts with a dash.
        for (index, character) in short.char_indices() {
            let option = command
                .get_arguments()
                .find(|option| option.get_short() == Some(character));
            let Some(option) = option else { break };
            if takes_separate_value(option) {
                if index + character.len_utf8() == short.len() {
                    args.next();
                }
                break;
            }
        }
    }
    None
}

fn takes_separate_value(option: &clap::Arg) -> bool {
    !option.is_require_equals_set()
        && option
            .get_num_args()
            .is_some_and(|arity| arity.min_values() > 0)
}

/// `"true"` and `"false"`, exactly; anything else, the empty string included, leaves the field
/// to the device's default, as Go's `parseBoolPtr` did.
#[must_use]
pub fn parse_bool_option(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// A decimal integer, as Go's `strconv.Atoi` reads it; anything else, the empty string included,
/// leaves the field to the device's default, as Go's `parseIntPtr` did.
#[must_use]
pub fn parse_int_option(value: &str) -> Option<i64> {
    value.parse().ok()
}

/// A boolean flag value as Go's `strconv.ParseBool` reads it.
///
/// # Errors
///
/// Any other spelling.
pub fn parse_go_bool(value: &str) -> Result<bool, String> {
    match value {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Ok(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Ok(false),
        _ => Err(format!("invalid boolean value {value:?}")),
    }
}

/// An integer flag value as Go's `flag.Int` reads it (`strconv.ParseInt` with base 0): an
/// optional sign, then decimal, `0x` hexadecimal, `0o` or leading-`0` octal, or `0b` binary, with
/// `_` allowed between digits and after a base prefix.
///
/// # Errors
///
/// Anything that is not such a literal, or out of the `i64` range.
pub fn parse_go_int(value: &str) -> Result<i64, String> {
    let invalid = || format!("invalid integer value {value:?}");
    let (negative, unsigned) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let lower = unsigned.get(..2).map(str::to_ascii_lowercase);
    let (radix, digits, prefixed) = match lower.as_deref() {
        Some("0x") => (16, &unsigned[2..], true),
        Some("0o") => (8, &unsigned[2..], true),
        Some("0b") => (2, &unsigned[2..], true),
        _ if unsigned.len() > 1 && unsigned.starts_with('0') => (8, &unsigned[1..], true),
        _ => (10, unsigned, false),
    };
    if !underscores_ok(digits, prefixed) {
        return Err(invalid());
    }
    let digits: String = digits
        .chars()
        .filter(|&character| character != '_')
        .collect();
    if digits.is_empty() || digits.starts_with(['+', '-']) {
        return Err(invalid());
    }
    let magnitude = u64::from_str_radix(&digits, radix).map_err(|_| invalid())?;
    if negative {
        0_i64.checked_sub_unsigned(magnitude).ok_or_else(invalid)
    } else {
        i64::try_from(magnitude).map_err(|_| invalid())
    }
}

/// Go's rule for `_` in a base-0 literal: only between digits, or right after a base prefix.
fn underscores_ok(digits: &str, prefixed: bool) -> bool {
    let bytes = digits.as_bytes();
    bytes.iter().enumerate().all(|(index, &byte)| {
        if byte != b'_' {
            return true;
        }
        let before_ok = if index == 0 {
            prefixed
        } else {
            bytes[index - 1] != b'_'
        };
        let after_ok = bytes.get(index + 1).is_some_and(|&next| next != b'_');
        before_ok && after_ok
    })
}

fn parse_log_format(value: &str) -> Result<LogFormat, String> {
    match value {
        "text" => Ok(LogFormat::Text),
        "json" => Ok(LogFormat::Json),
        _ => Err(format!("{value:?}: must be \"text\" or \"json\"")),
    }
}

fn parse_text_case(value: &str) -> Result<TextCaseMode, String> {
    match parse_go_int(value)? {
        0 => Ok(TextCaseMode::GlobalSetting),
        1 => Ok(TextCaseMode::Uppercase),
        2 => Ok(TextCaseMode::AsIs),
        other => Err(format!("text case {other}: must be 0, 1 or 2")),
    }
}

fn parse_push_icon(value: &str) -> Result<PushIconBehavior, String> {
    match parse_go_int(value)? {
        0 => Ok(PushIconBehavior::Static),
        1 => Ok(PushIconBehavior::Scroll),
        2 => Ok(PushIconBehavior::Fixed),
        other => Err(format!("push icon {other}: must be 0, 1 or 2")),
    }
}

#[cfg(test)]
mod tests;
