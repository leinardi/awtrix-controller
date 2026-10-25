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

//! The application's logging: one global `tracing` subscriber writing to standard output, as
//! tracing-subscriber's text or JSON (Q8), at a level resolved as the Go implementation resolved
//! it. Crates that log through the `log` facade are bridged in.

use std::io;

use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::MakeWriter;

use crate::config::LogFormat;

/// The level used when neither the flag nor the environment gives a usable one.
const DEFAULT_LEVEL: &str = "info";

/// Installs the global subscriber, writing to standard output, and bridges the `log` facade.
///
/// # Errors
///
/// A global subscriber or `log` logger is already installed.
pub fn init(level: LevelFilter, format: LogFormat) -> Result<(), InitError> {
    tracing_log::LogTracer::init().map_err(|_| InitError)?;
    tracing::subscriber::set_global_default(subscriber(level, format, io::stdout))
        .map_err(|_| InitError)
}

/// A global logger was already installed.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("a global logger is already installed")]
pub struct InitError;

/// The subscriber [`init`] installs, writing to `writer`; tests install it in a scope.
pub fn subscriber<W>(
    level: LevelFilter,
    format: LogFormat,
    writer: W,
) -> Box<dyn Subscriber + Send + Sync>
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    let builder = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_ansi(false)
        .with_writer(writer);

    match format {
        LogFormat::Text => Box::new(builder.finish()),
        LogFormat::Json => Box::new(builder.json().flatten_event(true).finish()),
    }
}

/// The level to log at, with Go's priority: an explicitly given `--log-level` flag wins, then a
/// non-empty `AWTRIX_LOG_LEVEL`, then the default, `info`.
#[must_use]
pub fn effective_level(flag: Option<&str>, environment: Option<&str>) -> LevelFilter {
    match (flag, environment) {
        (Some(flag), _) => resolve_level(flag),
        (None, Some(environment)) if !environment.is_empty() => resolve_level(environment),
        (None, _) => resolve_level(DEFAULT_LEVEL),
    }
}

/// The log format, with the same priority: an explicit `--log-format` flag wins over the
/// configuration's `log_format`.
#[must_use]
pub fn effective_format(flag: Option<LogFormat>, config: LogFormat) -> LogFormat {
    flag.unwrap_or(config)
}

/// Parses a level the way Go's `slog.Level.UnmarshalText` did: `debug`, `info`, `warn` or
/// `error` in any case, optionally offset by `+N` or `-N` (slog's levels are -4, 0, 4 and 8
/// apart), and `info` for anything it cannot read.
#[must_use]
pub fn resolve_level(text: &str) -> LevelFilter {
    slog_level(text).map_or(LevelFilter::INFO, filter_for_slog_level)
}

/// slog's numeric level for a level name with an optional offset.
fn slog_level(text: &str) -> Option<i64> {
    let (name, offset) = match text.find(['+', '-']) {
        Some(index) => (&text[..index], Some(&text[index..])),
        None => (text, None),
    };

    let base = match name.to_ascii_uppercase().as_str() {
        "DEBUG" => -4,
        "INFO" => 0,
        "WARN" => 4,
        "ERROR" => 8,
        _ => return None,
    };

    match offset {
        None => Some(base),
        Some(offset) => {
            let digits = &offset[1..];
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let value: i64 = offset.parse().ok()?;
            base.checked_add(value)
        }
    }
}

/// The tracing filter that lets through the records slog would have: a record at slog level `r`
/// is written when `r >= threshold`.
fn filter_for_slog_level(threshold: i64) -> LevelFilter {
    match threshold {
        ..=-4 => LevelFilter::DEBUG,
        -3..=0 => LevelFilter::INFO,
        1..=4 => LevelFilter::WARN,
        5..=8 => LevelFilter::ERROR,
        _ => LevelFilter::OFF,
    }
}

#[cfg(test)]
mod tests {
    use tracing::{debug, info};

    use super::*;
    use crate::test_support::Captured;

    fn capture(level: LevelFilter, format: LogFormat, emit: impl FnOnce()) -> String {
        let captured = Captured::default();
        tracing::subscriber::with_default(subscriber(level, format, captured.clone()), emit);
        captured.text()
    }

    /// Go: `TestLBeforeInitReturnsNonNil`. There is no lazily created logger to return: logging
    /// before a subscriber is installed is a no-op, never a panic.
    #[test]
    fn logging_without_a_subscriber_is_harmless() {
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            info!("nobody is listening");
        });
    }

    /// Go: `TestInitDebugEmitsDebugRecords`
    #[test]
    fn debug_level_emits_debug_records() {
        let output = capture(LevelFilter::DEBUG, LogFormat::Json, || {
            debug!("test debug message");
        });
        assert!(output.contains("test debug message"), "{output}");
    }

    /// Go: `TestInitInfoSuppressesDebugRecords`
    #[test]
    fn info_level_suppresses_debug_records() {
        let output = capture(LevelFilter::INFO, LogFormat::Json, || {
            debug!("should not appear");
        });
        assert!(!output.contains("should not appear"), "{output}");
    }

    /// Q8: the text format is tracing-subscriber's, with the fields as `key=value`.
    #[test]
    fn text_format() {
        let output = capture(LevelFilter::INFO, LogFormat::Text, || {
            info!(client_id = "abc", "broker: client connected");
        });
        assert!(output.contains(" INFO "), "{output}");
        assert!(output.contains("broker: client connected"), "{output}");
        assert!(output.contains("client_id=\"abc\""), "{output}");
        assert!(!output.contains('\u{1b}'), "ANSI escapes in {output}");
    }

    /// Q8: the JSON format is one object per line, with the event's fields at the top level.
    #[test]
    fn json_format() {
        let output = capture(LevelFilter::INFO, LogFormat::Json, || {
            info!(client_id = "abc", "broker: client connected");
        });
        let line = output.lines().next().expect("one line");
        let record: serde_json::Value = serde_json::from_str(line).expect("JSON");
        assert_eq!(record["level"], "INFO");
        assert_eq!(record["message"], "broker: client connected");
        assert_eq!(record["client_id"], "abc");
    }

    /// Records from crates that log through the `log` facade reach the subscriber.
    #[test]
    fn log_facade_is_bridged() {
        // Installing the bridge twice fails harmlessly; another test may have done it.
        let _ = tracing_log::LogTracer::init();
        let output = capture(LevelFilter::INFO, LogFormat::Text, || {
            log::info!("from the log facade");
        });
        assert!(output.contains("from the log facade"), "{output}");
    }

    /// Go: `TestEffectiveLevel_TC19_EnvVarSetsLevel`
    #[test]
    fn environment_sets_level_when_flag_not_given() {
        assert_eq!(effective_level(None, Some("debug")), LevelFilter::DEBUG);
        assert_eq!(effective_level(None, Some("error")), LevelFilter::ERROR);
        assert_eq!(effective_level(None, Some("")), LevelFilter::INFO);
        assert_eq!(effective_level(None, None), LevelFilter::INFO);
    }

    /// Go: `TestEffectiveLevel_TC20_FlagWinsOverEnvVar`
    #[test]
    fn flag_wins_over_environment() {
        assert_eq!(
            effective_level(Some("warn"), Some("debug")),
            LevelFilter::WARN
        );
        // An explicit flag wins even when it is the default value.
        assert_eq!(
            effective_level(Some("info"), Some("debug")),
            LevelFilter::INFO
        );
    }

    #[test]
    fn effective_format_priority() {
        assert_eq!(
            effective_format(Some(LogFormat::Json), LogFormat::Text),
            LogFormat::Json
        );
        assert_eq!(effective_format(None, LogFormat::Json), LogFormat::Json);
        assert_eq!(effective_format(None, LogFormat::Text), LogFormat::Text);
    }

    /// slog's level names and offsets, as `slog.Level.UnmarshalText` reads them.
    #[test]
    fn resolve_level_like_slog() {
        let cases = [
            ("debug", LevelFilter::DEBUG),
            ("DEBUG", LevelFilter::DEBUG),
            ("Info", LevelFilter::INFO),
            ("warn", LevelFilter::WARN),
            ("error", LevelFilter::ERROR),
            ("info+4", LevelFilter::WARN),
            ("info+1", LevelFilter::WARN),
            ("warn-4", LevelFilter::INFO),
            ("debug-4", LevelFilter::DEBUG),
            ("error+1", LevelFilter::OFF),
            ("warning", LevelFilter::INFO),
            ("", LevelFilter::INFO),
            ("info+", LevelFilter::INFO),
            ("info+x", LevelFilter::INFO),
            ("trace", LevelFilter::INFO),
        ];
        for (text, want) in cases {
            assert_eq!(resolve_level(text), want, "{text:?}");
        }
    }
}
