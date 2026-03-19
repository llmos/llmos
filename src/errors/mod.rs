//! Error types used across the `llmos` crate.

/// Errors produced while running an agent.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agent error: {0}")]
    Message(String),
}

impl AgentError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

