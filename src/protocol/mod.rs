//! Public protocol types for inputs/outputs and agent turns.
//!
//! For **chat-shaped** transcripts (roles, tool calls, windowing), use [`crate::session`].
//! The [`Memory`] trait below is a minimal append-only log of [`AgentTurn`] for simple
//! `Agent` implementations; it is not what the gRPC harness uses for history.

/// One agent turn request (lightweight; expand as you add tools/models).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurn {
    pub agent_id: String,
    pub input: String,
}

/// One agent turn response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentReply {
    pub agent_id: String,
    pub output: String,
}

/// Abstraction for storing and retrieving past turns (simple `Agent` path).
pub trait Memory: Send {
    fn remember(&mut self, turn: AgentTurn);
    fn history(&self) -> &[AgentTurn];
}

/// In-memory history for [`AgentTurn`] (default for local agents).
#[derive(Debug, Default, Clone)]
pub struct InMemoryMemory {
    history: Vec<AgentTurn>,
}

impl Memory for InMemoryMemory {
    fn remember(&mut self, turn: AgentTurn) {
        self.history.push(turn);
    }

    fn history(&self) -> &[AgentTurn] {
        &self.history
    }
}
