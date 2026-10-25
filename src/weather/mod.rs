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

//! The Open-Meteo forecast: fetching the `minutely_15` data, mapping WMO weather codes to display
//! overlays, detecting severe-weather events, the notification state of each event type, and the
//! controller that polls and publishes them.

mod controller;
mod events;
mod fetch;
mod mapping;
mod simulate;
mod state;

pub use controller::{Controller, FetchFunction, FetchFuture, PublishFuture, Publisher};
pub use events::{EventCandidate, Severity, detect_events};
pub use fetch::{DEFAULT_BASE_URL, FetchError, Fetcher, ForecastPoint};
pub use mapping::{EventType, select_overlay, wmo_to_overlay};
pub use simulate::simulate_fetch;
pub use state::{EventState, StateManager};
