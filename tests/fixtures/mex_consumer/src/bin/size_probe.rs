//! Matched native/WASM consumer: descriptor, raw serialization and executor.
use std::{future::Future, hint::black_box, task::Context};
use wa::{MexDoc, MexOperation, mex_operation, wacore::iq::mex_operations::join_newsletter};

fn main() {
    let operation = mex_operation!(join_newsletter);
    let request = operation.request(join_newsletter::Variables {
        newsletter_id: Some(black_box("123456789@newsletter").into()),
    });
    black_box(request.doc());
    black_box(request.missing_variables().expect("serializable variables"));
    let custom = MexOperation::<serde_json::Value>::from_raw_parts(
        MexDoc {
            name: "ConsumerCustomQuery",
            id: "123456789",
        },
        &["optional"],
    );
    black_box(custom.raw_request(serde_json::json!({})).doc());

    // Never connects or sends. Keeping an opaque optional client makes the
    // canonical future's poll reachable to the linker on both targets.
    if let Some(client) = black_box(None::<&wa::Client>) {
        let mex = client.mex();
        let mut future = std::pin::pin!(mex.execute(request));
        let _ = black_box(
            future
                .as_mut()
                .poll(&mut Context::from_waker(std::task::Waker::noop())),
        );
    }
}
