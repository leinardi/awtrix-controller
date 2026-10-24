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

//! The broker hook: authentication, the registry of connected devices, and the routing of what
//! devices publish.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use rmqtt::hook::{Handler, HookResult, Parameter, ReturnType};
use rmqtt::types::{AuthResult, Id};
use tracing::{debug, info, warn};

use crate::clientstate::Registry;
use crate::model::Stats;

/// Called with a client ID. Called on the broker's runtime from inside a hook, so it must return
/// quickly and hand any real work to the controller (a channel, a spawn on its own runtime).
pub type ClientCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// What the controller is told about devices.
#[derive(Clone)]
pub struct Callbacks {
    /// A device connected: push its settings.
    pub on_connected: ClientCallback,
    /// A device published its first stats on this connection: it has subscribed and is ready for
    /// notifications.
    pub on_device_ready: ClientCallback,
}

/// A test-only stall: every publish by `client_id` waits `delay` in the hook. `None` in
/// production.
#[derive(Clone, Debug)]
pub struct PublishDelay {
    /// The client whose publishes stall.
    pub client_id: String,
    /// How long each stalls.
    pub delay: Duration,
}

/// One network session of a client. A takeover replaces the session before the old one's
/// disconnect arrives, so a disconnect only unregisters the client when its session is still
/// the current one (Go compared `*mqtt.Client` pointers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SessionKey {
    create_time: i64,
    remote_addr: Option<SocketAddr>,
}

impl SessionKey {
    fn of(id: &Id) -> Self {
        Self {
            create_time: id.create_time,
            remote_addr: id.remote_addr,
        }
    }
}

/// The current session of a client and whether it has reported ready.
#[derive(Clone, Copy, Debug)]
struct Connection {
    session: SessionKey,
    ready: bool,
}

/// The broker's single authority on authentication and the devices' state.
pub(super) struct ControllerHook {
    username: String,
    password: String,
    registry: Arc<Registry>,
    callbacks: Callbacks,
    closing: Arc<AtomicBool>,
    connections: Mutex<HashMap<String, Connection>>,
    publish_delay: Option<PublishDelay>,
}

impl ControllerHook {
    pub(super) fn new(
        username: String,
        password: String,
        registry: Arc<Registry>,
        callbacks: Callbacks,
        closing: Arc<AtomicBool>,
        publish_delay: Option<PublishDelay>,
    ) -> Self {
        Self {
            username,
            password,
            registry,
            callbacks,
            closing,
            connections: Mutex::new(HashMap::new()),
            publish_delay,
        }
    }

    /// `Allow` only for an exact username and password match; any other credentials, absent ones
    /// included, are `BadUsernameOrPassword`; everything is `NotAuthorized` once the broker is
    /// closing. Never `NotFound`, which rmqtt would treat as "ask someone else".
    pub(super) fn authenticate(
        &self,
        username: Option<&str>,
        password: Option<&[u8]>,
    ) -> AuthResult {
        if self.closing.load(Ordering::SeqCst) {
            return AuthResult::NotAuthorized;
        }
        let username_matches = username == Some(self.username.as_str());
        let password_matches = password == Some(self.password.as_bytes());
        if username_matches && password_matches {
            AuthResult::Allow(false, None)
        } else {
            AuthResult::BadUsernameOrPassword
        }
    }

    fn connected(&self, id: &Id) {
        let client_id = id.client_id.to_string();
        self.lock().insert(
            client_id.clone(),
            Connection {
                session: SessionKey::of(id),
                ready: false,
            },
        );
        self.registry.register(&client_id);
        info!(client_id, "broker: client connected");
        (self.callbacks.on_connected)(&client_id);
    }

    fn disconnected(&self, id: &Id, reason: &str) {
        let client_id = id.client_id.to_string();
        let current = {
            let mut connections = self.lock();
            let current = connections
                .get(&client_id)
                .is_some_and(|connection| connection.session == SessionKey::of(id));
            if current {
                connections.remove(&client_id);
            }
            current
        };
        if current {
            self.registry.unregister(&client_id);
        }
        info!(
            client_id,
            reason,
            stale = !current,
            "broker: client disconnected"
        );
    }

    /// Routes a device's publish by topic suffix: stats (the first on a connection also reports
    /// the device ready), the current app, and button presses.
    fn published(&self, id: &Id, topic: &str, payload: &[u8]) {
        let client_id = &*id.client_id;
        debug!(
            client_id,
            topic,
            payload = %String::from_utf8_lossy(payload),
            "broker: message received"
        );

        if topic.ends_with("/stats") {
            match Stats::from_json(payload) {
                Ok(stats) => self.registry.update_stats(client_id, stats),
                Err(error) => warn!(
                    client_id,
                    topic,
                    error = %error,
                    "broker: failed to parse stats payload"
                ),
            }
            if self.mark_ready(id) {
                (self.callbacks.on_device_ready)(client_id);
            }
        } else if topic.ends_with("/stat/currentApp") {
            let app = String::from_utf8_lossy(payload);
            self.registry.update_current_app(client_id, &app);
            debug!(client_id, app = %app, "broker: current app updated");
        } else if let Some(button) = ["left", "select", "right"]
            .into_iter()
            .find(|button| topic.ends_with(&format!("/button/{button}")))
        {
            info!(client_id, button, "broker: button event");
        } else {
            debug!(
                client_id,
                topic,
                payload = %String::from_utf8_lossy(payload),
                "broker: unhandled topic"
            );
        }
    }

    /// Marks the current session of the publisher ready, and reports whether it just became so.
    fn mark_ready(&self, id: &Id) -> bool {
        let mut connections = self.lock();
        match connections.get_mut(&*id.client_id) {
            Some(connection) if connection.session == SessionKey::of(id) && !connection.ready => {
                connection.ready = true;
                true
            }
            Some(_) | None => false,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Connection>> {
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// The hook as registered with rmqtt, one instance shared by every event type.
pub(super) struct SharedHook(pub(super) Arc<ControllerHook>);

#[async_trait]
impl Handler for SharedHook {
    async fn hook(&self, parameter: &Parameter, accumulated: Option<HookResult>) -> ReturnType {
        let hook = &self.0;
        if let Parameter::ClientAuthenticate(connect_info) = parameter {
            let username = connect_info.username().map(|username| &**username);
            let password = connect_info.password().map(|password| &**password);
            let result = hook.authenticate(username, password);
            // The only authority: stop the chain with the result.
            return (false, Some(HookResult::AuthResult(result)));
        }

        // While closing, device events are dropped: no registry change, no callback.
        if hook.closing.load(Ordering::SeqCst) {
            return (true, accumulated);
        }

        // rmqtt's parameter enum has a variant per hook type; only the registered ones arrive.
        if let Parameter::ClientConnected(session) = parameter {
            hook.connected(&session.id);
        } else if let Parameter::ClientDisconnected(session, reason) = parameter {
            hook.disconnected(&session.id, &reason.to_string());
        } else if let Parameter::MessagePublish(Some(session), _from, publish) = parameter {
            if let Some(stall) = &hook.publish_delay
                && *session.id.client_id == *stall.client_id
            {
                tokio::time::sleep(stall.delay).await;
            }
            hook.published(&session.id, &publish.topic, &publish.payload);
        }
        (true, accumulated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook() -> ControllerHook {
        let noop: ClientCallback = Arc::new(|_| {});
        ControllerHook::new(
            "user".to_owned(),
            "pass".to_owned(),
            Arc::new(Registry::new()),
            Callbacks {
                on_connected: Arc::clone(&noop),
                on_device_ready: noop,
            },
            Arc::new(AtomicBool::new(false)),
            None,
        )
    }

    fn allowed(result: &AuthResult) -> bool {
        matches!(result, AuthResult::Allow(false, None))
    }

    /// Only the exact credentials are allowed; everything else is a bad username or password.
    #[test]
    fn authentication_requires_the_exact_credentials() {
        let hook = hook();
        assert!(allowed(&hook.authenticate(Some("user"), Some(b"pass"))));
        for (username, password) in [
            (None, None),
            (Some("user"), None),
            (None, Some(&b"pass"[..])),
            (Some("other"), Some(&b"pass"[..])),
            (Some("user"), Some(&b"wrong"[..])),
            (Some("User"), Some(&b"pass"[..])),
            (Some("user"), Some(&b"pass "[..])),
            (Some(""), Some(&b""[..])),
        ] {
            let result = hook.authenticate(username, password);
            assert!(
                matches!(result, AuthResult::BadUsernameOrPassword),
                "{username:?} {password:?}: {result:?}"
            );
        }
    }

    /// Once closing, even the right credentials are not authorized.
    #[test]
    fn closing_refuses_everyone() {
        let hook = hook();
        hook.closing.store(true, Ordering::SeqCst);
        let result = hook.authenticate(Some("user"), Some(b"pass"));
        assert!(matches!(result, AuthResult::NotAuthorized), "{result:?}");
    }
}
