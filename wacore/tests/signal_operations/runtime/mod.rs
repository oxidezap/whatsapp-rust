// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! A runtime for tests driven by `block_on`: blocking work runs inline.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use wacore::runtime::{AbortHandle, Runtime};

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub struct InlineRuntime;

impl Runtime for InlineRuntime {
    fn spawn(&self, future: BoxFuture) -> AbortHandle {
        std::thread::spawn(move || futures::executor::block_on(future));
        AbortHandle::new(|| {})
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move { std::thread::sleep(duration) })
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture {
        Box::pin(async move { f() })
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }
}
