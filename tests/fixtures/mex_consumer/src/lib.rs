//! Standalone package: renamed dependency and only supported public paths.
extern crate wa as whatsapp_rust;

#[cfg(test)]
#[path = "../../../mex_public_api.rs"]
mod contracts;

/// The macro must bind the variables type even across a package boundary.
/// ```compile_fail
/// use wa::{mex_operation, wacore::iq::mex_operations::{join_newsletter, get_username}};
/// mex_operation!(join_newsletter).request(get_username::Variables {});
/// ```
/// Request metadata cannot be overwritten independently.
/// ```compile_fail
/// use wa::{mex_operation, wacore::iq::mex_operations::join_newsletter};
/// let mut request = mex_operation!(join_newsletter).request(join_newsletter::Variables { newsletter_id: None });
/// request.doc.id = "another operation";
/// ```
pub fn generated_descriptor() -> whatsapp_rust::MexOperation<
    whatsapp_rust::wacore::iq::mex_operations::join_newsletter::Variables,
> {
    whatsapp_rust::mex_operation!(whatsapp_rust::wacore::iq::mex_operations::join_newsletter)
}
