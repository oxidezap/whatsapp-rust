//! The emitters, one per committed artifact.
//!
//! Each returns the file's full text; nothing here touches the filesystem, so an
//! emitter is testable from a fixture and `--check` can compare without writing.

pub mod abprops;
pub mod appstate;
pub mod enums;
pub mod iq_targets;
pub mod mex;
pub mod notif;
pub mod proto;
pub mod tokens;
pub mod version;

/// The two-line preamble every generated Rust file opens with. The version is
/// part of it so a partially refreshed tree is visible in a diff.
pub fn header(what: &str, wa_version: &str) -> String {
    format!("//! Auto-generated {what} (WhatsApp {wa_version}). DO NOT EDIT.\n//!\n")
}

/// A Rust string literal for a wire value. Wire values are ASCII identifiers in
/// practice, so this only has to survive a quote or a backslash appearing.
pub fn rust_str(s: &str) -> String {
    format!("{s:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_stamps_the_version() {
        assert_eq!(
            header("tokens", "2.3000.1"),
            "//! Auto-generated tokens (WhatsApp 2.3000.1). DO NOT EDIT.\n//!\n"
        );
    }
}
