use std::fmt;

/// Holds a boxed concrete cause while exposing that cause to derived `Error::source`.
///
/// This wrapper intentionally does not implement `Error`: the derive must dereference
/// to the concrete cause rather than expose the box as the source.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BoxedErrorSource<E>(Box<E>);

impl<E> BoxedErrorSource<E> {
    pub fn new(error: E) -> Self {
        Self(Box::new(error))
    }

    pub fn into_box(self) -> Box<E> {
        self.0
    }
}

impl<E> From<Box<E>> for BoxedErrorSource<E> {
    fn from(error: Box<E>) -> Self {
        Self(error)
    }
}

impl<E> AsRef<E> for BoxedErrorSource<E> {
    fn as_ref(&self) -> &E {
        self.0.as_ref()
    }
}

impl<E> std::ops::Deref for BoxedErrorSource<E> {
    type Target = E;

    fn deref(&self) -> &E {
        self.0.as_ref()
    }
}

impl<E: fmt::Display> fmt::Display for BoxedErrorSource<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0.as_ref(), f)
    }
}

impl<E: fmt::Debug> fmt::Debug for BoxedErrorSource<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_ref(), f)
    }
}
