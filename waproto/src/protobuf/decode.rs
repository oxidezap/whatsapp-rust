#![forbid(unsafe_code)]

use super::DecodeError;
use std::cell::Cell;

pub const MAX_MESSAGE_BYTES: u32 = 0x7fff_ffff;
pub const RECURSION_LIMIT: u32 = 100;
pub const DEFAULT_UNKNOWN_FIELD_LIMIT: usize = 1_000_000;
pub const DEFAULT_ELEMENT_MEMORY_LIMIT: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct DecodeContext<'a> {
    depth: u32,
    unknown: &'a Cell<usize>,
    elements: Option<&'a Cell<usize>>,
}

impl<'a> DecodeContext<'a> {
    pub fn new(depth: u32, unknown_field_limit: &'a Cell<usize>) -> Self {
        Self {
            depth,
            unknown: unknown_field_limit,
            elements: None,
        }
    }

    pub fn with_element_memory(mut self, element_memory_limit: &'a Cell<usize>) -> Self {
        self.elements = Some(element_memory_limit);
        self
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }
    pub fn remaining_unknown_fields(&self) -> usize {
        self.unknown.get()
    }
    pub fn remaining_element_memory(&self) -> Option<usize> {
        self.elements.map(Cell::get)
    }

    pub fn descend(self) -> Result<Self, DecodeError> {
        Ok(Self {
            depth: self
                .depth
                .checked_sub(1)
                .ok_or(DecodeError::RecursionLimitExceeded)?,
            ..self
        })
    }

    pub fn register_unknown_field(&self) -> Result<(), DecodeError> {
        let remaining = self
            .unknown
            .get()
            .checked_sub(1)
            .ok_or(DecodeError::UnknownFieldLimitExceeded)?;
        self.unknown.set(remaining);
        Ok(())
    }

    pub fn register_element_memory(&self, bytes: usize) -> Result<(), DecodeError> {
        if let Some(budget) = self.elements {
            let remaining = budget
                .get()
                .checked_sub(bytes)
                .ok_or(DecodeError::ElementMemoryLimitExceeded)?;
            budget.set(remaining);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DecodeOptions {
    recursion_limit: u32,
    max_message_size: usize,
    unknown_field_limit: usize,
    element_memory_limit: usize,
}

impl DecodeOptions {
    pub fn new() -> Self {
        Self {
            recursion_limit: RECURSION_LIMIT,
            max_message_size: MAX_MESSAGE_BYTES as usize,
            unknown_field_limit: DEFAULT_UNKNOWN_FIELD_LIMIT,
            element_memory_limit: DEFAULT_ELEMENT_MEMORY_LIMIT,
        }
    }
    pub fn with_recursion_limit(mut self, depth: u32) -> Self {
        self.recursion_limit = depth;
        self
    }
    pub fn with_max_message_size(mut self, bytes: usize) -> Self {
        debug_assert!(
            bytes <= MAX_MESSAGE_BYTES as usize,
            "slice size budget exceeds protobuf maximum"
        );
        self.max_message_size = bytes.min(MAX_MESSAGE_BYTES as usize);
        self
    }
    pub fn with_unknown_field_limit(mut self, count: usize) -> Self {
        self.unknown_field_limit = count;
        self
    }
    pub fn with_element_memory_limit(mut self, bytes: usize) -> Self {
        self.element_memory_limit = bytes;
        self
    }

    pub(crate) fn run<R>(
        &self,
        input_size: usize,
        decode: impl for<'a> FnOnce(DecodeContext<'a>) -> Result<R, DecodeError>,
    ) -> Result<R, DecodeError> {
        if input_size > self.max_message_size {
            return Err(DecodeError::MessageTooLarge);
        }
        let unknown = Cell::new(self.unknown_field_limit);
        let elements = Cell::new(self.element_memory_limit);
        decode(DecodeContext::new(self.recursion_limit, &unknown).with_element_memory(&elements))
    }
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_siblings_share_budgets_and_failed_charges_leave_them_unchanged() {
        DecodeOptions::new()
            .with_unknown_field_limit(2)
            .with_element_memory_limit(9)
            .run(1, |root| {
                let child = root.descend()?;
                child.register_element_memory(5)?;
                assert_eq!(root.remaining_element_memory(), Some(4));
                assert_eq!(
                    root.register_element_memory(5),
                    Err(DecodeError::ElementMemoryLimitExceeded)
                );
                assert_eq!(root.remaining_element_memory(), Some(4));
                child.register_unknown_field()?;
                root.register_unknown_field()?;
                assert_eq!(
                    child.register_unknown_field(),
                    Err(DecodeError::UnknownFieldLimitExceeded)
                );
                assert_eq!(root.remaining_unknown_fields(), 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn manual_context_retains_the_legacy_optional_element_budget() {
        let unknown = Cell::new(1);
        let context = DecodeContext::new(0, &unknown);
        assert_eq!(context.remaining_element_memory(), None);
        context.register_element_memory(usize::MAX).unwrap();
        assert!(matches!(
            context.descend(),
            Err(DecodeError::RecursionLimitExceeded)
        ));
    }

    #[test]
    fn size_rejection_happens_before_any_decoding_work() {
        let options = DecodeOptions::new().with_max_message_size(3);
        assert_eq!(
            options.run::<()>(4, |_| panic!("must not decode")),
            Err(DecodeError::MessageTooLarge)
        );
    }
}
