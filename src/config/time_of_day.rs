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

//! The two calendar formats the configuration uses, parsed exactly as Go's `time.Parse` parsed
//! them in the Go implementation.

use std::fmt;
use std::str::FromStr;

use thiserror::Error;

/// The number of days in each month of a leap year: Go's `time.Parse("01-02", ...)` parses into
/// year 0, which is a leap year, so `02-29` is a valid date.
const DAYS_IN_MONTH: [u8; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// A wall-clock time of day, HH:MM. The hour may have one digit (`7:00`), as Go's `15:04`
/// layout allows; the minute always has two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay {
    hour: u8,
    minute: u8,
}

/// A day of the year without a year, MM-DD with two digits each, as Go's `01-02` layout reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonthDay {
    month: u8,
    day: u8,
}

/// A value that is not a valid time of day or month-day.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("{value:?} is not a valid {expected}")]
pub struct ParseError {
    value: String,
    expected: &'static str,
}

impl TimeOfDay {
    /// The time of day at an hour (0–23) and minute (0–59), if both are in range.
    #[must_use]
    pub const fn new(hour: u8, minute: u8) -> Option<Self> {
        if hour < 24 && minute < 60 {
            Some(Self { hour, minute })
        } else {
            None
        }
    }

    /// The hour, 0–23.
    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// The minute, 0–59.
    #[must_use]
    pub const fn minute(self) -> u8 {
        self.minute
    }
}

impl FromStr for TimeOfDay {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || ParseError {
            value: value.to_owned(),
            expected: "time of day (HH:MM)",
        };

        let (hour, minute) = value.split_once(':').ok_or_else(invalid)?;
        let hour_ok = matches!(hour.len(), 1 | 2) && hour.bytes().all(|byte| byte.is_ascii_digit());
        let minute_ok = minute.len() == 2 && minute.bytes().all(|byte| byte.is_ascii_digit());
        if !hour_ok || !minute_ok {
            return Err(invalid());
        }

        let hour = hour.parse().map_err(|_| invalid())?;
        let minute = minute.parse().map_err(|_| invalid())?;
        Self::new(hour, minute).ok_or_else(invalid)
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl MonthDay {
    /// The month, 1–12.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    /// The day of the month, 1–31.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }
}

impl FromStr for MonthDay {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || ParseError {
            value: value.to_owned(),
            expected: "date (MM-DD)",
        };

        let (month, day) = value.split_once('-').ok_or_else(invalid)?;
        let two_digits =
            |part: &str| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit());
        if !two_digits(month) || !two_digits(day) {
            return Err(invalid());
        }

        let month: u8 = month.parse().map_err(|_| invalid())?;
        let day: u8 = day.parse().map_err(|_| invalid())?;
        let days_in_month = DAYS_IN_MONTH
            .get(usize::from(month).wrapping_sub(1))
            .ok_or_else(invalid)?;
        if day == 0 || day > *days_in_month {
            return Err(invalid());
        }

        Ok(Self { month, day })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Go baseline's `time.Parse("15:04", ...)` results, captured with a throwaway program.
    #[test]
    fn time_of_day_parses_like_go() {
        let valid = [
            ("7:00", 7, 0),
            ("07:00", 7, 0),
            ("23:59", 23, 59),
            ("0:00", 0, 0),
        ];
        for (text, hour, minute) in valid {
            let parsed: TimeOfDay = text.parse().expect(text);
            assert_eq!((parsed.hour(), parsed.minute()), (hour, minute), "{text}");
        }

        let invalid = [
            "7:5", "24:00", "007:00", " 7:00", "7:00 ", "-1:00", "+7:00", "07:60", "1:2:3",
            "７:00", "", ":", "07", "aa:bb",
        ];
        for text in invalid {
            assert!(text.parse::<TimeOfDay>().is_err(), "{text:?} parsed");
        }
    }

    /// The Go baseline's `time.Parse("01-02", ...)` results, captured with a throwaway program.
    #[test]
    fn month_day_parses_like_go() {
        for text in ["01-01", "02-29", "12-31", "04-30"] {
            assert!(text.parse::<MonthDay>().is_ok(), "{text} rejected");
        }

        for text in [
            "1-01",
            "02-30",
            "04-31",
            "13-01",
            "00-10",
            "01-00",
            "01-1",
            "01-01x",
            "not-a-date",
            "",
        ] {
            assert!(text.parse::<MonthDay>().is_err(), "{text:?} parsed");
        }
    }

    #[test]
    fn time_of_day_displays_with_two_digit_hour() {
        let parsed: TimeOfDay = "7:05".parse().expect("parse");
        assert_eq!(parsed.to_string(), "07:05");
    }
}
