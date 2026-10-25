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

//! Notifications (`{clientID}/notify`) and custom apps (`{clientID}/custom/{appName}`), which
//! share their display fields through [`AppContent`].

use serde::{Deserialize, Serialize};

use super::enums::is_unset_name;
use super::{
    DrawInstruction, Effect, EffectSettings, LifetimeResetMode, OverlayEffect, PushIconBehavior,
    TextCaseMode, TextContent, is_zero,
};

/// The display fields shared by [`Notification`] and [`CustomApp`], flattened into both. Every
/// field is optional; an omitted one leaves the device default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppContent {
    /// The text shown on the matrix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextContent>,
    /// The capitalisation of the text.
    #[serde(skip_serializing_if = "is_zero")]
    pub text_case: TextCaseMode,
    /// Draws the text at the top of the matrix.
    #[serde(skip_serializing_if = "is_zero")]
    pub top_text: bool,
    /// The horizontal text offset in pixels.
    #[serde(skip_serializing_if = "is_zero")]
    pub text_offset: i64,
    /// Centres the text; `Some(false)` sends the non-default explicitly (device default: true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<bool>,
    /// The text colour, e.g. "#FF0000".
    #[serde(skip_serializing_if = "is_zero")]
    pub color: String,
    /// Hex colours for a gradient text effect.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gradient: Vec<String>,
    /// The blink interval in milliseconds; 0 for none.
    #[serde(skip_serializing_if = "is_zero")]
    pub blink_text: i64,
    /// The fade interval in milliseconds; 0 for none.
    #[serde(skip_serializing_if = "is_zero")]
    pub fade_text: i64,
    /// The background colour.
    #[serde(skip_serializing_if = "is_zero")]
    pub background: String,
    /// Cycles rainbow colours over the text.
    #[serde(skip_serializing_if = "is_zero")]
    pub rainbow: bool,
    /// The `LaMetric` icon ID shown beside the text.
    #[serde(skip_serializing_if = "is_zero")]
    pub icon: String,
    /// How the icon moves relative to scrolling text.
    #[serde(skip_serializing_if = "is_zero")]
    pub push_icon: PushIconBehavior,
    /// How many times the content scrolls; `Some(0)` sends "once" explicitly, -1 is infinite
    /// (device default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<i64>,
    /// How long the content is shown, in seconds.
    #[serde(skip_serializing_if = "is_zero")]
    pub duration: i64,
    /// Values drawn as a bar chart.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bar: Vec<i64>,
    /// Values drawn as a line chart.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub line: Vec<i64>,
    /// Scales chart values to the matrix; `Some(false)` sends the non-default explicitly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autoscale: Option<bool>,
    /// The background colour of the bar chart bars.
    #[serde(rename = "barBC", skip_serializing_if = "is_zero")]
    pub bar_bc: String,
    /// A progress bar value, 0 to 100; `Some(0)` sends an empty bar explicitly, -1 hides it
    /// (device default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<i64>,
    /// The progress bar foreground colour.
    #[serde(rename = "progressC", skip_serializing_if = "is_zero")]
    pub progress_c: String,
    /// The progress bar background colour.
    #[serde(rename = "progressBC", skip_serializing_if = "is_zero")]
    pub progress_bc: String,
    /// Drawing commands rendered on the matrix.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub draw: Vec<DrawInstruction>,
    /// Shows the text statically instead of scrolling it.
    #[serde(skip_serializing_if = "is_zero")]
    pub no_scroll: bool,
    /// The scroll speed in pixels per frame.
    #[serde(skip_serializing_if = "is_zero")]
    pub scroll_speed: i64,
    /// The background animation behind the text.
    #[serde(skip_serializing_if = "is_unset_name")]
    pub effect: Option<Effect>,
    /// The background animation's speed, palette and blending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_settings: Option<EffectSettings>,
    /// A weather overlay for this content.
    #[serde(skip_serializing_if = "is_unset_name")]
    pub overlay: Option<OverlayEffect>,
}

/// The payload of `{clientID}/notify`: a transient message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Notification {
    /// The shared display fields, written first.
    #[serde(flatten)]
    pub content: AppContent,
    /// Keeps the notification until it is dismissed.
    #[serde(skip_serializing_if = "is_zero")]
    pub hold: bool,
    /// A sound file on the device to play.
    #[serde(skip_serializing_if = "is_zero")]
    pub sound: String,
    /// An RTTTL melody to play.
    #[serde(skip_serializing_if = "is_zero")]
    pub rtttl: String,
    /// Loops the sound while the notification is shown.
    #[serde(skip_serializing_if = "is_zero")]
    pub loop_sound: bool,
    /// Queues behind the current notification; `Some(false)` sends "don't stack" explicitly
    /// (device default: true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<bool>,
    /// Wakes the display from sleep first.
    #[serde(skip_serializing_if = "is_zero")]
    pub wakeup: bool,
    /// Restricts delivery to these client IDs; empty for all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clients: Vec<String>,
}

/// The payload of `{clientID}/custom/{appName}`: a persistent app in the device's rotation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomApp {
    /// The shared display fields, written first.
    #[serde(flatten)]
    pub content: AppContent,
    /// The position in the rotation.
    #[serde(skip_serializing_if = "is_zero")]
    pub pos: i64,
    /// How long the app lives, in seconds; 0 for ever.
    #[serde(skip_serializing_if = "is_zero")]
    pub lifetime: i64,
    /// When the lifetime counter resets.
    #[serde(skip_serializing_if = "is_zero")]
    pub lifetime_mode: LifetimeResetMode,
    /// Keeps the app across device reboots.
    #[serde(skip_serializing_if = "is_zero")]
    pub save: bool,
}
