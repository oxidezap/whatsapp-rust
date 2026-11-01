//! The LINK projection shares qualification and oracle validation with UNLINK.
use anyhow::Result;
pub fn generate(wa_version: &str) -> Result<String> {
    super::group_normalization::generate_link(wa_version)
}
