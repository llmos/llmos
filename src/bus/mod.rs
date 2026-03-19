//! Internal message shapes for adapters (gRPC, CLI, channels).

/// Normalized inbound user turn (thin client → harness).
#[derive(Debug, Clone)]
pub struct InboundTurn {
    pub session_id: String,
    pub agent_id: String,
    pub text: String,
}

/// Outbound assistant text chunk (harness → client).
#[derive(Debug, Clone)]
pub struct OutboundChunk {
    pub text: String,
}
