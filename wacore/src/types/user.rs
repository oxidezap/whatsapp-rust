use waproto::whatsapp as wa;

#[derive(Debug, Clone)]
pub struct VerifiedName {
    pub certificate: Box<wa::VerifiedNameCertificate>,
    pub details: Box<wa::verified_name_certificate::Details>,
}
