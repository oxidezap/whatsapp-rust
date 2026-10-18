//! Migration examples compile with defaults and plugins disabled, including wasm.
use std::sync::Arc;
use whatsapp_rust::{
    Bot, Client, ClientBuilder, DecryptFailureAdmission, DecryptFailureContext,
    DecryptFailureDecision, RetryAdmission, RetryContext, RetryDecision,
};

pub struct Policy;
impl RetryAdmission for Policy {
    fn decide(&self, context: &RetryContext<'_>) -> RetryDecision {
        if context.retry_count <= 2 {
            RetryDecision::Accept
        } else {
            RetryDecision::Drop
        }
    }
}
impl DecryptFailureAdmission for Policy {
    fn decide(&self, context: &DecryptFailureContext<'_>) -> DecryptFailureDecision {
        if context.retry_count <= 2 {
            DecryptFailureDecision::RequestRetry
        } else {
            DecryptFailureDecision::AcknowledgeWithoutRetry
        }
    }
}

pub fn owned() -> ClientBuilder {
    Client::builder()
        .with_retry_admission(Policy)
        .with_decrypt_failure_admission(Policy)
}

pub fn shared(
    retry: Arc<dyn RetryAdmission>,
    decrypt: Arc<dyn DecryptFailureAdmission>,
) -> ClientBuilder {
    Client::builder()
        .with_retry_admission_arc(retry)
        .with_decrypt_failure_admission_arc(decrypt)
}

pub fn bot() {
    let _ = Bot::builder()
        .with_retry_admission(Policy)
        .with_decrypt_failure_admission(Policy);
    let _ = Bot::builder()
        .with_retry_admission_arc(Arc::new(Policy))
        .with_decrypt_failure_admission_arc(Arc::new(Policy));
}

// wasm policies may keep local state without Send/Sync.
#[cfg(target_arch = "wasm32")]
pub struct LocalPolicy(pub std::rc::Rc<std::cell::Cell<usize>>);
#[cfg(target_arch = "wasm32")]
impl DecryptFailureAdmission for LocalPolicy {
    fn decide(&self, _: &DecryptFailureContext<'_>) -> DecryptFailureDecision {
        self.0.set(self.0.get() + 1);
        DecryptFailureDecision::RequestRetry
    }
}

#[test]
fn migration_builders_accept_owned_and_shared_policies() {
    let _ = owned();
    let _ = shared(Arc::new(Policy), Arc::new(Policy));
    bot();
}
