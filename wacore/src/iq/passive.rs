//! Passive mode IQ specification.
//!
//! Passive mode tells the server whether the client is actively receiving
//! notifications or is in a background/passive state.
//!
//! ## Wire Format
//! ```xml
//! <!-- Set Active (foreground) -->
//! <iq xmlns="passive" type="set" to="s.whatsapp.net" id="...">
//!   <active/>
//! </iq>
//!
//! <!-- Set Passive (background) -->
//! <iq xmlns="passive" type="set" to="s.whatsapp.net" id="...">
//!   <passive/>
//! </iq>
//!
//! <!-- Response -->
//! <iq from="s.whatsapp.net" id="..." type="result">
//!   <active/>
//! </iq>
//! ```

use crate::iq::node::required_child;
use crate::iq::spec::IqSpec;
use crate::request::InfoQuery;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Server};
use wacore_binary::{NodeContent, NodeRef};

/// IQ namespace for passive mode.
pub const PASSIVE_NAMESPACE: &str = "passive";

/// Sets the client's passive/active mode.
#[derive(Debug, Clone)]
pub struct PassiveModeSpec {
    /// Whether to set passive mode (true) or active mode (false).
    pub passive: bool,
}

impl PassiveModeSpec {
    /// Create a spec to set passive mode (background).
    pub fn passive() -> Self {
        Self { passive: true }
    }

    /// Create a spec to set active mode (foreground).
    pub fn active() -> Self {
        Self { passive: false }
    }

    /// Create a spec from a boolean (true = passive, false = active).
    pub fn new(passive: bool) -> Self {
        Self { passive }
    }
}

impl IqSpec for PassiveModeSpec {
    type Response = ();

    fn build_iq(&self) -> InfoQuery<'static> {
        let tag = if self.passive { "passive" } else { "active" };
        let child_node = NodeBuilder::new(tag).build();

        InfoQuery::set(
            PASSIVE_NAMESPACE,
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![child_node])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        // WA Web's `ActiveIQResponseSuccess` / `PassiveIQResponseSuccess`: the
        // answer echoes the requested mode as a child (`<active/>` or
        // `<passive/>`), and the parser gates on its presence without reading
        // anything off it (see the generated
        // `super::join_shapes::PASSIVE_ACTIVE_SUCCESS` /
        // `PASSIVE_PASSIVE_SUCCESS`). A result without it is not this RPC's
        // success.
        let tag = if self.passive { "passive" } else { "active" };
        required_child(response, tag)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::Node;

    #[test]
    fn test_passive_mode_spec_passive() {
        let spec = PassiveModeSpec::passive();
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, PASSIVE_NAMESPACE);
        assert_eq!(iq.query_type, crate::request::InfoQueryType::Set);

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].tag, "passive");
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_passive_mode_spec_active() {
        let spec = PassiveModeSpec::active();
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, PASSIVE_NAMESPACE);
        assert_eq!(iq.query_type, crate::request::InfoQueryType::Set);

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].tag, "active");
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    /// Locks both mode parsers to the generated shape constants: the answer
    /// must echo the requested mode child. If the next sync flips the
    /// constants, the parsers above owe a re-read.
    #[test]
    fn test_passive_mode_shapes_match_the_ir_lock() {
        use crate::iq::join_shapes;
        assert_eq!(
            join_shapes::PASSIVE_ACTIVE_SUCCESS,
            &[("ActiveIQResponseSuccess", &["active"][..])]
        );
        assert_eq!(
            join_shapes::PASSIVE_PASSIVE_SUCCESS,
            &[("PassiveIQResponseSuccess", &["passive"][..])]
        );
    }

    fn result_iq(child_tag: Option<&'static str>) -> Node {
        let mut iq = NodeBuilder::new("iq").attr("type", "result");
        if let Some(tag) = child_tag {
            iq = iq.children([NodeBuilder::new(tag).build()]);
        }
        iq.build()
    }

    #[test]
    fn test_passive_mode_spec_parse_response() {
        let spec = PassiveModeSpec::passive();
        let result = spec.parse_response(&result_iq(Some("passive")).as_node_ref());
        assert!(result.is_ok());
    }

    #[test]
    fn test_passive_mode_spec_rejects_result_without_mode_child() {
        let spec = PassiveModeSpec::passive();
        let result = spec.parse_response(&result_iq(None).as_node_ref());
        assert!(result.is_err());
    }

    #[test]
    fn test_passive_mode_spec_rejects_wrong_mode_child() {
        let spec = PassiveModeSpec::passive();
        let result = spec.parse_response(&result_iq(Some("active")).as_node_ref());
        assert!(result.is_err());
    }
}
