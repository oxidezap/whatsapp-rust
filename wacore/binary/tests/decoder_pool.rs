//! Compatibility checks for the benchmark-only pooled representation.
use wacore_binary::bench_internals as decoder_pool;
pub use wacore_binary::*;

#[test]
fn pooled_frames_preserve_values_and_consumers() {
    let mut fixtures: Vec<_> = [0, 1, 32, 128]
        .into_iter()
        .map(decoder_pool::fixture)
        .collect();
    for node in [
        builder::NodeBuilder::new("message").build(),
        builder::NodeBuilder::new("message")
            .string_content("receipt")
            .build(),
        builder::NodeBuilder::new("message")
            .string_content("A12345678901234567890123456789012345678901234567890123456789012345")
            .build(),
    ] {
        fixtures.push(marshal_shallow(&node).unwrap()[1..].to_vec());
    }
    for data in fixtures {
        let tree = marshal::unmarshal_ref(&data).unwrap();
        assert_eq!(decoder_pool::baseline(&data).unwrap(), tree);
        for (capacity, two_pass) in [(1, false), (32, false), (128, false), (0, true)] {
            let frame = decoder_pool::decode(&data, capacity, two_pass).unwrap();
            assert_eq!(frame.to_owned(), tree.to_owned());
            assert_eq!(
                frame.consume().unwrap(),
                decoder_pool::consume_tree(&tree).unwrap()
            );
        }
    }
}

#[test]
fn pooled_frames_preserve_rejections() {
    let fixture = decoder_pool::fixture(2);
    let mut inputs: Vec<Vec<u8>> = (0..fixture.len()).map(|n| fixture[..n].to_vec()).collect();
    let mut trailing = fixture.clone();
    trailing.push(0);
    inputs.push(trailing);
    // Mutate token, packed strings, typed JIDs, lengths and opaque payloads.
    for i in 0..fixture.len() {
        let mut mutated = fixture.clone();
        mutated[i] ^= 0xff;
        inputs.push(mutated);
    }
    for data in inputs {
        let expected = marshal::unmarshal_ref(&data);
        for two_pass in [false, true] {
            let got = decoder_pool::decode(&data, 32, two_pass);
            match (&expected, got) {
                (Ok(tree), Ok(frame)) => assert_eq!(frame.to_owned(), tree.to_owned()),
                (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
                _ => panic!("pooled validation diverged from production"),
            }
        }
    }
}

#[test]
fn pooled_owner_is_send_sync_and_rejects_excessive_depth() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<decoder_pool::Frame<'static>>();
    let mut node = builder::NodeBuilder::new("message").build();
    for _ in 0..130 {
        node = builder::NodeBuilder::new("message")
            .children(vec![node])
            .build();
    }
    let data = marshal_shallow(&node).unwrap();
    for two_pass in [false, true] {
        assert!(matches!(
            decoder_pool::decode(&data[1..], 32, two_pass),
            Err(BinaryError::MaxDepthExceeded)
        ));
    }
}

#[test]
fn pooled_owned_frame_retains_bytes_and_can_cross_threads() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<decoder_pool::OwnedFrame>();
    for two_pass in [false, true] {
        let data = decoder_pool::fixture(32);
        let expected = marshal::unmarshal_ref(&data).unwrap().to_owned();
        let buffer = bytes::Bytes::from(data);
        let frame = std::sync::Arc::new(
            decoder_pool::OwnedFrame::new(buffer.clone(), 32, two_pass).unwrap(),
        );
        drop(buffer);
        let other = frame.clone();
        let got = std::thread::spawn(move || other.get().to_owned())
            .join()
            .unwrap();
        assert_eq!(got, expected);
        assert_eq!(frame.get().to_owned(), expected);
    }
}

#[test]
fn pooled_owned_errors_release_partial_and_complete_frames() {
    let data = decoder_pool::fixture(2);
    for two_pass in [false, true] {
        let truncated = bytes::Bytes::copy_from_slice(&data[..data.len() - 1]);
        assert!(matches!(
            decoder_pool::OwnedFrame::new(truncated, 32, two_pass),
            Err(BinaryError::UnexpectedEof)
        ));
        let mut trailing = data.clone();
        trailing.push(0);
        assert!(matches!(
            decoder_pool::OwnedFrame::new(trailing.into(), 32, two_pass),
            Err(BinaryError::LeftoverData(1))
        ));
    }
}
