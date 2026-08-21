//! The `w:stats` request that carries a buffer to the server.
//!
//! Lives here rather than in `wacore` on purpose: [`IqSpec`] is a public trait
//! over public types, so a plugin can define its own request without the core
//! gaining a namespace, a module or a line of surface for a subsystem it does
//! not otherwise know about.

use whatsapp_rust::wacore::iq::spec::IqSpec;
use whatsapp_rust::wacore::request::InfoQuery;
use whatsapp_rust::wacore_binary::builder::NodeBuilder;
use whatsapp_rust::wacore_binary::{Jid, NodeContent, NodeRef, Server};

/// The namespace the official client uploads a `regular`-channel buffer under.
pub const STATS_NAMESPACE: &str = "w:stats";

/// One buffer upload:
/// `<iq xmlns="w:stats" to="s.whatsapp.net" type="set"><add t="…">bytes</add></iq>`.
///
/// `t` is the unix second at which the upload was attempted, not a property of
/// the buffer: the official client stamps it when it takes the buffer out of
/// storage, so a buffer retried after a failure carries the retry's time. The
/// events inside carry their own timestamps.
#[derive(Debug, Clone)]
pub struct SendBufferSpec<'a> {
    /// Unix seconds.
    pub t: i64,
    pub buffer: &'a [u8],
}

impl IqSpec for SendBufferSpec<'_> {
    /// The server answers with a bare result. There is nothing in it to parse
    /// and nothing the client does with it beyond "the buffer is delivered".
    type Response = ();

    fn build_iq(&self) -> InfoQuery<'static> {
        let add = NodeBuilder::new("add")
            .attr("t", self.t)
            .bytes(self.buffer.to_vec())
            .build();
        InfoQuery::set(
            STATS_NAMESPACE,
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![add])),
        )
    }

    fn parse_response(&self, _response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::wacore::request::InfoQueryType;

    #[test]
    fn the_request_matches_the_official_shape() {
        let spec = SendBufferSpec {
            t: 1_755_000_000,
            buffer: &[0x57, 0x41, 0x4d],
        };
        let iq = spec.build_iq();
        assert_eq!(iq.namespace, "w:stats");
        assert_eq!(iq.query_type, InfoQueryType::Set);
        assert_eq!(iq.to, Jid::new("", Server::Pn));
        let Some(NodeContent::Nodes(children)) = &iq.content else {
            panic!("the buffer travels as one <add> child");
        };
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].tag, "add");
        assert_eq!(
            children[0].attrs.get("t").map(ToString::to_string),
            Some("1755000000".to_string())
        );
        // The buffer is the node's content, raw, not an attribute and not text.
        assert!(matches!(
            &children[0].content,
            Some(NodeContent::Bytes(b)) if b == &[0x57, 0x41, 0x4d]
        ));
    }
}
