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

//! The firmware's enumerated values. The integer enums are written as their number, the string
//! enums as the exact string the firmware expects.
//!
//! The string enums are open, as the Go string types were: besides the names listed here, which
//! are what the firmware documents, `Other` carries any name verbatim. The firmware advertises its
//! own effect list, and a name typed by the user (the test-notification flags) must reach the
//! device unchanged, whether or not this list knows it.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};

/// A string enum of the firmware: its known names, and `Other` for any name it does not list.
pub trait OpenName {
    /// The name as the firmware spells it.
    fn as_str(&self) -> &str;
}

/// Reports whether an open name is unset or the empty string: the `skip_serializing_if` test that
/// mirrors Go's `omitempty` on a string type.
#[expect(
    clippy::ref_option,
    reason = "serde's skip_serializing_if calls the predicate with a reference to the field"
)]
pub(super) fn is_unset_name<T: OpenName>(value: &Option<T>) -> bool {
    value.as_ref().is_none_or(|name| name.as_str().is_empty())
}

/// Declares an open string enum: one unit variant per known name, an `Other(String)` variant for
/// any other name, and serde, `as_str` and `from_name` over the names.
macro_rules! open_name_enum {
    (
        $(#[$enum_meta:meta])*
        $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $value:literal, )+
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
            /// A name this list does not know, sent verbatim.
            Other(String),
        }

        impl $name {
            /// The variant for a name: a known one, or `Other` carrying it verbatim.
            #[must_use]
            pub fn from_name(name: &str) -> Self {
                match name {
                    $( $value => Self::$variant, )+
                    other => Self::Other(other.to_owned()),
                }
            }
        }

        impl OpenName for $name {
            fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $value, )+
                    Self::Other(name) => name,
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok(Self::from_name(&String::deserialize(deserializer)?))
            }
        }
    };
}

/// The animation used when switching between apps (`TEFF`). The firmware default is `Slide`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum TransitionEffect {
    /// Picks a random effect each transition.
    #[default]
    Random = 0,
    /// Slides the next app in from the right (firmware default).
    Slide = 1,
    /// Fades the display to black, then back up.
    Dim = 2,
    /// Zooms in on the next app.
    Zoom = 3,
    /// Rotates the display during the transition.
    Rotate = 4,
    /// Pixelates the display during the transition.
    Pixelate = 5,
    /// Splits the display like a curtain.
    Curtain = 6,
    /// Produces a ripple distortion during the transition.
    Ripple = 7,
    /// Blinks the display during the transition.
    Blink = 8,
    /// Reloads the display content during the transition.
    Reload = 9,
    /// Fades between apps.
    Fade = 10,
}

open_name_enum! {
    /// A predefined time format accepted by `TFORMAT`, in the firmware's strftime-style tokens.
    TimeFormat {
        /// 24-hour time with seconds, e.g. "13:30:45".
        TwentyFourHourSeconds => "%H:%M:%S",
        /// 12-hour time with seconds, e.g. "1:30:45".
        TwelveHourSeconds => "%l:%M:%S",
        /// 24-hour time, e.g. "13:30".
        TwentyFourHour => "%H:%M",
        /// 24-hour time with a blinking colon.
        TwentyFourHourBlinking => "%H %M",
        /// 12-hour time, e.g. "1:30".
        TwelveHour => "%l:%M",
        /// 12-hour time with a blinking colon.
        TwelveHourBlinking => "%l %M",
        /// 12-hour time with an AM/PM indicator, e.g. "1:30 PM".
        TwelveHourAmPm => "%l:%M %p",
        /// 12-hour time with a blinking colon and an AM/PM indicator.
        TwelveHourBlinkingAmPm => "%l %M %p",
    }
}

open_name_enum! {
    /// A predefined date format accepted by `DFORMAT`.
    DateFormat {
        /// Day.Month.Year (short), e.g. "16.04.22".
        DayMonthYear => "%d.%m.%y",
        /// Day.Month, e.g. "16.04".
        DayMonth => "%d.%m",
        /// Year-Month-Day, e.g. "22-04-16".
        YearMonthDay => "%y-%m-%d",
        /// Month-Day, e.g. "04-16".
        MonthDay => "%m-%d",
        /// Month/Day/Year, e.g. "04/16/22".
        MonthDayYear => "%m/%d/%y",
        /// Month/Day, e.g. "04/16".
        MonthDaySlash => "%m/%d",
        /// Day/Month/Year, e.g. "16/04/22".
        DayMonthYearSlash => "%d/%m/%y",
        /// Day/Month, e.g. "16/04".
        DayMonthSlash => "%d/%m",
        /// Month-Day-Year, e.g. "04-16-22".
        MonthDayYearDash => "%m-%d-%y",
    }
}

open_name_enum! {
    /// A weather overlay drawn over the whole matrix, used by `Settings::overlay` (`OVERLAY`) and
    /// `AppContent::overlay`.
    OverlayEffect {
        /// Removes any active overlay.
        Clear => "clear",
        /// Falling snowflakes.
        Snow => "snow",
        /// Rain.
        Rain => "rain",
        /// Light drizzle.
        Drizzle => "drizzle",
        /// A storm.
        Storm => "storm",
        /// A thunderstorm.
        Thunder => "thunder",
        /// Frost.
        Frost => "frost",
    }
}

/// How the icon moves relative to scrolling text (`pushIcon`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum PushIconBehavior {
    /// The icon stays while the text scrolls (default).
    #[default]
    Static = 0,
    /// The icon scrolls out with the text, then re-appears.
    Scroll = 1,
    /// The icon stays fixed on the left while the text scrolls past it.
    Fixed = 2,
}

/// The capitalisation applied to notification text (`textCase`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum TextCaseMode {
    /// Inherits the device's `UPPERCASE` setting (firmware default).
    #[default]
    GlobalSetting = 0,
    /// Forces uppercase regardless of the device setting.
    Uppercase = 1,
    /// Shows the text exactly as sent.
    AsIs = 2,
}

/// When a custom app's lifetime counter resets (`lifetimeMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum LifetimeResetMode {
    /// Resets on each new message (default).
    #[default]
    OnMessage = 0,
    /// Resets each time the app is displayed.
    OnView = 1,
}

open_name_enum! {
    /// A built-in background animation (`effect`). The firmware lists the available names on
    /// `stats/effects` after startup.
    Effect {
        /// A brick-breaker game animation.
        BrickBreaker => "BrickBreaker",
        /// A colour-cycling checkerboard.
        Checkerboard => "Checkerboard",
        /// Colour waves sweeping across the matrix.
        ColorWaves => "ColorWaves",
        /// A colour fade cycle.
        Fade => "Fade",
        /// Fireworks.
        Fireworks => "Fireworks",
        /// A pair of eyes looking around.
        LookingEyes => "LookingEyes",
        /// Matrix-style falling code.
        Matrix => "Matrix",
        /// A coloured line sweeping across the matrix.
        MovingLine => "MovingLine",
        /// A gently rolling ocean surface.
        Pacifica => "Pacifica",
        /// A Pong game animation.
        PingPong => "PingPong",
        /// A plasma blob animation.
        Plasma => "Plasma",
        /// A softer, cloud-like plasma.
        PlasmaCloud => "PlasmaCloud",
        /// A radar sweep.
        Radar => "Radar",
        /// Concentric colour ripples.
        Ripple => "Ripple",
        /// A snake moving around the matrix.
        Snake => "Snake",
        /// Colours spiralling inward.
        SwirlIn => "SwirlIn",
        /// Colours spiralling outward.
        SwirlOut => "SwirlOut",
        /// A theatre-marquee chase.
        TheaterChase => "TheaterChase",
        /// Randomly twinkling stars.
        TwinklingStars => "TwinklingStars",
    }
}

open_name_enum! {
    /// A built-in 16-colour palette for `EffectSettings::palette`.
    EffectPalette {
        /// Soft white and grey tones.
        Cloud => "Cloud",
        /// Earthy green and brown tones.
        Forest => "Forest",
        /// Red, orange and yellow tones.
        Heat => "Heat",
        /// Deep red and orange tones.
        Lava => "Lava",
        /// Blue and teal tones.
        Ocean => "Ocean",
        /// Bright, saturated party colours.
        Party => "Party",
        /// Every hue of the rainbow.
        Rainbow => "Rainbow",
        /// Contrasting colour bands.
        Stripe => "Stripe",
    }
}

/// A native firmware app, for [`AppSwitch`](super::AppSwitch), whose `name` also takes a custom
/// app's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltInApp {
    /// The clock app.
    Time,
    /// The date app.
    Date,
    /// The temperature app.
    Temperature,
    /// The humidity app.
    Humidity,
    /// The battery app.
    Battery,
}

impl BuiltInApp {
    /// The name the firmware knows the app by.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Time => "Time",
            Self::Date => "Date",
            Self::Temperature => "Temperature",
            Self::Humidity => "Humidity",
            Self::Battery => "Battery",
        }
    }
}

/// The clock face of the native time app (`TMODE`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ClockMode {
    /// The time with a weekday bar at the bottom.
    #[default]
    WeekdayBar = 0,
    /// The time with a weekday bar and a calendar box for the day of the month; bar at the
    /// bottom.
    Calendar = 1,
    /// `Calendar` with the weekday bar at the top.
    CalendarTop = 2,
    /// The time with a weekday bar and an alternate calendar icon; bar at the bottom.
    CalendarAlt = 3,
    /// `CalendarAlt` with the weekday bar at the top.
    CalendarAltTop = 4,
    /// The time in a large font, optionally over a 32x8 animated GIF (`bigtime.gif`).
    BigTime = 5,
    /// Binary time: hours, minutes and seconds as three rows of six dots.
    Binary = 6,
}
