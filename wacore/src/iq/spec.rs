use crate::request::InfoQuery;

/// A reusable IQ specification that pairs a request builder with a response parser.
///
/// This keeps protocol-level IQ logic in `wacore`, while runtime orchestration
/// (sending, retries, timeouts) stays in the main crate.
pub trait IqSpec {
    /// The output type produced by parsing the IQ response.
    type Response;

    /// Build the IQ stanza for this spec.
    fn build_iq(&self) -> InfoQuery<'static>;

    /// Optionally encode the IQ stanza directly into a pre-sized buffer,
    /// bypassing the Node intermediate representation. Returns `true` if
    /// the fast path was used; `false` falls back to `build_iq()` + marshal.
    ///
    /// The buffer must contain the full binary-encoded `<iq>` stanza including
    /// the leading format byte. `request_id` is the IQ request ID.
    ///
    /// Note: this path uses the default 75s timeout. Specs that need custom
    /// timeouts (via `InfoQuery::with_timeout`) should not use the fast path.
    fn encode_iq_direct(
        &self,
        _request_id: &str,
        _out: &mut Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        Ok(false)
    }

    /// Parse the IQ response node into the typed response.
    fn parse_response(
        &self,
        response: &wacore_binary::NodeRef<'_>,
    ) -> Result<Self::Response, anyhow::Error>;
}

/// An [`IqSpec`] whose response can be consumed as it is decoded, without ever
/// being held as a tree.
///
/// A response of thousands of small children (the A/B props catalog) costs
/// more as a decoded tree than as wire bytes by an order of magnitude, and on
/// a target with a few hundred KB of heap it cannot be decoded that way at
/// all. A spec that implements this reads the response through a
/// [`NodeStream`](wacore_binary::NodeStream) instead: the client hands it the stream positioned inside
/// the `<iq type="result">` element, before its first child, and the spec walks
/// what it wants and keeps what it needs. Everything else about the request,
/// including how an error response is reported, is the [`IqSpec`] the spec
/// already is. A result the client had to decode whole anyway (a session with
/// a raw node observer attached) is replayed to [`Self::consume_response`]
/// over the bytes that tree came from, so this is the only parser the client
/// runs; [`IqSpec::parse_response`] is what a caller of `execute` gets, and
/// the two should agree on what they produce.
///
/// The stream runs on the read loop, so consumption should be work
/// proportional to the response, not I/O.
pub trait IqStreamSpec: IqSpec {
    /// Consume the `<iq type="result">` response from a stream positioned
    /// inside the root element, before its first child. Children not read
    /// before returning are skipped by the client.
    fn consume_response(
        &self,
        stream: &mut wacore_binary::NodeStream<'_>,
    ) -> Result<Self::Response, anyhow::Error>;
}
