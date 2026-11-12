#![forbid(unsafe_code)]

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub trait DefaultInstance: Default + 'static {
    fn default_instance() -> &'static Self;
}

pub trait ProtoBox<T>: Deref<Target = T> + DerefMut {
    fn new(value: T) -> Self;
    fn into_inner(self) -> T;
}

impl<T> ProtoBox<T> for Box<T> {
    fn new(value: T) -> Self {
        Box::new(value)
    }
    fn into_inner(self) -> T {
        *self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Inline<T>(pub T);

impl<T> Deref for Inline<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Inline<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> ProtoBox<T> for Inline<T> {
    fn new(value: T) -> Self {
        Self(value)
    }
    fn into_inner(self) -> T {
        self.0
    }
}

pub struct MessageField<T: Default, P = Box<T>> {
    inner: Option<P>,
    _marker: PhantomData<T>,
}

impl<T: Default, P: ProtoBox<T>> MessageField<T, P> {
    pub const fn none() -> Self {
        Self {
            inner: None,
            _marker: PhantomData,
        }
    }
    pub fn some(value: T) -> Self {
        Self::from_pointer(P::new(value))
    }
    pub fn from_pointer(value: P) -> Self {
        Self {
            inner: Some(value),
            _marker: PhantomData,
        }
    }
    pub fn is_set(&self) -> bool {
        self.inner.is_some()
    }
    pub fn is_unset(&self) -> bool {
        self.inner.is_none()
    }
    pub fn as_option(&self) -> Option<&T> {
        self.inner.as_deref()
    }
    pub fn as_option_mut(&mut self) -> Option<&mut T> {
        self.inner.as_deref_mut()
    }
    pub fn take(&mut self) -> Option<T> {
        self.inner.take().map(P::into_inner)
    }
    pub fn get_or_insert_default(&mut self) -> &mut T {
        self.inner.get_or_insert_with(|| P::new(T::default()))
    }
    pub fn modify<F: FnOnce(&mut T)>(&mut self, apply: F) {
        apply(self.get_or_insert_default());
    }
    pub fn into_option(self) -> Option<T> {
        self.inner.map(P::into_inner)
    }
    pub fn map<U, F: FnOnce(T) -> U>(self, apply: F) -> Option<U> {
        self.into_option().map(apply)
    }
    pub fn unwrap(self) -> T {
        self.expect("called MessageField::unwrap on an unset field")
    }
    pub fn expect(self, message: &str) -> T {
        self.into_option().expect(message)
    }
    pub fn ok_or<E>(self, error: E) -> Result<T, E> {
        self.into_option().ok_or(error)
    }
    pub fn ok_or_else<E, F: FnOnce() -> E>(self, error: F) -> Result<T, E> {
        self.into_option().ok_or_else(error)
    }
}

impl<T: Default> MessageField<T> {
    pub fn from_box(value: Box<T>) -> Self {
        Self::from_pointer(value)
    }
}

impl<T: Default, P: ProtoBox<T>> Default for MessageField<T, P> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T: DefaultInstance, P: ProtoBox<T>> Deref for MessageField<T, P> {
    type Target = T;
    fn deref(&self) -> &T {
        match self.as_option() {
            Some(value) => value,
            None => T::default_instance(),
        }
    }
}

impl<T: Default + Clone, P: ProtoBox<T> + Clone> Clone for MessageField<T, P> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: DefaultInstance + PartialEq, P: ProtoBox<T>> PartialEq for MessageField<T, P> {
    fn eq(&self, other: &Self) -> bool {
        (self.is_unset() && other.is_unset()) || self.deref() == other.deref()
    }
}

impl<T: DefaultInstance + Eq, P: ProtoBox<T>> Eq for MessageField<T, P> {}

impl<T: Default + fmt::Debug, P: ProtoBox<T>> fmt::Debug for MessageField<T, P> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(value) = self.as_option() {
            output
                .debug_tuple("MessageField::Set")
                .field(value)
                .finish()
        } else {
            output.write_str("MessageField::Unset")
        }
    }
}

impl<T: Default, P: ProtoBox<T>> From<Option<T>> for MessageField<T, P> {
    fn from(value: Option<T>) -> Self {
        value.map_or_else(Self::none, Self::some)
    }
}

impl<T: Default, P: ProtoBox<T>> From<T> for MessageField<T, P> {
    fn from(value: T) -> Self {
        Self::some(value)
    }
}

impl<T: Default, P: ProtoBox<T>> From<MessageField<T, P>> for Option<T> {
    fn from(value: MessageField<T, P>) -> Self {
        value.into_option()
    }
}

impl<T: Default + serde::Serialize, P: ProtoBox<T>> serde::Serialize for MessageField<T, P> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_option().serialize(serializer)
    }
}

impl<'de, T: Default + serde::Deserialize<'de>, P: ProtoBox<T>> serde::Deserialize<'de>
    for MessageField<T, P>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Payload {
        value: u32,
    }
    impl DefaultInstance for Payload {
        fn default_instance() -> &'static Self {
            static DEFAULT: Payload = Payload { value: 0 };
            &DEFAULT
        }
    }

    #[test]
    fn default_equality_does_not_erase_presence_or_serde_null() {
        let absent = MessageField::<Payload>::none();
        let present = MessageField::some(Payload::default());
        assert_eq!(absent, present);
        assert!(absent.is_unset());
        assert!(present.is_set());
        assert_eq!(
            serde_json::to_value(absent).unwrap(),
            serde_json::Value::Null
        );
        assert_eq!(
            serde_json::to_value(present).unwrap(),
            serde_json::json!({"value": 0})
        );
    }

    #[test]
    fn inline_and_boxed_mutation_publish_the_same_values() {
        let mut boxed = MessageField::<Payload>::none();
        let mut inline = MessageField::<Payload, Inline<Payload>>::none();
        boxed.modify(|value| value.value = 42);
        inline.modify(|value| value.value = 42);
        assert_eq!(boxed.value, 42);
        assert_eq!(inline.value, 42);
        assert_eq!(boxed.take(), inline.take());
        assert!(boxed.is_unset() && inline.is_unset());
    }
}
