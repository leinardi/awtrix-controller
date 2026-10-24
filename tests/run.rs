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

//! The whole controller, from the flags to the shutdown: the ports of
//! `cmd/awtrix-controller/run_test.go`, an end-to-end run with a device, and the shutdown budget.

#![expect(
    clippy::expect_used,
    reason = "an integration test crate: its helpers sit outside #[test] functions, where allow-expect-in-tests does not reach"
)]

use std::net::TcpListener;
use std::path::PathBuf;
use std::pin::pin;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use awtrix_controller::run::{Exit, Options, SHUTDOWN_BUDGET, run_with, run_with_options};
use awtrix_controller::weather::FetchFunction;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use tokio::sync::{Notify, mpsc};
use tokio::time::{Instant, timeout};
use tokio_util::sync::CancellationToken;

const USERNAME: &str = "testuser";
const PASSWORD: &str = "testpass";

/// How long a test waits for something that should happen.
const WAIT: Duration = Duration::from_secs(10);

/// How long the fake weather fetch hangs: far beyond the shutdown budget.
const HUNG_FETCH: Duration = Duration::from_secs(30);

/// How much later than the budget the run may end: scheduling slack.
const SLACK: Duration = Duration::from_millis(100);

fn no_env(_: &str) -> Option<String> {
    None
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("free port")
        .port()
}

/// A config file for `port`, removed when dropped.
struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(port: u16, weather: bool) -> Self {
        Self::with_extra(port, weather, "")
    }

    fn with_extra(port: u16, weather: bool, extra: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "awtrix-controller-run-{}-{port}.yaml",
            std::process::id()
        ));
        let content = format!(
            "mqtt:\n  username: {USERNAME}\n  password: {PASSWORD}\n  port: {port}\n\
             location:\n  latitude: 52.5\n  longitude: 13.4\ntimezone: UTC\n\
             weather:\n  enabled: {weather}\n{extra}"
        );
        std::fs::write(&path, content).expect("write config");
        Self(path)
    }

    fn path(&self) -> &str {
        self.0.to_str().expect("UTF-8 path")
    }
}

impl Drop for ConfigFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Runs the controller on a free port with `extra` flags while `driver` runs, then returns how
/// the run ended and what the driver returned. The driver cancels the token to stop the run. A
/// run that cannot bind its port (exit code 2) is retried on another.
async fn drive<Output, Driver, DriverFuture>(
    extra: &[&str],
    weather: bool,
    options: &Options,
    driver: Driver,
) -> (Exit, Output)
where
    Driver: Fn(u16, CancellationToken) -> DriverFuture,
    DriverFuture: Future<Output = Output>,
{
    for _ in 0..4 {
        let port = free_port();
        let config = ConfigFile::new(port, weather);
        let mut args = vec!["--config", config.path(), "--log-level", "error"];
        args.extend_from_slice(extra);
        let shutdown = CancellationToken::new();
        let mut run = pin!(run_with_options(
            args,
            &no_env,
            shutdown.clone(),
            options.clone()
        ));
        let mut driving = pin!(driver(port, shutdown));
        tokio::select! {
            exit = &mut run => {
                assert_eq!(exit.code, ExitCode::from(2), "the run ended on its own");
            }
            output = &mut driving => return (run.await, output),
        }
    }
    panic!("no free port could be bound");
}

/// A device: subscribes to `topics`, and forwards every message it receives.
async fn device(
    port: u16,
    client_id: &str,
    topics: &[&str],
) -> (AsyncClient, mpsc::UnboundedReceiver<(String, Vec<u8>)>) {
    let mut options = MqttOptions::new(client_id, "127.0.0.1", port);
    options.set_credentials(USERNAME, PASSWORD);
    options.set_keep_alive(Duration::from_secs(5));
    let (client, mut event_loop) = AsyncClient::new(options, 16);
    for topic in topics {
        client
            .subscribe(*topic, QoS::AtLeastOnce)
            .await
            .expect("subscribe");
    }
    let (messages, received) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            match event_loop.poll().await {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    let message = (publish.topic, publish.payload.to_vec());
                    if messages.send(message).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) if messages.is_closed() => break,
                // Not up yet, or gone: rumqttc reconnects on the next poll.
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    });
    (client, received)
}

/// The next message on `topic` whose payload contains `needle`.
async fn wait_for(
    received: &mut mpsc::UnboundedReceiver<(String, Vec<u8>)>,
    topic: &str,
    needle: &str,
) -> String {
    timeout(WAIT, async {
        loop {
            let (got, payload) = received.recv().await.expect("device running");
            let text = String::from_utf8(payload).expect("UTF-8 payload");
            if got == topic && text.contains(needle) {
                return text;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no message on {topic} containing {needle:?}"))
}

/// Go: `TestRunWithContext_TC10_GracefulShutdown`. Here the device sees its settings first, so
/// the shutdown runs with everything started.
#[tokio::test(flavor = "multi_thread")]
async fn graceful_shutdown_exits_zero() {
    let (exit, ()) = drive(
        &[],
        false,
        &Options::default(),
        |port, shutdown| async move {
            let (_client, mut received) = device(port, "dev1", &["dev1/settings"]).await;
            wait_for(&mut received, "dev1/settings", "CHCOL").await;
            shutdown.cancel();
        },
    )
    .await;
    assert_eq!(exit.code, ExitCode::SUCCESS);
    assert!(exit.deadline.is_some());
}

/// A shutdown requested before the run has started still ends it cleanly.
#[tokio::test(flavor = "multi_thread")]
async fn shutdown_requested_at_once_exits_zero() {
    let (exit, ()) = drive(&[], false, &Options::default(), |_, shutdown| async move {
        shutdown.cancel();
    })
    .await;
    assert_eq!(exit.code, ExitCode::SUCCESS);
}

/// Go: `TestRunWithContext_TC18_MissingConfig`
#[tokio::test]
async fn missing_config_exits_one() {
    let code = run_with(
        [
            "--config",
            "/nonexistent/awtrix-controller/config.yaml",
            "--log-level",
            "error",
        ],
        &no_env,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(code, ExitCode::from(1));
}

/// `AWTRIX_CONFIG` is read when `--config` is not given: the run starts with that file (the
/// default path does not exist here) and ends cleanly on the shutdown already requested.
#[tokio::test(flavor = "multi_thread")]
async fn config_path_from_the_environment() {
    for _ in 0..4 {
        let config = ConfigFile::new(free_port(), false);
        let config_path = config.path().to_owned();
        let getenv = move |name: &str| (name == "AWTRIX_CONFIG").then(|| config_path.clone());
        let shutdown = CancellationToken::new();
        shutdown.cancel();
        let code = run_with(["--log-level", "error"], &getenv, shutdown).await;
        if code != ExitCode::from(2) {
            assert_eq!(code, ExitCode::SUCCESS);
            return;
        }
    }
    panic!("no free port could be bound");
}

/// The warnings the configuration load logs (a skipped scheduled notification) reach the log:
/// the logger is up before the file is loaded. Run as a process, so the real global logger
/// writes to its standard output; a port already taken then ends it with exit code 2.
#[test]
fn configuration_warnings_reach_the_log() {
    let taken = TcpListener::bind("0.0.0.0:0").expect("bind");
    let port = taken.local_addr().expect("address").port();
    let config = ConfigFile::with_extra(
        port,
        false,
        "scheduled_notifications:\n  - name: typo\n    message: hi\n    repeat: dayly\n",
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_awtrix-controller"))
        .args(["--config", config.path(), "--log-format", "json"])
        .output()
        .expect("run the binary");
    let log = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(2), "{log}");
    assert!(
        log.contains("skipping scheduled_notification: unknown repeat")
            && log.contains(r#""repeat":"dayly""#),
        "{log}"
    );
    drop(taken);
}

/// `--version` and `--help` print and succeed; a bad flag fails with 1, as Go's parse errors
/// did.
#[tokio::test]
async fn flags_that_end_the_run_at_once() {
    for (args, expected) in [
        (vec!["--version"], ExitCode::SUCCESS),
        (vec!["-v"], ExitCode::SUCCESS),
        (vec!["--help"], ExitCode::SUCCESS),
        (vec!["--unknown"], ExitCode::from(1)),
        (vec!["-config", "x.yaml"], ExitCode::from(1)),
        (vec!["--log-format", "xml"], ExitCode::from(1)),
    ] {
        let code = run_with(args.clone(), &no_env, CancellationToken::new()).await;
        assert_eq!(code, expected, "{args:?}");
    }
}

/// A device gets its settings with the simulated weather overlay, and the test notification once
/// it reports its stats.
#[tokio::test(flavor = "multi_thread")]
async fn device_gets_settings_overlay_and_test_notification() {
    let extra = [
        "--weather-wmo",
        "95",
        "--test-notification-text",
        "hello from the test",
    ];
    let (exit, ()) = drive(
        &extra,
        true,
        &Options::default(),
        |port, shutdown| async move {
            let (client, mut received) =
                device(port, "dev1", &["dev1/settings", "dev1/notify"]).await;
            wait_for(&mut received, "dev1/settings", r#""OVERLAY":"thunder""#).await;
            client
                .publish("dev1/stats", QoS::AtLeastOnce, false, r#"{"bat":90}"#)
                .await
                .expect("publish stats");
            wait_for(&mut received, "dev1/notify", "hello from the test").await;
            shutdown.cancel();
        },
    )
    .await;
    assert_eq!(exit.code, ExitCode::SUCCESS);
}

/// The shutdown budget holds while a weather fetch hangs and a device is connected: from the
/// cancel to the end of the runtime, as `main` ends it, takes at most the budget.
#[test]
fn shutdown_within_the_budget_while_a_fetch_hangs() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let fetch_started = Arc::new(Notify::new());
    let fetching = Arc::clone(&fetch_started);
    // Ignores the cancellation and blocks a thread, the worst a fetch can do.
    let fetch: FetchFunction = Arc::new(move |_, _, _, _| {
        let fetching = Arc::clone(&fetching);
        Box::pin(async move {
            fetching.notify_one();
            let _ = tokio::task::spawn_blocking(|| std::thread::sleep(HUNG_FETCH)).await;
            Ok(Vec::new())
        })
    });
    let options = Options { fetch: Some(fetch) };

    let (exit, cancelled_at) = runtime.block_on(drive(&[], true, &options, |port, shutdown| {
        let fetch_started = Arc::clone(&fetch_started);
        async move {
            let (_client, mut received) = device(port, "dev1", &["dev1/settings"]).await;
            wait_for(&mut received, "dev1/settings", "CHCOL").await;
            timeout(WAIT, fetch_started.notified())
                .await
                .expect("the fetch started");
            let cancelled_at = Instant::now();
            shutdown.cancel();
            cancelled_at
        }
    }));
    let remaining = exit
        .deadline
        .expect("shut down")
        .saturating_duration_since(Instant::now());
    runtime.shutdown_timeout(remaining);
    let elapsed = cancelled_at.elapsed();

    assert_eq!(exit.code, ExitCode::SUCCESS);
    assert!(
        elapsed <= SHUTDOWN_BUDGET + SLACK,
        "shutdown took {elapsed:?}"
    );
}
