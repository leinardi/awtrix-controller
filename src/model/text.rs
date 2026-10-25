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

//! The firmware's `text` field, which takes either a plain string or a list of coloured fragments.

use serde::{Deserialize, Serialize, Serializer};

/// One coloured segment of a fragmented text. The colour is a 6-digit hex string without the
/// leading `#` (e.g. "FF0000"), the firmware's convention for this field.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFragment {
    /// The text of the segment.
    #[serde(rename = "t")]
    pub text: String,
    /// The colour of the segment.
    #[serde(rename = "c")]
    pub color: String,
}

/// The value of the `text` field: a JSON string, or a JSON array of [`TextFragment`]s.
///
/// Decoding picks the variant from the JSON value. Encoding writes a fragment list with no
/// fragments as the empty string, as Go did, so the output stays byte for byte what Go sent.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum TextContent {
    /// A plain string.
    Plain(String),
    /// Coloured fragments.
    Fragments(Vec<TextFragment>),
}

impl TextContent {
    /// A plain-string text.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self::Plain(text.into())
    }

    /// A text made of coloured fragments.
    #[must_use]
    pub fn fragmented(fragments: Vec<TextFragment>) -> Self {
        Self::Fragments(fragments)
    }
}

impl Serialize for TextContent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Plain(text) => serializer.serialize_str(text),
            Self::Fragments(fragments) if fragments.is_empty() => serializer.serialize_str(""),
            Self::Fragments(fragments) => fragments.serialize(serializer),
        }
    }
}
