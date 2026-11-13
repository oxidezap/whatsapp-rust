// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! A cancelled state-changing operation retires its instance before unlocking.

use crate::signal::OpsError;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct Held {
    // Dropped after our Drop body, so no waiter can acquire these locks before
    // a cancelled operation has made the instance unusable.
    _guards: Vec<async_lock::MutexGuardArc<()>>,
    usable: Arc<AtomicBool>,
    completed: bool,
}

impl Held {
    pub(crate) fn new(guards: Vec<async_lock::MutexGuardArc<()>>, usable: Arc<AtomicBool>) -> Self {
        Self {
            _guards: guards,
            usable,
            completed: false,
        }
    }

    /// Run `operation` under these locks. A completed operation returns its
    /// own result even if another operation retired the instance meanwhile:
    /// its writes are committed under locks it still holds, and discarding a
    /// decrypt's plaintext would lose a message whose keys are already gone.
    /// Retirement stops later operations at lock acquisition instead.
    pub(crate) async fn run<T>(
        mut self,
        operation: impl Future<Output = Result<T, OpsError>>,
    ) -> Result<T, OpsError> {
        let result = operation.await;
        self.completed = true;
        result
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if !self.completed {
            self.usable.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_completed_operation_keeps_its_result_after_another_retires_the_instance() {
        let usable = Arc::new(AtomicBool::new(true));
        let running = Held::new(Vec::new(), usable.clone());
        let cancelled = Held::new(Vec::new(), usable.clone());
        let result = futures::executor::block_on(running.run(async {
            // Another operation is dropped before it completes.
            drop(cancelled);
            Ok::<_, OpsError>("committed plaintext")
        }));
        assert_eq!(result.ok(), Some("committed plaintext"));
        assert!(
            !usable.load(Ordering::SeqCst),
            "the instance is still retired for later operations"
        );
    }
}
