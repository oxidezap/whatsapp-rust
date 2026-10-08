use ::core::mem::ManuallyDrop;

/// Internal owner with one shared destruction path for unknown wire records.
#[repr(transparent)]
#[derive(Default, PartialEq, Hash)]
pub struct Storage(ManuallyDrop<::buffa::UnknownFields>);

impl ::core::fmt::Debug for Storage {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&*self.0, f)
    }
}
impl Clone for Storage {
    #[inline]
    fn clone(&self) -> Self {
        if self.is_empty() {
            Self::default()
        } else {
            Self::from(clone_nonempty(self))
        }
    }
}
#[cold]
#[inline(never)]
fn clone_nonempty(value: &::buffa::UnknownFields) -> ::buffa::UnknownFields {
    value.clone()
}

impl Drop for Storage {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: Storage owns this initialized value and does not expose the
        // ManuallyDrop. Drop runs once; into_inner suppresses this destructor.
        unsafe { drop_storage(&mut self.0) }
    }
}
#[inline(never)]
unsafe fn drop_storage(value: &mut ManuallyDrop<::buffa::UnknownFields>) {
    // SAFETY: caller transfers the sole responsibility for dropping value.
    unsafe { ManuallyDrop::drop(value) }
}
impl ::core::ops::Deref for Storage {
    type Target = ::buffa::UnknownFields;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ::core::ops::DerefMut for Storage {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl From<::buffa::UnknownFields> for Storage {
    #[inline]
    fn from(value: ::buffa::UnknownFields) -> Self {
        Self(ManuallyDrop::new(value))
    }
}
impl From<Storage> for ::buffa::UnknownFields {
    #[inline]
    fn from(value: Storage) -> Self {
        let mut value = ManuallyDrop::new(value);
        // SAFETY: value is initialized; its outer destructor is suppressed,
        // and this transfers its inner owner exactly once to the caller.
        unsafe { ManuallyDrop::take(&mut value.0) }
    }
}
impl PartialEq<::buffa::UnknownFields> for Storage {
    fn eq(&self, other: &::buffa::UnknownFields) -> bool {
        &*self.0 == other
    }
}
impl PartialEq<Storage> for ::buffa::UnknownFields {
    fn eq(&self, other: &Storage) -> bool {
        self == &*other.0
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
    #[cold]
    #[inline(never)]
    pub(super) fn merge_unknown(
        &mut self,
        tag: ::buffa::encoding::Tag,
        buf: &mut impl ::buffa::bytes::Buf,
        ctx: ::buffa::DecodeContext<'_>,
    ) -> Result<(), ::buffa::DecodeError> {
        self.0
            .push(::buffa::encoding::decode_unknown_field(tag, buf, ctx)?);
        Ok(())
    }
    #[inline]
    pub fn encoded_len(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            unknown_len(self)
        }
    }
    #[inline]
    pub fn write_to(&self, buf: &mut impl ::buffa::EncodeSink) {
        if !self.is_empty() {
            write_unknown(self, buf);
        }
    }
    #[inline]
    pub fn clear(&mut self) {
        if !self.is_empty() {
            clear_unknown(self);
        }
    }
    #[cold]
    #[inline(never)]
    pub fn push(&mut self, field: ::buffa::UnknownField) {
        self.0.push(field);
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
#[cold]
#[inline(never)]
fn clear_unknown(value: &mut ::buffa::UnknownFields) {
    value.clear();
}
