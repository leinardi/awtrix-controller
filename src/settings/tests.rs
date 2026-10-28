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

use std::thread;

use serde_json::{Value, json};

use super::*;
use crate::config::ThemeColors;

struct FixedMode(Mode);

impl ModeProvider for FixedMode {
    fn current_mode(&self) -> Mode {
        self.0
    }
}

struct FixedEnergySaving(bool);

impl EnergySavingProvider for FixedEnergySaving {
    fn is_active(&self) -> bool {
        self.0
    }
}

fn theme() -> ThemeConfig {
    ThemeConfig {
        day: ThemeColors {
            calendar_accent: "#FFFFFF".to_owned(),
            content: "#AAAAAA".to_owned(),
        },
        night: ThemeColors {
            calendar_accent: "#000000".to_owned(),
            content: "#555555".to_owned(),
        },
    }
}

fn builder(mode: Mode, energy_saving: bool) -> Builder {
    Builder::new(
        theme(),
        Arc::new(FixedMode(mode)),
        Arc::new(FixedEnergySaving(energy_saving)),
    )
}

/// The payload as JSON, as the device receives it.
fn payload(builder: &Builder) -> Value {
    serde_json::to_value(builder.build().expect("healthy overlay")).expect("JSON")
}

/// Go: `TestBuildDayEnergyInactive`
#[test]
fn day_energy_inactive() {
    let payload = payload(&builder(Mode::Day, false));
    assert_eq!(payload["CHCOL"], "#FFFFFF");
    assert_eq!(payload["WDCA"], "#FFFFFF");
    assert_eq!(payload["CBCOL"], "#AAAAAA");
    assert_eq!(payload["ABRI"], true);
    assert!(payload.get("BRI").is_none(), "{payload}");
}

/// Go: `TestBuildNightEnergyInactive`
#[test]
fn night_energy_inactive() {
    let payload = payload(&builder(Mode::Night, false));
    assert_eq!(payload["CHCOL"], "#000000");
    assert_eq!(payload["CBCOL"], "#555555");
    assert_eq!(payload["ABRI"], true);
    assert!(payload.get("BRI").is_none(), "{payload}");
}

/// Go: `TestBuildDayEnergyActive`
#[test]
fn day_energy_active() {
    let payload = payload(&builder(Mode::Day, true));
    assert_eq!(payload["BRI"], 1);
    assert_eq!(payload["ABRI"], false);
    assert_eq!(payload["CHCOL"], "#FFFFFF");
}

/// Go: `TestBuildNightEnergyActive`
#[test]
fn night_energy_active() {
    let payload = payload(&builder(Mode::Night, true));
    assert_eq!(payload["CHCOL"], "#000000");
    assert_eq!(payload["BRI"], 1);
    assert_eq!(payload["ABRI"], false);
}

/// Go: `TestBuildOverlayPresent`
#[test]
fn overlay_present() {
    let builder = builder(Mode::Day, false);
    builder
        .set_overlay(Some(OverlayEffect::Snow))
        .expect("healthy overlay");
    assert_eq!(payload(&builder)["OVERLAY"], "snow");
}

/// Go: `TestBuildOverlayNullByDefault`
#[test]
fn overlay_null_by_default() {
    let payload = payload(&builder(Mode::Day, false));
    assert_eq!(payload.get("OVERLAY"), Some(&Value::Null));
}

/// Go: `TestBuildOverlayClearedAfterReset`
#[test]
fn overlay_cleared_after_reset() {
    let builder = builder(Mode::Day, false);
    builder
        .set_overlay(Some(OverlayEffect::Snow))
        .expect("healthy overlay");
    builder.set_overlay(None).expect("healthy overlay");
    assert_eq!(payload(&builder).get("OVERLAY"), Some(&Value::Null));
}

/// Go: `TestBuildOverlayConcurrentAccess`
#[test]
fn overlay_concurrent_access() {
    let builder = Arc::new(builder(Mode::Day, false));
    let writer = Arc::clone(&builder);
    let handle = thread::spawn(move || {
        for _ in 0..50 {
            writer
                .set_overlay(Some(OverlayEffect::Snow))
                .expect("healthy overlay");
            writer.set_overlay(None).expect("healthy overlay");
        }
    });
    for _ in 0..50 {
        let _ = builder.build().expect("healthy overlay");
    }
    handle.join().expect("writer");
}

/// Go: `TestBuildAllColorFieldsPopulated`. The whole payload, which is exactly what Go sent.
#[test]
fn whole_payload() {
    assert_eq!(
        payload(&builder(Mode::Day, false)),
        json!({
            "CHCOL": "#FFFFFF",
            "WDCA": "#FFFFFF",
            "CBCOL": "#AAAAAA",
            "WDCI": "#AAAAAA",
            "TIME_COL": "#AAAAAA",
            "DATE_COL": "#AAAAAA",
            "ABRI": true,
            "OVERLAY": null,
        })
    );
}

#[test]
fn settings_topic() {
    assert_eq!(topic("awtrix_abc"), "awtrix_abc/settings");
}

#[test]
fn poisoned_overlay_cannot_be_published_or_replaced() {
    let builder = Arc::new(builder(Mode::Day, false));
    let writer = Arc::clone(&builder);
    assert!(
        thread::spawn(move || {
            let _guard = writer.overlay.write().expect("healthy lock");
            panic!("simulate interrupted overlay update");
        })
        .join()
        .is_err()
    );
    let error = StateError::Poisoned("settings overlay");
    assert_eq!(builder.build(), Err(error));
    assert_eq!(builder.set_overlay(None), Err(error));
}
