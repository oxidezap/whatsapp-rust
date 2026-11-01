use crate::protocol::retry::should_drop_unknown_device_retry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetransmissionRoute {
    Direct,
    Group,
    Status,
    BroadcastList,
}

impl RetransmissionRoute {
    pub const fn uses_sender_key(self) -> bool {
        matches!(self, Self::Group | Self::Status)
    }

    pub fn device_checked(self, known: bool, keys_present: bool) -> Option<RetryAdmission> {
        (!should_drop_unknown_device_retry(keys_present, known)).then_some(RetryAdmission(self))
    }
}

#[must_use]
pub struct RetryAdmission(RetransmissionRoute);

pub enum RetryKeyOrder {
    BeforeLookup(InstallBeforeLookup),
    AfterLookup(RetryLookup),
}

#[must_use]
pub struct InstallBeforeLookup(RetransmissionRoute);
#[must_use]
pub struct RetryLookup(RetransmissionRoute);
#[must_use]
pub struct RetryRepair {
    route: RetransmissionRoute,
    history_bundle: bool,
}
#[must_use]
pub struct RetryReconciliation(RetransmissionRoute);
#[must_use]
pub struct RetryRateAdmission(RetransmissionRoute);
#[must_use]
pub struct RetryPublication(RetransmissionRoute);

impl RetryAdmission {
    pub fn operator_policy_applies(&self, own_device: bool) -> bool {
        self.0.uses_sender_key() && !own_device
    }

    pub fn admitted(self) -> RetryKeyOrder {
        match self.0 {
            RetransmissionRoute::Direct => RetryKeyOrder::AfterLookup(RetryLookup(self.0)),
            _ => RetryKeyOrder::BeforeLookup(InstallBeforeLookup(self.0)),
        }
    }
}

impl InstallBeforeLookup {
    pub fn installed(self) -> RetryLookup {
        RetryLookup(self.0)
    }
}

impl RetryLookup {
    pub fn repair_missing_sender_key(self, known_history_bundle: bool) -> bool {
        self.0.uses_sender_key() && !known_history_bundle
    }

    pub fn found(self, history_bundle: bool) -> RetryRepair {
        RetryRepair {
            route: self.0,
            history_bundle,
        }
    }
}

impl RetryRepair {
    pub fn refresh_sender_key(&self) -> bool {
        self.route.uses_sender_key() && !self.history_bundle
    }

    pub fn rotate_sender_key(&self, requester_is_lid: bool, participant_known: bool) -> bool {
        self.route == RetransmissionRoute::Group
            && !self.history_bundle
            && !requester_is_lid
            && !participant_known
    }

    pub fn install_keys_after_lookup(&self) -> bool {
        self.route == RetransmissionRoute::Direct
    }

    pub fn keys_ready(self) -> RetryReconciliation {
        RetryReconciliation(self.route)
    }
}

impl RetryReconciliation {
    pub fn reconciled(self) -> RetryRateAdmission {
        RetryRateAdmission(self.0)
    }
}

impl RetryRateAdmission {
    pub fn rate_limit_applies(&self) -> bool {
        self.0 == RetransmissionRoute::Group
    }
    pub fn admitted(self) -> RetryPublication {
        RetryPublication(self.0)
    }
}

impl RetryPublication {
    pub fn route(self) -> RetransmissionRoute {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_devices_require_keys_for_every_route() {
        for route in [
            RetransmissionRoute::Direct,
            RetransmissionRoute::Group,
            RetransmissionRoute::Status,
            RetransmissionRoute::BroadcastList,
        ] {
            assert!(route.device_checked(false, false).is_none());
            assert!(route.device_checked(false, true).is_some());
            assert!(route.device_checked(true, false).is_some());
        }
    }

    fn lookup(route: RetransmissionRoute) -> RetryLookup {
        match route.device_checked(true, false).unwrap().admitted() {
            RetryKeyOrder::BeforeLookup(keys) => keys.installed(),
            RetryKeyOrder::AfterLookup(lookup) => lookup,
        }
    }

    #[test]
    fn direct_keys_follow_lookup_and_other_routes_install_before_it() {
        assert!(matches!(
            RetransmissionRoute::Direct
                .device_checked(true, true)
                .unwrap()
                .admitted(),
            RetryKeyOrder::AfterLookup(_)
        ));
        for route in [
            RetransmissionRoute::Group,
            RetransmissionRoute::Status,
            RetransmissionRoute::BroadcastList,
        ] {
            assert!(matches!(
                route.device_checked(true, true).unwrap().admitted(),
                RetryKeyOrder::BeforeLookup(_)
            ));
        }
        assert!(
            lookup(RetransmissionRoute::Direct)
                .found(false)
                .install_keys_after_lookup()
        );
    }

    #[test]
    fn history_bundles_never_trigger_sender_key_repairs() {
        for route in [RetransmissionRoute::Group, RetransmissionRoute::Status] {
            assert!(!lookup(route).repair_missing_sender_key(true));
            let repair = lookup(route).found(true);
            assert!(!repair.refresh_sender_key());
            assert!(!repair.rotate_sender_key(false, false));
            assert!(lookup(route).found(false).refresh_sender_key());
        }
    }

    #[test]
    fn rotation_and_rate_limit_are_group_only() {
        for route in [
            RetransmissionRoute::Direct,
            RetransmissionRoute::Group,
            RetransmissionRoute::Status,
            RetransmissionRoute::BroadcastList,
        ] {
            let repair = lookup(route).found(false);
            assert_eq!(
                repair.rotate_sender_key(false, false),
                route == RetransmissionRoute::Group
            );
            assert!(!repair.rotate_sender_key(true, false));
            assert!(!repair.rotate_sender_key(false, true));
            let rate = repair.keys_ready().reconciled();
            assert_eq!(
                rate.rate_limit_applies(),
                route == RetransmissionRoute::Group
            );
            assert_eq!(rate.admitted().route(), route);
        }
    }

    #[test]
    fn own_devices_and_direct_messages_bypass_operator_throttling() {
        for route in [
            RetransmissionRoute::Direct,
            RetransmissionRoute::Group,
            RetransmissionRoute::Status,
            RetransmissionRoute::BroadcastList,
        ] {
            let admission = route.device_checked(true, false).unwrap();
            assert!(!admission.operator_policy_applies(true));
            assert_eq!(
                admission.operator_policy_applies(false),
                route.uses_sender_key()
            );
        }
    }
}
