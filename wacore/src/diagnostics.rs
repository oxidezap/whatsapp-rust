//! Safe formatting at boundaries accepting arbitrary backend error sources.
use std::{error::Error, fmt};

/// Preserves known protocol failures and I/O classification without rendering
/// arbitrary application/provider text. Sources remain available on the error.
/// Construction only borrows; formatting walks at most eight source links.
pub struct ErrorSummary<'a>(pub &'a (dyn Error + 'static));
impl fmt::Display for ErrorSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut current = Some(self.0);
        for depth in 0..8 {
            let Some(error) = current else { return Ok(()) };
            if depth > 0 {
                f.write_str(": ")?;
            }
            if let Some(e) = error.downcast_ref::<std::io::Error>() {
                return write!(f, "I/O {:?} (os={:?})", e.kind(), e.raw_os_error());
            }
            if let Some(e) = error.downcast_ref::<crate::request::IqError>() {
                return fmt::Display::fmt(e, f);
            }
            if let Some(e) = error.downcast_ref::<crate::libsignal::protocol::SignalProtocolError>()
            {
                return fmt::Display::fmt(e, f);
            }
            if let Some(e) = error.downcast_ref::<crate::handshake::NoiseError>() {
                return fmt::Display::fmt(e, f);
            }
            if let Some(e) = error.downcast_ref::<wacore_binary::error::BinaryError>() {
                return binary(e, f, 0);
            }
            f.write_str("external error")?;
            current = error.source();
        }
        if current.is_some() {
            f.write_str(": ...")?;
        }
        Ok(())
    }
}
impl fmt::Debug for ErrorSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

fn binary(
    error: &wacore_binary::error::BinaryError,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
) -> fmt::Result {
    use wacore_binary::{diagnostics::ProtocolText, error::BinaryError};
    match error {
        BinaryError::Io(e) => write!(f, "binary I/O {:?} (os={:?})", e.kind(), e.raw_os_error()),
        BinaryError::MissingAttr(key) => write!(f, "missing attribute {}", ProtocolText::new(key)),
        BinaryError::AttrParse(reason) => {
            let kind = if reason.starts_with("Failed to parse bool") {
                "bool"
            } else if reason.starts_with("Failed to parse u64") {
                "u64"
            } else if reason.starts_with("Failed to parse i64") {
                "i64"
            } else if reason.starts_with("Invalid JID") || reason.starts_with("JID_PAIR") {
                "JID"
            } else {
                "protocol"
            };
            write!(f, "invalid {kind} attribute")
        }
        BinaryError::Jid(wacore_binary::jid::JidError::Parse(e)) => {
            write!(f, "invalid JID component: {e}")
        }
        BinaryError::Jid(_) => f.write_str("invalid JID format"),
        BinaryError::Zlib(_) => f.write_str("invalid compressed data"),
        BinaryError::AttrList(list) => {
            write!(f, "{} attribute errors[", list.len())?;
            if depth < 3 {
                for (i, error) in list.iter().take(8).enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    binary(error, f, depth + 1)?;
                }
            }
            if list.len() > 8 || depth >= 3 {
                f.write_str("...")?;
            }
            f.write_str("]")
        }
        _ => fmt::Display::fmt(error, f),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::{
        attrs::{AttrParser, AttrParserRef},
        builder::NodeBuilder,
    };

    #[test]
    fn parser_producers_keep_their_safe_error_categories() {
        for (kind, key, expected) in [
            (0, "offline", "bool"),
            (1, "count", "u64"),
            (2, "t", "i64"),
            (3, "from", "JID"),
        ] {
            let node = NodeBuilder::new("message")
                .attr(key, "synthetic-private-value")
                .build();
            let borrowed = node.as_node_ref();
            let mut owned = AttrParser::new(&node);
            let mut reference = AttrParserRef::new(&borrowed);
            match kind {
                0 => {
                    owned.bool(key);
                    reference.bool(key);
                }
                1 => {
                    owned.optional_u64(key);
                    reference.optional_u64(key);
                }
                2 => {
                    owned.unix_time(key);
                    reference.unix_time(key);
                }
                _ => {
                    owned.jid(key);
                    reference.jid(key);
                }
            }
            for error in [
                owned.finish().expect_err("rejected value"),
                reference.finish().expect_err("rejected value"),
            ] {
                let summary = ErrorSummary(&error).to_string();
                if kind == 3 {
                    assert!(summary.contains("invalid JID"), "{summary}");
                } else {
                    assert_eq!(
                        summary,
                        format!("1 attribute errors[invalid {expected} attribute]")
                    );
                }
                assert!(!summary.contains("synthetic-private-value"));
            }
        }
    }

    #[test]
    fn jid_pair_decoder_keeps_its_safe_error_category() {
        use wacore_binary::{marshal::unmarshal_ref, token};

        let mut wire = vec![token::LIST_8, 3];
        for value in ["message", "from"] {
            wire.extend([token::BINARY_8, value.len() as u8]);
            wire.extend_from_slice(value.as_bytes());
        }
        wire.push(token::JID_PAIR);
        for value in ["19990000049", "synthetic-private-server"] {
            wire.extend([token::BINARY_8, value.len() as u8]);
            wire.extend_from_slice(value.as_bytes());
        }
        let error = unmarshal_ref(&wire).expect_err("unknown JID_PAIR server");
        assert!(
            matches!(&error, wacore_binary::BinaryError::AttrParse(reason) if reason.starts_with("JID_PAIR"))
        );
        assert_eq!(ErrorSummary(&error).to_string(), "invalid JID attribute");
    }
}
