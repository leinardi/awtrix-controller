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

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::*;

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("serialize")
}

fn from_json<T: DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).expect("deserialize")
}

fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> T {
    from_json(&to_json(value))
}

fn notification_with(content: AppContent) -> Notification {
    Notification {
        content,
        ..Notification::default()
    }
}

/// Go: `TestSettingsBriAbsentWhenNil`
#[test]
fn settings_bri_absent_when_unset() {
    assert!(!to_json(&Settings::default()).contains(r#""BRI""#));
}

/// Go: `TestSettingsBriPresentWhenSet`
#[test]
fn settings_bri_present_when_set() {
    let settings = Settings {
        bri: Some(1),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""BRI":1"#));
}

/// Go: `TestSettingsAbriAbsentWhenNil`
#[test]
fn settings_abri_absent_when_unset() {
    assert!(!to_json(&Settings::default()).contains(r#""ABRI""#));
}

/// Go: `TestSettingsAbriFalsePresentWhenPointerSet`
#[test]
fn settings_abri_false_present_when_set() {
    let settings = Settings {
        abri: Some(false),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""ABRI":false"#));
}

/// Go: `TestSettingsColorFieldsOmittedWhenEmpty`
#[test]
fn settings_color_fields_omitted_when_empty() {
    let json = to_json(&Settings::default());
    for key in ["CHCOL", "CBCOL", "WDCA", "WDCI", "TIME_COL", "DATE_COL"] {
        assert!(
            !json.contains(&format!("\"{key}\"")),
            "{key} present in {json}"
        );
    }
}

/// Go: `TestStatsRoundTrip`
#[test]
fn stats_round_trip() {
    let original = Stats {
        app: "clock".into(),
        bat: 85,
        bat_raw: 712,
        bri: 128,
        hum: 55,
        indicator1: true,
        indicator2: true,
        indicator3: true,
        ip_address: "192.168.1.42".into(),
        ldr_raw: 400,
        lux: 210,
        matrix: true,
        messages: 3,
        ram: 102_400,
        temp: 22,
        device_type: 1,
        uid: "AABBCCDD1122".into(),
        uptime: 3600,
        version: "0.96".into(),
        wifi_signal: -65,
    };
    let json = to_json(&original);
    assert_eq!(Stats::from_json(json.as_bytes()).expect("decode"), original);
}

#[test]
fn stats_decoder_handles_every_serialized_key() {
    let mut fields = serde_json::to_value(Stats::default()).expect("serialize stats");
    for (key, value) in fields.as_object_mut().expect("stats object") {
        *value = match value {
            serde_json::Value::String(_) => serde_json::json!("nonempty"),
            serde_json::Value::Bool(_) => serde_json::json!(true),
            serde_json::Value::Number(_) => serde_json::json!(42),
            serde_json::Value::Null
            | serde_json::Value::Array(_)
            | serde_json::Value::Object(_) => {
                panic!("unexpected stats field {key}: {value:?}")
            }
        };
    }
    let payload = serde_json::to_vec(&fields).expect("serialize populated stats");
    let decoded = Stats::from_json(&payload).expect("decode every field");
    assert_eq!(
        serde_json::to_value(decoded).expect("serialize decoded stats"),
        fields
    );
}

/// Go's `encoding/json` behaviour on the stats payload, which the Go decoder relied on without a
/// test of its own: missing keys and nulls leave zero values, unknown keys are ignored, keys
/// match case-insensitively, a null payload decodes to zero values, and a wrong type fails.
#[test]
fn stats_decode_like_encoding_json() {
    let decoded = Stats::from_json(br#"{"BAT":50,"app":null,"extra":{"x":1},"Wifi_Signal":-70}"#)
        .expect("decode");
    assert_eq!(
        decoded,
        Stats {
            bat: 50,
            wifi_signal: -70,
            ..Stats::default()
        }
    );

    assert_eq!(Stats::from_json(b"null").expect("decode"), Stats::default());

    // Several keys for one field: the last in the document wins, whatever their case.
    let duplicates = Stats::from_json(br#"{"bat":1,"BAT":2}"#).expect("decode");
    assert_eq!(duplicates.bat, 2);
    let duplicates = Stats::from_json(br#"{"BAT":2,"bat":1}"#).expect("decode");
    assert_eq!(duplicates.bat, 1);
    assert!(Stats::from_json(br#"{"bat":"full"}"#).is_err());
    assert!(Stats::from_json(br#"{"lux":12.5}"#).is_err());
    assert!(Stats::from_json(b"[1]").is_err());
    assert!(Stats::from_json(b"{not json").is_err());
}

#[test]
fn stats_duplicate_fields_preserve_null_and_type_error_semantics() {
    let stats = Stats::from_json(br#"{"bat":85,"BAT":null,"app":"Time","APP":null}"#)
        .expect("null does not replace a scalar");
    assert_eq!(stats.bat, 85);
    assert_eq!(stats.app, "Time");
    assert!(Stats::from_json(br#"{"bat":"invalid","BAT":85}"#).is_err());
    assert!(Stats::from_json(br#"{"APP":false,"app":"Time"}"#).is_err());
}

/// Go: `TestNotificationRoundTrip`
#[test]
fn notification_round_trip() {
    let original = notification_with(AppContent {
        text: Some(TextContent::plain("Happy Birthday!")),
        icon: "14004".into(),
        duration: 10,
        scroll_speed: 50,
        draw: vec![DrawInstruction::Pixel(3, 4, "#FF0000".into())],
        ..AppContent::default()
    });
    assert_eq!(round_trip(&original), original);
}

/// Go: `TestCustomAppRoundTrip`
#[test]
fn custom_app_round_trip() {
    let original = CustomApp {
        content: AppContent {
            text: Some(TextContent::plain("My App")),
            icon: "1234".into(),
            ..AppContent::default()
        },
        pos: 2,
        lifetime: 30,
        save: true,
        ..CustomApp::default()
    };
    assert_eq!(round_trip(&original), original);
}

/// Go: `TestDrawPixelMarshal`
#[test]
fn draw_pixel_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::Pixel(3, 4, "#FF0000".into())),
        r##"{"dp":[3,4,"#FF0000"]}"##
    );
}

/// Go: `TestDrawLineMarshal`
#[test]
fn draw_line_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::Line(0, 0, 31, 7, "#00FF00".into())),
        r##"{"dl":[0,0,31,7,"#00FF00"]}"##
    );
}

/// Go: `TestDrawRectMarshal`
#[test]
fn draw_rect_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::Rect(2, 2, 10, 5, "#FFFFFF".into())),
        r##"{"dr":[2,2,10,5,"#FFFFFF"]}"##
    );
}

/// Go: `TestDrawFilledRectMarshal`
#[test]
fn draw_filled_rect_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::FilledRect(0, 0, 8, 8, "#0000FF".into())),
        r##"{"df":[0,0,8,8,"#0000FF"]}"##
    );
}

/// Go: `TestDrawCircleMarshal`
#[test]
fn draw_circle_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::Circle(5, 5, 3, "#FF00FF".into())),
        r##"{"dc":[5,5,3,"#FF00FF"]}"##
    );
}

/// Go: `TestDrawFilledCircleMarshal`
#[test]
fn draw_filled_circle_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::FilledCircle(5, 5, 3, "#00FFFF".into())),
        r##"{"dfc":[5,5,3,"#00FFFF"]}"##
    );
}

/// Go: `TestDrawTextMarshal`
#[test]
fn draw_text_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::Text(0, 0, "Hi".into(), "#FFFFFF".into())),
        r##"{"dt":[0,0,"Hi","#FFFFFF"]}"##
    );
}

/// Go: `TestDrawBitmapMarshal`
#[test]
fn draw_bitmap_marshal() {
    assert_eq!(
        to_json(&DrawInstruction::Bitmap(
            0,
            0,
            2,
            1,
            vec![16_711_680, 65_280]
        )),
        r#"{"db":[0,0,2,1,[16711680,65280]]}"#
    );
}

/// Go: `TestDrawListMarshal`
#[test]
fn draw_list_marshal() {
    let draw_list = vec![
        DrawInstruction::Pixel(1, 2, "#FF0000".into()),
        DrawInstruction::Line(0, 0, 5, 5, "#00FF00".into()),
    ];
    assert_eq!(
        to_json(&draw_list),
        r##"[{"dp":[1,2,"#FF0000"]},{"dl":[0,0,5,5,"#00FF00"]}]"##
    );
}

fn every_draw_instruction() -> Vec<DrawInstruction> {
    vec![
        DrawInstruction::Pixel(1, 2, "#FF0000".into()),
        DrawInstruction::Line(0, 0, 31, 7, "#00FF00".into()),
        DrawInstruction::Rect(2, 2, 10, 5, "#FFFFFF".into()),
        DrawInstruction::FilledRect(0, 0, 8, 8, "#0000FF".into()),
        DrawInstruction::Circle(5, 5, 3, "#FF00FF".into()),
        DrawInstruction::FilledCircle(5, 5, 3, "#00FFFF".into()),
        DrawInstruction::Text(0, 0, "Hi".into(), "#FFFFFF".into()),
        DrawInstruction::Bitmap(0, 0, 2, 1, vec![16_711_680, 65_280]),
    ]
}

/// Go: `TestDrawPixelRoundTrip`, `TestDrawLineRoundTrip`, `TestDrawRectRoundTrip`,
/// `TestDrawFilledRectRoundTrip`, `TestDrawCircleRoundTrip`, `TestDrawFilledCircleRoundTrip`,
/// `TestDrawTextRoundTrip`, `TestDrawBitmapRoundTrip`: each command alone in a list.
#[test]
fn draw_instruction_round_trip_each() {
    for instruction in every_draw_instruction() {
        let original = vec![instruction];
        assert_eq!(round_trip(&original), original);
    }
}

/// Go: `TestDrawListRoundTrip`
#[test]
fn draw_list_round_trip() {
    let original = every_draw_instruction();
    assert_eq!(round_trip(&original), original);
}

fn draw_list_error(json: &str) -> bool {
    serde_json::from_str::<Vec<DrawInstruction>>(json).is_err()
}

/// Go: `TestDrawListUnmarshalUnknownKey`
#[test]
fn draw_list_unmarshal_unknown_key() {
    assert!(draw_list_error(r##"[{"zz":[1,2,"#FF"]}]"##));
}

/// Go: `TestDrawListUnmarshalWrongArgCount`
#[test]
fn draw_list_unmarshal_wrong_arg_count() {
    assert!(draw_list_error(r#"[{"dp":[1,2]}]"#));
    assert!(draw_list_error(r##"[{"dp":[1,2,"#FF0000",4]}]"##));
}

/// Go: `TestDrawListUnmarshalBadArgType`
#[test]
fn draw_list_unmarshal_bad_arg_type() {
    assert!(draw_list_error(r#"[{"dp":[1,2,99]}]"#));
}

/// Go: `TestDrawListUnmarshalNotArray`
#[test]
fn draw_list_unmarshal_not_array() {
    assert!(draw_list_error(r#""not-an-array""#));
}

/// Go: `TestDrawListUnmarshalMultiKey`
#[test]
fn draw_list_unmarshal_multi_key() {
    assert!(draw_list_error(
        r##"[{"dp":[3,4,"#FF0000"],"dl":[0,0,1,1,"#FF0000"]}]"##
    ));
}

/// Go: `TestPowerFalseIsPresent`
#[test]
fn power_false_is_present() {
    assert_eq!(to_json(&Power { power: false }), r#"{"power":false}"#);
}

/// Go: `TestSleepZeroIsPresent`
#[test]
fn sleep_zero_is_present() {
    assert_eq!(to_json(&Sleep { sleep: 0 }), r#"{"sleep":0}"#);
}

/// Go: `TestTransitionEffectRandomPresentWhenPointerSet`
#[test]
fn transition_effect_random_present_when_set() {
    let settings = Settings {
        teff: Some(TransitionEffect::Random),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""TEFF":0"#));
}

/// Go: `TestTransitionEffectAbsentWhenNil`
#[test]
fn transition_effect_absent_when_unset() {
    assert!(!to_json(&Settings::default()).contains(r#""TEFF""#));
}

/// Go: `TestTimeFormatMarshal`
#[test]
fn time_format_marshal() {
    let settings = Settings {
        tformat: Some(TimeFormat::TwentyFourHour),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""TFORMAT":"%H:%M""#));
}

/// Go: `TestDateFormatMarshal`
#[test]
fn date_format_marshal() {
    let settings = Settings {
        dformat: Some(DateFormat::DayMonthYear),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""DFORMAT":"%d.%m.%y""#));
}

/// Go: `TestOverlayEffectMarshalInSettings`
#[test]
fn overlay_effect_marshal_in_settings() {
    let settings = Settings {
        overlay: Some(OverlayEffect::Snow),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""OVERLAY":"snow""#));
}

/// Go: `TestOverlayEffectMarshalInNotification`
#[test]
fn overlay_effect_marshal_in_notification() {
    let notification = notification_with(AppContent {
        overlay: Some(OverlayEffect::Rain),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""overlay":"rain""#));
}

/// Go: `TestPushIconBehaviorMarshal`
#[test]
fn push_icon_behavior_marshal() {
    let notification = notification_with(AppContent {
        push_icon: PushIconBehavior::Scroll,
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""pushIcon":1"#));
}

/// Go: `TestTextCaseModeMarshal`
#[test]
fn text_case_mode_marshal() {
    let notification = notification_with(AppContent {
        text_case: TextCaseMode::AsIs,
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""textCase":2"#));
}

/// Go: `TestLifetimeResetModeMarshal`
#[test]
fn lifetime_reset_mode_marshal() {
    let app = CustomApp {
        lifetime_mode: LifetimeResetMode::OnView,
        ..CustomApp::default()
    };
    assert!(to_json(&app).contains(r#""lifetimeMode":1"#));
}

/// Go: `TestCenterFalsePresentWhenPointerSet`
#[test]
fn center_false_present_when_set() {
    let notification = notification_with(AppContent {
        center: Some(false),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""center":false"#));
}

/// Go: `TestAutoscaleFalsePresentWhenPointerSet`
#[test]
fn autoscale_false_present_when_set() {
    let notification = notification_with(AppContent {
        autoscale: Some(false),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""autoscale":false"#));
}

/// Go: `TestStackFalsePresentWhenPointerSet`
#[test]
fn stack_false_present_when_set() {
    let notification = Notification {
        stack: Some(false),
        ..Notification::default()
    };
    assert!(to_json(&notification).contains(r#""stack":false"#));
}

/// Go: `TestProgressZeroPresentWhenPointerSet`
#[test]
fn progress_zero_present_when_set() {
    let notification = notification_with(AppContent {
        progress: Some(0),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""progress":0"#));
}

/// Go: `TestRepeatZeroPresentWhenPointerSet`
#[test]
fn repeat_zero_present_when_set() {
    let notification = notification_with(AppContent {
        repeat: Some(0),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""repeat":0"#));
}

/// Go: `TestBarBCMarshal`
#[test]
fn bar_bc_marshal() {
    let notification = notification_with(AppContent {
        bar_bc: "#00FF00".into(),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r##""barBC":"#00FF00""##));
}

/// Go: `TestEffectMarshalInNotification`
#[test]
fn effect_marshal_in_notification() {
    let notification = notification_with(AppContent {
        effect: Some(Effect::Plasma),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""effect":"Plasma""#));
}

/// Go: `TestEffectPaletteMarshalInEffectSettings`
#[test]
fn effect_palette_marshal_in_effect_settings() {
    let settings = EffectSettings {
        palette: Some(EffectPalette::Rainbow),
        ..EffectSettings::default()
    };
    assert!(to_json(&settings).contains(r#""palette":"Rainbow""#));
}

/// Go: `TestClockModeMarshalAbsentWhenNil`
#[test]
fn clock_mode_absent_when_unset() {
    assert!(!to_json(&Settings::default()).contains(r#""TMODE""#));
}

/// Go: `TestClockModeWeekdayBarPresentWhenPointerSet`
#[test]
fn clock_mode_weekday_bar_present_when_set() {
    let settings = Settings {
        tmode: Some(ClockMode::WeekdayBar),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""TMODE":0"#));
}

/// Go: `TestClockModeCalendarMarshal`
#[test]
fn clock_mode_calendar_marshal() {
    let settings = Settings {
        tmode: Some(ClockMode::Calendar),
        ..Settings::default()
    };
    assert!(to_json(&settings).contains(r#""TMODE":1"#));
}

/// Go: `TestPlainTextMarshal`
#[test]
fn plain_text_marshal() {
    assert_eq!(to_json(&TextContent::plain("Hi")), r#""Hi""#);
}

/// Go: `TestEmptyPlainTextMarshal`
#[test]
fn empty_plain_text_marshal() {
    assert_eq!(to_json(&TextContent::plain("")), r#""""#);
}

/// An empty fragment list is written as the empty string, as Go's `TextContent` did.
#[test]
fn empty_fragmented_text_marshal() {
    assert_eq!(to_json(&TextContent::fragmented(Vec::new())), r#""""#);
}

fn fragment(text: &str, color: &str) -> TextFragment {
    TextFragment {
        text: text.into(),
        color: color.into(),
    }
}

/// Go: `TestFragmentedTextMarshal`
#[test]
fn fragmented_text_marshal() {
    let text = TextContent::fragmented(vec![fragment("Hello", "FF0000"), fragment("!", "00FF00")]);
    assert_eq!(
        to_json(&text),
        r#"[{"t":"Hello","c":"FF0000"},{"t":"!","c":"00FF00"}]"#
    );
}

/// Go: `TestPlainTextRoundTrip`
#[test]
fn plain_text_round_trip() {
    let original = TextContent::plain("Round trip!");
    assert_eq!(round_trip(&original), original);
}

/// Go: `TestFragmentedTextRoundTrip`
#[test]
fn fragmented_text_round_trip() {
    let original = TextContent::fragmented(vec![
        fragment("Hello", "FF0000"),
        fragment("World", "0000FF"),
    ]);
    assert_eq!(round_trip(&original), original);
}

/// Go: `TestTextContentUnmarshalInvalidJSON`
#[test]
fn text_content_unmarshal_invalid_json() {
    assert!(serde_json::from_str::<TextContent>("{invalid}").is_err());
}

/// Go: `TestNotificationWithFragmentedText`
#[test]
fn notification_with_fragmented_text() {
    let notification = notification_with(AppContent {
        text: Some(TextContent::fragmented(vec![fragment("Hi", "FF0000")])),
        ..AppContent::default()
    });
    assert!(to_json(&notification).contains(r#""text":[{"t":"Hi","c":"FF0000"}]"#));
}

/// Go: `TestSoundMarshal`
#[test]
fn sound_marshal() {
    let sound = Sound {
        sound: "alarm".into(),
    };
    assert_eq!(to_json(&sound), r#"{"sound":"alarm"}"#);
}

/// Go: `TestAppSwitchMarshal`
#[test]
fn app_switch_marshal() {
    let switch = AppSwitch {
        name: "Time".into(),
    };
    assert_eq!(to_json(&switch), r#"{"name":"Time"}"#);
}

/// Go: `TestAppSwitchWithBuiltInApp`
#[test]
fn app_switch_with_built_in_app() {
    let switch = AppSwitch {
        name: BuiltInApp::Time.as_str().into(),
    };
    assert_eq!(to_json(&switch), r#"{"name":"Time"}"#);
}

/// Go: `TestBuiltInAppValues`
#[test]
fn built_in_app_values() {
    let cases = [
        (BuiltInApp::Time, "Time"),
        (BuiltInApp::Date, "Date"),
        (BuiltInApp::Temperature, "Temperature"),
        (BuiltInApp::Humidity, "Humidity"),
        (BuiltInApp::Battery, "Battery"),
    ];
    for (app, want) in cases {
        assert_eq!(app.as_str(), want);
        assert_eq!(to_json(&app), format!("\"{want}\""));
    }
}

/// Go: `TestMoodLightKelvinMarshal`
#[test]
fn mood_light_kelvin_marshal() {
    let mood_light = MoodLight {
        brightness: 170,
        kelvin: 2300,
        ..MoodLight::default()
    };
    assert_eq!(to_json(&mood_light), r#"{"brightness":170,"kelvin":2300}"#);
}

/// Go: `TestMoodLightKelvinAbsentWhenZero`
#[test]
fn mood_light_kelvin_absent_when_zero() {
    let mood_light = MoodLight {
        brightness: 170,
        color: "#FF0000".into(),
        ..MoodLight::default()
    };
    assert!(!to_json(&mood_light).contains(r#""kelvin""#));
}

/// The settings payload the controller sends when nothing is set is exactly Go's: every field
/// left out but the overlay, which is always written.
#[test]
fn empty_settings_is_only_the_overlay() {
    assert_eq!(to_json(&Settings::default()), r#"{"OVERLAY":null}"#);
}

/// The flattened display fields come first and in Go's field order, followed by the
/// notification's own fields, so the payload is byte for byte what Go sent.
#[test]
fn notification_field_order_matches_go() {
    let notification = Notification {
        content: AppContent {
            text: Some(TextContent::plain("Hi")),
            icon: "14004".into(),
            repeat: Some(1),
            duration: 10,
            rainbow: true,
            scroll_speed: 50,
            ..AppContent::default()
        },
        rtttl: "melody:d=4:c".into(),
        stack: Some(false),
        wakeup: true,
        ..Notification::default()
    };
    assert_eq!(
        to_json(&notification),
        concat!(
            r#"{"text":"Hi","rainbow":true,"icon":"14004","repeat":1,"duration":10,"#,
            r#""scrollSpeed":50,"rtttl":"melody:d=4:c","stack":false,"wakeup":true}"#
        )
    );
}

/// Q7: `<`, `>` and `&` are written literally, where Go escaped them as `\u003c`, `\u003e`
/// and `\u0026`.
#[test]
fn html_characters_are_not_escaped() {
    let notification = notification_with(AppContent {
        text: Some(TextContent::plain("<b>Tom & Jerry</b>")),
        ..AppContent::default()
    });
    assert_eq!(to_json(&notification), r#"{"text":"<b>Tom & Jerry</b>"}"#);
}

/// The string enums are open, as Go's string types were: a name the list does not know is sent
/// verbatim, and decodes back to the same name.
#[test]
fn unlisted_names_serialize_verbatim() {
    let notification = notification_with(AppContent {
        effect: Some(Effect::from_name("Aurora")),
        overlay: Some(OverlayEffect::from_name("hail")),
        ..AppContent::default()
    });
    assert_eq!(
        to_json(&notification),
        r#"{"effect":"Aurora","overlay":"hail"}"#
    );
    assert_eq!(round_trip(&notification), notification);

    let settings = Settings {
        tformat: Some(TimeFormat::from_name("%H")),
        dformat: Some(DateFormat::from_name("%d")),
        overlay: Some(OverlayEffect::from_name("hail")),
        ..Settings::default()
    };
    assert_eq!(
        to_json(&settings),
        r#"{"TFORMAT":"%H","DFORMAT":"%d","OVERLAY":"hail"}"#
    );

    let effect_settings = EffectSettings {
        palette: Some(EffectPalette::from_name("Sunset")),
        ..EffectSettings::default()
    };
    assert_eq!(to_json(&effect_settings), r#"{"palette":"Sunset"}"#);

    assert_eq!(Effect::from_name("Plasma"), Effect::Plasma);
    assert_eq!(Effect::Plasma.as_str(), "Plasma");
}

/// An empty name is left out, as Go's `omitempty` left out an empty string type; the overlay of
/// the settings payload, which Go always wrote, is written even when empty.
#[test]
fn empty_names_are_omitted_like_omitempty() {
    let notification = notification_with(AppContent {
        effect: Some(Effect::from_name("")),
        overlay: Some(OverlayEffect::from_name("")),
        ..AppContent::default()
    });
    assert_eq!(to_json(&notification), "{}");

    let settings = Settings {
        tformat: Some(TimeFormat::from_name("")),
        overlay: Some(OverlayEffect::from_name("")),
        ..Settings::default()
    };
    assert_eq!(to_json(&settings), r#"{"OVERLAY":""}"#);
}
