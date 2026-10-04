//! Independent community construction and partial-error consumer.
//!
//! Inputs use constructors rather than exhaustive literals:
//! ```compile_fail,E0639
//! use whatsapp_rust::{CreateCommunityOptions, GroupSubject};
//! let _ = CreateCommunityOptions {
//!     name: GroupSubject::new("Fictitious community").unwrap(),
//!     description: None,
//!     closed: false,
//!     allow_non_admin_sub_group_creation: false,
//!     create_general_chat: true,
//! };
//! ```
//! Failures also permit future fields:
//! ```compile_fail,E0639
//! use whatsapp_rust::{SubgroupFailure, Jid};
//! let _ = SubgroupFailure { jid: "120363000000000001@g.us".parse::<Jid>().unwrap(), code: 403 };
//! ```
//! Configuration steps require a fallback:
//! ```compile_fail,E0004
//! use whatsapp_rust::CommunityConfigurationStep;
//! fn exhaustive(step: CommunityConfigurationStep) {
//!     match step { CommunityConfigurationStep::SetDescription => {} }
//! }
//! ```

use whatsapp_rust::{
    Client, CommunityConfigurationStep, CommunityError, CreateCommunityOptions,
    CreateCommunityResult, GroupDescription, Jid, LinkSubgroupsResult, PreviousDescription,
    SubgroupFailure, UnlinkSubgroupsResult,
};

pub fn options() -> Result<CreateCommunityOptions, Box<dyn std::error::Error>> {
    let mut options = CreateCommunityOptions::new("Fictitious community")?
        .with_description(GroupDescription::new("Requested description")?);
    options.closed = true;
    options.allow_non_admin_sub_group_creation = true;
    options.create_general_chat = false;
    Ok(options)
}

/// Resume a configuration error on its created JID without issuing create again.
/// Reading the current token matters when a failed IQ may have committed remotely.
pub async fn resume_description<'a>(
    client: &Client,
    error: &'a CommunityError,
    requested: GroupDescription,
) -> Result<Jid, Box<dyn std::error::Error + 'a>> {
    match error {
        CommunityError::ConfigurationFailed {
            created_jid,
            step: CommunityConfigurationStep::SetDescription,
            ..
        } => {
            // Borrowing leaves the initial typed cause with the caller.
            client
                .groups()
                .set_description(created_jid, Some(requested), PreviousDescription::Resolve)
                .await?;
            Ok(created_jid.clone())
        }
        other => Err(Box::new(other)),
    }
}

pub fn created_jid(result: &CreateCommunityResult) -> &Jid {
    &result.metadata.id
}

pub fn linked(result: LinkSubgroupsResult) -> (Vec<Jid>, Vec<SubgroupFailure>) {
    let LinkSubgroupsResult {
        linked_jids,
        failed_groups,
        ..
    } = result;
    (linked_jids, failed_groups)
}

pub fn unlinked(result: UnlinkSubgroupsResult) -> (Vec<Jid>, Vec<SubgroupFailure>) {
    let UnlinkSubgroupsResult {
        unlinked_jids,
        failed_groups,
        ..
    } = result;
    (unlinked_jids, failed_groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::{GroupError, request::IqError};

    #[test]
    fn validated_construction_and_defaults() {
        let defaults = CreateCommunityOptions::new("Fictitious community").unwrap();
        assert!(defaults.create_general_chat);
        assert!(!defaults.closed);
        assert!(defaults.description.is_none());
        assert!(CreateCommunityOptions::new("a".repeat(101)).is_err());
        assert!(GroupDescription::new("a".repeat(2049)).is_err());
        assert_eq!(
            options().unwrap().description.unwrap().as_str(),
            "Requested description"
        );
    }

    #[test]
    fn partial_error_keeps_jid_step_and_typed_cause() {
        let jid: Jid = "120363000000000001@g.us".parse().unwrap();
        let error = CommunityError::ConfigurationFailed {
            created_jid: jid.clone(),
            step: CommunityConfigurationStep::SetDescription,
            source: Box::new(GroupError::Iq(IqError::Timeout)),
        };
        assert!(std::error::Error::source(&error).is_some());
        match error {
            CommunityError::ConfigurationFailed {
                created_jid,
                step,
                source,
            } => {
                assert_eq!(created_jid, jid);
                assert_eq!(step, CommunityConfigurationStep::SetDescription);
                assert!(matches!(*source, GroupError::Iq(IqError::Timeout)));
            }
            _ => panic!("expected configuration failure"),
        }
    }

    #[test]
    fn failure_fields_preserve_unknown_codes() {
        let failure =
            SubgroupFailure::new("120363000000000001@g.us".parse::<Jid>().unwrap(), u32::MAX);
        let SubgroupFailure { jid, code, .. } = failure;
        assert_eq!(jid.to_string(), "120363000000000001@g.us");
        assert_eq!(code, u32::MAX);
    }
}
