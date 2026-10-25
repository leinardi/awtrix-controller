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

//! Time zone names, looked up as Go's `time.LoadLocation` looked them up.

use jiff::tz::{TimeZone, TimeZoneDatabase};
use thiserror::Error;

/// A time zone name that names no time zone.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("unknown time zone {name}")]
pub struct UnknownTimeZone {
    name: String,
}

/// Looks up an IANA time zone name.
///
/// The lookup uses the database compiled into the binary, never the host's: the static image has
/// none, and a host copy may be trimmed (without the `backward` aliases such as `US/Eastern`), so
/// the same configuration resolves the same way everywhere.
///
/// As in Go, the name is case-sensitive (jiff on its own matches case-insensitively), `UTC` and
/// the empty name are UTC, and `Local` is the system time zone.
///
/// # Errors
///
/// The name is not in the database.
pub fn load_time_zone(name: &str) -> Result<TimeZone, UnknownTimeZone> {
    let unknown = || UnknownTimeZone {
        name: name.to_owned(),
    };

    match name {
        "" | "UTC" => return Ok(TimeZone::UTC),
        "Local" => return Ok(TimeZone::system()),
        _ => {}
    }

    let zone = TimeZoneDatabase::bundled()
        .get(name)
        .map_err(|_| unknown())?;
    if zone.iana_name() == Some(name) {
        Ok(zone)
    } else {
        Err(unknown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Go baseline's `time.LoadLocation` results for these names, captured with a throwaway
    /// program (`Local` and `UTC` are special-cased there too).
    #[test]
    fn loads_names_like_go() {
        for name in [
            "Europe/Berlin",
            "UTC",
            "",
            "Local",
            "EST5EDT",
            "Etc/GMT+1",
            "US/Eastern",
        ] {
            assert!(load_time_zone(name).is_ok(), "{name} rejected");
        }
        for name in ["europe/berlin", "Nowhere/City", "GMT+1", "utc"] {
            assert!(load_time_zone(name).is_err(), "{name:?} accepted");
        }
    }
}
