//! `wacore/src/version/generated.rs`: the WhatsApp build every other
//! committed artifact was extracted from.
//!
//! Stamping it as code rather than as a comment is what keeps the registries and
//! the version a device announces from drifting apart: they are regenerated
//! together or not at all.

use anyhow::{Context, Result, bail};

const HEADER: &str = "\
//! The WhatsApp Web build the committed whatspec artifacts were extracted from.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand.

";

pub fn generate(wa_version: &str) -> Result<String> {
    let (primary, secondary, tertiary) = parse(wa_version)?;
    let mut out = super::header("WhatsApp Web version stamp", wa_version);
    out.push_str(HEADER);
    out.push_str(&format!(
        "/// The version string whatspec stamped on the IR: `{wa_version}`.\n\
         pub const WA_WEB_VERSION_STR: &str = \"{wa_version}\";\n\n\
         /// The same version as the `(primary, secondary, tertiary)` triple the\n\
         /// client payload carries.\n\
         pub const WA_WEB_VERSION: (u32, u32, u32) = ({primary}, {secondary}, {tertiary});\n"
    ));
    Ok(out)
}

fn parse(wa_version: &str) -> Result<(u32, u32, u32)> {
    let mut parts = wa_version.split('.');
    let mut next = |what: &'static str| -> Result<u32> {
        parts
            .next()
            .with_context(|| format!("whatspec waVersion {wa_version:?} has no {what} component"))?
            .parse::<u32>()
            .with_context(|| format!("whatspec waVersion {wa_version:?} has a non-numeric {what}"))
    };
    let triple = (next("primary")?, next("secondary")?, next("tertiary")?);
    if parts.next().is_some() {
        bail!("whatspec waVersion {wa_version:?} has more than three components");
    }
    Ok(triple)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_the_string_and_the_triple() {
        let out = generate("2.3000.1044659339").expect("version");
        assert!(out.contains("(WhatsApp 2.3000.1044659339). DO NOT EDIT."));
        assert!(out.contains("pub const WA_WEB_VERSION_STR: &str = \"2.3000.1044659339\";"));
        assert!(out.contains("pub const WA_WEB_VERSION: (u32, u32, u32) = (2, 3000, 1044659339);"));
    }

    #[test]
    fn a_version_that_is_not_a_numeric_triple_is_rejected() {
        // The triple reaches the wire in the client payload, so a version the
        // tool cannot parse must stop the run rather than emit a plausible zero.
        for bad in ["2.3000", "2.3000.1.4", "2.3000.beta", ""] {
            assert!(generate(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn a_revision_at_the_top_of_the_range_still_parses() {
        // Revisions are already past 2^30 and climb with every rollout.
        let out = generate("2.3000.4294967295").expect("version");
        assert!(out.contains("(2, 3000, 4294967295)"));
        assert!(generate("2.3000.4294967296").is_err());
    }
}
