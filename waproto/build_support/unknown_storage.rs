use ::buffa::alloc::boxed::Box;

/// Unknown records allocate only when present; ordinary messages carry one pointer.
#[derive(Default)]
pub struct Storage(Option<Box<::buffa::UnknownFields>>);

impl ::core::fmt::Debug for Storage {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&**self, f)
    }
}
impl PartialEq for Storage {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
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
    Storage(Some(Box::new(value.clone())))
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
            Self(Some(Box::new(value)))
        }
    }
}
impl From<Storage> for ::buffa::UnknownFields {
    fn from(value: Storage) -> Self {
        value.0.map_or_else(Self::new, |fields| *fields)
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
        if let Some(fields) = &self.0 {
            write_unknown(fields, buf);
        }
    }
    #[inline]
    pub fn clear(&mut self) {
        self.0 = None;
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
