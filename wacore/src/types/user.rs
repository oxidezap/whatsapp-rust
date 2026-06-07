use chrono::{DateTime, Utc};
use waproto::whatsapp as wa;

#[derive(Debug, Clone)]
pub struct VerifiedName {
    pub certificate: Box<wa::VerifiedNameCertificate>,
    pub details: Box<wa::verified_name_certificate::Details>,
}

#[derive(Debug, Clone, Default)]
pub struct LocalChatSettings {
    pub found: bool,
    pub muted_until: Option<DateTime<Utc>>,
    pub pinned: bool,
    pub archived: bool,
}
