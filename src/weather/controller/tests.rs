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

use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::mpsc;
use tracing::level_filters::LevelFilter;

use super::*;
use crate::clock::TokioClock;
use crate::test_support::Captured;

fn instant(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn base_time() -> Timestamp {
    instant("2025-01-01T12:00:00Z")
}

/// The Go tests' configuration.
fn config() -> WeatherConfig {
    WeatherConfig {
        enabled: true,
        poll_interval_minutes: 15,
        overlay_horizon_minutes: 60,
        notification_horizon_hours: 8,
        notification_repeat_minutes: 60,
        inactive_after_missing_polls: 2,
        data_stale_ttl_minutes: 45,
        notification_text_repeat: 3,
        gust_warn_kmh: 45.0,
        gust_severe_kmh: 60.0,
        heavy_rain_mm_per_15_min: 5.0,
        fog_visibility_warn_m: 1000.0,
        fog_visibility_severe_m: 200.0,
        frost_temp_c: 2.0,
        frost_dew_point_delta_c: 2.0,
        frost_warn_precip_window_h: 2.0,
        frost_warn_precip_mm: 0.2,
        notify_thunderstorm: true,
        notify_freezing_precip: true,
        notify_frost_risk: true,
        notify_heavy_rain: true,
        notify_strong_gusts: true,
        notify_snow: true,
        notify_fog: true,
        rtttl: String::new(),
    }
}

fn location() -> LocationConfig {
    LocationConfig {
        latitude: 48.0,
        longitude: 11.0,
    }
}

/// One step at [`base_time`] with WMO `code` and nothing else notable.
fn points(code: i64) -> Vec<ForecastPoint> {
    vec![ForecastPoint {
        time: base_time(),
        weather_code: Some(code),
        precipitation: Some(2.0),
        snowfall: Some(0.0),
        temperature_2m: Some(15.0),
        dew_point_2m: Some(10.0),
        wind_gusts_10m: Some(30.0),
        visibility: None,
    }]
}

/// What one fetch returns.
#[derive(Clone)]
enum Response {
    Points(Vec<ForecastPoint>),
    Fail,
    /// Never answers until cancelled.
    Hang,
}

/// Records what the controller publishes.
struct Recorder {
    clients: Vec<String>,
    published: mpsc::UnboundedSender<(String, Arc<Notification>)>,
    overlays: mpsc::UnboundedSender<Option<OverlayEffect>>,
}

impl Publisher for Recorder {
    fn connected_ids(&self) -> Vec<String> {
        self.clients.clone()
    }

    fn publish(&self, client_id: &str, notification: Arc<Notification>) -> PublishFuture {
        if client_id == "unreachable" {
            return Box::pin(std::future::ready(Err("device unreachable".into())));
        }
        let _ = self.published.send((client_id.to_owned(), notification));
        Box::pin(std::future::ready(Ok(())))
    }

    fn set_overlay(
        &self,
        overlay: Option<OverlayEffect>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let _ = self.overlays.send(overlay);
        Box::pin(std::future::ready(()))
    }
}

/// The next message, failing the test if none arrives within a virtual day: on a paused clock
/// the recurring poll keeps time moving, so an unbounded wait would never end.
async fn next<T>(receiver: &mut mpsc::UnboundedReceiver<T>) -> T {
    tokio::time::timeout(Duration::from_hours(24), receiver.recv())
        .await
        .expect("nothing arrived within a day")
        .expect("channel open")
}

/// A started controller on a paused clock, and what it does.
struct Harness {
    controller: Arc<Controller>,
    scheduler: Scheduler,
    cancel: CancellationToken,
    published: mpsc::UnboundedReceiver<(String, Arc<Notification>)>,
    overlays: mpsc::UnboundedReceiver<Option<OverlayEffect>>,
    /// The time of each fetch.
    fetches: mpsc::UnboundedReceiver<Timestamp>,
}

impl Harness {
    /// Stops polling, waiting for a poll in progress.
    async fn stop(&self) {
        self.scheduler.stop().await;
    }
}

/// Starts a controller whose `n`th fetch returns `responses[n]`, the last one repeating, its
/// steps shifted by the time elapsed since [`base_time`].
fn harness(
    config: WeatherConfig,
    responses: Vec<Response>,
    clients: &[&str],
    timezone: &str,
) -> Harness {
    let clock = TokioClock::new(base_time());
    let cancel = CancellationToken::new();
    let scheduler = Scheduler::new(Arc::new(clock), &cancel);
    let (published_tx, published) = mpsc::unbounded_channel();
    let (overlays_tx, overlays) = mpsc::unbounded_channel();
    let (fetches_tx, fetches) = mpsc::unbounded_channel();

    let calls = Arc::new(AtomicUsize::new(0));
    let fetch: FetchFunction = Arc::new(move |cancel: CancellationToken, _, _, _| {
        let call = calls.fetch_add(1, Ordering::SeqCst);
        let response = responses[call.min(responses.len() - 1)].clone();
        let _ = fetches_tx.send(clock.now());
        // The forecast moves with the clock, as a real one would.
        let elapsed = clock.now().duration_since(base_time());
        Box::pin(async move {
            match response {
                Response::Points(mut points) => {
                    for point in &mut points {
                        point.time += elapsed;
                    }
                    Ok(points)
                }
                Response::Fail => Err(FetchError::HttpStatus(503)),
                Response::Hang => {
                    cancel.cancelled().await;
                    Err(FetchError::Cancelled)
                }
            }
        })
    });
    let recorder = Arc::new(Recorder {
        clients: clients.iter().map(|&client| client.to_owned()).collect(),
        published: published_tx,
        overlays: overlays_tx,
    });

    let controller = Controller::with_fetch_function(
        config,
        &location(),
        timezone,
        Arc::new(clock),
        fetch,
        recorder,
        &cancel,
    );
    controller.start(&scheduler);
    Harness {
        controller,
        scheduler,
        cancel,
        published,
        overlays,
        fetches,
    }
}

/// Go: `TestControllerPollSuccess`
#[tokio::test(start_paused = true)]
async fn poll_success() {
    let mut harness = harness(
        config(),
        vec![Response::Points(points(95))],
        &["device1"],
        "UTC",
    );
    assert_eq!(
        next(&mut harness.overlays).await,
        Some(OverlayEffect::Thunder)
    );
    let (client_id, _) = next(&mut harness.published).await;
    assert_eq!(client_id, "device1");
    harness.stop().await;
}

/// Go: `TestControllerPollFetchError`
#[tokio::test(start_paused = true)]
async fn poll_fetch_error_publishes_nothing() {
    let mut harness = harness(config(), vec![Response::Fail], &["device1"], "UTC");
    next(&mut harness.fetches).await;
    harness.stop().await;
    assert!(harness.published.try_recv().is_err());
    assert!(
        harness.overlays.try_recv().is_err(),
        "the overlay was already none"
    );
}

/// Go: `TestControllerOverlayDedup`
#[tokio::test(start_paused = true)]
async fn overlay_is_applied_once_while_unchanged() {
    let mut harness = harness(
        config(),
        vec![Response::Points(points(95))],
        &["device1"],
        "UTC",
    );
    next(&mut harness.fetches).await;
    next(&mut harness.fetches).await;
    harness.stop().await;
    assert_eq!(
        next(&mut harness.overlays).await,
        Some(OverlayEffect::Thunder)
    );
    assert!(
        harness.overlays.try_recv().is_err(),
        "the same overlay was applied twice"
    );
}

/// Go: `TestControllerNotifySentToAllClients`
#[tokio::test(start_paused = true)]
async fn notification_is_sent_to_all_clients() {
    let mut harness = harness(
        config(),
        vec![Response::Points(points(95))],
        &["device1", "device2"],
        "UTC",
    );
    let (first, _) = next(&mut harness.published).await;
    let (second, _) = next(&mut harness.published).await;
    assert_eq!([first, second], ["device1", "device2"]);
    harness.stop().await;
}

/// Go: `TestControllerNoNotifyWhenDisabled`
#[tokio::test(start_paused = true)]
async fn no_notification_when_disabled() {
    let disabled = WeatherConfig {
        notify_thunderstorm: false,
        ..config()
    };
    let mut harness = harness(
        disabled,
        vec![Response::Points(points(95))],
        &["device1"],
        "UTC",
    );
    assert_eq!(
        next(&mut harness.overlays).await,
        Some(OverlayEffect::Thunder)
    );
    harness.stop().await;
    assert!(harness.published.try_recv().is_err());
}

/// Go: `TestControllerOnDeviceConnectedDelivers`
#[tokio::test(start_paused = true)]
async fn device_connecting_within_the_interval_gets_the_warnings() {
    let mut harness = harness(config(), vec![Response::Points(points(95))], &[], "UTC");
    next(&mut harness.overlays).await;
    harness.stop().await;
    assert!(
        harness.published.try_recv().is_err(),
        "nothing was connected"
    );

    harness.controller.on_device_connected("device1").await;
    let (client_id, notification) = next(&mut harness.published).await;
    assert_eq!(client_id, "device1");
    assert_eq!(
        notification.content.text,
        Some(TextContent::plain("Thunderstorm from 12:00"))
    );
}

/// Go: `TestControllerOnDeviceConnectedStale`
#[tokio::test(start_paused = true)]
async fn device_connecting_after_the_interval_gets_nothing() {
    let mut harness = harness(config(), vec![Response::Points(points(95))], &[], "UTC");
    next(&mut harness.overlays).await;
    harness.stop().await;

    tokio::time::advance(Duration::from_mins(61)).await;
    harness.controller.on_device_connected("device1").await;
    assert!(harness.published.try_recv().is_err());
}

/// Go: `TestControllerOnDeviceConnectedNoPendingEvents`
#[tokio::test(start_paused = true)]
async fn device_connecting_without_events_gets_nothing() {
    let mut clear = points(0);
    clear[0].visibility = Some(10_000.0);
    let mut harness = harness(config(), vec![Response::Points(clear)], &[], "UTC");
    next(&mut harness.fetches).await;
    harness.stop().await;

    harness.controller.on_device_connected("device1").await;
    assert!(harness.published.try_recv().is_err());
}

/// Go: `TestControllerSevereNotificationIsYellow`
#[tokio::test(start_paused = true)]
async fn severe_notification_is_yellow() {
    let mut harness = harness(
        config(),
        vec![Response::Points(points(96))],
        &["device1"],
        "UTC",
    );
    let (_, notification) = next(&mut harness.published).await;
    assert_eq!(notification.content.color, "#FFFF00");
    harness.stop().await;
}

/// Go: `TestControllerWarningNotificationHasNoColor`
#[tokio::test(start_paused = true)]
async fn warning_notification_has_no_color() {
    let mut harness = harness(
        config(),
        vec![Response::Points(points(95))],
        &["device1"],
        "UTC",
    );
    let (_, notification) = next(&mut harness.published).await;
    assert_eq!(notification.content.color, "");
    harness.stop().await;
}

/// Go: `TestControllerRescheduleInterval`. The first poll runs at once, the next ones every
/// `poll_interval_minutes`, which the Go original could not reach.
#[tokio::test(start_paused = true)]
async fn polls_now_and_every_interval() {
    let mut harness = harness(config(), vec![Response::Points(Vec::new())], &[], "UTC");
    for minutes in [0, 15, 30] {
        let fetched = next(&mut harness.fetches).await;
        assert_eq!(fetched, base_time() + SignedDuration::from_mins(minutes));
    }
    harness.stop().await;
}

/// The whole notification: text with the start in the configured time zone, icon, repeat,
/// melody and wakeup.
#[tokio::test(start_paused = true)]
async fn notification_payload() {
    let with_melody = WeatherConfig {
        rtttl: "alarm:d=4,o=5,b=100:c".to_owned(),
        ..config()
    };
    let mut harness = harness(
        with_melody,
        vec![Response::Points(points(95))],
        &["device1"],
        "Europe/Berlin",
    );
    let (_, notification) = next(&mut harness.published).await;
    harness.stop().await;
    assert_eq!(
        *notification,
        Notification {
            content: AppContent {
                text: Some(TextContent::plain("Thunderstorm from 13:00")),
                icon: "49299".to_owned(),
                repeat: Some(3),
                ..AppContent::default()
            },
            rtttl: "alarm:d=4,o=5,b=100:c".to_owned(),
            wakeup: true,
            ..Notification::default()
        }
    );
}

/// Every event type's text and icon, as Go wrote them.
#[test]
fn event_labels_and_icons() {
    let cases = [
        (EventType::Thunderstorm, "Thunderstorm", "49299"),
        (EventType::FreezingPrecip, "Freezing rain", "49301"),
        (EventType::FrostRisk, "Frost risk", "4807"),
        (EventType::HeavyRain, "Heavy rain", "49300"),
        (EventType::StrongGusts, "Strong gusts", "3363"),
        (EventType::Snow, "Snow", "2289"),
        (EventType::Fog, "Fog", "17056"),
    ];
    for (event_type, label, icon) in cases {
        assert_eq!(
            (event_label(event_type), event_icon(event_type)),
            (label, icon)
        );
    }
}

/// A failed fetch clears an overlay that was set; the next good poll sets it again.
#[tokio::test(start_paused = true)]
async fn failed_fetch_clears_the_overlay() {
    let responses = vec![
        Response::Points(points(95)),
        Response::Fail,
        Response::Points(points(95)),
    ];
    let mut harness = harness(config(), responses, &[], "UTC");
    assert_eq!(
        next(&mut harness.overlays).await,
        Some(OverlayEffect::Thunder)
    );
    assert_eq!(next(&mut harness.overlays).await, None);
    assert_eq!(
        next(&mut harness.overlays).await,
        Some(OverlayEffect::Thunder)
    );
    harness.stop().await;
}

/// A fetch that does not answer within the poll timeout counts as failed.
#[tokio::test(start_paused = true)]
async fn hung_fetch_times_out() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);

    let responses = vec![Response::Points(points(95)), Response::Hang];
    let mut harness = harness(config(), responses, &[], "UTC");
    assert_eq!(
        next(&mut harness.overlays).await,
        Some(OverlayEffect::Thunder)
    );
    assert_eq!(next(&mut harness.overlays).await, None);
    let log = captured.text();
    assert!(
        log.contains("weather: fetch failed; clearing overlay"),
        "{log}"
    );
    harness.stop().await;
}

/// Shutting down abandons a fetch in flight and leaves the overlay alone.
#[tokio::test(start_paused = true)]
async fn cancelling_abandons_the_fetch_in_flight() {
    let responses = vec![Response::Points(points(95)), Response::Hang];
    let mut harness = harness(config(), responses, &[], "UTC");
    assert_eq!(
        next(&mut harness.overlays).await,
        Some(OverlayEffect::Thunder)
    );
    next(&mut harness.fetches).await;
    next(&mut harness.fetches).await;

    harness.cancel.cancel();
    harness.stop().await;
    assert!(
        harness.overlays.try_recv().is_err(),
        "the overlay changed on shutdown"
    );
}

/// A device that cannot be reached is logged; the others still get the warning.
#[tokio::test(start_paused = true)]
async fn publish_failure_is_logged_and_skipped() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);

    let mut harness = harness(
        config(),
        vec![Response::Points(points(95))],
        &["unreachable", "device1"],
        "UTC",
    );
    let (client_id, _) = next(&mut harness.published).await;
    assert_eq!(client_id, "device1");
    harness.stop().await;
    let log = captured.text();
    assert!(
        log.contains("weather: publish notification failed"),
        "{log}"
    );
    assert!(log.contains(r#""client_id":"unreachable""#), "{log}");
}

/// An empty time zone is UTC, with a warning, as in Go.
#[tokio::test(start_paused = true)]
async fn empty_timezone_is_utc() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);

    let mut harness = harness(
        config(),
        vec![Response::Points(points(95))],
        &["device1"],
        "",
    );
    let (_, notification) = next(&mut harness.published).await;
    assert_eq!(
        notification.content.text,
        Some(TextContent::plain("Thunderstorm from 12:00"))
    );
    harness.stop().await;
    assert!(
        captured
            .text()
            .contains("weather: no timezone configured; using UTC")
    );
}
