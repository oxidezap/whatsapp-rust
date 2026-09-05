#![cfg(target_family = "wasm")]

use js_sys::Date;
use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_test::wasm_bindgen_test;
use whatsapp_rust_sqlite_storage::test_retry_backoff;

#[wasm_bindgen_test]
async fn retry_backoff_installs_and_cancels_the_real_timer() {
    let global: JsValue = js_sys::global().into();
    let set_key = JsValue::from_str("setTimeout");
    let clear_key = JsValue::from_str("clearTimeout");
    let original_set = js_sys::Reflect::get(&global, &set_key).expect("setTimeout");
    let original_clear = js_sys::Reflect::get(&global, &clear_key).expect("clearTimeout");
    let set_calls = Rc::new(Cell::new(0u32));
    let clear_calls = Rc::new(Cell::new(0u32));
    let set_fn: js_sys::Function = original_set.clone().dyn_into().expect("set fn");
    let clear_fn: js_sys::Function = original_clear.clone().dyn_into().expect("clear fn");
    let global_for_set = global.clone();
    let set_count = set_calls.clone();
    let set_hook = Closure::wrap(Box::new(move |handler: JsValue, timeout: JsValue| -> JsValue {
        set_count.set(set_count.get() + 1);
        set_fn.call2(&global_for_set, &handler, &timeout).expect("set")
    }) as Box<dyn FnMut(JsValue, JsValue) -> JsValue>);
    let global_for_clear = global.clone();
    let clear_count = clear_calls.clone();
    let clear_hook = Closure::wrap(Box::new(move |handle: JsValue| {
        clear_count.set(clear_count.get() + 1);
        clear_fn.call1(&global_for_clear, &handle).expect("clear");
    }) as Box<dyn FnMut(JsValue)>);
    js_sys::Reflect::set(&global, &set_key, set_hook.as_ref()).expect("install set");
    js_sys::Reflect::set(&global, &clear_key, clear_hook.as_ref()).expect("install clear");

    let mut pending = Box::pin(test_retry_backoff(1_000));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(matches!(pending.as_mut().poll(&mut cx), Poll::Pending));
    assert_eq!(set_calls.get(), 1);
    drop(pending);
    assert_eq!(clear_calls.get(), 1);
    js_sys::Reflect::set(&global, &set_key, &original_set).expect("restore set");
    js_sys::Reflect::set(&global, &clear_key, &original_clear).expect("restore clear");

    let start = Date::now();
    test_retry_backoff(10).await;
    assert!(Date::now() - start >= 5.0);
}
