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

//! The status the device publishes on `{clientID}/stats`.

use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

/// The device status. The keys are the firmware's snake-case ones.
///
/// This is the one payload the controller decodes. Decode it with [`Stats::from_json`], which
/// reads it the way Go's `encoding/json` did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the firmware's payload carries four independent flags; the struct mirrors it"
)]
pub struct Stats {
    /// The app currently shown.
    #[serde(deserialize_with = "null_as_zero")]
    pub app: String,
    /// The battery level in percent.
    #[serde(deserialize_with = "null_as_zero")]
    pub bat: i64,
    /// The raw battery reading.
    #[serde(deserialize_with = "null_as_zero")]
    pub bat_raw: i64,
    /// The matrix brightness.
    #[serde(deserialize_with = "null_as_zero")]
    pub bri: i64,
    /// The relative humidity in percent.
    #[serde(deserialize_with = "null_as_zero")]
    pub hum: i64,
    /// Whether indicator 1 is on.
    #[serde(deserialize_with = "null_as_zero")]
    pub indicator1: bool,
    /// Whether indicator 2 is on.
    #[serde(deserialize_with = "null_as_zero")]
    pub indicator2: bool,
    /// Whether indicator 3 is on.
    #[serde(deserialize_with = "null_as_zero")]
    pub indicator3: bool,
    /// The device's IP address.
    #[serde(deserialize_with = "null_as_zero")]
    pub ip_address: String,
    /// The raw light-sensor reading.
    #[serde(deserialize_with = "null_as_zero")]
    pub ldr_raw: i64,
    /// The ambient light in lux.
    #[serde(deserialize_with = "null_as_zero")]
    pub lux: i64,
    /// Whether the matrix is on.
    #[serde(deserialize_with = "null_as_zero")]
    pub matrix: bool,
    /// The number of queued messages.
    #[serde(deserialize_with = "null_as_zero")]
    pub messages: i64,
    /// The free RAM in bytes.
    #[serde(deserialize_with = "null_as_zero")]
    pub ram: i64,
    /// The temperature.
    #[serde(deserialize_with = "null_as_zero")]
    pub temp: i64,
    /// The hardware type.
    #[serde(rename = "type", deserialize_with = "null_as_zero")]
    pub device_type: i64,
    /// The device's unique ID.
    #[serde(deserialize_with = "null_as_zero")]
    pub uid: String,
    /// The uptime in seconds.
    #[serde(deserialize_with = "null_as_zero")]
    pub uptime: i64,
    /// The firmware version.
    #[serde(deserialize_with = "null_as_zero")]
    pub version: String,
    /// The Wi-Fi signal strength in dBm.
    #[serde(deserialize_with = "null_as_zero")]
    pub wifi_signal: i64,
}

impl Stats {
    /// Decodes a stats payload the way Go's `encoding/json` did: a key matches its field
    /// case-insensitively, and when several keys match the same field the last one in the
    /// document wins; a missing key or a `null` value leaves the field at its zero value, an
    /// unknown key is ignored, a payload that is `null` as a whole decodes to all zero values,
    /// and a value of the wrong type fails the whole payload.
    ///
    /// # Errors
    ///
    /// The payload is not JSON, is neither an object nor `null`, or holds a value of the wrong
    /// type for its field.
    pub fn from_json(payload: &[u8]) -> Result<Self, serde_json::Error> {
        let Some(DocumentOrder(entries)) = serde_json::from_slice(payload)? else {
            return Ok(Self::default());
        };

        // Folding in document order, into a map that keeps the last value per key, gives Go's
        // "last matching key wins".
        let folded: Map<String, Value> = entries
            .into_iter()
            .map(|(key, field_value)| (key.to_lowercase(), field_value))
            .collect();

        serde_json::from_value(Value::Object(folded))
    }
}

/// A JSON object's entries in document order. `serde_json::Map` is sorted by key, which would
/// lose the order Go's decoder resolves duplicate keys by.
struct DocumentOrder(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for DocumentOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntriesVisitor;

        impl<'de> Visitor<'de> for EntriesVisitor {
            type Value = DocumentOrder;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut access: A,
            ) -> Result<DocumentOrder, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = access.next_entry::<String, Value>()? {
                    entries.push(entry);
                }
                Ok(DocumentOrder(entries))
            }
        }

        deserializer.deserialize_map(EntriesVisitor)
    }
}

/// Decodes a JSON `null` as the zero value, which is what Go's `encoding/json` does to a field it
/// cannot set to nil; serde would refuse it.
fn null_as_zero<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}
