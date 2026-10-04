use whatsapp_rust::voip::CallEvent;

fn legacy(event: &CallEvent) {
    if let CallEvent::VideoStateChanged { .. } = event {}
}

fn main() {
    legacy(&CallEvent::RelayAllocated);
}
