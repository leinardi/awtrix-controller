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

use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// The device status. The keys are the firmware's snake-case ones.
///
/// This is the one payload the controller decodes. Decode it with [`Stats::from_json`], which
/// reads it the way Go's `encoding/json` did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the firmware's payload carries four independent flags; the struct mirrors it"
)]
pub struct Stats {
    /// The app currently shown.
    pub app: String,
    /// The battery level in percent.
    pub bat: i64,
    /// The raw battery reading.
    pub bat_raw: i64,
    /// The matrix brightness.
    pub bri: i64,
    /// The relative humidity in percent.
    pub hum: i64,
    /// Whether indicator 1 is on.
    pub indicator1: bool,
    /// Whether indicator 2 is on.
    pub indicator2: bool,
    /// Whether indicator 3 is on.
    pub indicator3: bool,
    /// The device's IP address.
    pub ip_address: String,
    /// The raw light-sensor reading.
    pub ldr_raw: i64,
    /// The ambient light in lux.
    pub lux: i64,
    /// Whether the matrix is on.
    pub matrix: bool,
    /// The number of queued messages.
    pub messages: i64,
    /// The free RAM in bytes.
    pub ram: i64,
    /// The temperature.
    pub temp: i64,
    /// The hardware type.
    #[serde(rename = "type")]
    pub device_type: i64,
    /// The device's unique ID.
    pub uid: String,
    /// The uptime in seconds.
    pub uptime: i64,
    /// The firmware version.
    pub version: String,
    /// The Wi-Fi signal strength in dBm.
    pub wifi_signal: i64,
}

impl Stats {
    /// Decodes a stats payload the way Go's `encoding/json` did: a key matches its field
    /// case-insensitively, and when several keys match the same field the last one in the
    /// document wins; a missing key leaves zero and a `null` leaves the field unchanged, an
    /// unknown key is ignored, a payload that is `null` as a whole decodes to all zero values,
    /// and a value of the wrong type fails the whole payload.
    ///
    /// # Errors
    ///
    /// The payload is not JSON, is neither an object nor `null`, or holds a value of the wrong
    /// type for its field.
    pub fn from_json(payload: &[u8]) -> Result<Self, serde_json::Error> {
        let decoded: Option<DecodedStats> = serde_json::from_slice(payload)?;
        Ok(decoded.map_or_else(Self::default, |decoded| decoded.0))
    }
}

/// Decodes each occurrence before reading the next, so a later duplicate cannot hide a type
/// error and a null cannot overwrite an earlier scalar.
struct DecodedStats(Stats);

impl<'de> Deserialize<'de> for DecodedStats {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntriesVisitor;

        impl<'de> Visitor<'de> for EntriesVisitor {
            type Value = DecodedStats;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<DecodedStats, A::Error> {
                let mut stats = Stats::default();
                while let Some(key) = access.next_key::<String>()? {
                    match key.to_lowercase().as_str() {
                        "app" => replace_non_null(&mut access, &mut stats.app)?,
                        "bat" => replace_non_null(&mut access, &mut stats.bat)?,
                        "bat_raw" => replace_non_null(&mut access, &mut stats.bat_raw)?,
                        "bri" => replace_non_null(&mut access, &mut stats.bri)?,
                        "hum" => replace_non_null(&mut access, &mut stats.hum)?,
                        "indicator1" => replace_non_null(&mut access, &mut stats.indicator1)?,
                        "indicator2" => replace_non_null(&mut access, &mut stats.indicator2)?,
                        "indicator3" => replace_non_null(&mut access, &mut stats.indicator3)?,
                        "ip_address" => replace_non_null(&mut access, &mut stats.ip_address)?,
                        "ldr_raw" => replace_non_null(&mut access, &mut stats.ldr_raw)?,
                        "lux" => replace_non_null(&mut access, &mut stats.lux)?,
                        "matrix" => replace_non_null(&mut access, &mut stats.matrix)?,
                        "messages" => replace_non_null(&mut access, &mut stats.messages)?,
                        "ram" => replace_non_null(&mut access, &mut stats.ram)?,
                        "temp" => replace_non_null(&mut access, &mut stats.temp)?,
                        "type" => replace_non_null(&mut access, &mut stats.device_type)?,
                        "uid" => replace_non_null(&mut access, &mut stats.uid)?,
                        "uptime" => replace_non_null(&mut access, &mut stats.uptime)?,
                        "version" => replace_non_null(&mut access, &mut stats.version)?,
                        "wifi_signal" => replace_non_null(&mut access, &mut stats.wifi_signal)?,
                        _ => {
                            access.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(DecodedStats(stats))
            }
        }

        deserializer.deserialize_map(EntriesVisitor)
    }
}

fn replace_non_null<'de, A, T>(access: &mut A, field: &mut T) -> Result<(), A::Error>
where
    A: MapAccess<'de>,
    T: Deserialize<'de>,
{
    if let Some(value) = access.next_value::<Option<T>>()? {
        *field = value;
    }
    Ok(())
}
