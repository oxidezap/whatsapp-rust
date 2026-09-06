// Re-export everything from wacore-noise
pub use wacore_noise::{
    EdgeRoutingError, HandshakeError, HandshakeResult as Result, HandshakeUtils, IkFallbackInputs,
    IkHandshakeOutcome, IkHandshakeState, IkServerHelloOutcome, MAX_EDGE_ROUTING_LEN,
    NoiseCertPolicy, NoiseCipher, NoiseError, NoiseHandshake, VerifiedServerCertChain,
    WA_CERT_PUB_KEY, XxFallbackHandshakeState, XxHandshakeOutcome, XxHandshakeState,
    build_edge_routing_preintro, build_handshake_header, generate_iv,
};

pub mod runner;
pub use runner::{
    HandshakeError as HandshakeExecutionError, HandshakePattern, HandshakeSuccess, recv_frame,
    run_ik_handshake, run_xx_handshake, select_pattern, send_first_handshake_message,
};
