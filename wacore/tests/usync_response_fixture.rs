#[path = "../benches/support/usync_response.rs"]
mod response;

#[test]
fn codspeed_fixtures_retain_all_users_and_companions() {
    for width in [1, 32, 128, 512] {
        let fixture = response::Fixture::new(width);
        let plain = fixture.receive(&fixture.plain);
        for compressed in [
            fixture.receive(&fixture.compressed),
            fixture.receive_streaming(&fixture.plain),
            fixture.receive_streaming(&fixture.compressed),
        ] {
            assert_eq!(plain.device_lists.len(), width);
            assert_eq!(compressed.device_lists.len(), width);
            for (a, b) in plain.device_lists.iter().zip(&compressed.device_lists) {
                assert_eq!(a.user, b.user);
                assert_eq!(a.phash, b.phash);
                assert_eq!(a.key_index_bytes, b.key_index_bytes);
                assert_eq!(a.devices.len(), 4);
                assert_eq!(b.devices.len(), 4);
                for (index, (plain, compressed)) in a.devices.iter().zip(&b.devices).enumerate() {
                    assert_eq!(plain.device, index as u16);
                    assert_eq!(plain.key_index, Some(index as u32));
                    assert_eq!(plain.device, compressed.device);
                    assert_eq!(plain.key_index, compressed.key_index);
                    assert_eq!(plain.is_hosted, compressed.is_hosted);
                }
            }
        }
    }
}
