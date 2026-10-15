//! Standalone package: renamed dependency and only supported public paths.

#[cfg(all(test, feature = "contracts"))]
#[path = "../../../mex_public_api.rs"]
mod contracts;

/// The macro must bind the variables type even across a package boundary.
/// ```compile_fail
/// use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::{join_newsletter, get_username}};
/// mex_operation!(join_newsletter).request(get_username::Variables::default());
/// ```
/// Request metadata cannot be overwritten independently.
/// ```compile_fail
/// use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::join_newsletter};
/// let mut request = mex_operation!(join_newsletter).request(join_newsletter::Variables::default());
/// request.doc.id = "another operation";
/// ```
/// The positional constructor is removed; explicit raw construction remains.
/// ```compile_fail
/// use whatsapp_rust::MexRequest;
/// let request = MexRequest::new("CustomQuery", "123456789", &[], ());
/// ```
/// Only the canonical executor is public.
/// ```compile_fail
/// # async fn removed(client: &whatsapp_rust::Client) {
/// use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::get_username};
/// client.mex().query(mex_operation!(get_username).request(get_username::Variables::default())).await;
/// # }
/// ```
/// ```compile_fail
/// # async fn removed(client: &whatsapp_rust::Client) {
/// use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::get_username};
/// client.mex().mutate(mex_operation!(get_username).request(get_username::Variables::default())).await;
/// # }
/// ```
/// Fatal extension failures retain their source in GraphQl, not ExtensionError.
/// ```compile_fail
/// use whatsapp_rust::MexError;
/// let error = MexError::ExtensionError { code: 404, message: "absent".into() };
/// ```
pub fn generated_descriptor() -> whatsapp_rust::MexOperation<
    whatsapp_rust::wacore::iq::mex_operations::join_newsletter::Variables,
> {
    whatsapp_rust::mex_operation!(whatsapp_rust::wacore::iq::mex_operations::join_newsletter)
}

/// Generated contracts allow adding optional fields without supporting literals.
/// ```compile_fail
/// use whatsapp_rust::wacore::iq::mex_operations::join_newsletter;
/// let _ = join_newsletter::Variables { newsletter_id: None };
/// ```
/// ```compile_fail
/// use whatsapp_rust::wacore::iq::mex_operations::contact_manager_customer_profiles;
/// let _ = contact_manager_customer_profiles::Input {
///     candidate_lids: None, ..Default::default()
/// };
/// ```
/// ```compile_fail
/// use whatsapp_rust::wacore::iq::mex_operations::join_newsletter;
/// let _ = join_newsletter::Response { xwa2_newsletter_join_v2: None };
/// ```
/// ```compile_fail
/// use whatsapp_rust::wacore::iq::mex_operations::create_labyrinth_backup;
/// let _ = create_labyrinth_backup::Variables { input: None };
/// ```
/// ```compile_fail
/// use whatsapp_rust::wacore::iq::mex_operations::join_newsletter;
/// let response = join_newsletter::Response::default();
/// let join_newsletter::Response { xwa2_newsletter_join_v2 } = response;
/// ```
pub fn evolving_shapes() {}
