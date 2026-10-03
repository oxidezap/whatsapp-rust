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
/// The positional constructor is removed; explicit raw construction remains.
/// ```compile_fail
/// use wa::MexRequest;
/// let request = MexRequest::new("CustomQuery", "123456789", &[], ());
/// ```
/// Only the canonical executor is public.
/// ```compile_fail
/// # async fn removed(client: &wa::Client) {
/// use wa::{mex_operation, wacore::iq::mex_operations::get_username};
/// client.mex().query(mex_operation!(get_username).request(get_username::Variables {})).await;
/// # }
/// ```
/// ```compile_fail
/// # async fn removed(client: &wa::Client) {
/// use wa::{mex_operation, wacore::iq::mex_operations::get_username};
/// client.mex().mutate(mex_operation!(get_username).request(get_username::Variables {})).await;
/// # }
/// ```
/// Fatal extension failures retain their source in GraphQl, not ExtensionError.
/// ```compile_fail
/// use wa::MexError;
/// let error = MexError::ExtensionError { code: 404, message: "absent".into() };
/// ```
pub fn generated_descriptor() -> whatsapp_rust::MexOperation<
    whatsapp_rust::wacore::iq::mex_operations::join_newsletter::Variables,
> {
    whatsapp_rust::mex_operation!(whatsapp_rust::wacore::iq::mex_operations::join_newsletter)
}
