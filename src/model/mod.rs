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

//! The JSON payloads exchanged with the Awtrix3 LED matrix firmware over MQTT. The module holds
//! only type definitions and their wire format, no behaviour.
//!
//! The wire format is the Go implementation's, field for field and in the same order, with one
//! deliberate difference: `<`, `>` and `&` are written literally, where Go's `encoding/json`
//! escaped them as `\u003c`, `\u003e` and `\u0026` (both are valid JSON for the same string).
//!
//! Go's `omitempty` is mirrored per field: a string, number, boolean or list field is left out
//! when it holds its zero value (an open name such as an effect included, when empty), and a field Go held behind a pointer, so that its zero value can
//! be sent explicitly, is an `Option` left out when `None`. The fields Go wrote without
//! `omitempty` are always written: `Power::power`, `Sleep::sleep`, `AppSwitch::name`, and
//! `Settings::overlay`, as `null` when unset.
//!
//! Only [`Stats`] is decoded in production; it decodes the way Go's `encoding/json` did (missing
//! keys and `null` values leave the zero value, unknown keys are ignored). The other payloads
//! derive `Deserialize` so that the tests can round-trip them.

mod draw;
mod enums;
mod misc;
mod notification;
mod settings;
mod stats;
mod text;

#[cfg(test)]
mod tests;

pub use draw::DrawInstruction;
pub use enums::{
    BuiltInApp, ClockMode, DateFormat, Effect, EffectPalette, LifetimeResetMode, OpenName,
    OverlayEffect, PushIconBehavior, TextCaseMode, TimeFormat, TransitionEffect,
};
pub use misc::{AppSwitch, EffectSettings, Indicator, MoodLight, Power, Rtttl, Sleep, Sound};
pub use notification::{AppContent, CustomApp, Notification};
pub use settings::Settings;
pub use stats::Stats;
pub use text::{TextContent, TextFragment};

/// Reports whether a value is its type's zero value: the `skip_serializing_if` test that mirrors
/// Go's `omitempty` for strings, numbers, booleans and the enums whose zero value Go omitted.
fn is_zero<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
