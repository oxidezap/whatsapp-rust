//! Plugin IQ reply capabilities remain usable without a raw Client reference.

use std::sync::Arc;

use whatsapp_rust::{
    PluginCapability, PluginContext, PluginInterceptorRegistration, PluginIqInterceptor,
    PluginIqReply, PluginIqReplyError, PluginManifest, PluginResourceError,
};

pub fn manifest() -> PluginManifest {
    PluginManifest::new("consumer.iq-extension", "test")
        .with_capability(PluginCapability::StanzaInterception)
        .with_capability(PluginCapability::IqReply)
}

pub fn register(
    context: &PluginContext,
    interceptor: Arc<dyn PluginIqInterceptor>,
) -> Result<PluginInterceptorRegistration, PluginIqReplyError> {
    context
        .stanza_interception()
        .ok_or(PluginResourceError::NotActive)?
        .register_iq(interceptor)
}

pub async fn result(reply: PluginIqReply) -> Result<(), PluginIqReplyError> {
    reply.result(Vec::new()).await
}

pub async fn error(reply: PluginIqReply) -> Result<(), PluginIqReplyError> {
    reply.error(501).await
}

pub fn prelude_grant(reply: whatsapp_rust::prelude::PluginIqReply) -> PluginIqReply {
    reply
}

pub fn prelude_handler(
    handler: Arc<dyn whatsapp_rust::prelude::PluginIqInterceptor>,
) -> Arc<dyn PluginIqInterceptor> {
    handler
}

#[test]
fn reply_authorization_is_independent_of_outgoing_iq_execution() {
    let capabilities = manifest().capabilities();
    assert!(capabilities.contains(PluginCapability::StanzaInterception));
    assert!(capabilities.contains(PluginCapability::IqReply));
    assert!(!capabilities.contains(PluginCapability::Iq));
    assert_eq!(PluginCapability::IqReply.identifier(), "iq.reply");
}
