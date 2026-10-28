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

//! Fatal shared-state failures, propagated without reading a poisoned guard or unwinding a task.

use std::sync::LockResult;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Shared state whose invariants can no longer be trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    /// A writer panicked while holding this component's state lock.
    #[error("{0}: state lock poisoned")]
    Poisoned(&'static str),
}

/// Drops a poisoned guard rather than exposing possibly inconsistent data.
pub(crate) fn checked<T>(result: LockResult<T>, component: &'static str) -> Result<T, StateError> {
    result.map_err(|_| StateError::Poisoned(component))
}

/// The first fatal failure wins; reporting it prevents further scheduled actions immediately.
#[derive(Clone)]
pub(crate) struct Failures {
    first: watch::Sender<Option<StateError>>,
    cancel: CancellationToken,
}

impl Failures {
    pub(crate) fn new(cancel: CancellationToken) -> Self {
        Self {
            first: watch::channel(None).0,
            cancel,
        }
    }

    pub(crate) fn report(&self, error: StateError) {
        self.first.send_if_modified(|first| {
            if first.is_some() {
                return false;
            }
            *first = Some(error);
            true
        });
        self.cancel.cancel();
    }

    pub(crate) fn error(&self) -> Option<StateError> {
        *self.first.borrow()
    }

    pub(crate) async fn failed(&self) -> StateError {
        let mut receiver = self.first.subscribe();
        loop {
            if let Some(error) = *receiver.borrow_and_update() {
                return error;
            }
            // This method holds a sender, so the channel cannot close while waiting.
            let _ = receiver.changed().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn failure_is_sticky_and_cancels_scheduling() {
        let cancel = CancellationToken::new();
        let failures = Failures::new(cancel.clone());
        let first = StateError::Poisoned("registry");
        failures.report(first);
        failures.report(StateError::Poisoned("overlay"));
        assert!(cancel.is_cancelled());
        assert_eq!(failures.failed().await, first);
        assert_eq!(failures.failed().await, first);
    }
}
