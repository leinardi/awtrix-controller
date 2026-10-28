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

//! Recurring jobs. Each job is one task that sleeps until its fire time, runs its action, asks
//! its `reschedule` function for the next fire time, and goes round again, until the scheduler is
//! stopped.
//!
//! Fire times are wall-clock instants read against the injected [`Clock`]; the wait itself is a
//! tokio sleep, so a test drives both with a paused runtime and a
//! [`TokioClock`](crate::clock::TokioClock).

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use jiff::Timestamp;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::info;

use crate::clock::Clock;
use crate::state::Failures;

/// Runs recurring jobs until stopped.
pub struct Scheduler {
    clock: Arc<dyn Clock>,
    cancel: CancellationToken,
    tasks: TaskTracker,
    failures: Failures,
}

impl Scheduler {
    /// A scheduler reading the time from `clock`. Cancelling `parent` stops it as
    /// [`cancel`](Self::cancel) does.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, parent: &CancellationToken) -> Self {
        let cancel = parent.child_token();
        Self {
            clock,
            failures: Failures::new(cancel.clone()),
            cancel,
            tasks: TaskTracker::new(),
        }
    }

    pub(crate) fn failures(&self) -> Failures {
        self.failures.clone()
    }

    /// Registers a recurring job: it fires at `fire_at` (at once if that has passed), runs
    /// `action`, then fires again at `reschedule(fire_at)`, computed from the time it was due
    /// rather than the time it ran, and so on.
    ///
    /// An action that has started always runs to completion; stopping the scheduler only
    /// prevents the next one. Must be called within a tokio runtime.
    pub fn schedule<Action, ActionFuture, Reschedule>(
        &self,
        name: &str,
        fire_at: Timestamp,
        mut action: Action,
        reschedule: Reschedule,
    ) where
        Action: FnMut() -> ActionFuture + Send + 'static,
        ActionFuture: Future<Output = ()> + Send + 'static,
        Reschedule: FnMut(Timestamp) -> Timestamp + Send + 'static,
    {
        self.schedule_with_due(name, fire_at, move |_| action(), reschedule);
    }

    /// Supplies the due instant directly to boundary-driven controllers, without shared timer state.
    pub(crate) fn schedule_with_due<Action, ActionFuture, Reschedule>(
        &self,
        name: &str,
        fire_at: Timestamp,
        mut action: Action,
        mut reschedule: Reschedule,
    ) where
        Action: FnMut(Timestamp) -> ActionFuture + Send + 'static,
        ActionFuture: Future<Output = ()> + Send + 'static,
        Reschedule: FnMut(Timestamp) -> Timestamp + Send + 'static,
    {
        info!(job = name, fire_at = %fire_at, "scheduler: job registered");

        let clock = Arc::clone(&self.clock);
        let cancel = self.cancel.clone();
        self.tasks.spawn(async move {
            let mut fire_at = fire_at;
            loop {
                let delay = Duration::try_from(fire_at.duration_since(clock.now()))
                    .unwrap_or(Duration::ZERO);

                tokio::select! {
                    biased;
                    () = cancel.cancelled() => return,
                    () = tokio::time::sleep(delay) => {}
                }

                action(fire_at).await;

                if cancel.is_cancelled() {
                    return;
                }
                fire_at = reschedule(fire_at);
            }
        });
    }

    /// Stops every job from firing again, without waiting for an action already running.
    pub fn cancel(&self) {
        self.cancel.cancel();
        self.tasks.close();
    }

    /// Stops every job from firing again and waits for the actions already running to finish.
    pub async fn stop(&self) {
        self.cancel();
        self.tasks.wait().await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use jiff::SignedDuration;
    use tokio::sync::mpsc;

    use super::*;
    use crate::clock::TokioClock;

    fn base_time() -> Timestamp {
        "2024-06-15T12:00:00Z".parse().expect("timestamp")
    }

    fn far_future() -> Timestamp {
        base_time() + SignedDuration::from_hours(24 * 365)
    }

    fn scheduler(parent: &CancellationToken) -> Scheduler {
        Scheduler::new(Arc::new(TokioClock::new(base_time())), parent)
    }

    /// An action that counts its calls and reports each one on a channel.
    fn counting(
        counter: &Arc<AtomicUsize>,
        fired: &mpsc::UnboundedSender<Timestamp>,
        clock: TokioClock,
    ) -> impl FnMut() -> std::future::Ready<()> + Send + 'static {
        let counter = Arc::clone(counter);
        let fired = fired.clone();
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
            let _ = fired.send(clock.now());
            std::future::ready(())
        }
    }

    /// Go: `TestJobFiresOnceThenReschedules`
    #[tokio::test(start_paused = true)]
    async fn job_fires_once_then_reschedules() {
        let parent = CancellationToken::new();
        let scheduler = scheduler(&parent);
        let counter = Arc::new(AtomicUsize::new(0));
        let (fired_tx, mut fired_rx) = mpsc::unbounded_channel();
        let clock = TokioClock::new(base_time());

        let past = base_time() - SignedDuration::from_mins(1);
        scheduler.schedule("once", past, counting(&counter, &fired_tx, clock), |_| {
            far_future()
        });

        let fired_at = fired_rx.recv().await.expect("fired");
        assert_eq!(fired_at, base_time(), "a past fire time fires at once");
        scheduler.stop().await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// Go: `TestStopBeforeFire`
    #[tokio::test(start_paused = true)]
    async fn stop_before_fire() {
        let parent = CancellationToken::new();
        let scheduler = scheduler(&parent);
        let counter = Arc::new(AtomicUsize::new(0));
        let (fired_tx, _fired_rx) = mpsc::unbounded_channel();
        let clock = TokioClock::new(base_time());

        scheduler.schedule(
            "future",
            far_future(),
            counting(&counter, &fired_tx, clock),
            |fired| fired + SignedDuration::from_hours(24),
        );

        scheduler.stop().await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    /// Go: `TestContextCancellationStopsRescheduling`
    #[tokio::test(start_paused = true)]
    async fn cancelled_parent_stops_everything() {
        let parent = CancellationToken::new();
        let scheduler = scheduler(&parent);
        parent.cancel();
        let counter = Arc::new(AtomicUsize::new(0));
        let (fired_tx, _fired_rx) = mpsc::unbounded_channel();
        let clock = TokioClock::new(base_time());

        let past = base_time() - SignedDuration::from_mins(1);
        scheduler.schedule(
            "ctx-cancel",
            past,
            counting(&counter, &fired_tx, clock),
            |fired| fired + SignedDuration::from_secs(1),
        );

        scheduler.stop().await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    /// Go: `TestTwoConcurrentJobsBothFire`
    #[tokio::test(start_paused = true)]
    async fn two_concurrent_jobs_both_fire() {
        let parent = CancellationToken::new();
        let scheduler = scheduler(&parent);
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let (fired_tx, mut fired_rx) = mpsc::unbounded_channel();
        let clock = TokioClock::new(base_time());

        scheduler.schedule(
            "jobA",
            base_time() - SignedDuration::from_mins(2),
            counting(&first, &fired_tx, clock),
            |_| far_future(),
        );
        scheduler.schedule(
            "jobB",
            base_time() - SignedDuration::from_mins(1),
            counting(&second, &fired_tx, clock),
            |_| far_future(),
        );

        fired_rx.recv().await.expect("first fired");
        fired_rx.recv().await.expect("second fired");
        scheduler.stop().await;
        assert_eq!(first.load(Ordering::SeqCst), 1);
        assert_eq!(second.load(Ordering::SeqCst), 1);
    }

    /// A recurring job fires at each fire time, measured on the wall clock, and the next fire
    /// time is computed from the one that was due.
    #[tokio::test(start_paused = true)]
    async fn job_recurs_at_each_fire_time() {
        let parent = CancellationToken::new();
        let scheduler = scheduler(&parent);
        let counter = Arc::new(AtomicUsize::new(0));
        let (fired_tx, mut fired_rx) = mpsc::unbounded_channel();
        let clock = TokioClock::new(base_time());

        let first = base_time() + SignedDuration::from_mins(10);
        scheduler.schedule(
            "every-hour",
            first,
            counting(&counter, &fired_tx, clock),
            |fired| fired + SignedDuration::from_hours(1),
        );

        for hours in 0..3 {
            let fired_at = fired_rx.recv().await.expect("fired");
            assert_eq!(fired_at, first + SignedDuration::from_hours(hours));
        }
        scheduler.stop().await;
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    /// `stop` waits for an action that is running, and no further action starts.
    #[tokio::test(start_paused = true)]
    async fn stop_waits_for_in_flight_action() {
        let parent = CancellationToken::new();
        let scheduler = scheduler(&parent);
        let finished = Arc::new(AtomicUsize::new(0));
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();

        let action_finished = Arc::clone(&finished);
        scheduler.schedule(
            "slow",
            base_time(),
            move || {
                let finished = Arc::clone(&action_finished);
                let started = started_tx.clone();
                async move {
                    let _ = started.send(());
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    finished.fetch_add(1, Ordering::SeqCst);
                }
            },
            |fired| fired + SignedDuration::from_secs(1),
        );

        started_rx.recv().await.expect("action started");
        scheduler.stop().await;
        assert_eq!(
            finished.load(Ordering::SeqCst),
            1,
            "stop returned before the action"
        );
        assert!(
            started_rx.try_recv().is_err(),
            "a second action started after stop"
        );
    }

    /// `cancel` returns without waiting for a running action.
    #[tokio::test(start_paused = true)]
    async fn cancel_does_not_wait() {
        let parent = CancellationToken::new();
        let scheduler = scheduler(&parent);
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();

        scheduler.schedule(
            "slow",
            base_time(),
            move || {
                let started = started_tx.clone();
                async move {
                    let _ = started.send(());
                    std::future::pending::<()>().await;
                }
            },
            |fired| fired,
        );

        started_rx.recv().await.expect("action started");
        scheduler.cancel();
    }
}
