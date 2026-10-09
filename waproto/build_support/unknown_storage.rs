use ::buffa::alloc::boxed::Box;
use ::core::mem::ManuallyDrop;

#[inline(never)]
pub(super) fn decode_boxed<T: ::buffa::Message>(
    buf: &mut impl ::buffa::bytes::Buf,
    ctx: ::buffa::DecodeContext<'_>,
) -> Result<Box<T>, ::buffa::DecodeError> {
    // Decode into the eventual oneof allocation rather than moving a complete
    // child from a stack temporary after a successful merge.
    let mut value = Box::<T>::default();
    T::merge_length_delimited(&mut value, buf, ctx)?;
    Ok(value)
}

// Parents share a child's allocation/default initialization together with its
// decoder. Keep the existing buffer specialization and merge-in-place behavior.
#[inline(never)]
pub(super) fn merge_message<T: ::buffa::Message, P: ::buffa::ProtoBox<T>>(
    field: &mut ::buffa::MessageField<T, P>,
    buf: &mut impl ::buffa::bytes::Buf,
    ctx: ::buffa::DecodeContext<'_>,
) -> Result<(), ::buffa::DecodeError> {
    T::merge_length_delimited(field.get_or_insert_default(), buf, ctx)
}

/// Unknown records allocate only when present; ordinary messages carry one pointer.
#[derive(Default)]
pub struct Storage(ManuallyDrop<Option<Box<::buffa::UnknownFields>>>);

impl ::core::fmt::Debug for Storage {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&**self, f)
    }
}
impl PartialEq for Storage {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_none() && other.0.is_none() {
            return true;
        }
        equal_nonempty(self, other)
    }
}
#[cold]
#[inline(never)]
fn equal_nonempty(left: &Storage, right: &Storage) -> bool {
    **left == **right
}

impl Drop for Storage {
    #[inline]
    fn drop(&mut self) {
        // The shared helper consumes the allocation. Suppress automatic field
        // drop so each generated message does not also repeat its cold glue.
        if self.0.is_some() {
            drop_nonempty(&mut self.0);
        }
    }
}
#[cold]
#[inline(never)]
fn drop_nonempty(fields: &mut Option<Box<::buffa::UnknownFields>>) {
    *fields = None;
}
impl ::core::hash::Hash for Storage {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&**self, state);
    }
}
impl Clone for Storage {
    #[inline]
    fn clone(&self) -> Self {
        if self.is_empty() {
            Self::default()
        } else {
            clone_nonempty(self)
        }
    }
}
#[cold]
#[inline(never)]
fn clone_nonempty(value: &::buffa::UnknownFields) -> Storage {
    Storage(ManuallyDrop::new(Some(Box::new(value.clone()))))
}
impl ::core::ops::Deref for Storage {
    type Target = ::buffa::UnknownFields;
    #[inline]
    fn deref(&self) -> &Self::Target {
        static EMPTY: ::buffa::UnknownFields = ::buffa::UnknownFields::new();
        self.0.as_deref().unwrap_or(&EMPTY)
    }
}
impl ::core::ops::DerefMut for Storage {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.get_or_insert_with(Box::default)
    }
}
impl From<::buffa::UnknownFields> for Storage {
    fn from(value: ::buffa::UnknownFields) -> Self {
        if value.is_empty() {
            Self::default()
        } else {
            Self(ManuallyDrop::new(Some(Box::new(value))))
        }
    }
}
impl From<Storage> for ::buffa::UnknownFields {
    fn from(mut value: Storage) -> Self {
        value.0.take().map_or_else(Self::new, |fields| *fields)
    }
}
impl PartialEq<::buffa::UnknownFields> for Storage {
    fn eq(&self, other: &::buffa::UnknownFields) -> bool {
        &**self == other
    }
}
impl PartialEq<Storage> for ::buffa::UnknownFields {
    fn eq(&self, other: &Storage) -> bool {
        self == &**other
    }
}
impl<'a> IntoIterator for &'a Storage {
    type Item = &'a ::buffa::UnknownField;
    type IntoIter = ::core::slice::Iter<'a, ::buffa::UnknownField>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl IntoIterator for Storage {
    type Item = ::buffa::UnknownField;
    type IntoIter = <::buffa::UnknownFields as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        ::buffa::UnknownFields::from(self).into_iter()
    }
}
impl Storage {
    /// Heap header owned by this wrapper, excluding the record buffer and payloads.
    #[inline]
    pub fn allocated_header_bytes(&self) -> usize {
        if self.0.is_some() {
            ::core::mem::size_of::<::buffa::UnknownFields>()
        } else {
            0
        }
    }
    #[cold]
    #[inline(never)]
    pub(super) fn merge_unknown(
        &mut self,
        tag: ::buffa::encoding::Tag,
        buf: &mut impl ::buffa::bytes::Buf,
        ctx: ::buffa::DecodeContext<'_>,
    ) -> Result<(), ::buffa::DecodeError> {
        self.push_decoded(::buffa::encoding::decode_unknown_field(tag, buf, ctx)?, ctx)
    }
    #[cold]
    pub(super) fn push_decoded(
        &mut self,
        field: ::buffa::UnknownField,
        ctx: ::buffa::DecodeContext<'_>,
    ) -> Result<(), ::buffa::DecodeError> {
        if self.0.is_none() {
            ctx.register_element_memory(::core::mem::size_of::<::buffa::UnknownFields>())?;
        }
        self.push(field);
        Ok(())
    }
    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.0.as_ref().map_or(0, |fields| unknown_len(fields))
    }
    #[inline]
    pub fn write_to(&self, buf: &mut impl ::buffa::EncodeSink) {
        if let Some(fields) = &*self.0 {
            write_unknown(fields, buf);
        }
    }
    #[inline]
    pub fn clear(&mut self) {
        *self.0 = None;
    }
    #[cold]
    #[inline(never)]
    pub fn push(&mut self, field: ::buffa::UnknownField) {
        self.0.get_or_insert_with(Box::default).push(field);
    }
}
#[cold]
#[inline(never)]
fn unknown_len(value: &::buffa::UnknownFields) -> usize {
    value.encoded_len()
}
#[cold]
#[inline(never)]
fn write_unknown(value: &::buffa::UnknownFields, buf: &mut impl ::buffa::EncodeSink) {
    value.write_to(buf);
}
