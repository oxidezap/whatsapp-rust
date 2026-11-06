use base64::Engine as _;
use zeroize::Zeroizing;

use super::{FIELD_LIMIT, FORM_LIMIT, RegistrationWireError};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ByteEncoding {
    EveryByte,
    Rfc3986,
    FormText,
}

/// References differ on Signal-field padding; select it explicitly per profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base64Padding {
    Padded,
    Unpadded,
}

impl Base64Padding {
    pub(crate) fn encode(self, bytes: &[u8]) -> String {
        match self {
            Self::Padded => base64::engine::general_purpose::URL_SAFE.encode(bytes),
            Self::Unpadded => base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
        }
    }
}

/// Ordered, already-wire-encoded fields. Values may contain secrets; no Debug/serde.
///
/// Registration references concatenate these fields verbatim. Applying a generic
/// URL form encoder afterward would double-encode the percent bytes in `id`.
pub struct RegistrationForm {
    fields: Vec<(String, Zeroizing<String>)>,
    bytes: usize,
}

impl RegistrationForm {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            bytes: 0,
        }
    }

    /// Append an explicitly encoded value, preserving its exact bytes and order.
    pub fn push_wire(&mut self, name: &str, value: &str) -> Result<(), RegistrationWireError> {
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(RegistrationWireError::InvalidField("name"));
        }
        // Ampersands create new fields; fragments cannot be mirrored into a URL.
        if value
            .bytes()
            .any(|b| !(0x21..=0x7e).contains(&b) || b == b'&' || b == b'#')
        {
            return Err(RegistrationWireError::InvalidField("encoded value"));
        }
        let mut cursor = 0;
        let raw = value.as_bytes();
        while cursor < raw.len() {
            if raw[cursor] == b'%' {
                if cursor + 2 >= raw.len()
                    || !raw[cursor + 1..cursor + 3]
                        .iter()
                        .all(u8::is_ascii_hexdigit)
                {
                    return Err(RegistrationWireError::InvalidField("percent encoding"));
                }
                cursor += 3;
            } else {
                cursor += 1;
            }
        }
        if self.fields.iter().any(|(key, _)| key == name) {
            return Err(RegistrationWireError::DuplicateField);
        }
        let size = self
            .bytes
            .checked_add(name.len() + value.len() + 2)
            .ok_or(RegistrationWireError::SizeLimit)?;
        if size > FORM_LIMIT || self.fields.len() >= FIELD_LIMIT {
            return Err(RegistrationWireError::SizeLimit);
        }
        self.fields
            .push((name.to_owned(), Zeroizing::new(value.to_owned())));
        self.bytes = size;
        Ok(())
    }

    /// Percent-encode every input byte, including bytes that would otherwise be safe.
    pub fn push_percent_bytes(
        &mut self,
        name: &str,
        value: &[u8],
    ) -> Result<(), RegistrationWireError> {
        self.push_encoded_bytes(name, value, ByteEncoding::EveryByte)
    }

    /// Encode raw bytes using RFC 3986's unreserved set and uppercase escapes.
    /// Select this explicitly when the reference profile preserves unreserved
    /// bytes; `push_percent_bytes` continues to escape every byte.
    pub fn push_rfc3986_bytes(
        &mut self,
        name: &str,
        value: &[u8],
    ) -> Result<(), RegistrationWireError> {
        self.push_encoded_bytes(name, value, ByteEncoding::Rfc3986)
    }

    /// Encode UTF-8 text as an application/x-www-form-urlencoded value.
    /// Spaces become `+`, `*` remains literal and `~` is escaped. The result is
    /// already encoded and must not pass through another form encoder.
    pub fn push_form_text(&mut self, name: &str, value: &str) -> Result<(), RegistrationWireError> {
        self.push_encoded_bytes(name, value.as_bytes(), ByteEncoding::FormText)
    }

    fn push_encoded_bytes(
        &mut self,
        name: &str,
        value: &[u8],
        encoding: ByteEncoding,
    ) -> Result<(), RegistrationWireError> {
        if value.len() > FORM_LIMIT / 3 {
            return Err(RegistrationWireError::SizeLimit);
        }
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut encoded = Zeroizing::new(String::with_capacity(value.len() * 3));
        for &byte in value {
            let literal = match encoding {
                ByteEncoding::EveryByte => false,
                ByteEncoding::Rfc3986 => byte.is_ascii_alphanumeric() || b"-._~".contains(&byte),
                ByteEncoding::FormText => byte.is_ascii_alphanumeric() || b"-._*".contains(&byte),
            };
            if literal {
                encoded.push(char::from(byte));
                continue;
            }
            if encoding == ByteEncoding::FormText && byte == b' ' {
                encoded.push('+');
                continue;
            }
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 15) as usize] as char);
        }
        self.push_wire(name, &encoded)
    }

    pub fn push_base64(
        &mut self,
        name: &str,
        value: &[u8],
        padding: Base64Padding,
    ) -> Result<(), RegistrationWireError> {
        if value.len() > FORM_LIMIT / 2 {
            return Err(RegistrationWireError::SizeLimit);
        }
        let encoded = Zeroizing::new(padding.encode(value));
        self.push_wire(name, &encoded)
    }

    /// Reorder known fields by a profile's declared order; retain other fields afterward.
    pub fn encode(&self, order: &[&str]) -> Result<Zeroizing<String>, RegistrationWireError> {
        if order.len() > FIELD_LIMIT {
            return Err(RegistrationWireError::SizeLimit);
        }
        for (index, name) in order.iter().enumerate() {
            if order[..index].contains(name) {
                return Err(RegistrationWireError::DuplicateField);
            }
        }
        let mut output = Zeroizing::new(String::with_capacity(self.bytes));
        let mut append = |name: &str, value: &str| {
            if !output.is_empty() {
                output.push('&');
            }
            output.push_str(name);
            output.push('=');
            output.push_str(value);
        };
        for name in order {
            if let Some((key, value)) = self.fields.iter().find(|(key, _)| key == name) {
                append(key, value);
            }
        }
        for (name, value) in &self.fields {
            if !order.contains(&name.as_str()) {
                append(name, value);
            }
        }
        Ok(output)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.fields.iter().any(|(key, _)| key == name)
    }

    pub(crate) fn append(&mut self, other: &RegistrationForm) -> Result<(), RegistrationWireError> {
        for (name, value) in &other.fields {
            self.push_wire(name, value)?;
        }
        Ok(())
    }
}

impl Default for RegistrationForm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_text_and_byte_policies_do_not_double_encode() {
        let mut form = RegistrationForm::new();
        form.push_percent_bytes("legacy", b"A ~*+").unwrap();
        form.push_rfc3986_bytes("bytes", b"A ~*+").unwrap();
        form.push_form_text("text", "A ~*+é").unwrap();
        assert_eq!(
            &*form.encode(&[]).unwrap(),
            "legacy=%41%20%7E%2A%2B&bytes=A%20~%2A%2B&text=A+%7E*%2B%C3%A9"
        );
    }
    #[test]
    fn order_padding_percent_and_empty_values_are_preserved() {
        let mut form = RegistrationForm::new();
        form.push_wire("cc", "999").unwrap();
        form.push_percent_bytes("id", &[0, 0x20, 0xff]).unwrap();
        form.push_base64("e_regid", &[0, 0, 0, 1], Base64Padding::Unpadded)
            .unwrap();
        form.push_wire("empty", "").unwrap();
        assert_eq!(
            &*form.encode(&["id", "e_regid"]).unwrap(),
            "id=%00%20%FF&e_regid=AAAAAQ&cc=999&empty="
        );
    }
    #[test]
    fn invalid_fields_are_atomic() {
        let mut form = RegistrationForm::new();
        form.push_wire("key", "value").unwrap();
        for value in ["a&code=x", "a\r\nHeader:x", "%zz", "a#fragment", "a b"] {
            assert!(form.push_wire("other", value).is_err());
        }
        assert!(form.push_wire("key", "replacement").is_err());
        assert_eq!(&*form.encode(&[]).unwrap(), "key=value");
    }
}
