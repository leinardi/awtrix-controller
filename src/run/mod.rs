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

//! The wiring: parses the flags, loads the configuration, starts the broker and every controller,
//! and shuts them all down on SIGINT, SIGTERM or a cancelled token.

use std::ffi::OsString;
use std::future;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use jiff::tz::TimeZone;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info, warn};

use crate::broker::{Broker, BrokerConfig, BrokerError, BrokerHandle, Callbacks, ClientCallback};
use crate::cli::{Cli, single_dash_long_form};
use crate::clientstate::Registry;
use crate::clock::{Clock, SystemClock};
use crate::config::{self, Config, DEFAULT_CONFIG_PATH, LogFormat};
use crate::daynight::{DayNight, OnModeChange};
use crate::energysaving::{EnergySaving, OnActiveChange};
use crate::logger;
use crate::model::{Notification, OverlayEffect};
use crate::notification::{ConnectedIds, Publish, ScheduledNotifier};
use crate::scheduler::Scheduler;
use crate::settings::{self, Builder};
use crate::version::{COMMIT, DATE, VERSION};
use crate::weather::{self, FetchFunction, PublishFuture, simulate_fetch};

/// The time from a shutdown signal to the process exiting: the broker, the controllers and the
/// runtime all have to be done by then.
pub const SHUTDOWN_BUDGET: Duration = Duration::from_secs(2);

/// A configuration that cannot be read, or a bad flag.
const EXIT_CONFIG: u8 = 1;
/// A component that cannot start, such as a port already in use.
const EXIT_STARTUP: u8 = 2;

/// Reads an environment variable; `None` when unset.
pub type Getenv<'getenv> = &'getenv dyn Fn(&str) -> Option<String>;

/// Test seams; production uses the default.
#[derive(Clone, Default)]
pub struct Options {
    /// Replaces the Open-Meteo fetch.
    pub fetch: Option<FetchFunction>,
}

/// How a run ended.
#[derive(Debug)]
pub struct Exit {
    /// The process exit code: 0 after a clean shutdown, 1 for a configuration or flag error, 2
    /// when a component cannot start.
    pub code: ExitCode,
    /// When the shutdown started, plus [`SHUTDOWN_BUDGET`]: the time by which the caller must
    /// have ended its runtime too. `None` when the run never got that far.
    pub deadline: Option<Instant>,
}

impl Exit {
    fn early(code: u8) -> Self {
        Self {
            code: ExitCode::from(code),
            deadline: None,
        }
    }
}

/// Runs the controller with `args` (the program name excluded) until SIGINT, SIGTERM or
/// `shutdown`, and returns the exit code.
pub async fn run_with<Args, Arg>(
    args: Args,
    getenv: Getenv<'_>,
    shutdown: CancellationToken,
) -> ExitCode
where
    Args: IntoIterator<Item = Arg>,
    Arg: Into<OsString>,
{
    run_with_options(args, getenv, shutdown, Options::default())
        .await
        .code
}

/// [`run_with`], with test seams, also returning the shutdown deadline.
pub async fn run_with_options<Args, Arg>(
    args: Args,
    getenv: Getenv<'_>,
    shutdown: CancellationToken,
    options: Options,
) -> Exit
where
    Args: IntoIterator<Item = Arg>,
    Arg: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if let Some((given, meant)) = single_dash_long_form(&args) {
        let _ = writeln!(
            io::stderr(),
            "error: {given}: the single-dash long form is not accepted; use {meant}"
        );
        return Exit::early(EXIT_CONFIG);
    }
    let program = OsString::from("awtrix-controller");
    let parsed = Cli::try_parse_from(std::iter::once(program).chain(args));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(parse_error) => {
            // Help goes to stdout and is a success; an error goes to stderr. A failed write
            // leaves nowhere to report it.
            let _ = parse_error.print();
            return Exit::early(if parse_error.use_stderr() {
                EXIT_CONFIG
            } else {
                0
            });
        }
    };

    if cli.version == Some(true) {
        let _ = writeln!(io::stdout(), "{VERSION} {COMMIT} {DATE}");
        return Exit::early(0);
    }

    let config_path = cli
        .config
        .clone()
        .or_else(|| non_empty(getenv("AWTRIX_CONFIG")).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let level = logger::effective_level(
        cli.log_level.as_deref(),
        getenv("AWTRIX_LOG_LEVEL").as_deref(),
    );
    let test_notification = cli.test_notification.notification().map(Arc::new);

    // The format can come from the file, and loading the file logs the entries it skips, so the
    // logger starts first with the format alone; a file that cannot be read is reported in the
    // flag's format, or text.
    let file_format = config::peek_log_format(&config_path).unwrap_or_default();
    init_logger(level, logger::effective_format(cli.log_format, file_format));

    if test_notification.is_some() {
        warn!("test-notification mode active — sending test notification on device ready");
    }

    let config = match config::load(&config_path) {
        Ok(config) => config,
        Err(load_error) => {
            error!(
                path = %config_path.display(),
                error = %load_error,
                "failed to load configuration"
            );
            return Exit::early(EXIT_CONFIG);
        }
    };
    debug!(config = ?config.debug_view(), "config: effective configuration loaded");

    let Some(zone) = time_zone(&config) else {
        return Exit::early(EXIT_CONFIG);
    };

    start(
        &config,
        zone,
        cli.weather_wmo,
        test_notification,
        shutdown,
        options,
    )
    .await
}

/// Starts every component, waits for the stop and shuts them down.
async fn start(
    config: &Config,
    zone: TimeZone,
    weather_wmo: Option<i64>,
    test_notification: Option<Arc<Notification>>,
    shutdown: CancellationToken,
    options: Options,
) -> Exit {
    // Cancelling it stops the scheduler and abandons an in-flight weather fetch; the guard
    // does so on every early return too.
    let components = CancellationToken::new();
    let _cancel_components = components.clone().drop_guard();

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let registry = Arc::new(Registry::new());
    let scheduler = Scheduler::new(Arc::clone(&clock), &components);
    // Everything that reaches the devices goes through one queue, served once the broker is up.
    let (events, mut queued) = mpsc::unbounded_channel();

    let Some(builder) = start_modes(config, &zone, &clock, &scheduler, &events) else {
        return Exit::early(EXIT_STARTUP);
    };
    let broker = match start_broker(config, &registry, &events).await {
        Ok(broker) => broker,
        Err(broker_error) => {
            error!(error = %broker_error, "failed to create MQTT broker");
            return Exit::early(EXIT_STARTUP);
        }
    };
    let devices = Arc::new(Devices {
        registry,
        broker: broker.handle(),
        builder,
    });
    start_notifications(config, zone, &clock, &scheduler, &devices);
    let weather = config.weather.enabled.then(|| {
        let fetch = options.fetch.or_else(|| simulation(weather_wmo));
        let publisher: Arc<dyn weather::Publisher> = Arc::clone(&devices) as _;
        let controller = match fetch {
            Some(fetch) => weather::Controller::with_fetch_function(
                config.weather.clone(),
                &config.location,
                &config.timezone,
                Arc::clone(&clock),
                fetch,
                publisher,
                &components,
            ),
            None => weather::Controller::new(
                config.weather.clone(),
                &config.location,
                &config.timezone,
                Arc::clone(&clock),
                publisher,
                &components,
            ),
        };
        controller.start(&scheduler);
        controller
    });

    info!(
        version = VERSION,
        commit = COMMIT,
        date = DATE,
        mqtt_port = config.mqtt.port,
        "awtrix-controller started"
    );

    let stop = wait_for_stop(&shutdown);
    tokio::pin!(stop);
    loop {
        tokio::select! {
            () = &mut stop => break,
            Some(event) = queued.recv() => {
                tokio::spawn(handle(
                    event,
                    Arc::clone(&devices),
                    weather.clone(),
                    test_notification.clone(),
                ));
            }
        }
    }

    info!("shutdown signal received");
    let deadline = Instant::now() + SHUTDOWN_BUDGET;
    // Neither waits for a weather fetch in flight: a hung one is left to the runtime's end.
    components.cancel();
    scheduler.cancel();
    broker.shutdown(deadline).await;
    info!("shutdown complete");
    Exit {
        code: ExitCode::SUCCESS,
        deadline: Some(deadline),
    }
}

/// Starts the day/night and energy-saving controllers, each change queueing a settings push to
/// every device, and returns the settings builder reading them. `None`, logged, when the
/// energy-saving window cannot be read.
fn start_modes(
    config: &Config,
    zone: &TimeZone,
    clock: &Arc<dyn Clock>,
    scheduler: &Scheduler,
    events: &mpsc::UnboundedSender<Event>,
) -> Option<Arc<Builder>> {
    let on_mode_change: OnModeChange = {
        let events = events.clone();
        Arc::new(move |mode| {
            info!(mode = mode.as_str(), "day/night mode changed");
            send(&events, Event::PushAll);
            Box::pin(future::ready(()))
        })
    };
    let day_night = DayNight::new(
        config.location,
        zone.clone(),
        Arc::clone(clock),
        on_mode_change,
    );
    day_night.start(scheduler);

    let on_active_change: OnActiveChange = {
        let events = events.clone();
        Arc::new(move |active| {
            info!(active, "energy-saving mode changed");
            send(&events, Event::PushAll);
            Box::pin(future::ready(()))
        })
    };
    let energy_saving = match EnergySaving::new(
        &config.energy_saving,
        zone.clone(),
        Arc::clone(clock),
        on_active_change,
    ) {
        Ok(energy_saving) => energy_saving,
        Err(parse_error) => {
            error!(error = %parse_error, "failed to start energy-saving controller");
            return None;
        }
    };
    energy_saving.start(scheduler);

    Some(Arc::new(Builder::new(
        config.theme.clone(),
        day_night,
        energy_saving,
    )))
}

/// Schedules the configured notifications to every connected device.
fn start_notifications(
    config: &Config,
    zone: TimeZone,
    clock: &Arc<dyn Clock>,
    scheduler: &Scheduler,
    devices: &Arc<Devices>,
) {
    let publish: Publish = {
        let devices = Arc::clone(devices);
        Arc::new(move |client_id, notification| {
            let devices = Arc::clone(&devices);
            Box::pin(async move {
                devices
                    .publish_notification(&client_id, &notification)
                    .await
                    .map_err(Into::into)
            })
        })
    };
    let connected_ids: ConnectedIds = {
        let devices = Arc::clone(devices);
        Arc::new(move || devices.registry.connected_ids())
    };
    ScheduledNotifier::new(
        config.scheduled_notifications.clone(),
        zone,
        Arc::clone(clock),
        publish,
    )
    .start(scheduler, &connected_ids);
}

/// The simulated forecast `--weather-wmo` asks for; `None` for 0 or no flag.
fn simulation(weather_wmo: Option<i64>) -> Option<FetchFunction> {
    weather_wmo.filter(|&code| code != 0).map(|code| {
        warn!(
            wmo_code = code,
            "weather: simulation mode active — real API disabled"
        );
        simulate_fetch(code)
    })
}

/// What the devices need from the controller, queued by the callbacks.
#[derive(Debug)]
enum Event {
    /// A device connected: push its settings.
    Connected(String),
    /// A device is ready: send it the weather warnings and the test notification.
    Ready(String),
    /// The mode or the energy-saving state changed: push every device's settings.
    PushAll,
}

fn send(events: &mpsc::UnboundedSender<Event>, event: Event) {
    // The queue only closes once the run is over, and then there is no one left to tell.
    let _ = events.send(event);
}

async fn handle(
    event: Event,
    devices: Arc<Devices>,
    weather: Option<Arc<weather::Controller>>,
    test_notification: Option<Arc<Notification>>,
) {
    match event {
        Event::Connected(client_id) => devices.push_settings(&client_id).await,
        Event::Ready(client_id) => {
            if let Some(weather) = weather {
                weather.on_device_connected(&client_id).await;
            }
            if let Some(notification) = test_notification {
                let published = devices
                    .publish_notification(&client_id, &notification)
                    .await;
                if let Err(publish_error) = published {
                    warn!(client_id, error = %publish_error, "test-notification: publish failed");
                }
            }
        }
        Event::PushAll => devices.push_settings_to_all().await,
    }
}

/// The broker on every interface, IPv6 included where the host has it (Go's `:port`), with
/// the callbacks queueing events.
async fn start_broker(
    config: &Config,
    registry: &Arc<Registry>,
    events: &mpsc::UnboundedSender<Event>,
) -> Result<Broker, BrokerError> {
    let any_address = if TcpListener::bind((Ipv6Addr::UNSPECIFIED, 0)).is_ok() {
        IpAddr::V6(Ipv6Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    };
    let address = |port: i64| {
        u16::try_from(port)
            .map(|port| SocketAddr::new(any_address, port))
            .map_err(|_| BrokerError::Setup {
                step: "listen address",
                message: format!("port {port} is out of range"),
            })
    };
    let broker_config = BrokerConfig {
        tcp_addr: address(config.mqtt.port)?,
        ws_addr: config.mqtt.ws_port.map(address).transpose()?,
        username: config.mqtt.username.clone(),
        password: config.mqtt.password.clone(),
        publish_delay: None,
    };
    let queue = |make: fn(String) -> Event| -> ClientCallback {
        let events = events.clone();
        Arc::new(move |client_id| send(&events, make(client_id.to_owned())))
    };
    let callbacks = Callbacks {
        on_connected: queue(Event::Connected),
        on_device_ready: queue(Event::Ready),
    };
    Broker::start(broker_config, Arc::clone(registry), callbacks).await
}

/// The connected devices, reached through the broker.
#[derive(Clone)]
struct Devices {
    registry: Arc<Registry>,
    broker: BrokerHandle,
    builder: Arc<Builder>,
}

impl Devices {
    async fn push_settings(&self, client_id: &str) {
        let pushed = settings::push(client_id, &self.broker, &self.builder.build()).await;
        if let Err(push_error) = pushed {
            warn!(client_id, error = %push_error, "failed to push settings to client");
        }
    }

    async fn push_settings_to_all(&self) {
        for client_id in self.registry.connected_ids() {
            self.push_settings(&client_id).await;
        }
    }

    /// Publishes a notification to `{client_id}/notify`, at `QoS` 1 and not retained.
    async fn publish_notification(
        &self,
        client_id: &str,
        notification: &Notification,
    ) -> Result<(), NotifyError> {
        let payload =
            serde_json::to_vec(notification).map_err(|source| NotifyError::Serialize {
                client_id: client_id.to_owned(),
                source,
            })?;
        let topic = format!("{client_id}/notify");
        self.broker
            .publish(&topic, payload, false)
            .await
            .map_err(|source| NotifyError::Publish { topic, source })
    }
}

impl weather::Publisher for Devices {
    fn connected_ids(&self) -> Vec<String> {
        self.registry.connected_ids()
    }

    fn publish(&self, client_id: &str, notification: Arc<Notification>) -> PublishFuture {
        let devices = self.clone();
        let client_id = client_id.to_owned();
        Box::pin(async move {
            devices
                .publish_notification(&client_id, &notification)
                .await
                .map_err(Into::into)
        })
    }

    fn set_overlay(
        &self,
        overlay: Option<OverlayEffect>,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        self.builder.set_overlay(overlay);
        let devices = self.clone();
        Box::pin(async move { devices.push_settings_to_all().await })
    }
}

/// A notification that could not be published.
#[derive(Debug, thiserror::Error)]
enum NotifyError {
    /// The payload could not be serialized.
    #[error("run: marshal notification for {client_id}: {source}")]
    Serialize {
        client_id: String,
        #[source]
        source: serde_json::Error,
    },
    /// The broker could not publish it.
    #[error("run: publish notification to {topic}: {source}")]
    Publish {
        topic: String,
        #[source]
        source: BrokerError,
    },
}

/// The configured time zone, or the system's when none is configured. `None`, logged, when the
/// name is unknown, which validation already rules out.
fn time_zone(config: &Config) -> Option<TimeZone> {
    if config.timezone.is_empty() {
        warn!("no timezone configured; using system timezone");
        return Some(TimeZone::system());
    }
    match config::load_time_zone(&config.timezone) {
        Ok(zone) => Some(zone),
        Err(zone_error) => {
            error!(timezone = config.timezone, error = %zone_error, "invalid timezone");
            None
        }
    }
}

fn init_logger(level: LevelFilter, format: LogFormat) {
    // Fails only when a logger is already installed, by an earlier run in the same process
    // (the tests); it keeps logging.
    if logger::init(level, format).is_err() {
        debug!("logger already installed; keeping it");
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// Waits for SIGINT, SIGTERM or `shutdown`.
async fn wait_for_stop(shutdown: &CancellationToken) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    () = shutdown.cancelled() => {}
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(signal_error) => {
                warn!(error = %signal_error, "run: cannot listen for SIGTERM");
                tokio::select! {
                    () = shutdown.cancelled() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::select! {
            () = shutdown.cancelled() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
}
