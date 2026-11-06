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

//! The awtrix-controller binary.

use std::io::{self, Write};
use std::process::ExitCode;

use awtrix_controller::run::{Options, run_with_options};
use service_kit::runtime::{build_runtime, run_until_exit};
use tokio_util::sync::CancellationToken;

/// The runtime could not be built.
const EXIT_STARTUP: u8 = 2;

fn main() -> ExitCode {
    // Built by hand rather than with #[tokio::main], so that the end of the shutdown budget also
    // bounds the runtime: dropping it would wait for every blocking task, a weather fetch still
    // in flight included. run_until_exit gives them what is left of the run's deadline.
    let runtime = match build_runtime(None) {
        Ok(runtime) => runtime,
        Err(runtime_error) => {
            let _ = writeln!(
                io::stderr(),
                "awtrix-controller: start the runtime: {runtime_error}"
            );
            return ExitCode::from(EXIT_STARTUP);
        }
    };

    let getenv = |name: &str| std::env::var(name).ok();
    run_until_exit(runtime, || {
        run_with_options(
            std::env::args_os().skip(1),
            &getenv,
            CancellationToken::new(),
            Options::default(),
        )
    })
}
