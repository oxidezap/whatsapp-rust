#[cfg(test)]
mod tests {
    use super::put_shared_bytes_field;
    use ::bytes::Bytes;

    #[test]
    fn byte_writer_preserves_presence_length_and_shared_segments() {
        for length in [0, 1, 127, 128, 4096] {
            let payload = Bytes::from(vec![42; length]);
            for number in [1, 15, 16, 2047] {
                let mut expected = Vec::new();
                ::buffa::types::put_shared_bytes_field(number, &payload, &mut expected);
                let mut contiguous = Vec::new();
                put_shared_bytes_field(number, &payload, &mut contiguous);
                assert_eq!(contiguous, expected);
                let mut rope = ::buffa::Rope::with_min_segment(1);
                put_shared_bytes_field(number, &payload, &mut rope);
                let segments = rope.into_segments();
                assert_eq!(segments.concat(), expected);
                if !payload.is_empty() {
                    assert!(segments.iter().any(|part| part.as_ptr() == payload.as_ptr() && part.len() == payload.len()));
                }
            }
        }
    }

    #[test]
    fn byte_writer_preserves_borrowed_backing_capture_and_owned_fallback() {
        let backing = Bytes::from(vec![43; 4096]);
        let borrowed = &backing[7..2048];
        let mut expected = Vec::new();
        ::buffa::types::put_shared_bytes_field(16, &borrowed, &mut expected);
        let mut rope = ::buffa::Rope::with_min_segment(1).with_backing(backing.clone());
        put_shared_bytes_field(16, &borrowed, &mut rope);
        let segments = rope.into_segments();
        assert_eq!(segments.concat(), expected);
        assert!(segments.iter().any(|part| part.as_ptr() == borrowed.as_ptr() && part.len() == borrowed.len()));
        let owned = borrowed.to_vec();
        let mut fallback = ::buffa::Rope::with_min_segment(1);
        put_shared_bytes_field(16, &owned, &mut fallback);
        assert_eq!(fallback.into_segments().concat(), expected);
    }
}
