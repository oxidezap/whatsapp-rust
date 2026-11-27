#![forbid(unsafe_code)]

const INLINE_CAPACITY: usize = 16;

pub struct SizeCache {
    inline: [u32; INLINE_CAPACITY],
    spill: Vec<u32>,
    len: u32,
    cursor: u32,
}

impl SizeCache {
    #[inline]
    pub const fn new() -> Self {
        Self {
            inline: [0; INLINE_CAPACITY],
            spill: Vec::new(),
            len: 0,
            cursor: 0,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
        self.cursor = 0;
        self.spill.clear();
    }

    pub fn with_spill_buffer(mut spill: Vec<u32>) -> Self {
        spill.clear();
        Self {
            spill,
            ..Self::new()
        }
    }

    pub fn into_spill_buffer(self) -> Vec<u32> {
        self.spill
    }

    #[inline]
    pub fn reserve(&mut self) -> usize {
        let next = self
            .len
            .checked_add(1)
            .expect("size cache slot count overflow");
        let index = self.len as usize;
        if index < INLINE_CAPACITY {
            self.inline[index] = 0;
        } else {
            self.spill.push(0);
        }
        self.len = next;
        index
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn set(&mut self, index: usize, size: u32) {
        assert!(
            index < self.len as usize,
            "size cache slot was not reserved"
        );
        if index < INLINE_CAPACITY {
            self.inline[index] = size;
        } else {
            self.spill[index - INLINE_CAPACITY] = size;
        }
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn consume_next(&mut self) -> u32 {
        assert!(
            self.cursor < self.len,
            "size cache traversal exceeded computed slots"
        );
        let index = self.cursor as usize;
        self.cursor += 1;
        let size = if index < INLINE_CAPACITY {
            self.inline[index]
        } else {
            self.spill[index - INLINE_CAPACITY]
        };
        debug_assert!(
            size <= 0x7fff_ffff,
            "cached message exceeds protobuf size limit"
        );
        size
    }
}

impl Default for SizeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SizeCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SizeCache")
            .field("len", &self.len)
            .field("cursor", &self.cursor)
            .field(
                "inline",
                &&self.inline[..(self.len as usize).min(INLINE_CAPACITY)],
            )
            .field("spill", &self.spill)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preorder_parent_sizes_can_be_completed_after_their_children() {
        let mut cache = SizeCache::new();
        let parent = cache.reserve();
        let child = cache.reserve();
        cache.set(child, 3);
        cache.set(parent, 7);
        assert_eq!(cache.consume_next(), 7);
        assert_eq!(cache.consume_next(), 3);
    }

    #[test]
    fn inline_spill_boundary_and_reuse_match_the_reference() {
        let mut candidate = SizeCache::new();
        let mut reference = buffa::SizeCache::new();
        for count in [0, 1, 15, 16, 17, 100, 3] {
            candidate.clear();
            reference.clear();
            for index in 0..count {
                assert_eq!(candidate.reserve(), reference.reserve());
                candidate.set(index, index as u32 * 7);
                reference.set(index, index as u32 * 7);
            }
            for _ in 0..count {
                assert_eq!(candidate.consume_next(), reference.consume_next());
            }
        }
        let capacity = candidate.spill.capacity();
        let mut candidate = SizeCache::with_spill_buffer(candidate.into_spill_buffer());
        assert_eq!(candidate.spill.capacity(), capacity);
        assert_eq!(candidate.reserve(), 0);
        assert_eq!(candidate.consume_next(), 0);
    }

    #[test]
    #[should_panic(expected = "not reserved")]
    fn unreserved_slots_are_rejected() {
        SizeCache::new().set(0, 1);
    }

    #[test]
    #[should_panic(expected = "exceeded")]
    fn divergent_write_traversal_is_rejected() {
        SizeCache::new().consume_next();
    }

    #[test]
    fn overflow_rejects_reservation_without_mutating_cache() {
        let mut cache = SizeCache::new();
        cache.len = u32::MAX;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cache.reserve()));
        assert!(result.is_err());
        assert_eq!(cache.len, u32::MAX);
        assert_eq!(cache.cursor, 0);
        assert!(cache.spill.is_empty());
        assert_eq!(cache.inline, [0; INLINE_CAPACITY]);
    }

    #[test]
    fn spill_bounds_and_clear_preserve_invariants() {
        let mut cache = SizeCache::new();
        for value in 0..(INLINE_CAPACITY + 2) {
            let index = cache.reserve();
            cache.set(index, value as u32);
        }
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cache.set(INLINE_CAPACITY + 2, 99);
        }));
        assert!(error.is_err());
        for value in 0..(INLINE_CAPACITY + 2) {
            assert_eq!(cache.consume_next(), value as u32);
        }
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cache.consume_next()));
        assert!(error.is_err());
        let capacity = cache.spill.capacity();
        cache.clear();
        assert_eq!(cache.spill.capacity(), capacity);
        assert_eq!(cache.reserve(), 0);
        assert_eq!(cache.consume_next(), 0);
    }

    #[test]
    fn clearing_between_reserve_and_set_rejects_the_stale_slot() {
        #[derive(Clone, Default, PartialEq)]
        struct CustomChild;

        impl super::super::DefaultInstance for CustomChild {
            fn default_instance() -> &'static Self {
                static INSTANCE: CustomChild = CustomChild;
                &INSTANCE
            }
        }

        impl super::super::Message for CustomChild {
            fn compute_size(&self, cache: &mut SizeCache) -> u32 {
                cache.clear();
                0
            }

            fn write_to(&self, _: &mut SizeCache, _: &mut impl super::super::EncodeSink) {}

            fn merge_field(
                &mut self,
                _: super::super::encoding::Tag,
                _: &mut impl bytes::Buf,
                _: super::super::DecodeContext<'_>,
            ) -> Result<(), super::super::DecodeError> {
                Ok(())
            }

            fn clear(&mut self) {}
        }
        let mut cache = SizeCache::new();
        let slot = cache.reserve();
        super::super::Message::compute_size(&CustomChild, &mut cache);
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cache.set(slot, 1)));
        assert!(error.is_err());
        assert_eq!(cache.len, 0);
    }
}
