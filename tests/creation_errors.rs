//! Creation errors preserve typed causes without exposing secret material.

use std::error::Error;

use whatsapp_rust::{
    InvalidMessageSecret, MessageId, MessageRefError, MessageSecret, PollError, SendError,
};

#[test]
fn invalid_secret_wrappers_keep_the_typed_length_source_and_display() {
    for length in [0, 31, 33] {
        let cause = MessageSecret::try_from(vec![177; length]).unwrap_err();
        let errors: [Box<dyn Error>; 2] = [
            Box::new(PollError::from(cause)),
            Box::new(SendError::from(cause)),
        ];
        for error in errors {
            assert_eq!(error.to_string(), cause.to_string());
            let source = error
                .source()
                .expect("the validated length error must remain in the chain")
                .downcast_ref::<InvalidMessageSecret>()
                .expect("the cause must retain its concrete type");
            assert_eq!(source.actual, length);
            assert!(!format!("{error:?}").contains("177"));
            assert!(!format!("{error:#?}").contains("177"));
        }
    }
}

#[test]
fn poll_reference_wrapper_keeps_the_typed_addressing_source_and_display() {
    let cause = MessageId::new("").unwrap_err();
    let error = PollError::from(cause);
    assert_eq!(error.to_string(), cause.to_string());
    assert_eq!(
        error
            .source()
            .expect("the addressing error must remain in the chain")
            .downcast_ref::<MessageRefError>(),
        Some(&MessageRefError::EmptyMessageId),
    );
}
