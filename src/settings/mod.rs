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

//! The settings payload the controller manages on each device: the day or night theme colours,
//! the energy-saving brightness, and the weather overlay, pushed retained to
//! `{client_id}/settings`.

use std::sync::{Arc, PoisonError, RwLock};

use tracing::debug;

use crate::broker::{BrokerError, BrokerHandle};
use crate::config::ThemeConfig;
use crate::daynight::{Mode, ModeProvider};
use crate::energysaving::EnergySaving;
use crate::model::{OverlayEffect, Settings};

/// The brightness while energy saving is active.
const ENERGY_SAVING_BRIGHTNESS: i64 = 1;

/// Whether energy saving is active, for the settings builder.
pub trait EnergySavingProvider: Send + Sync {
    /// Whether energy saving is active now.
    fn is_active(&self) -> bool;
}

impl EnergySavingProvider for EnergySaving {
    fn is_active(&self) -> bool {
        Self::is_active(self)
    }
}

/// Composes the settings payload from the current mode, energy-saving state and overlay, read
/// when [`build`](Self::build) is called.
pub struct Builder {
    theme: ThemeConfig,
    mode: Arc<dyn ModeProvider>,
    energy_saving: Arc<dyn EnergySavingProvider>,
    overlay: RwLock<Option<OverlayEffect>>,
}

impl Builder {
    /// A builder reading the given providers.
    #[must_use]
    pub fn new(
        theme: ThemeConfig,
        mode: Arc<dyn ModeProvider>,
        energy_saving: Arc<dyn EnergySavingProvider>,
    ) -> Self {
        Self {
            theme,
            mode,
            energy_saving,
            overlay: RwLock::new(None),
        }
    }

    /// Sets the overlay of the next payloads; `None` clears it (the payload then carries
    /// `"OVERLAY": null`).
    pub fn set_overlay(&self, overlay: Option<OverlayEffect>) {
        *self.overlay.write().unwrap_or_else(PoisonError::into_inner) = overlay;
    }

    /// The payload: the theme's calendar accent as `CHCOL` and `WDCA`, its content colour as
    /// `CBCOL`, `WDCI`, `TIME_COL` and `DATE_COL`, the overlay, and `BRI` 1 with `ABRI` false
    /// while energy saving is active, `ABRI` true otherwise.
    #[must_use]
    pub fn build(&self) -> Settings {
        let colors = match self.mode.current_mode() {
            Mode::Day => &self.theme.day,
            Mode::Night => &self.theme.night,
        };
        let overlay = self
            .overlay
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let energy_saving = self.energy_saving.is_active();

        Settings {
            ch_col: colors.calendar_accent.clone(),
            cb_col: colors.content.clone(),
            wdca: colors.calendar_accent.clone(),
            wdci: colors.content.clone(),
            time_col: colors.content.clone(),
            date_col: colors.content.clone(),
            overlay,
            bri: energy_saving.then_some(ENERGY_SAVING_BRIGHTNESS),
            abri: Some(!energy_saving),
            ..Settings::default()
        }
    }
}

/// The topic a device reads its settings from.
#[must_use]
pub fn topic(client_id: &str) -> String {
    format!("{client_id}/settings")
}

/// Publishes `settings` to `{client_id}/settings`, retained at `QoS` 1, so a device that
/// connects later still gets them.
///
/// # Errors
///
/// The payload cannot be serialized or the broker cannot publish it.
pub async fn push(
    client_id: &str,
    broker: &BrokerHandle,
    settings: &Settings,
) -> Result<(), PushError> {
    let payload = serde_json::to_vec(settings).map_err(|source| PushError::Serialize {
        client_id: client_id.to_owned(),
        source,
    })?;
    let topic = topic(client_id);
    debug!(
        client_id,
        topic,
        payload = %String::from_utf8_lossy(&payload),
        "settings: pushing to client"
    );
    broker
        .publish(&topic, payload, true)
        .await
        .map_err(|source| PushError::Publish { topic, source })
}

/// A settings push that failed.
#[derive(Debug, thiserror::Error)]
pub enum PushError {
    /// The payload could not be serialized.
    #[error("settings: marshal payload for {client_id}: {source}")]
    Serialize {
        /// The device.
        client_id: String,
        /// Why.
        #[source]
        source: serde_json::Error,
    },
    /// The broker could not publish it.
    #[error("settings: publish to {topic}: {source}")]
    Publish {
        /// The topic.
        topic: String,
        /// Why.
        #[source]
        source: BrokerError,
    },
}

#[cfg(test)]
mod tests;
