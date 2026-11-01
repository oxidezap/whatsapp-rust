//! Ordered warning boundary for the qualified LINK/UNLINK null-result paths.
use super::normalized::GroupParseError;

/// A caller can record or forward the two official warning operations. Failures
/// propagate at their original position relative to payload validation.
pub trait GroupWarningSink {
    fn warn(
        &mut self,
        template: &'static [&'static str],
        value: &str,
    ) -> Result<(), GroupParseError>;
    fn send_logs(&mut self, event: &'static str) -> Result<(), GroupParseError>;
}

pub(super) struct IgnoreWarnings;
impl GroupWarningSink for IgnoreWarnings {
    fn warn(&mut self, _: &'static [&'static str], _: &str) -> Result<(), GroupParseError> {
        Ok(())
    }
    fn send_logs(&mut self, _: &'static str) -> Result<(), GroupParseError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WarningSpec {
    pub template: &'static [&'static str],
    pub event: &'static str,
}
impl WarningSpec {
    pub fn emit(self, value: &str, sink: &mut dyn GroupWarningSink) -> Result<(), GroupParseError> {
        sink.warn(self.template, value)?;
        sink.send_logs(self.event)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Decision {
    Action(&'static str),
    Null(WarningSpec),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stanza::groups::{
        link_normalized::parse_link_with_warnings,
        normalized::{
            GroupParseContext, parse_normalized_group_notification_with_warnings,
            parse_unlink_with_warnings,
        },
    };
    use wacore_binary::builder::NodeBuilder;
    #[derive(Default)]
    struct Recorder {
        calls: Vec<&'static str>,
        fail_warn: bool,
        fail_send: bool,
    }
    impl GroupWarningSink for Recorder {
        fn warn(&mut self, _: &'static [&'static str], _: &str) -> Result<(), GroupParseError> {
            self.calls.push("WARN");
            if self.fail_warn {
                return Err(GroupParseError::Rejected("warning failure".into()));
            }
            Ok(())
        }
        fn send_logs(&mut self, _: &'static str) -> Result<(), GroupParseError> {
            self.calls.push("sendLogs");
            if self.fail_send {
                return Err(GroupParseError::Rejected("send failure".into()));
            }
            Ok(())
        }
    }
    #[test]
    fn link_null_warns_before_payload_reads_and_propagates_both_failures() {
        let node = NodeBuilder::new("link")
            .attr("link_type", "future_type")
            .children([NodeBuilder::new("group").build()])
            .build();
        let mut recorder = Recorder::default();
        let result = parse_link_with_warnings(&node.as_node_ref(), &mut recorder).unwrap();
        assert!(result.kind.is_none() && result.groups.is_empty());
        assert_eq!(recorder.calls, ["WARN", "sendLogs"]);
        let mut recorder = Recorder {
            fail_warn: true,
            ..Default::default()
        };
        assert_eq!(
            parse_link_with_warnings(&node.as_node_ref(), &mut recorder).unwrap_err(),
            GroupParseError::Rejected("warning failure".into())
        );
        assert_eq!(recorder.calls, ["WARN"]);
        let mut recorder = Recorder {
            fail_send: true,
            ..Default::default()
        };
        assert_eq!(
            parse_link_with_warnings(&node.as_node_ref(), &mut recorder).unwrap_err(),
            GroupParseError::Rejected("send failure".into())
        );
        assert_eq!(recorder.calls, ["WARN", "sendLogs"]);
    }
    #[test]
    fn unlink_payload_errors_precede_unknown_type_warning() {
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .children([NodeBuilder::new("group").build()])
            .build();
        let mut recorder = Recorder {
            fail_warn: true,
            ..Default::default()
        };
        assert_eq!(
            parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder).unwrap_err(),
            GroupParseError::Field("jid")
        );
        assert!(recorder.calls.is_empty());
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .build();
        assert_eq!(
            parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder).unwrap_err(),
            GroupParseError::Rejected("warning failure".into())
        );
        assert_eq!(recorder.calls, ["WARN"]);
    }
    #[test]
    fn null_actions_are_omitted_from_the_complete_notification() {
        let context = GroupParseContext {
            me_device_lid: None,
            me_device_pn: None,
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &|_| false,
            normalize_country_code: &|_| None,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .children([
                NodeBuilder::new("link")
                    .attr("link_type", "future_type")
                    .children([NodeBuilder::new("group").build()])
                    .build(),
                NodeBuilder::new("unlink")
                    .attr("unlink_type", "future_type")
                    .build(),
            ])
            .build();
        let mut recorder = Recorder::default();
        let result = parse_normalized_group_notification_with_warnings(
            &node.as_node_ref(),
            &context,
            &mut recorder,
        )
        .unwrap();
        assert!(result.actions.is_empty());
        assert_eq!(recorder.calls, ["WARN", "sendLogs", "WARN", "sendLogs"]);
    }
}
