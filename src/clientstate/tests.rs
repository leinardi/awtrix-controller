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

use std::thread;

use tracing::level_filters::LevelFilter;

use super::*;
use crate::test_support::Captured;

fn stats(app: &str, bat: i64) -> Stats {
    Stats {
        app: app.to_owned(),
        bat,
        ..Stats::default()
    }
}

/// Go: `TestConnectedIDs_SortedAfterRegisterTwo`
#[test]
fn connected_ids_are_sorted() {
    let registry = Registry::new();
    registry.register("zebra").expect("registry");
    registry.register("alpha").expect("registry");
    assert_eq!(
        registry.connected_ids().expect("registry"),
        ["alpha", "zebra"]
    );
}

/// Go: `TestUpdateStats_IndependentClients`
#[test]
fn stats_of_one_client_do_not_affect_another() {
    let registry = Registry::new();
    registry.register("client-a").expect("registry");
    registry.register("client-b").expect("registry");
    registry
        .update_stats("client-a", stats("clockface", 90))
        .expect("registry");

    let first = registry
        .snapshot("client-a")
        .expect("registry")
        .expect("client-a");
    let second = registry
        .snapshot("client-b")
        .expect("registry")
        .expect("client-b");
    assert_eq!(first.stats.as_deref(), Some(&stats("clockface", 90)));
    assert_eq!(second.stats, None);
}

/// Go: `TestUnregister_RemovesClient`
#[test]
fn unregister_removes_the_client() {
    let registry = Registry::new();
    registry.register("dev-1").expect("registry");
    registry.register("dev-2").expect("registry");
    registry.unregister("dev-1").expect("registry");
    registry.unregister("never-registered").expect("registry");
    assert_eq!(registry.connected_ids().expect("registry"), ["dev-2"]);
}

/// Go: `TestUpdateStats_UnknownClient_NoPanic`. The warning, which Go could not assert, is
/// checked too.
#[test]
fn stats_for_an_unknown_client_are_dropped_with_a_warning() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);
    let registry = Registry::new();
    registry
        .update_stats("ghost", Stats::default())
        .expect("registry");
    assert!(registry.snapshot("ghost").expect("registry").is_none());
    let log = captured.text();
    assert!(log.contains("not registered"), "{log}");
    assert!(log.contains(r#""client_id":"ghost""#), "{log}");
}

/// Go: `TestSnapshot_UnknownClient_ReturnsFalse`
#[test]
fn snapshot_of_an_unknown_client_is_none() {
    assert_eq!(
        Registry::new().snapshot("nonexistent").expect("registry"),
        None
    );
}

/// Go: `TestUpdateCurrentApp`
#[test]
fn current_app_is_stored() {
    let registry = Registry::new();
    registry.register("dev-1").expect("registry");
    registry
        .update_current_app("dev-1", "clock")
        .expect("registry");
    registry
        .update_current_app("ghost", "clock")
        .expect("registry");
    assert_eq!(
        registry
            .snapshot("dev-1")
            .expect("registry")
            .expect("dev-1")
            .current_app,
        "clock"
    );
    assert_eq!(registry.snapshot("ghost").expect("registry"), None);
}

/// Go: `TestSnapshot_ReturnsCopy`
#[test]
fn snapshot_is_a_copy() {
    let registry = Registry::new();
    registry.register("dev-1").expect("registry");
    registry
        .update_current_app("dev-1", "original")
        .expect("registry");
    let mut snapshot = registry
        .snapshot("dev-1")
        .expect("registry")
        .expect("dev-1");
    snapshot.current_app = "mutated".to_owned();
    assert_eq!(
        registry
            .snapshot("dev-1")
            .expect("registry")
            .expect("dev-1")
            .current_app,
        "original"
    );
}

/// Registering again resets the state, as Go's `Register` did.
#[test]
fn register_again_resets_the_state() {
    let registry = Registry::new();
    registry.register("dev-1").expect("registry");
    registry
        .update_current_app("dev-1", "clock")
        .expect("registry");
    registry.register("dev-1").expect("registry");
    assert_eq!(
        registry.snapshot("dev-1").expect("registry"),
        Some(ClientState::default())
    );
}

/// Go: `TestConcurrentAccess`
#[test]
fn concurrent_access() {
    const CONCURRENCY: i64 = 50;
    let registry = Arc::new(Registry::new());
    let mut handles = Vec::new();
    for index in 0..CONCURRENCY {
        let client_id = format!("client-{}", index % 26);
        let writer = Arc::clone(&registry);
        let id = client_id.clone();
        handles.push(thread::spawn(move || {
            writer.register(&id).expect("registry");
        }));
        let updater = Arc::clone(&registry);
        let id = client_id.clone();
        handles.push(thread::spawn(move || {
            updater
                .update_stats(&id, stats("", index))
                .expect("registry");
        }));
        let lister = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            let _ = lister.connected_ids().expect("registry");
        }));
        let reader = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            let _ = reader.snapshot(&client_id).expect("registry");
        }));
    }
    for handle in handles {
        handle.join().expect("thread");
    }
    assert_eq!(registry.connected_ids().expect("registry").len(), 26);
}

#[test]
fn poisoned_registry_rejects_reads_and_writes() {
    let registry = Arc::new(Registry::new());
    poison(&registry);
    let error = StateError::Poisoned("client registry");
    assert_eq!(registry.register("device"), Err(error));
    assert_eq!(registry.unregister("device"), Err(error));
    assert_eq!(
        registry.update_stats("device", Stats::default()),
        Err(error)
    );
    assert_eq!(registry.update_current_app("device", "Time"), Err(error));
    assert_eq!(registry.connected_ids(), Err(error));
    assert_eq!(registry.snapshot("device"), Err(error));
}

pub(crate) fn poison(registry: &Arc<Registry>) {
    let writer = Arc::clone(registry);
    assert!(
        thread::spawn(move || {
            let _guard = writer.clients.write().expect("healthy lock");
            panic!("simulate interrupted state mutation");
        })
        .join()
        .is_err()
    );
}
