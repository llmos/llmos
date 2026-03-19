//! Core agent traits and built-in agent implementations.

use std::fmt;

use crate::errors::AgentError;

/// Something that can process text input and return a response.
pub trait Agent: Send {
    fn id(&self) -> &str;

    /// Handle one turn of input. Override this for real behavior.
    fn handle(&mut self, input: &str) -> Result<String, AgentError>;
}

/// Minimal echo agent for wiring tests and the CLI before you plug in real logic.
#[derive(Debug, Default, Clone)]
pub struct EchoAgent {
    id: String,
}

impl EchoAgent {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

impl Agent for EchoAgent {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn handle(&mut self, input: &str) -> Result<String, AgentError> {
        Ok(format!("[{}] {input}", self.id))
    }
}

impl fmt::Display for EchoAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EchoAgent({})", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_agent_prefixes_id() {
        let mut a = EchoAgent::new("demo");
        let out = a.handle("hello").unwrap();
        assert_eq!(out, "[demo] hello");
    }
}

