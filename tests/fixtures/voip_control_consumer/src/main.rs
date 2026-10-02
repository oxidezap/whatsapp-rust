//! A small linked acquisition/drain consumer for matched native/WASM release-size probes.
use std::hint::black_box;
use whatsapp_rust::voip::CallHandle;

fn drain(call: &CallHandle) -> usize {
    let Some(mut events) = call.take_events() else {
        return 0;
    };
    let mut count = 0;
    while events.try_recv().is_ok() {
        count += 1;
    }
    count
}

fn main() {
    // Retain a real reachable acquisition/drain adapter without authenticating or placing calls.
    black_box(drain as fn(&CallHandle) -> usize);
}
