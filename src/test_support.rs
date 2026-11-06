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

use tracing::dispatcher::DefaultGuard;
use tracing::level_filters::LevelFilter;
use tracing_setup::testing::Capture;

use crate::config::LogFormat;

/// Installs the JSON subscriber writing to `capture` as the default on this thread until the
/// guard is dropped, through rust-common's `set_default`, which never loses a record to tracing's
/// interest cache. A current-thread tokio test sees the records of the tasks it spawns too.
pub fn install(capture: &Capture, level: LevelFilter) -> DefaultGuard {
    tracing_setup::testing::set_default(&crate::logger::subscriber(
        level,
        LogFormat::Json,
        capture.clone(),
    ))
}
