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
    registry.register("zebra");
    registry.register("alpha");
    assert_eq!(registry.connected_ids(), ["alpha", "zebra"]);
}

/// Go: `TestUpdateStats_IndependentClients`
#[test]
fn stats_of_one_client_do_not_affect_another() {
    let registry = Registry::new();
    registry.register("client-a");
    registry.register("client-b");
    registry.update_stats("client-a", stats("clockface", 90));

    let first = registry.snapshot("client-a").expect("client-a");
    let second = registry.snapshot("client-b").expect("client-b");
    assert_eq!(first.stats.as_deref(), Some(&stats("clockface", 90)));
    assert_eq!(second.stats, None);
}

/// Go: `TestUnregister_RemovesClient`
#[test]
fn unregister_removes_the_client() {
    let registry = Registry::new();
    registry.register("dev-1");
    registry.register("dev-2");
    registry.unregister("dev-1");
    registry.unregister("never-registered");
    assert_eq!(registry.connected_ids(), ["dev-2"]);
}

/// Go: `TestUpdateStats_UnknownClient_NoPanic`. The warning, which Go could not assert, is
/// checked too.
#[test]
fn stats_for_an_unknown_client_are_dropped_with_a_warning() {
    let captured = Captured::default();
    let _guard = captured.install(LevelFilter::WARN);
    let registry = Registry::new();
    registry.update_stats("ghost", Stats::default());
    assert!(registry.snapshot("ghost").is_none());
    let log = captured.text();
    assert!(log.contains("not registered"), "{log}");
    assert!(log.contains(r#""client_id":"ghost""#), "{log}");
}

/// Go: `TestSnapshot_UnknownClient_ReturnsFalse`
#[test]
fn snapshot_of_an_unknown_client_is_none() {
    assert_eq!(Registry::new().snapshot("nonexistent"), None);
}

/// Go: `TestUpdateCurrentApp`
#[test]
fn current_app_is_stored() {
    let registry = Registry::new();
    registry.register("dev-1");
    registry.update_current_app("dev-1", "clock");
    registry.update_current_app("ghost", "clock");
    assert_eq!(
        registry.snapshot("dev-1").expect("dev-1").current_app,
        "clock"
    );
    assert_eq!(registry.snapshot("ghost"), None);
}

/// Go: `TestSnapshot_ReturnsCopy`
#[test]
fn snapshot_is_a_copy() {
    let registry = Registry::new();
    registry.register("dev-1");
    registry.update_current_app("dev-1", "original");
    let mut snapshot = registry.snapshot("dev-1").expect("dev-1");
    snapshot.current_app = "mutated".to_owned();
    assert_eq!(
        registry.snapshot("dev-1").expect("dev-1").current_app,
        "original"
    );
}

/// Registering again resets the state, as Go's `Register` did.
#[test]
fn register_again_resets_the_state() {
    let registry = Registry::new();
    registry.register("dev-1");
    registry.update_current_app("dev-1", "clock");
    registry.register("dev-1");
    assert_eq!(registry.snapshot("dev-1"), Some(ClientState::default()));
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
        handles.push(thread::spawn(move || writer.register(&id)));
        let updater = Arc::clone(&registry);
        let id = client_id.clone();
        handles.push(thread::spawn(move || {
            updater.update_stats(&id, stats("", index));
        }));
        let lister = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            let _ = lister.connected_ids();
        }));
        let reader = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            let _ = reader.snapshot(&client_id);
        }));
    }
    for handle in handles {
        handle.join().expect("thread");
    }
    assert_eq!(registry.connected_ids().len(), 26);
}
