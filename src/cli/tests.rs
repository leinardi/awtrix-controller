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

use clap::Parser;

use super::*;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("awtrix-controller").chain(args.iter().copied()))
}

fn notification(args: &[&str]) -> Option<Notification> {
    parse(args)
        .expect("valid flags")
        .test_notification
        .notification()
}

/// Go: `TestBuildTestNotification_TC_NoFlags_ReturnsNil`
#[test]
fn no_test_notification_flag_gives_none() {
    assert_eq!(notification(&["--config", "/tmp/config.yaml"]), None);
}

/// Go: `TestBuildTestNotification_TC_EmptyVisited_ReturnsNil`
#[test]
fn no_flags_at_all_give_none() {
    assert_eq!(notification(&[]), None);
}

/// Go: `TestBuildTestNotification_TC_OverlayOnly`
#[test]
fn overlay_only() {
    let result = notification(&["--test-notification-overlay", "snow"]).expect("notification");
    assert_eq!(result.content.overlay, Some(OverlayEffect::Snow));
    assert_eq!(result.content.text, None);
    assert_eq!(result.rtttl, "");
}

/// Go: `TestBuildTestNotification_TC_RtttlOnly`
#[test]
fn rtttl_only() {
    const MELODY: &str = "Batman:d=16,o=5,b=180:8p,8b,8b,8b,2e.";
    let result = notification(&["--test-notification-rtttl", MELODY]).expect("notification");
    assert_eq!(result.rtttl, MELODY);
    assert_eq!(result.content.overlay, None);
}

/// Go: `TestBuildTestNotification_TC_TextAndIcon`
#[test]
fn text_and_icon() {
    let result = notification(&[
        "--test-notification-text",
        "Hello",
        "--test-notification-icon",
        "1234",
    ])
    .expect("notification");
    assert_eq!(result.content.text, Some(TextContent::plain("Hello")));
    assert_eq!(result.content.icon, "1234");
}

/// Go: `TestBuildTestNotification_TC_BoolPtrCenter_True`
#[test]
fn center_true() {
    let result = notification(&["--test-notification-center", "true"]).expect("notification");
    assert_eq!(result.content.center, Some(true));
}

/// Go: `TestBuildTestNotification_TC_BoolPtrCenter_False`
#[test]
fn center_false() {
    let result = notification(&["--test-notification-center", "false"]).expect("notification");
    assert_eq!(result.content.center, Some(false));
}

/// Go: `TestBuildTestNotification_TC_BoolPtrCenter_Empty`
#[test]
fn center_empty_is_left_to_the_device() {
    let result = notification(&[
        "--test-notification-overlay",
        "rain",
        "--test-notification-center",
        "",
    ])
    .expect("notification");
    assert_eq!(result.content.center, None);
}

/// Go: `TestBuildTestNotification_TC_RepeatPtr`
#[test]
fn repeat() {
    let result = notification(&["--test-notification-repeat", "3"]).expect("notification");
    assert_eq!(result.content.repeat, Some(3));
}

/// Go: `TestBuildTestNotification_TC_RepeatPtr_Empty`
#[test]
fn repeat_empty_is_left_to_the_device() {
    let result = notification(&[
        "--test-notification-overlay",
        "frost",
        "--test-notification-repeat",
        "",
    ])
    .expect("notification");
    assert_eq!(result.content.repeat, None);
}

/// Go: `TestBuildTestNotification_TC_StackPtr`
#[test]
fn stack_false() {
    let result = notification(&["--test-notification-stack", "false"]).expect("notification");
    assert_eq!(result.stack, Some(false));
}

/// Go: `TestBuildTestNotification_TC_FullNotification`
#[test]
fn full_notification() {
    let result = notification(&[
        "--test-notification-text",
        "Full test",
        "--test-notification-icon",
        "42",
        "--test-notification-duration",
        "30",
        "--test-notification-rainbow",
        "--test-notification-scroll-speed",
        "75",
        "--test-notification-no-scroll",
        "--test-notification-color",
        "#FF0000",
        "--test-notification-background",
        "#000000",
        "--test-notification-overlay",
        "thunder",
        "--test-notification-effect",
        "Plasma",
        "--test-notification-blink-text",
        "500",
        "--test-notification-fade-text",
        "200",
        "--test-notification-text-case",
        "1",
        "--test-notification-top-text",
        "--test-notification-text-offset",
        "5",
        "--test-notification-push-icon",
        "2",
        "--test-notification-hold",
        "--test-notification-sound",
        "alarm",
        "--test-notification-rtttl",
        "scale:d=4,o=5,b=100:c,d,e,f",
        "--test-notification-loop-sound",
        "--test-notification-wakeup",
    ])
    .expect("notification");

    let content = &result.content;
    assert_eq!(content.text, Some(TextContent::plain("Full test")));
    assert_eq!(content.icon, "42");
    assert_eq!(content.duration, 30);
    assert!(content.rainbow);
    assert_eq!(content.scroll_speed, 75);
    assert!(content.no_scroll);
    assert_eq!(content.color, "#FF0000");
    assert_eq!(content.background, "#000000");
    assert_eq!(content.overlay, Some(OverlayEffect::Thunder));
    assert_eq!(content.effect, Some(Effect::Plasma));
    assert_eq!(content.blink_text, 500);
    assert_eq!(content.fade_text, 200);
    assert_eq!(content.text_case, TextCaseMode::Uppercase);
    assert!(content.top_text);
    assert_eq!(content.text_offset, 5);
    assert_eq!(content.push_icon, PushIconBehavior::Fixed);
    assert!(result.hold);
    assert_eq!(result.sound, "alarm");
    assert_eq!(result.rtttl, "scale:d=4,o=5,b=100:c,d,e,f");
    assert!(result.loop_sound);
    assert!(result.wakeup);
}

/// A boolean flag set to false still turns the mode on, as a visited Go flag did.
#[test]
fn boolean_flag_set_to_false_turns_the_mode_on() {
    let result = notification(&["--test-notification-hold=false"]).expect("notification");
    assert!(!result.hold);
}

/// A name the model does not know reaches the device verbatim.
#[test]
fn unknown_effect_and_overlay_names_are_kept() {
    let result = notification(&[
        "--test-notification-effect",
        "NewEffect",
        "--test-notification-overlay",
        "hail",
    ])
    .expect("notification");
    assert_eq!(
        result.content.effect,
        Some(Effect::Other("NewEffect".to_owned()))
    );
    assert_eq!(
        result.content.overlay,
        Some(OverlayEffect::Other("hail".to_owned()))
    );
}

/// Q9: the short and long forms work, the single-dash long form does not.
#[test]
fn short_and_long_forms() {
    let long = parse(&["--config", "a.yaml", "--log-level", "debug"]).expect("long");
    assert_eq!(long.config, Some(PathBuf::from("a.yaml")));
    assert_eq!(long.log_level.as_deref(), Some("debug"));
    let short = parse(&["-c", "b.yaml", "-l", "warn", "-v"]).expect("short");
    assert_eq!(short.config, Some(PathBuf::from("b.yaml")));
    assert_eq!(short.log_level.as_deref(), Some("warn"));
    assert_eq!(short.version, Some(true));
    let equals = parse(&["--config=c.yaml", "--log-format=json"]).expect("equals");
    assert_eq!(equals.config, Some(PathBuf::from("c.yaml")));
    assert_eq!(equals.log_format, Some(LogFormat::Json));
    assert!(parse(&["-config", "a.yaml"]).is_err());
    assert!(parse(&["--log-format", "xml"]).is_err());
    assert!(parse(&["--unknown"]).is_err());
}

/// Like Go's flags: the last of a repeated flag wins, and a value may start with a dash.
#[test]
fn repeated_flags_and_dash_values() {
    let cli = parse(&[
        "--config",
        "first.yaml",
        "-c",
        "second.yaml",
        "--test-notification-repeat",
        "-1",
        "--test-notification-text-offset",
        "-5",
    ])
    .expect("valid");
    assert_eq!(cli.config, Some(PathBuf::from("second.yaml")));
    let result = cli.test_notification.notification().expect("notification");
    assert_eq!(result.content.repeat, Some(-1));
    assert_eq!(result.content.text_offset, -5);
}

/// A boolean flag takes its value only after `=`, so the next word stays an argument.
#[test]
fn boolean_flags_take_no_separate_value() {
    let cli = parse(&["--test-notification-rainbow", "--config", "x.yaml"]).expect("valid");
    assert_eq!(cli.test_notification.rainbow, Some(true));
    assert_eq!(cli.config, Some(PathBuf::from("x.yaml")));
    assert!(parse(&["--test-notification-rainbow", "false"]).is_err());
    assert_eq!(
        parse(&["--version=false"]).expect("valid").version,
        Some(false)
    );
}

/// `strconv.ParseBool`'s spellings.
#[test]
fn go_booleans() {
    for value in ["1", "t", "T", "TRUE", "true", "True"] {
        assert_eq!(parse_go_bool(value), Ok(true), "{value}");
    }
    for value in ["0", "f", "F", "FALSE", "false", "False"] {
        assert_eq!(parse_go_bool(value), Ok(false), "{value}");
    }
    for value in ["", "yes", "tRUE", "2"] {
        assert!(parse_go_bool(value).is_err(), "{value}");
    }
}

/// `strconv.ParseInt(value, 0, 64)`'s literals.
#[test]
fn go_integers() {
    let valid = [
        ("0", 0),
        ("42", 42),
        ("+42", 42),
        ("-42", -42),
        ("0x1F", 31),
        ("0X1f", 31),
        ("0o17", 15),
        ("017", 15),
        ("0b101", 5),
        ("1_000", 1000),
        ("0x_1F", 31),
        ("-9223372036854775808", i64::MIN),
        ("9223372036854775807", i64::MAX),
    ];
    for (value, expected) in valid {
        assert_eq!(parse_go_int(value), Ok(expected), "{value}");
    }
    for value in [
        "",
        "-",
        "0x",
        "08",
        "1__0",
        "_1",
        "1_",
        "1.5",
        "--1",
        "+-1",
        "9223372036854775808",
        "-9223372036854775809",
        " 1",
    ] {
        assert!(parse_go_int(value).is_err(), "{value}");
    }
}

/// The text-case and push-icon modes are the firmware's three values.
#[test]
fn mode_flags_take_the_firmware_values() {
    assert!(parse(&["--test-notification-text-case", "3"]).is_err());
    assert!(parse(&["--test-notification-push-icon", "-1"]).is_err());
    let cli = parse(&["--test-notification-text-case", "0x2"]).expect("valid");
    assert_eq!(cli.test_notification.text_case, Some(TextCaseMode::AsIs));
}

/// `parseBoolPtr` and `parseIntPtr`: anything unrecognised leaves the field to the device.
#[test]
fn optional_values() {
    assert_eq!(parse_bool_option("true"), Some(true));
    assert_eq!(parse_bool_option("false"), Some(false));
    assert_eq!(parse_bool_option("TRUE"), None);
    assert_eq!(parse_bool_option(""), None);
    assert_eq!(parse_int_option("-1"), Some(-1));
    assert_eq!(parse_int_option("+3"), Some(3));
    assert_eq!(parse_int_option("0x3"), None);
    assert_eq!(parse_int_option(""), None);
}

/// Q9: Go's single-dash long form is named and refused, not read as a cluster of short flags.
#[test]
fn single_dash_long_form_is_detected() {
    let args = |list: &[&str]| -> Vec<OsString> { list.iter().map(OsString::from).collect() };
    assert_eq!(
        single_dash_long_form(&args(&["-config", "x.yaml"])),
        Some(("-config".to_owned(), "--config".to_owned()))
    );
    assert_eq!(
        single_dash_long_form(&args(&["-c", "x.yaml", "-log-level=debug"])),
        Some(("-log-level=debug".to_owned(), "--log-level".to_owned()))
    );
    assert_eq!(
        single_dash_long_form(&args(&["-test-notification-text", "hi"])),
        Some((
            "-test-notification-text".to_owned(),
            "--test-notification-text".to_owned()
        ))
    );
    assert_eq!(
        single_dash_long_form(&args(&["-help"])).map(|found| found.1),
        Some("--help".to_owned())
    );
    for list in [
        &["--config", "x.yaml"][..],
        &["-c", "x.yaml", "-v"],
        &["-vc", "x.yaml"],
        &["-l", "debug"],
        &["--", "-config"],
    ] {
        assert_eq!(single_dash_long_form(&args(list)), None, "{list:?}");
    }
}
