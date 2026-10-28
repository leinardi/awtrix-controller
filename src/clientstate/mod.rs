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

//! The registry of connected devices: which are connected, their last stats and the app they
//! show.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tracing::warn;

use crate::model::Stats;
use crate::state::{self, StateError};

/// The last known state of one connected device.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientState {
    /// The last stats it published, `None` before the first. Replaced, never mutated, so a
    /// snapshot can share it.
    pub stats: Option<Arc<Stats>>,
    /// The app it last reported showing, empty before the first report.
    pub current_app: String,
}

/// The connected devices by client ID. Safe to share between threads.
#[derive(Debug, Default)]
pub struct Registry {
    clients: RwLock<HashMap<String, ClientState>>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `client_id` with an empty state, replacing any state it had.
    ///
    /// # Errors
    /// The registry lock is poisoned.
    pub fn register(&self, client_id: &str) -> Result<(), StateError> {
        self.write()?
            .insert(client_id.to_owned(), ClientState::default());
        Ok(())
    }

    /// Removes `client_id`; nothing happens when it is not registered.
    ///
    /// # Errors
    /// The registry lock is poisoned.
    pub fn unregister(&self, client_id: &str) -> Result<(), StateError> {
        self.write()?.remove(client_id);
        Ok(())
    }

    /// Stores the stats of `client_id`, or logs a warning when it is not registered.
    ///
    /// # Errors
    /// The registry lock is poisoned.
    pub fn update_stats(&self, client_id: &str, stats: Stats) -> Result<(), StateError> {
        let updated = if let Some(state) = self.write()?.get_mut(client_id) {
            state.stats = Some(Arc::new(stats));
            true
        } else {
            false
        };
        if !updated {
            warn!(
                client_id,
                "clientstate: update_stats for a client that is not registered"
            );
        }
        Ok(())
    }

    /// Stores the current app of `client_id`; nothing happens when it is not registered.
    ///
    /// # Errors
    /// The registry lock is poisoned.
    pub fn update_current_app(&self, client_id: &str, app: &str) -> Result<(), StateError> {
        if let Some(state) = self.write()?.get_mut(client_id) {
            app.clone_into(&mut state.current_app);
        }
        Ok(())
    }

    /// The registered client IDs, sorted.
    ///
    /// # Errors
    /// The registry lock is poisoned.
    pub fn connected_ids(&self) -> Result<Vec<String>, StateError> {
        let mut ids: Vec<String> = self.read()?.keys().cloned().collect();
        ids.sort();
        Ok(ids)
    }

    /// A copy of the state of `client_id`, or `None` when it is not registered.
    ///
    /// # Errors
    /// The registry lock is poisoned.
    pub fn snapshot(&self, client_id: &str) -> Result<Option<ClientState>, StateError> {
        Ok(self.read()?.get(client_id).cloned())
    }

    fn read(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, HashMap<String, ClientState>>, StateError> {
        state::checked(self.clients.read(), "client registry")
    }

    fn write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, ClientState>>, StateError> {
        state::checked(self.clients.write(), "client registry")
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use tests::poison;
