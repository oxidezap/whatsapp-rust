// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! A cancelled state-changing operation retires its instance before unlocking.

use crate::OpsError;
use crate::error::interrupted_operation;
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

    pub(crate) async fn run<T>(
        mut self,
        operation: impl Future<Output = Result<T, OpsError>>,
    ) -> Result<T, OpsError> {
        let result = operation.await;
        self.completed = true;
        if self.usable.load(Ordering::SeqCst) {
            result
        } else {
            Err(interrupted_operation())
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if !self.completed {
            self.usable.store(false, Ordering::SeqCst);
        }
    }
}
