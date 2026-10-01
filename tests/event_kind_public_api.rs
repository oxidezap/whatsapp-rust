// Run the standalone host's coverage assertion through the root test harness too.
include!("event-kind-consumer/src/main.rs");

#[test]
fn event_kind_host_dispositions_cover_all_kinds() {
    main();
}
