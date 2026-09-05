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
async fn retry_backoff_uses_and_cancels_a_real_wasm_timer() {
    let global: JsValue = js_sys::global().into();
    let set_key = JsValue::from_str("setTimeout");
    let clear_key = JsValue::from_str("clearTimeout");
    let original_set = js_sys::Reflect::get(&global, &set_key).expect("setTimeout");
    let original_clear = js_sys::Reflect::get(&global, &clear_key).expect("clearTimeout");
    let set_calls = Rc::new(Cell::new(0u32));
    let clear_calls = Rc::new(Cell::new(0u32));

    let set_fn: js_sys::Function = original_set.clone().dyn_into().expect("setTimeout fn");
    let clear_fn: js_sys::Function = original_clear.clone().dyn_into().expect("clearTimeout fn");
    let global_for_set = global.clone();
    let set_calls_for_hook = set_calls.clone();
    let set_hook = Closure::wrap(
        Box::new(move |handler: JsValue, timeout: JsValue| -> JsValue {
            set_calls_for_hook.set(set_calls_for_hook.get() + 1);
            set_fn
                .call2(&global_for_set, &handler, &timeout)
                .expect("setTimeout")
        }) as Box<dyn FnMut(JsValue, JsValue) -> JsValue>,
    );
    let global_for_clear = global.clone();
    let clear_calls_for_hook = clear_calls.clone();
    let clear_hook = Closure::wrap(Box::new(move |handle: JsValue| {
        clear_calls_for_hook.set(clear_calls_for_hook.get() + 1);
        clear_fn
            .call1(&global_for_clear, &handle)
            .expect("clearTimeout");
    }) as Box<dyn FnMut(JsValue)>);
    js_sys::Reflect::set(&global, &set_key, set_hook.as_ref()).expect("install setTimeout hook");
    js_sys::Reflect::set(&global, &clear_key, clear_hook.as_ref())
        .expect("install clearTimeout hook");

    let mut pending = Box::pin(test_retry_backoff(1_000));
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(pending.as_mut().poll(&mut context), Poll::Pending));
    assert_eq!(set_calls.get(), 1, "poll must install the browser timer");
    drop(pending);
    assert_eq!(clear_calls.get(), 1, "dropping must call clearTimeout");

    js_sys::Reflect::set(&global, &set_key, original_set).expect("restore setTimeout");
    js_sys::Reflect::set(&global, &clear_key, original_clear).expect("restore clearTimeout");

    let start = Date::now();
    test_retry_backoff(10).await;
    assert!(Date::now() - start >= 5.0);
}
