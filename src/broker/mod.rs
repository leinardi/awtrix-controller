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

//! The embedded MQTT broker (rmqtt) the devices connect to.
//!
//! The broker runs on its own tokio runtime, on a dedicated thread, owned by [`Broker`]. rmqtt
//! spawns a detached task per accepted connection, so stopping its server future only stops the
//! listeners; owning the runtime is what lets [`Broker::shutdown`] end every connection. Calls
//! from the controller cross over with `Handle::spawn` and await the join handle.

mod hook;

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use futures::future::join_all;
use rmqtt::codec::types::Publish as CodecPublish;
use rmqtt::context::ServerContext;
use rmqtt::hook::Type;
use rmqtt::net::Builder;
use rmqtt::server::MqttServer;
use rmqtt::session::SessionState;
use rmqtt::types::{From, Id, QoS};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::oneshot;
use tokio::task::{AbortHandle, JoinError};
use tokio::time::Instant;
use tracing::{error, info, warn};

pub use self::hook::{Callbacks, ClientCallback, PublishDelay};
use self::hook::{ControllerHook, SharedHook};
use crate::clientstate::Registry;

/// The client ID server-originated publishes come from.
const SERVER_CLIENT_ID: &str = "awtrix-controller";

/// How often shutdown checks whether every session has gone.
const SESSION_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long past its deadline shutdown may take to collect the broker thread.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(100);

/// How the broker listens and whom it lets in.
#[derive(Clone)]
pub struct BrokerConfig {
    /// The MQTT-over-TCP address.
    pub tcp_addr: SocketAddr,
    /// The MQTT-over-WebSocket address, if any.
    pub ws_addr: Option<SocketAddr>,
    /// The only accepted username.
    pub username: String,
    /// The only accepted password.
    pub password: String,
    /// A test-only publish stall; `None` in production.
    pub publish_delay: Option<PublishDelay>,
}

impl fmt::Debug for BrokerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerConfig")
            .field("tcp_addr", &self.tcp_addr)
            .field("ws_addr", &self.ws_addr)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("publish_delay", &self.publish_delay)
            .finish()
    }
}

/// A broker that failed to start or to publish.
#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    /// The broker could not be set up.
    #[error("broker: {step}: {message}")]
    Setup {
        /// What was being done.
        step: &'static str,
        /// What went wrong.
        message: String,
    },
    /// A publish failed.
    #[error("broker: publish to {topic}: {message}")]
    Publish {
        /// The topic.
        topic: String,
        /// What went wrong.
        message: String,
    },
    /// The broker's runtime is gone: it has shut down.
    #[error("broker: stopped: {0}")]
    Stopped(#[from] JoinError),
}

fn setup_error(step: &'static str) -> impl FnOnce(rmqtt::Error) -> BrokerError {
    move |error| BrokerError::Setup {
        step,
        message: error.to_string(),
    }
}

/// Whether a server publish is replayed to future subscribers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    /// Store the latest payload for future subscribers.
    Retained,
    /// Deliver only to current subscribers.
    Transient,
}

/// A cheap handle for publishing through the broker from the controller's runtime.
#[derive(Clone)]
pub struct BrokerHandle {
    runtime: Handle,
    context: ServerContext,
}

impl BrokerHandle {
    /// Publishes `payload` to `topic` at `QoS` 1 from the server, retained or not.
    ///
    /// # Errors
    ///
    /// rmqtt could not forward it, or the broker has shut down.
    pub async fn publish(
        &self,
        topic: &str,
        payload: Vec<u8>,
        retention: Retention,
    ) -> Result<(), BrokerError> {
        let context = self.context.clone();
        let owned_topic = topic.to_owned();
        let forwarded = self
            .runtime
            .spawn(async move {
                let publish = CodecPublish {
                    dup: false,
                    retain: retention == Retention::Retained,
                    qos: QoS::AtLeastOnce,
                    topic: owned_topic.into(),
                    packet_id: None,
                    payload: payload.into(),
                    properties: None,
                };
                let from = From::from_system(Id::from(context.node.id(), SERVER_CLIENT_ID.into()));
                SessionState::forwards(&context, from, publish.into(), false, None)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await?;
        forwarded.map_err(|message| BrokerError::Publish {
            topic: topic.to_owned(),
            message,
        })
    }
}

/// The running broker, its runtime and its thread.
pub struct Broker {
    handle: BrokerHandle,
    registry: Arc<Registry>,
    closing: Arc<AtomicBool>,
    server: AbortHandle,
    /// Tells the broker thread how long its runtime's shutdown may take.
    shutdown_request: std::sync::mpsc::Sender<Duration>,
    /// Fires when the broker thread has shut its runtime down.
    finished: oneshot::Receiver<()>,
}

/// What the broker thread hands back once the server is listening.
struct Started {
    runtime: Handle,
    context: ServerContext,
    server: AbortHandle,
}

impl Broker {
    /// Starts the broker on its own runtime and thread, listening on the configured addresses
    /// with authentication required on every listener.
    ///
    /// # Errors
    ///
    /// The runtime, the thread or a listener could not be set up.
    pub async fn start(
        config: BrokerConfig,
        registry: Arc<Registry>,
        callbacks: Callbacks,
    ) -> Result<Self, BrokerError> {
        let closing = Arc::new(AtomicBool::new(false));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("broker")
            .build()
            .map_err(|error| BrokerError::Setup {
                step: "build runtime",
                message: error.to_string(),
            })?;

        let (started_sender, started) = oneshot::channel();
        let (shutdown_request, shutdown_requested) = std::sync::mpsc::channel();
        let (finished_sender, finished) = oneshot::channel();
        let hook = Arc::new(ControllerHook::new(
            config.username.clone(),
            config.password.clone(),
            Arc::clone(&registry),
            callbacks,
            Arc::clone(&closing),
            config.publish_delay.clone(),
        ));

        thread::Builder::new()
            .name("broker".to_owned())
            .spawn(move || {
                broker_thread(runtime, config, hook, started_sender, &shutdown_requested);
                let _ = finished_sender.send(());
            })
            .map_err(|error| BrokerError::Setup {
                step: "spawn thread",
                message: error.to_string(),
            })?;

        let Started {
            runtime,
            context,
            server,
        } = started.await.map_err(|_| BrokerError::Setup {
            step: "start",
            message: "the broker thread ended during startup".to_owned(),
        })??;

        Ok(Self {
            handle: BrokerHandle { runtime, context },
            registry,
            closing,
            server,
            shutdown_request,
            finished,
        })
    }

    /// A handle for publishing.
    #[must_use]
    pub fn handle(&self) -> BrokerHandle {
        self.handle.clone()
    }

    /// The registry the broker keeps.
    #[must_use]
    pub const fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Stops accepting devices: from now on every authentication is refused and device events
    /// are dropped without callbacks. The first step of [`shutdown`](Self::shutdown).
    pub fn stop_accepting(&self) {
        self.closing.store(true, Ordering::SeqCst);
    }

    /// Shuts the broker down by `deadline`, each step getting only the time that remains:
    ///
    /// 1. refuse new authentications and drop device events;
    /// 2. stop the server future, which closes the listeners;
    /// 3. kick every live session, all at once (rmqtt's `Entry::kick` may wait 5 s for a session
    ///    that does not answer, so one at a time could take 5 s each);
    /// 4. wait for the session count to reach zero;
    /// 5. shut the broker runtime down, which drops any connection still left;
    /// 6. wait for the broker thread to finish, leaving it behind with a warning if it does not.
    ///
    /// Returns at most `SHUTDOWN_GRACE` after `deadline`.
    pub async fn shutdown(self, deadline: Instant) {
        let Self {
            handle,
            closing,
            server,
            shutdown_request,
            finished,
            ..
        } = self;
        let remaining = || deadline.saturating_duration_since(Instant::now());

        closing.store(true, Ordering::SeqCst);
        server.abort();

        let kick_context = handle.context.clone();
        let kicked = handle.runtime.spawn(kick_all(kick_context));
        if tokio::time::timeout(remaining(), kicked).await.is_err() {
            warn!("broker: kicking the sessions did not finish before the shutdown deadline");
        }

        let count_context = handle.context.clone();
        let drained = handle.runtime.spawn(wait_for_no_sessions(count_context));
        if tokio::time::timeout(remaining(), drained).await.is_err() {
            warn!("broker: sessions remained at the shutdown deadline");
        }

        let _ = shutdown_request.send(remaining());
        match tokio::time::timeout(remaining() + SHUTDOWN_GRACE, finished).await {
            Ok(_) => info!("broker: stopped"),
            Err(_) => {
                warn!(
                    "broker: the broker thread did not finish by the deadline; leaving it behind"
                );
            }
        }
    }
}

/// The broker thread: sets the server up on `runtime`, reports, then waits for the shutdown
/// request and shuts the runtime down within the time it carries.
fn broker_thread(
    runtime: Runtime,
    config: BrokerConfig,
    hook: Arc<ControllerHook>,
    started: oneshot::Sender<Result<Started, BrokerError>>,
    shutdown_requested: &std::sync::mpsc::Receiver<Duration>,
) {
    let setup = runtime.block_on(set_up(config, hook));
    let setup = setup.map(|(context, server)| Started {
        runtime: runtime.handle().clone(),
        context,
        server,
    });
    let failed = setup.is_err();
    let _ = started.send(setup);
    if failed {
        runtime.shutdown_background();
        return;
    }

    // A dropped `Broker` closes the channel: shut down at once.
    let budget = shutdown_requested.recv().unwrap_or(Duration::ZERO);
    runtime.shutdown_timeout(budget);
}

/// Builds the server context, registers the retainer and the hook, binds the listeners and
/// spawns the server.
async fn set_up(
    config: BrokerConfig,
    hook: Arc<ControllerHook>,
) -> Result<(ServerContext, AbortHandle), BrokerError> {
    let context = ServerContext::new().build().await;
    rmqtt_retainer::register(&context, true, false)
        .await
        .map_err(setup_error("register the retainer"))?;

    let register = context.extends.hook_mgr().register();
    for event in [
        Type::ClientAuthenticate,
        Type::ClientConnected,
        Type::ClientDisconnected,
        Type::MessagePublish,
    ] {
        register
            .add(event, Box::new(SharedHook(Arc::clone(&hook))))
            .await;
    }
    register.start().await;

    let mut server = MqttServer::new(context.clone()).listener(
        listener("tcp", config.tcp_addr)?
            .tcp()
            .map_err(setup_error("TCP listener"))?,
    );
    if let Some(ws_addr) = config.ws_addr {
        server = server.listener(
            listener("ws", ws_addr)?
                .ws()
                .map_err(setup_error("WebSocket listener"))?,
        );
    }
    let server = server.build();

    // `run`, not `start`: `start` exits the process when the server fails.
    let task = tokio::spawn(async move {
        let ran = server.run().await;
        if let Err(error) = ran {
            error!(error = %error, "broker: server stopped unexpectedly");
        }
    });
    Ok((context, task.abort_handle()))
}

/// Binds a listener. Every listener goes through here so none can skip
/// `allow_anonymous(false)`: rmqtt-net defaults it to true, and rmqtt then accepts a CONNECT
/// without a username before any hook runs. `SO_REUSEADDR` is set, as Go's listeners set it, so a
/// restart can bind the port while connections of the previous run are still closing.
fn listener(name: &str, addr: SocketAddr) -> Result<rmqtt::net::Listener, BrokerError> {
    Builder::new()
        .name(name)
        .laddr(addr)
        .allow_anonymous(false)
        .reuseaddr(Some(true))
        .bind()
        .map_err(|error| BrokerError::Setup {
            step: "bind",
            message: format!("{name} listener on {addr}: {error}"),
        })
}

/// Kicks every session at once. The registry is rmqtt 0.24's `Shared` behind
/// `ServerContext::extends.shared()`: `iter` lists the entries, `entry(id)` gives one by ID, and
/// `Entry::kick(clean_start, clear_subscriptions, is_admin)` sends the session a kick message,
/// waits up to 5 s for it to answer (`shared.rs`, `LockEntry::kick`), then removes the entry, which
/// `sessions_count` then no longer counts.
async fn kick_all(context: ServerContext) {
    // The registry lock is released before kicking: removing an entry takes it again.
    let entries: Vec<_> = {
        let shared = context.extends.shared().await;
        shared
            .iter()
            .map(|entry| shared.entry(entry.id()))
            .collect()
    };
    join_all(entries.into_iter().map(|mut entry| async move {
        let _ = entry.kick(true, true, true).await;
    }))
    .await;
}

/// Waits until rmqtt holds no session.
async fn wait_for_no_sessions(context: ServerContext) {
    loop {
        if context.extends.shared().await.sessions_count() == 0 {
            return;
        }
        tokio::time::sleep(SESSION_POLL_INTERVAL).await;
    }
}
