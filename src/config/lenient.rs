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

//! Field decoders that read YAML the way the Go implementation's yaml.v3 did, where serde would
//! refuse it: a `null` leaves the zero value, a boolean also accepts YAML 1.1's `yes`/`no`/`on`/
//! `off`/`y`/`n`, an integer accepts a float (truncated toward zero) and the underscores and
//! base prefixes Go's `strconv.ParseInt` reads, a leading `0` included as octal.
//!
//! Each decoder reads a field that is absent as `None` too (with `#[serde(default)]`), so the
//! file layer can tell "not given" from any given value.
//!
//! One difference remains: serde cannot see whether a scalar was quoted, so a quoted number
//! (`port: "1883"`) is read as the number where yaml.v3 refused it.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, Unexpected, Visitor};

/// Decodes a value whose `null` means "the zero value": a string, a nested section.
pub(super) fn or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Decodes a list whose `null`, and whose `null` elements, mean the zero value.
pub(super) fn list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    let items = Option::<Vec<Option<T>>>::deserialize(deserializer)?.unwrap_or_default();
    Ok(items.into_iter().map(Option::unwrap_or_default).collect())
}

/// Decodes an optional integer.
pub(super) fn int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    deserializer.deserialize_any(IntVisitor)
}

/// Decodes an optional float.
pub(super) fn float<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    deserializer.deserialize_any(FloatVisitor)
}

/// Decodes an optional boolean.
pub(super) fn boolean<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    deserializer.deserialize_any(BoolVisitor)
}

/// A YAML number as yaml.v3 resolves a plain scalar.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Number {
    Int(i64),
    Float(f64),
}

/// Resolves a scalar the way yaml.v3 resolves a plain one that starts like a number:
/// underscores are dropped, then it is an integer in `strconv.ParseInt`'s base-0 syntax, or a
/// float in YAML's float syntax.
fn resolve_number(text: &str) -> Option<Number> {
    if !text.starts_with(|first: char| first.is_ascii_digit() || "+-.".contains(first)) {
        return None;
    }

    let plain: String = text.chars().filter(|character| *character != '_').collect();

    if let Some(integer) = parse_int_base0(&plain) {
        return Some(Number::Int(integer));
    }

    if is_yaml_float(&plain) {
        return plain.parse::<f64>().ok().map(Number::Float);
    }

    None
}

/// Go's `strconv.ParseInt(text, 0, 64)`: an optional sign, then `0x`, `0o`, `0b` or a leading
/// `0` (octal) selects the base, decimal otherwise.
fn parse_int_base0(text: &str) -> Option<i64> {
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };

    let lower = unsigned.to_ascii_lowercase();
    let (radix, digits) = if let Some(hex) = lower.strip_prefix("0x") {
        (16, hex)
    } else if let Some(octal) = lower.strip_prefix("0o") {
        (8, octal)
    } else if let Some(binary) = lower.strip_prefix("0b") {
        (2, binary)
    } else if lower.len() > 1 && lower.starts_with('0') {
        (8, &lower[1..])
    } else {
        (10, lower.as_str())
    };

    if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
        return None;
    }

    let magnitude = i128::from(u64::from_str_radix(digits, radix).ok()?);
    i64::try_from(if negative { -magnitude } else { magnitude }).ok()
}

/// yaml.v3's `^[-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?$`.
fn is_yaml_float(text: &str) -> bool {
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(index) => (&unsigned[..index], Some(&unsigned[index + 1..])),
        None => (unsigned, None),
    };

    let all_digits = |part: &str| part.chars().all(|digit| digit.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some(("", fraction)) => !fraction.is_empty() && all_digits(fraction),
        Some((whole, fraction)) => all_digits(whole) && all_digits(fraction),
        None => !mantissa.is_empty() && all_digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|exponent| {
        let digits = exponent.strip_prefix(['-', '+']).unwrap_or(exponent);
        !digits.is_empty() && all_digits(digits)
    });

    mantissa_ok && exponent_ok
}

/// Truncates a float toward zero into an integer, as yaml.v3 does for an integer field, when the
/// result fits.
fn truncate<E: de::Error>(value: f64) -> Result<i64, E> {
    // i64::MAX is not exactly representable; 2^63 is the first float above the range.
    const TWO_TO_THE_63: f64 = 9_223_372_036_854_775_808.0;

    if value.is_finite() && (-TWO_TO_THE_63..TWO_TO_THE_63).contains(&value) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "truncation toward zero is the point, and the range is checked above"
        )]
        Ok(value.trunc() as i64)
    } else {
        Err(E::invalid_value(Unexpected::Float(value), &"an integer"))
    }
}

struct IntVisitor;

impl<'de> Visitor<'de> for IntVisitor {
    type Value = Option<i64>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Some(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        i64::try_from(value)
            .map(Some)
            .map_err(|_| E::invalid_value(Unexpected::Unsigned(value), &self))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        truncate(value).map(Some)
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        match resolve_number(value) {
            Some(Number::Int(integer)) => Ok(Some(integer)),
            Some(Number::Float(float)) => truncate(float).map(Some),
            None => Err(E::invalid_type(Unexpected::Str(value), &self)),
        }
    }
}

struct FloatVisitor;

impl<'de> Visitor<'de> for FloatVisitor {
    type Value = Option<f64>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a number")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "yaml.v3 converts an integer to a float field the same way"
    )]
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Some(value as f64))
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "yaml.v3 converts an integer to a float field the same way"
    )]
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Some(value as f64))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(Some(value))
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "yaml.v3 converts an integer to a float field the same way"
    )]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        match resolve_number(value) {
            Some(Number::Int(integer)) => Ok(Some(integer as f64)),
            Some(Number::Float(float)) => Ok(Some(float)),
            None => Err(E::invalid_type(Unexpected::Str(value), &self)),
        }
    }
}

struct BoolVisitor;

impl<'de> Visitor<'de> for BoolVisitor {
    type Value = Option<bool>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a boolean")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Some(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        // yaml.v3's YAML 1.1 compatibility, for a field typed as a boolean.
        match value {
            "y" | "Y" | "yes" | "Yes" | "YES" | "on" | "On" | "ON" => Ok(Some(true)),
            "n" | "N" | "no" | "No" | "NO" | "off" | "Off" | "OFF" => Ok(Some(false)),
            _ => Err(E::invalid_type(Unexpected::Str(value), &self)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_numbers_like_yaml_v3() {
        let cases = [
            ("1_000", Some(Number::Int(1000))),
            ("010", Some(Number::Int(8))),
            ("0o10", Some(Number::Int(8))),
            ("0x10", Some(Number::Int(16))),
            ("-0x10", Some(Number::Int(-16))),
            ("0b11", Some(Number::Int(3))),
            ("+5", Some(Number::Int(5))),
            ("0", Some(Number::Int(0))),
            ("1_0.5", Some(Number::Float(10.5))),
            ("60.5", Some(Number::Float(60.5))),
            (".5", Some(Number::Float(0.5))),
            ("1.", Some(Number::Float(1.0))),
            ("1e3", Some(Number::Float(1000.0))),
            ("099", Some(Number::Float(99.0))),
            ("abc", None),
            ("1.2.3", None),
            ("", None),
            ("-", None),
            ("0x", None),
        ];
        for (text, want) in cases {
            assert_eq!(resolve_number(text), want, "{text:?}");
        }
    }
}
