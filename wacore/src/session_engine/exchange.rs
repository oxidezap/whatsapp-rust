/// A registered exchange cannot consume a response before its send completes.
///
/// ```compile_fail
/// use wacore::session_engine::exchange::RegisteredExchange;
/// let registered = RegisteredExchange::new(());
/// registered.response(42);
/// ```
#[must_use]
#[repr(transparent)]
pub struct RegisteredExchange<T>(T);

#[must_use]
#[repr(transparent)]
pub struct SendingExchange<T>(T);

#[must_use]
#[repr(transparent)]
pub struct AwaitingExchange<T>(T);

#[must_use]
pub struct CompletedExchange<T, R> {
    registration: T,
    response: R,
}

impl<T> RegisteredExchange<T> {
    pub fn new(registration: T) -> Self {
        Self(registration)
    }
    pub fn begin_send(self) -> SendingExchange<T> {
        SendingExchange(self.0)
    }
}

impl<T> SendingExchange<T> {
    pub fn sent(self) -> AwaitingExchange<T> {
        AwaitingExchange(self.0)
    }
}

impl<T> AwaitingExchange<T> {
    pub fn response<R>(self, response: R) -> CompletedExchange<T, R> {
        CompletedExchange {
            registration: self.0,
            response,
        }
    }
}

impl<T, R> CompletedExchange<T, R> {
    pub fn into_response(self) -> R {
        drop(self.registration);
        self.response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Registration(Arc<AtomicUsize>);
    impl Drop for Registration {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn cancellation_in_each_inflight_phase_releases_one_registration() {
        let releases = Arc::new(AtomicUsize::new(0));
        drop(RegisteredExchange::new(Registration(releases.clone())));
        drop(RegisteredExchange::new(Registration(releases.clone())).begin_send());
        drop(
            RegisteredExchange::new(Registration(releases.clone()))
                .begin_send()
                .sent(),
        );
        assert_eq!(releases.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn completion_releases_registration_before_exposing_the_response() {
        let releases = Arc::new(AtomicUsize::new(0));
        let result = RegisteredExchange::new(Registration(releases.clone()))
            .begin_send()
            .sent()
            .response(42);
        assert_eq!(releases.load(Ordering::SeqCst), 0);
        assert_eq!(result.into_response(), 42);
        assert_eq!(releases.load(Ordering::SeqCst), 1);
        assert_eq!(
            size_of::<RegisteredExchange<Registration>>(),
            size_of::<Registration>()
        );
        assert_eq!(
            size_of::<AwaitingExchange<Registration>>(),
            size_of::<Registration>()
        );
    }
}
