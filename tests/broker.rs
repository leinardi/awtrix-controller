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

//! The embedded broker against real MQTT clients over TCP and WebSocket: the ports of
//! the Go broker tests (`internal/broker`), the spike's takeover and WebSocket tests, the
//! authentication-bypass tests, and shutdown.

#![expect(
    clippy::expect_used,
    reason = "an integration test crate: its helpers sit outside #[test] functions, where allow-expect-in-tests does not reach"
)]

use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use awtrix_controller::broker::{Broker, BrokerConfig, BrokerHandle, Callbacks, PublishDelay};
use awtrix_controller::clientstate::Registry;
use awtrix_controller::model::Settings;
use awtrix_controller::settings;
use futures::{SinkExt, StreamExt};
use rumqttc::{
    AsyncClient, ConnAck, ConnectReturnCode, ConnectionError, Event, EventLoop, MqttOptions,
    Outgoing, Packet, QoS, Transport,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const USERNAME: &str = "testuser";
const PASSWORD: &str = "testpass";

/// How long a test waits for something that should happen.
const WAIT: Duration = Duration::from_secs(5);

/// A loopback address nothing listens on.
fn free_addr() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("free port")
}

/// A started broker and what its callbacks saw.
struct Harness {
    broker: Option<Broker>,
    handle: BrokerHandle,
    registry: Arc<Registry>,
    tcp: SocketAddr,
    ws: SocketAddr,
    connected: Arc<Mutex<Vec<String>>>,
    ready: Arc<Mutex<Vec<String>>>,
    /// Every connected client ID, as the settings pusher saw it.
    pushed: mpsc::UnboundedReceiver<String>,
}

impl Harness {
    fn connected_count(&self) -> usize {
        self.connected
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    fn ready_ids(&self) -> Vec<String> {
        self.ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn take_broker(&mut self) -> Broker {
        self.broker.take().expect("broker running")
    }

    async fn shutdown(&mut self) {
        if let Some(broker) = self.broker.take() {
            broker
                .shutdown(Instant::now() + Duration::from_secs(2))
                .await;
        }
    }
}

async fn start() -> Harness {
    start_with(None).await
}

/// Starts a broker; every connect pushes settings with `BRI` 100 through `settings::push`, as the
/// controller does.
async fn start_with(publish_delay: Option<PublishDelay>) -> Harness {
    // A port found free can be taken by another test's connection before the broker binds it;
    // try again with new ports.
    for _ in 0..4 {
        if let Some(harness) = try_start(publish_delay.clone()).await {
            return harness;
        }
    }
    try_start(publish_delay).await.expect("broker starts")
}

async fn try_start(publish_delay: Option<PublishDelay>) -> Option<Harness> {
    let tcp = free_addr();
    let ws = free_addr();
    let registry = Arc::new(Registry::new());
    let connected = Arc::new(Mutex::new(Vec::new()));
    let ready = Arc::new(Mutex::new(Vec::new()));
    let (connected_tx, mut connected_rx) = mpsc::unbounded_channel::<String>();

    let seen = Arc::clone(&connected);
    let became_ready = Arc::clone(&ready);
    let callbacks = Callbacks {
        on_connected: Arc::new(move |client_id| {
            seen.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(client_id.to_owned());
            let _ = connected_tx.send(client_id.to_owned());
        }),
        on_device_ready: Arc::new(move |client_id| {
            became_ready
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(client_id.to_owned());
        }),
    };
    let config = BrokerConfig {
        tcp_addr: tcp,
        ws_addr: Some(ws),
        username: USERNAME.to_owned(),
        password: PASSWORD.to_owned(),
        publish_delay,
    };
    let broker = Broker::start(config, Arc::clone(&registry), callbacks)
        .await
        .ok()?;
    let handle = broker.handle();

    let (pushed_tx, pushed) = mpsc::unbounded_channel();
    let settings_publisher = handle.clone();
    tokio::spawn(async move {
        while let Some(client_id) = connected_rx.recv().await {
            let settings = Settings {
                bri: Some(100),
                ..Settings::default()
            };
            let _ = settings::push(&client_id, &settings_publisher, &settings).await;
            let _ = pushed_tx.send(client_id);
        }
    });

    Some(Harness {
        broker: Some(broker),
        handle,
        registry,
        tcp,
        ws,
        connected,
        ready,
        pushed,
    })
}

/// A connected rumqttc client and its events.
struct Device {
    client: AsyncClient,
    events: mpsc::UnboundedReceiver<Result<Event, String>>,
}

async fn connect(
    addr: SocketAddr,
    client_id: &str,
    password: &str,
    websocket: bool,
) -> Result<Device, ConnectReturnCode> {
    let mut options = if websocket {
        let mut options = MqttOptions::new(client_id, format!("ws://{addr}/mqtt"), addr.port());
        options.set_transport(Transport::Ws);
        options
    } else {
        MqttOptions::new(client_id, addr.ip().to_string(), addr.port())
    };
    options
        .set_credentials(USERNAME, password)
        .set_keep_alive(Duration::from_secs(5));
    let (client, eventloop) = AsyncClient::new(options, 16);
    let (sender, mut events) = mpsc::unbounded_channel();
    drive(eventloop, sender);
    loop {
        let event = timeout(WAIT, events.recv())
            .await
            .expect("CONNACK in time")
            .expect("event loop running");
        match event {
            Ok(Event::Incoming(Packet::ConnAck(ack))) if ack.code == ConnectReturnCode::Success => {
                return Ok(Device { client, events });
            }
            Ok(Event::Incoming(Packet::ConnAck(ack))) => return Err(ack.code),
            Ok(_) => {}
            Err(error) => panic!("connection failed: {error}"),
        }
    }
}

/// Runs a client's event loop, forwarding its events and ending with its first error.
fn drive(mut eventloop: EventLoop, events: mpsc::UnboundedSender<Result<Event, String>>) {
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(event) => {
                    // rumqttc keeps the socket open after DISCONNECT until the next poll; a real
                    // device closes it, so stop polling, which drops the socket.
                    let disconnected = matches!(event, Event::Outgoing(Outgoing::Disconnect));
                    if events.send(Ok(event)).is_err() || disconnected {
                        return;
                    }
                }
                Err(ConnectionError::ConnectionRefused(code)) => {
                    let _ = events.send(Ok(Event::Incoming(Packet::ConnAck(ConnAck {
                        session_present: false,
                        code,
                    }))));
                    return;
                }
                Err(error) => {
                    let _ = events.send(Err(error.to_string()));
                    return;
                }
            }
        }
    });
}

impl Device {
    async fn publish_acked(&mut self, topic: &str, payload: &[u8]) {
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload.to_vec())
            .await
            .expect("queued");
        self.wait_for(|event| matches!(event, Event::Incoming(Packet::PubAck(_))))
            .await;
    }

    async fn wait_for(&mut self, predicate: impl Fn(&Event) -> bool) -> Event {
        loop {
            let event = timeout(WAIT, self.events.recv())
                .await
                .expect("event in time")
                .expect("event loop running")
                .expect("no connection error");
            if predicate(&event) {
                return event;
            }
        }
    }

    /// Waits for the connection to end, with an error or the event loop stopping.
    async fn wait_for_close(&mut self) {
        let closed = timeout(WAIT, async {
            loop {
                match self.events.recv().await {
                    None | Some(Err(_)) => return,
                    Some(Ok(_)) => {}
                }
            }
        })
        .await;
        assert!(closed.is_ok(), "the connection stayed open");
    }
}

/// Polls `check` until it holds or `WAIT` passes.
async fn eventually(check: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    check()
}

/// Go: `TestBrokerConnectValidCredentials`. Extended with the spike's check that the settings
/// arrive retained.
#[tokio::test(flavor = "multi_thread")]
async fn valid_credentials_connect_and_receive_retained_settings() {
    let mut harness = start().await;
    let mut device = connect(harness.tcp, "device01", PASSWORD, false)
        .await
        .expect("connected");
    let pushed = timeout(WAIT, harness.pushed.recv())
        .await
        .expect("pushed in time");
    assert_eq!(pushed.as_deref(), Some("device01"));
    assert!(harness.registry.snapshot("device01").is_some());

    // Retained: subscribing after the push still delivers them.
    device
        .client
        .subscribe("device01/settings", QoS::AtLeastOnce)
        .await
        .expect("subscribe");
    let event = device
        .wait_for(|event| matches!(event, Event::Incoming(Packet::Publish(_))))
        .await;
    let Event::Incoming(Packet::Publish(publish)) = event else {
        panic!("not a publish: {event:?}");
    };
    assert!(publish.retain, "settings arrive retained");
    assert_eq!(publish.qos, QoS::AtLeastOnce);
    assert_eq!(publish.payload.as_ref(), br#"{"BRI":100,"OVERLAY":null}"#);
    harness.shutdown().await;
}

/// Go: `TestBrokerConnectInvalidCredentials`
#[tokio::test(flavor = "multi_thread")]
async fn invalid_credentials_are_rejected() {
    let mut harness = start().await;
    let refused = connect(harness.tcp, "badclient", "wrong", false).await;
    assert!(
        matches!(refused, Err(ConnectReturnCode::BadUserNamePassword)),
        "{:?}",
        refused.err()
    );
    assert!(harness.registry.connected_ids().is_empty());
    assert_eq!(harness.connected_count(), 0);
    harness.shutdown().await;
}

/// Go: `TestBrokerPublishStats`. The registry is updated before the PUBACK.
#[tokio::test(flavor = "multi_thread")]
async fn stats_are_stored_before_the_puback() {
    let mut harness = start().await;
    let mut device = connect(harness.tcp, "device02", PASSWORD, false)
        .await
        .expect("connected");
    device
        .publish_acked("device02/stats", br#"{"bat":85,"lux":450,"ram":28000}"#)
        .await;
    let stats = harness
        .registry
        .snapshot("device02")
        .and_then(|state| state.stats)
        .expect("stats stored");
    assert_eq!((stats.bat, stats.lux, stats.ram), (85, 450, 28000));

    device
        .publish_acked("device02/stat/currentApp", b"Time")
        .await;
    let device_state = harness.registry.snapshot("device02").expect("registered");
    assert_eq!(device_state.current_app, "Time");
    harness.shutdown().await;
}

/// Go: `TestBrokerDisconnect`
#[tokio::test(flavor = "multi_thread")]
async fn disconnect_unregisters() {
    let mut harness = start().await;
    let device = connect(harness.tcp, "device03", PASSWORD, false)
        .await
        .expect("connected");
    assert_eq!(harness.registry.connected_ids(), ["device03"]);
    device.client.disconnect().await.expect("disconnect");
    let registry = Arc::clone(&harness.registry);
    assert!(eventually(|| registry.connected_ids().is_empty()).await);
    harness.shutdown().await;
}

/// Go: `TestBrokerOnDeviceReadyTriggeredOnStats`
#[tokio::test(flavor = "multi_thread")]
async fn first_stats_report_the_device_ready() {
    let mut harness = start().await;
    let mut device = connect(harness.tcp, "device04", PASSWORD, false)
        .await
        .expect("connected");
    assert!(harness.ready_ids().is_empty());
    device
        .publish_acked("device04/stats", br#"{"bat":75,"lux":300,"ram":16000}"#)
        .await;
    assert_eq!(harness.ready_ids(), ["device04"]);
    harness.shutdown().await;
}

/// Go: `TestBrokerOnDeviceReadyFiredOnce`. Ready again after a reconnect, once per connection.
#[tokio::test(flavor = "multi_thread")]
async fn device_ready_fires_once_per_connection() {
    let mut harness = start().await;
    let mut device = connect(harness.tcp, "device05", PASSWORD, false)
        .await
        .expect("connected");
    for _ in 0..2 {
        device
            .publish_acked("device05/stats", br#"{"bat":90}"#)
            .await;
    }
    // Unparsable stats still count as the device reporting in, as in Go.
    device.publish_acked("device05/stats", b"not json").await;
    assert_eq!(harness.ready_ids(), ["device05"]);

    device.client.disconnect().await.expect("disconnect");
    let registry = Arc::clone(&harness.registry);
    assert!(eventually(|| registry.connected_ids().is_empty()).await);
    let mut again = connect(harness.tcp, "device05", PASSWORD, false)
        .await
        .expect("reconnected");
    again
        .publish_acked("device05/stats", br#"{"bat":91}"#)
        .await;
    assert_eq!(harness.ready_ids(), ["device05", "device05"]);
    harness.shutdown().await;
}

/// Go: `TestBrokerStaleDisconnectDoesNotUnregister`
#[tokio::test(flavor = "multi_thread")]
async fn reconnect_after_clean_disconnect_keeps_the_registration() {
    let mut harness = start().await;
    let mut first = connect(harness.tcp, "device06", PASSWORD, false)
        .await
        .expect("connected");
    first
        .publish_acked("device06/stats", br#"{"bat":60}"#)
        .await;
    first.client.disconnect().await.expect("disconnect");
    let registry = Arc::clone(&harness.registry);
    assert!(eventually(|| registry.connected_ids().is_empty()).await);

    let mut second = connect(harness.tcp, "device06", PASSWORD, false)
        .await
        .expect("reconnected");
    assert!(registry.snapshot("device06").is_some());
    second
        .publish_acked("device06/stats", br#"{"bat":70}"#)
        .await;
    let stats = registry
        .snapshot("device06")
        .and_then(|state| state.stats)
        .expect("stats after reconnect");
    assert_eq!(stats.bat, 70);
    harness.shutdown().await;
}

/// The spike's takeover test: the same client ID connects while its old session is alive, and
/// the old session's disconnect must not unregister the new one.
#[tokio::test(flavor = "multi_thread")]
async fn takeover_does_not_unregister_the_new_session() {
    let mut harness = start().await;
    let mut first = connect(harness.tcp, "device07", PASSWORD, false)
        .await
        .expect("connected");
    let mut second = connect(harness.tcp, "device07", PASSWORD, false)
        .await
        .expect("took over");
    first.wait_for_close().await;
    second
        .publish_acked("device07/stats", br#"{"bat":80}"#)
        .await;
    let stats = harness
        .registry
        .snapshot("device07")
        .and_then(|state| state.stats)
        .expect("takeover kept the new session");
    assert_eq!(stats.bat, 80);
    assert_eq!(harness.ready_ids(), ["device07"]);
    harness.shutdown().await;
}

/// The spike's WebSocket test.
#[tokio::test(flavor = "multi_thread")]
async fn websocket_listener_accepts_devices() {
    let mut harness = start().await;
    let mut device = connect(harness.ws, "device08", PASSWORD, true)
        .await
        .expect("connected");
    device
        .publish_acked("device08/stats", br#"{"bat":90}"#)
        .await;
    let stats = harness
        .registry
        .snapshot("device08")
        .and_then(|state| state.stats)
        .expect("stats");
    assert_eq!(stats.bat, 90);
    harness.shutdown().await;
}

/// A server publish that is not retained reaches a subscriber at `QoS` 1, as notifications do.
#[tokio::test(flavor = "multi_thread")]
async fn server_publish_reaches_a_subscriber() {
    let mut harness = start().await;
    let mut device = connect(harness.tcp, "device09", PASSWORD, false)
        .await
        .expect("connected");
    device
        .client
        .subscribe("device09/notify", QoS::AtLeastOnce)
        .await
        .expect("subscribe");
    device
        .wait_for(|event| matches!(event, Event::Incoming(Packet::SubAck(_))))
        .await;
    harness
        .handle
        .publish("device09/notify", br#"{"text":"hi"}"#.to_vec(), false)
        .await
        .expect("published");
    let event = device
        .wait_for(|event| {
            matches!(event, Event::Incoming(Packet::Publish(publish)) if publish.topic == "device09/notify")
        })
        .await;
    let Event::Incoming(Packet::Publish(publish)) = event else {
        panic!("not a publish: {event:?}");
    };
    assert_eq!(publish.qos, QoS::AtLeastOnce);
    assert!(!publish.retain);
    harness.shutdown().await;
}

// --- Authentication bypass: raw CONNECT packets ---

/// The credentials a raw CONNECT carries.
#[derive(Clone, Copy, Debug)]
struct Credentials {
    username: Option<&'static str>,
    password: Option<&'static str>,
}

/// The cases that must be refused with a bad username or password.
const BAD_CREDENTIALS: [Credentials; 4] = [
    Credentials {
        username: None,
        password: None,
    },
    Credentials {
        username: Some(USERNAME),
        password: None,
    },
    Credentials {
        username: Some("intruder"),
        password: Some(PASSWORD),
    },
    Credentials {
        username: Some(USERNAME),
        password: Some("wrong"),
    },
];

const GOOD_CREDENTIALS: Credentials = Credentials {
    username: Some(USERNAME),
    password: Some(PASSWORD),
};

/// MQTT 3.1.1 (level 4) or 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Version {
    V311,
    V5,
}

/// A raw CONNECT packet, so a case rumqttc cannot express (a username without a password) can
/// be sent as well.
fn connect_packet(version: Version, client_id: &str, credentials: Credentials) -> Vec<u8> {
    fn string(bytes: &mut Vec<u8>, value: &[u8]) {
        let length = u16::try_from(value.len()).expect("short string");
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(value);
    }

    let mut flags = 0x02; // clean session
    if credentials.username.is_some() {
        flags |= 0x80;
    }
    if credentials.password.is_some() {
        flags |= 0x40;
    }
    let mut body = Vec::new();
    string(&mut body, b"MQTT");
    body.push(match version {
        Version::V311 => 4,
        Version::V5 => 5,
    });
    body.push(flags);
    body.extend_from_slice(&30_u16.to_be_bytes());
    if version == Version::V5 {
        body.push(0); // no properties
    }
    string(&mut body, client_id.as_bytes());
    if let Some(username) = credentials.username {
        string(&mut body, username.as_bytes());
    }
    if let Some(password) = credentials.password {
        string(&mut body, password.as_bytes());
    }

    let mut packet = vec![0x10];
    let mut remaining = body.len();
    loop {
        let mut byte = u8::try_from(remaining % 128).expect("7 bits");
        remaining /= 128;
        if remaining > 0 {
            byte |= 0x80;
        }
        packet.push(byte);
        if remaining == 0 {
            break;
        }
    }
    packet.extend_from_slice(&body);
    packet
}

/// The return code (3.1.1) or reason code (5) of a CONNACK.
fn connack_code(version: Version, connack: &[u8]) -> u8 {
    assert_eq!(connack.first(), Some(&0x20), "not a CONNACK: {connack:?}");
    let _ = version;
    *connack.get(3).expect("CONNACK code")
}

/// The code a refused CONNACK must carry.
const fn refusal(version: Version, closing: bool) -> u8 {
    match (version, closing) {
        (Version::V311, false) => 4,  // bad user name or password
        (Version::V311, true) => 5,   // not authorized
        (Version::V5, false) => 0x86, // bad user name or password
        (Version::V5, true) => 0x87,  // not authorized
    }
}

/// Sends a raw CONNECT over TCP and returns the CONNACK, asserting the socket is closed after it.
async fn raw_tcp_connect(addr: SocketAddr, packet: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(addr).await.expect("TCP connect");
    stream.write_all(packet).await.expect("send CONNECT");
    let mut connack = vec![0_u8; 2];
    timeout(WAIT, stream.read_exact(&mut connack))
        .await
        .expect("CONNACK in time")
        .expect("CONNACK header");
    let mut rest = vec![0_u8; usize::from(connack[1])];
    timeout(WAIT, stream.read_exact(&mut rest))
        .await
        .expect("CONNACK in time")
        .expect("CONNACK body");
    connack.extend_from_slice(&rest);

    let mut probe = [0_u8; 1];
    let closed = timeout(WAIT, stream.read(&mut probe))
        .await
        .expect("closed in time");
    assert!(
        matches!(closed, Ok(0) | Err(_)),
        "socket still open: {closed:?}"
    );
    connack
}

/// Sends a raw CONNECT over WebSocket and returns the CONNACK, asserting the connection is
/// closed after it.
async fn raw_ws_connect(addr: SocketAddr, packet: &[u8]) -> Vec<u8> {
    let mut request = format!("ws://{addr}/mqtt")
        .into_client_request()
        .expect("request");
    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", "mqtt".parse().expect("header"));
    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("WebSocket connect");
    socket
        .send(Message::binary(packet.to_vec()))
        .await
        .expect("send CONNECT");

    let mut connack = None;
    let closed = timeout(WAIT, async {
        while let Some(message) = socket.next().await {
            match message {
                Ok(Message::Binary(bytes)) if connack.is_none() => connack = Some(bytes.to_vec()),
                Ok(Message::Binary(bytes)) => panic!("data after the CONNACK: {bytes:?}"),
                Ok(Message::Close(_)) | Err(_) => return,
                Ok(_) => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "WebSocket still open");
    connack.expect("a CONNACK")
}

/// Every bad credential set, as 3.1.1 and 5, is refused with a bad username or password on TCP
/// and WebSocket: the socket is closed, nothing registered, no callback called.
#[tokio::test(flavor = "multi_thread")]
async fn bad_credentials_are_refused_on_every_listener() {
    let mut harness = start().await;
    for version in [Version::V311, Version::V5] {
        for (index, credentials) in BAD_CREDENTIALS.into_iter().enumerate() {
            let packet = connect_packet(version, &format!("intruder{index}"), credentials);
            let tcp = raw_tcp_connect(harness.tcp, &packet).await;
            assert_eq!(
                connack_code(version, &tcp),
                refusal(version, false),
                "TCP {version:?} {credentials:?}"
            );
            let ws = raw_ws_connect(harness.ws, &packet).await;
            assert_eq!(
                connack_code(version, &ws),
                refusal(version, false),
                "WS {version:?} {credentials:?}"
            );
        }
    }
    assert!(harness.registry.connected_ids().is_empty());
    assert_eq!(harness.connected_count(), 0);

    // The same packets with the right credentials are accepted, so the refusals above are real.
    for version in [Version::V311, Version::V5] {
        let packet = connect_packet(version, "control", GOOD_CREDENTIALS);
        let mut stream = TcpStream::connect(harness.tcp).await.expect("TCP connect");
        stream.write_all(&packet).await.expect("send CONNECT");
        let mut connack = [0_u8; 4];
        timeout(WAIT, stream.read_exact(&mut connack))
            .await
            .expect("in time")
            .expect("CONNACK");
        assert_eq!(connack_code(version, &connack), 0, "{version:?}");
    }
    harness.shutdown().await;
}

/// Once the broker is closing, even the right credentials are refused as not authorized, on TCP
/// and WebSocket, with nothing registered and no callback.
#[tokio::test(flavor = "multi_thread")]
async fn correct_credentials_are_refused_while_closing() {
    let mut harness = start().await;
    let broker = harness.take_broker();
    broker.stop_accepting();
    for version in [Version::V311, Version::V5] {
        let packet = connect_packet(version, "late", GOOD_CREDENTIALS);
        let tcp = raw_tcp_connect(harness.tcp, &packet).await;
        assert_eq!(
            connack_code(version, &tcp),
            refusal(version, true),
            "TCP {version:?}"
        );
        let ws = raw_ws_connect(harness.ws, &packet).await;
        assert_eq!(
            connack_code(version, &ws),
            refusal(version, true),
            "WS {version:?}"
        );
    }
    assert!(harness.registry.connected_ids().is_empty());
    assert_eq!(harness.connected_count(), 0);
    broker
        .shutdown(Instant::now() + Duration::from_secs(2))
        .await;
}

// --- Shutdown ---

/// Whether both ports can be bound again, as a restarted broker would bind them.
async fn ports_are_free(harness: &Harness) -> bool {
    let (tcp, ws) = (harness.tcp, harness.ws);
    eventually(move || TcpListener::bind(tcp).is_ok() && TcpListener::bind(ws).is_ok()).await
}

/// Shutdown with a TCP and a WebSocket device: both see the connection close, a later publish
/// fails, the callbacks stop, and both ports are free again, while the test runtime keeps
/// running.
#[tokio::test(flavor = "multi_thread")]
async fn shutdown_closes_every_connection_and_frees_the_ports() {
    let mut harness = start().await;
    let mut tcp_device = connect(harness.tcp, "tcp-device", PASSWORD, false)
        .await
        .expect("TCP connected");
    let mut ws_device = connect(harness.ws, "ws-device", PASSWORD, true)
        .await
        .expect("WS connected");
    tcp_device
        .publish_acked("tcp-device/stats", br#"{"bat":1}"#)
        .await;
    ws_device
        .publish_acked("ws-device/stats", br#"{"bat":2}"#)
        .await;
    let connected = harness.connected_count();
    let ready = harness.ready_ids();
    let stats = harness
        .registry
        .snapshot("tcp-device")
        .and_then(|state| state.stats);

    let broker = harness.take_broker();
    broker
        .shutdown(Instant::now() + Duration::from_secs(2))
        .await;

    tcp_device.wait_for_close().await;
    ws_device.wait_for_close().await;
    let published = harness
        .handle
        .publish("tcp-device/notify", b"{}".to_vec(), false)
        .await;
    assert!(published.is_err(), "published after shutdown");

    // Reconnecting fails; nothing changes.
    let reconnect = TcpStream::connect(harness.tcp).await;
    assert!(reconnect.is_err(), "the TCP port still accepts connections");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(harness.connected_count(), connected);
    assert_eq!(harness.ready_ids(), ready);
    assert_eq!(
        harness
            .registry
            .snapshot("tcp-device")
            .and_then(|state| state.stats),
        stats
    );
    assert!(ports_are_free(&harness).await, "a port is still bound");
}

/// A session stalled for 30 s inside the hook does not hold shutdown past its deadline: with
/// two healthy sessions beside it, shutdown with a 1 s deadline returns within 1.1 s, every
/// socket closed and the ports free.
#[tokio::test(flavor = "multi_thread")]
async fn stalled_session_does_not_hold_shutdown() {
    let mut harness = start_with(Some(PublishDelay {
        client_id: "stalled".to_owned(),
        delay: Duration::from_secs(30),
    }))
    .await;
    let mut healthy_tcp = connect(harness.tcp, "healthy-tcp", PASSWORD, false)
        .await
        .expect("connected");
    let mut healthy_ws = connect(harness.ws, "healthy-ws", PASSWORD, true)
        .await
        .expect("connected");
    let mut stalled = connect(harness.tcp, "stalled", PASSWORD, false)
        .await
        .expect("connected");
    stalled
        .client
        .publish("stalled/stats", QoS::AtLeastOnce, false, b"{}".to_vec())
        .await
        .expect("queued");
    // Let the publish reach the hook and stall there.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let broker = harness.take_broker();
    let started = Instant::now();
    broker.shutdown(started + Duration::from_secs(1)).await;
    let elapsed = started.elapsed();
    assert!(
        elapsed <= Duration::from_millis(1100),
        "shutdown took {elapsed:?}"
    );

    healthy_tcp.wait_for_close().await;
    healthy_ws.wait_for_close().await;
    stalled.wait_for_close().await;
    assert!(ports_are_free(&harness).await, "a port is still bound");
}
