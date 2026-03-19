//! Agent execution runtime (turn loop).

use crate::core::Agent;
use crate::errors::AgentError;
use crate::protocol::AgentReply;

/// A minimal runtime that drives a single agent turn.
pub struct AgentRuntime;

impl Default for AgentRuntime {
    fn default() -> Self {
        Self
    }
}

impl AgentRuntime {
    /// Run exactly one turn: feed `input` to `agent` and return a structured reply.
    pub fn run_turn<A: Agent>(&self, agent: &mut A, input: &str) -> Result<AgentReply, AgentError> {
        let output = agent.handle(input)?;
        Ok(AgentReply {
            agent_id: agent.id().to_string(),
            output,
        })
    }
}

