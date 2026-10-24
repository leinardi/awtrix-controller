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

//! The drawing commands of the `draw` field. Each command is a one-key JSON object whose value is
//! a positional array: `{"dp":[x,y,color]}`, `{"dl":[x0,y0,x1,y1,color]}`, and so on.
//!
//! The commands are one enum of tuple variants, so serde's externally tagged representation is
//! the firmware's format as it stands: decoding refuses an unknown key, an object with more than
//! one key, a wrong argument count and a wrong argument type, as the Go decoder did.

use serde::{Deserialize, Serialize};

/// One drawing command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawInstruction {
    /// A single pixel: x, y, colour.
    #[serde(rename = "dp")]
    Pixel(i64, i64, String),
    /// A line between two points: x0, y0, x1, y1, colour.
    #[serde(rename = "dl")]
    Line(i64, i64, i64, i64, String),
    /// A rectangle outline: x, y, width, height, colour.
    #[serde(rename = "dr")]
    Rect(i64, i64, i64, i64, String),
    /// A filled rectangle: x, y, width, height, colour.
    #[serde(rename = "df")]
    FilledRect(i64, i64, i64, i64, String),
    /// A circle outline: x, y, radius, colour.
    #[serde(rename = "dc")]
    Circle(i64, i64, i64, String),
    /// A filled circle: x, y, radius, colour.
    #[serde(rename = "dfc")]
    FilledCircle(i64, i64, i64, String),
    /// Text: x, y, text, colour.
    #[serde(rename = "dt")]
    Text(i64, i64, String, String),
    /// A bitmap: x, y, width, height, and the pixels as RGB888 integers.
    #[serde(rename = "db")]
    Bitmap(i64, i64, i64, i64, Vec<i64>),
}
