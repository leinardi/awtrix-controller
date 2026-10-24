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

//! Helpers shared by the unit tests.

use std::io;
use std::sync::{Arc, Mutex, Once, PoisonError};

use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::MakeWriter;

use crate::config::LogFormat;

/// A writer that keeps everything written to it.
#[derive(Clone, Default)]
pub struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    /// Everything written so far.
    pub fn text(&self) -> String {
        let bytes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Installs a JSON subscriber writing here as the default on this thread until the guard is
    /// dropped. A current-thread tokio test sees the records of the tasks it spawns too.
    ///
    /// tracing caches each callsite's interest across all live dispatchers. A callsite another
    /// test thread registers while this subscriber is being installed can cache "never" from a
    /// snapshot without it, and this thread then silently loses the record. A global subscriber
    /// that is interested in everything is in every later snapshot, so no shared callsite caches
    /// "never"; the rebuild heals one cached before that global was installed.
    pub fn install(&self, level: LevelFilter) -> tracing::subscriber::DefaultGuard {
        static GLOBAL: Once = Once::new();
        GLOBAL.call_once(|| {
            // Fails only when a global default already exists, which then serves the same purpose.
            let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry());
        });
        let guard = tracing::subscriber::set_default(crate::logger::subscriber(
            level,
            LogFormat::Json,
            self.clone(),
        ));
        tracing::callsite::rebuild_interest_cache();
        guard
    }
}

impl io::Write for Captured {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for Captured {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}
