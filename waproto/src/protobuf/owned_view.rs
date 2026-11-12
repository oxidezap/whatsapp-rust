// SPDX-License-Identifier: Apache-2.0
// Adapted from buffa 0.9.2 src/view.rs. See ../../NOTICE and ../../licenses/BUFFA-APACHE-2.0.

use super::{DecodeError, DecodeOptions, Message, MessageView, ViewReborrow};
use bytes::Bytes;
use std::mem::MaybeUninit;
use std::ops::Deref;

pub struct OwnedView<V> {
    // View must drop while its backing bytes still exist. No Drop impl may be
    // added here: into_bytes relies on moving both fields and dropping V once.
    view: MaybeDangling<V>,
    bytes: Bytes,
}

#[repr(transparent)]
struct MaybeDangling<T> {
    inner: MaybeUninit<T>,
}

impl<T> MaybeDangling<T> {
    const fn new(value: T) -> Self {
        Self {
            inner: MaybeUninit::new(value),
        }
    }
}

impl<T> Deref for MaybeDangling<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: construction initializes T, and this is unreachable after
        // Drop. MaybeUninit avoids retagging the synthetic static references.
        unsafe { self.inner.assume_init_ref() }
    }
}

impl<T> Drop for MaybeDangling<T> {
    fn drop(&mut self) {
        // SAFETY: T is initialized and dropped exactly once. Reading it into
        // an ordinary local would restore the validity properties suppressed
        // by MaybeUninit and make freeing its backing bytes unsound under Miri.
        unsafe { self.inner.as_mut_ptr().drop_in_place() }
    }
}

#[cold]
#[inline(never)]
fn conversion_contract_failed(error: DecodeError) -> ! {
    panic!(
        "OwnedView conversion failed ({error:?}): the wrapped MessageView impl violates the wire-decode => convert contract"
    )
}

impl<V: MessageView<'static>> OwnedView<V> {
    pub fn decode(bytes: Bytes) -> Result<Self, DecodeError> {
        // SAFETY: Bytes keeps its data pointer stable across moves. Every
        // borrow decoded from this slice is retained by bytes until view drops.
        let view = unsafe {
            let input: &'static [u8] = std::mem::transmute::<&[u8], &'static [u8]>(&bytes);
            V::decode_view(input)?
        };
        Ok(Self {
            view: MaybeDangling::new(view),
            bytes,
        })
    }
    pub fn decode_with_options(bytes: Bytes, options: &DecodeOptions) -> Result<Self, DecodeError> {
        // SAFETY: identical ownership and drop-order invariants to decode.
        let view = unsafe {
            let input: &'static [u8] = std::mem::transmute::<&[u8], &'static [u8]>(&bytes);
            options.decode_view::<V>(input)?
        };
        Ok(Self {
            view: MaybeDangling::new(view),
            bytes,
        })
    }
    pub fn from_owned(message: &V::Owned) -> Result<Self, DecodeError> {
        Self::decode(Bytes::from(
            message
                .try_encode_to_vec()
                .map_err(|_| DecodeError::MessageTooLarge)?,
        ))
    }
    pub fn to_owned_message(&self) -> V::Owned {
        self.view
            .to_owned_from_source(Some(&self.bytes))
            .unwrap_or_else(|error| conversion_contract_failed(error))
    }
    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
    /// # Safety
    /// The view must have been wire-decoded from bytes or a sub-slice fully
    /// contained by bytes. Every borrow must remain valid while bytes lives,
    /// and successful decoding must imply successful conversion to Owned.
    pub unsafe fn from_parts(bytes: Bytes, view: V) -> Self {
        Self {
            view: MaybeDangling::new(view),
            bytes,
        }
    }
    pub fn into_bytes(self) -> Bytes {
        let Self { view, bytes } = self;
        drop(view);
        bytes
    }
    pub fn reborrow<'a>(&'a self) -> &'a V::Reborrowed<'a>
    where
        V: ViewReborrow,
    {
        V::reborrow(&self.view)
    }
}

impl<V: std::fmt::Debug> std::fmt::Debug for OwnedView<V> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&*self.view, out)
    }
}
impl<V: Clone> Clone for OwnedView<V> {
    fn clone(&self) -> Self {
        // Bytes::clone shares the allocation, so cloned view references keep
        // pointing into storage retained by this handle.
        Self {
            view: MaybeDangling::new((*self.view).clone()),
            bytes: self.bytes.clone(),
        }
    }
}
impl<V: PartialEq> PartialEq for OwnedView<V> {
    fn eq(&self, other: &Self) -> bool {
        *self.view == *other.view
    }
}
impl<V: Eq> Eq for OwnedView<V> {}
impl<V: serde::Serialize> serde::Serialize for OwnedView<V> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&*self.view, serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protobuf::generated::whatsapp::{
        MessageView as FullView, session_structure::chain::MessageKeyView,
    };
    use std::cell::RefCell;

    thread_local! {static EVENTS:RefCell<Vec<&'static str>>=const{RefCell::new(Vec::new())};}

    struct Owner(Vec<u8>);
    impl AsRef<[u8]> for Owner {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }
    impl Drop for Owner {
        fn drop(&mut self) {
            EVENTS.with(|events| events.borrow_mut().push("bytes"));
        }
    }

    struct Probe<'a> {
        bytes: &'a [u8],
    }
    impl<'a> MessageView<'a> for Probe<'a> {
        type Owned = crate::protobuf::generated::whatsapp::Message;
        fn decode_view(bytes: &'a [u8]) -> Result<Self, DecodeError> {
            Ok(Self { bytes })
        }
        fn merge_view_field(
            &mut self,
            _: crate::protobuf::encoding::Tag,
            cur: &'a [u8],
            _: &'a [u8],
            _: crate::protobuf::DecodeContext<'_>,
        ) -> Result<&'a [u8], DecodeError> {
            Ok(cur)
        }
        fn to_owned_message(&self) -> Result<Self::Owned, DecodeError> {
            Ok(Self::Owned::default())
        }
    }
    impl Drop for Probe<'_> {
        fn drop(&mut self) {
            assert!(!self.bytes.is_empty());
            EVENTS.with(|events| events.borrow_mut().push("view"));
            if self.bytes[0] == b'p' {
                panic!("probe drop");
            }
        }
    }

    #[test]
    fn moved_and_cloned_handles_keep_the_original_borrows_alive() {
        fn move_handle(value: OwnedView<FullView<'static>>) -> OwnedView<FullView<'static>> {
            value
        }
        let bytes = Bytes::from(vec![10, 3, b'o', b'l', b'd']);
        let pointer = bytes[2..].as_ptr();
        let handle = move_handle(OwnedView::<FullView<'static>>::decode(bytes).unwrap());
        let clone = handle.clone();
        drop(handle);
        assert_eq!(clone.reborrow().conversation.unwrap().as_ptr(), pointer);
        assert_eq!(clone.reborrow().conversation, Some("old"));
        assert_eq!(clone.into_bytes().as_ref(), [10, 3, b'o', b'l', b'd']);
    }

    #[test]
    fn shared_bytes_promotion_retains_slices_without_copying() {
        let bytes = Bytes::from(vec![18, 3, 1, 2, 3]);
        let pointer = bytes[2..].as_ptr();
        let handle = OwnedView::<MessageKeyView<'static>>::decode(bytes).unwrap();
        let owned = handle.to_owned_message();
        drop(handle);
        assert_eq!(owned.cipher_key.as_ref().unwrap().as_ptr(), pointer);
        assert_eq!(owned.cipher_key.as_deref(), Some([1, 2, 3].as_slice()));
    }

    #[test]
    fn normal_drop_and_into_bytes_drop_the_view_first() {
        EVENTS.with(|events| events.borrow_mut().clear());
        drop(OwnedView::<Probe<'static>>::decode(Bytes::from_owner(Owner(vec![b'x']))).unwrap());
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["view", "bytes"]));
        EVENTS.with(|events| events.borrow_mut().clear());
        let bytes = OwnedView::<Probe<'static>>::decode(Bytes::from_owner(Owner(vec![b'x'])))
            .unwrap()
            .into_bytes();
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["view"]));
        drop(bytes);
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["view", "bytes"]));
    }

    #[test]
    fn destructor_unwind_drops_each_field_once() {
        EVENTS.with(|events| events.borrow_mut().clear());
        let value =
            OwnedView::<Probe<'static>>::decode(Bytes::from_owner(Owner(vec![b'p']))).unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| value.into_bytes())).is_err()
        );
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["view", "bytes"]));
    }

    #[test]
    fn auto_traits_follow_the_actual_view_without_manual_unsafe_impls() {
        fn assert_traits<T: Send + Sync>() {}
        assert_traits::<OwnedView<FullView<'static>>>();
    }
}
