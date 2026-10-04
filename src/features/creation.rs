//! Shared implementation of the poll/event creation contracts, not a send pipeline.

/// A fresh secret for one message. Callers retain their own validation,
/// identity selection and context construction before publishing it.
pub(super) fn generate_message_secret() -> crate::MessageSecret {
    use rand::Rng;
    let mut bytes = [0u8; wacore::reporting_token::MESSAGE_SECRET_SIZE];
    rand::rng().fill_bytes(&mut bytes);
    crate::MessageSecret::from_bytes(bytes)
}

macro_rules! creation_types {
    ($created:ident, $reference:ident, $accessor:ident) => {
        /// Successfully sent creation, its actual creator identity and its secret.
        ///
        /// The send result describes this creation envelope, not a later vote or
        /// RSVP. Its protobuf remains in the original `Arc`, without a deep clone.
        /// `Debug` redacts the entire result because the nested protobuf also
        /// carries the secret. Explicit access/ownership exposes sensitive data.
        #[derive(Clone)]
        pub struct $created {
            send_result: SendResult,
            creator: Jid,
            secret: crate::MessageSecret,
        }

        impl $created {
            pub(crate) fn new(
                send_result: SendResult,
                creator: Jid,
                secret: crate::MessageSecret,
            ) -> Self {
                Self {
                    send_result,
                    creator,
                    secret,
                }
            }

            pub fn send_result(&self) -> &SendResult {
                &self.send_result
            }
            pub fn creator(&self) -> &Jid {
                &self.creator
            }
            pub fn secret(&self) -> &crate::MessageSecret {
                &self.secret
            }

            /// Borrow the addressing and key material; does not retain or clone
            /// the message protobuf. As with SendResult::message_ref, malformed
            /// raw result addressing is reported rather than panicking.
            pub fn $accessor(&self) -> Result<$reference<'_>, crate::MessageRefError> {
                $reference::new(self.send_result.message_ref()?, &self.creator, &self.secret)
            }

            /// Explicit ownership escape. The returned SendResult's Debug is
            /// NOT redacted and its message includes secret material.
            pub fn into_parts(self) -> (SendResult, Jid, crate::MessageSecret) {
                (self.send_result, self.creator, self.secret)
            }
        }

        impl std::fmt::Debug for $created {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($created), "([REDACTED])"))
            }
        }

        /// Borrowed creation addressing plus the creator needed by HKDF and a
        /// validated secret. Constructible by external hosts for received or
        /// manually persisted messages, without any creation protobuf clone.
        #[derive(Clone)]
        pub struct $reference<'a> {
            message: crate::MessageRef<'a>,
            creator: &'a Jid,
            secret: &'a crate::MessageSecret,
        }

        impl<'a> $reference<'a> {
            /// The creator is the original crypto identity; it may be a PN/LID
            /// alias of `message.sender()`. This constructor validates shape,
            /// not identity equivalence or the secret's association with a
            /// creation. External hosts must supply the matching metadata.
            pub fn new(
                message: crate::MessageRef<'a>,
                creator: &'a Jid,
                secret: &'a crate::MessageSecret,
            ) -> Result<Self, crate::MessageRefError> {
                message.require_chat_operation()?;
                if creator.user.is_empty() {
                    return Err(crate::MessageRefError::MissingSender);
                }
                Ok(Self {
                    message,
                    creator,
                    secret,
                })
            }
            pub fn message(&self) -> &crate::MessageRef<'a> {
                &self.message
            }
            pub fn creator(&self) -> &'a Jid {
                self.creator
            }
            pub fn secret(&self) -> &'a crate::MessageSecret {
                self.secret
            }
        }

        impl std::fmt::Debug for $reference<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($reference), "([REDACTED])"))
            }
        }
    };
}

pub(crate) use creation_types;
