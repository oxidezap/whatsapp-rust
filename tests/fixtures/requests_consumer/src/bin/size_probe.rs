fn main() {
    // Keep the boxed downstream entry reachable without contacting WhatsApp.
    std::hint::black_box(requests_consumer::boxed);
}
