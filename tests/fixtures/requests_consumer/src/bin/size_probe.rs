fn main() {
    // A function ITEM is zero-sized. Retain a real pointer so the linker keeps
    // the send/edit future's poll/drop graph, without contacting WhatsApp.
    let keep: for<'a> fn(&'a wa::Client, &'a wa::Jid) -> requests_consumer::BoxedSend<'a> =
        requests_consumer::boxed;
    std::hint::black_box(keep);
}
