#![forbid(unsafe_code)]

use super::{
    DecodeContext, DecodeError, DecodeOptions, EncodeError, EncodeSink, Message, SizeCache,
    checked_encode_size, encode_size_overflow, encoding::Tag,
};
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::Deref;

pub trait MessageView<'a>: Sized {
    type Owned: Message;
    fn decode_view(input: &'a [u8]) -> Result<Self, DecodeError>;
    fn decode_view_with_ctx(
        input: &'a [u8],
        _context: DecodeContext<'_>,
    ) -> Result<Self, DecodeError> {
        Self::decode_view(input)
    }
    fn decode_view_ctx(input: &'a [u8], context: DecodeContext<'_>) -> Result<Self, DecodeError>
    where
        Self: Default,
    {
        let mut value = Self::default();
        value.merge_into_view(input, context)?;
        Ok(value)
    }
    fn merge_into_view(
        &mut self,
        mut input: &'a [u8],
        context: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        while !input.is_empty() {
            let before = input;
            let tag = Tag::decode(&mut input)?;
            input = self.merge_view_field(tag, input, before, context)?;
        }
        Ok(())
    }
    fn merge_view_field(
        &mut self,
        tag: Tag,
        input: &'a [u8],
        before_tag: &'a [u8],
        context: DecodeContext<'_>,
    ) -> Result<&'a [u8], DecodeError>;
    fn to_owned_message(&self) -> Result<Self::Owned, DecodeError>;
    fn to_owned_from_source(
        &self,
        _source: Option<&bytes::Bytes>,
    ) -> Result<Self::Owned, DecodeError> {
        self.to_owned_message()
    }
}

pub trait HasMessageView: Message + Sized {
    type View<'a>: MessageView<'a, Owned = Self> + super::OwnedViewSource + Send + Sync;
    type ViewHandle: From<super::OwnedView<Self::View<'static>>>
        + AsRef<super::OwnedView<Self::View<'static>>>
        + Send
        + Sync
        + 'static;

    fn decode_view(input: &[u8]) -> Result<Self::View<'_>, DecodeError> {
        Self::View::decode_view(input)
    }

    fn decode_view_with_options<'a>(
        input: &'a [u8],
        options: &DecodeOptions,
    ) -> Result<Self::View<'a>, DecodeError> {
        options.decode_view(input)
    }

    fn decode_view_handle(bytes: bytes::Bytes) -> Result<Self::ViewHandle, DecodeError> {
        Ok(super::OwnedView::<Self::View<'static>>::decode(bytes)?.into())
    }

    fn decode_view_handle_with_options(
        bytes: bytes::Bytes,
        options: &DecodeOptions,
    ) -> Result<Self::ViewHandle, DecodeError> {
        Ok(super::OwnedView::<Self::View<'static>>::decode_with_options(bytes, options)?.into())
    }
}

pub trait ViewReborrow: MessageView<'static> {
    type Reborrowed<'a>: MessageView<'a, Owned = <Self as MessageView<'static>>::Owned>
    where
        Self: 'a;
    fn reborrow<'a>(this: &'a Self) -> &'a Self::Reborrowed<'a>;
}

pub trait DefaultViewInstance {
    fn default_view_instance<'a>() -> &'a Self
    where
        Self: 'a;
}

pub trait ViewEncode<'a>: MessageView<'a> {
    fn compute_size(&self, cache: &mut SizeCache) -> u32;
    fn write_to(&self, cache: &mut SizeCache, output: &mut impl EncodeSink);
    fn try_encoded_len(&self) -> Result<u32, EncodeError> {
        checked_encode_size(self.compute_size(&mut SizeCache::new()))
    }
    fn encoded_len(&self) -> u32 {
        self.try_encoded_len()
            .unwrap_or_else(|_| encode_size_overflow())
    }
    fn try_encode(&self, output: &mut impl EncodeSink) -> Result<(), EncodeError> {
        self.try_encode_with_cache(&mut SizeCache::new(), output)
    }
    fn encode(&self, output: &mut impl EncodeSink) {
        self.try_encode(output)
            .unwrap_or_else(|_| encode_size_overflow());
    }
    fn try_encode_with_cache(
        &self,
        cache: &mut SizeCache,
        output: &mut impl EncodeSink,
    ) -> Result<(), EncodeError> {
        cache.clear();
        checked_encode_size(self.compute_size(cache))?;
        self.write_to(cache, output);
        Ok(())
    }
    fn encode_with_cache(&self, cache: &mut SizeCache, output: &mut impl EncodeSink) {
        self.try_encode_with_cache(cache, output)
            .unwrap_or_else(|_| encode_size_overflow());
    }
    fn try_encode_to_vec(&self) -> Result<Vec<u8>, EncodeError> {
        let mut cache = SizeCache::new();
        let length = checked_encode_size(self.compute_size(&mut cache))? as usize;
        let mut output = Vec::with_capacity(length);
        self.write_to(&mut cache, &mut output);
        debug_assert_eq!(length, output.len());
        Ok(output)
    }
    fn encode_to_vec(&self) -> Vec<u8> {
        self.try_encode_to_vec()
            .unwrap_or_else(|_| encode_size_overflow())
    }
}

impl DecodeOptions {
    pub fn decode_view<'a, V: MessageView<'a>>(&self, input: &'a [u8]) -> Result<V, DecodeError> {
        self.run(input.len(), |context| {
            V::decode_view_with_ctx(input, context)
        })
    }
}

#[derive(Clone, Debug)]
pub struct MessageFieldView<V> {
    inner: Option<Box<V>>,
}
impl<V> MessageFieldView<V> {
    pub const fn unset() -> Self {
        Self { inner: None }
    }
    pub fn set(value: V) -> Self {
        Self {
            inner: Some(Box::new(value)),
        }
    }
    pub fn some(value: V) -> Self {
        Self::set(value)
    }
    pub const fn is_set(&self) -> bool {
        self.inner.is_some()
    }
    pub const fn is_unset(&self) -> bool {
        self.inner.is_none()
    }
    pub fn as_option(&self) -> Option<&V> {
        self.inner.as_deref()
    }
    pub fn as_mut(&mut self) -> Option<&mut V> {
        self.inner.as_deref_mut()
    }
}
impl<V> Default for MessageFieldView<V> {
    fn default() -> Self {
        Self::unset()
    }
}
impl<V> From<V> for MessageFieldView<V> {
    fn from(value: V) -> Self {
        Self::set(value)
    }
}
impl<V: DefaultViewInstance> Deref for MessageFieldView<V> {
    type Target = V;
    fn deref(&self) -> &V {
        match self.as_option() {
            Some(value) => value,
            None => V::default_view_instance(),
        }
    }
}
impl<V: DefaultViewInstance + PartialEq> PartialEq for MessageFieldView<V> {
    fn eq(&self, other: &Self) -> bool {
        (self.is_unset() && other.is_unset()) || self.deref() == other.deref()
    }
}
impl<V: DefaultViewInstance + Eq> Eq for MessageFieldView<V> {}
impl<'a, V: ViewEncode<'a>> MessageFieldView<V> {
    pub fn compute_size(&self, cache: &mut SizeCache) -> u32 {
        self.as_option()
            .map_or(0, |value| value.compute_size(cache))
    }
    pub fn write_to(&self, cache: &mut SizeCache, output: &mut impl EncodeSink) {
        if let Some(value) = self.as_option() {
            value.write_to(cache, output);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatedView<'a, T> {
    elements: Vec<T>,
    _marker: PhantomData<&'a ()>,
}
impl<'a, T> RepeatedView<'a, T> {
    pub const fn new(elements: Vec<T>) -> Self {
        Self {
            elements,
            _marker: PhantomData,
        }
    }
    pub fn len(&self) -> usize {
        self.elements.len()
    }
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
    pub fn push(&mut self, value: T) {
        self.elements.push(value);
    }
    pub fn reserve(&mut self, additional: usize) {
        self.elements.reserve(additional);
    }
    pub fn as_mut_vec(&mut self) -> &mut Vec<T> {
        &mut self.elements
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.elements.iter()
    }
}
impl<'a, T> Default for RepeatedView<'a, T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
impl<'a, T> Deref for RepeatedView<'a, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.elements
    }
}
impl<'a, T> From<Vec<T>> for RepeatedView<'a, T> {
    fn from(elements: Vec<T>) -> Self {
        Self::new(elements)
    }
}
impl<'a, T> FromIterator<T> for RepeatedView<'a, T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}
impl<'a, T> IntoIterator for RepeatedView<'a, T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}
impl<'b, 'a, T> IntoIterator for &'b RepeatedView<'a, T> {
    type Item = &'b T;
    type IntoIter = std::slice::Iter<'b, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapView<'a, K, V> {
    entries: Vec<(K, V)>,
    _marker: PhantomData<&'a ()>,
}
impl<'a, K, V> MapView<'a, K, V> {
    pub const fn new(entries: Vec<(K, V)>) -> Self {
        Self {
            entries,
            _marker: PhantomData,
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn push(&mut self, key: K, value: V) {
        self.entries.push((key, value));
    }
    pub fn iter(&self) -> std::slice::Iter<'_, (K, V)> {
        self.entries.iter()
    }
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|entry| &entry.0)
    }
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|entry| &entry.1)
    }
    pub fn get<Q: PartialEq + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.0.borrow() == key)
            .map(|entry| &entry.1)
    }
    pub fn contains_key<Q: PartialEq + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.get(key).is_some()
    }
    pub fn iter_unique(&self) -> impl Iterator<Item = &(K, V)>
    where
        K: PartialEq,
    {
        // The public PartialEq bound supports keys without hashing. Keep this
        // allocation-free compatibility method out of generated decode paths.
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                self.entries[index + 1..]
                    .iter()
                    .all(|later| later.0 != entry.0)
                    .then_some(entry)
            })
    }
    pub fn len_unique(&self) -> usize
    where
        K: PartialEq,
    {
        self.iter_unique().count()
    }
}
impl<'a, K, V> From<Vec<(K, V)>> for MapView<'a, K, V> {
    fn from(entries: Vec<(K, V)>) -> Self {
        Self::new(entries)
    }
}
impl<'a, K, V> FromIterator<(K, V)> for MapView<'a, K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}
impl<'a, K, V> Default for MapView<'a, K, V> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
impl<'a, K, V> IntoIterator for MapView<'a, K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
impl<'b, 'a, K, V> IntoIterator for &'b MapView<'a, K, V> {
    type Item = &'b (K, V);
    type IntoIter = std::slice::Iter<'b, (K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub fn bytes_from_source(source: Option<&bytes::Bytes>, field: &[u8]) -> bytes::Bytes {
    if field.is_empty() {
        return bytes::Bytes::new();
    }
    if let Some(source) = source {
        let base = source.as_ptr() as usize;
        if let Some(start) = (field.as_ptr() as usize).checked_sub(base)
            && let Some(end) = start.checked_add(field.len())
            && end <= source.len()
        {
            return source.slice(start..end);
        }
    }
    bytes::Bytes::copy_from_slice(field)
}
