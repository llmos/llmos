//! Model/provider interfaces.

mod chat;

pub use chat::{
    parse_tool_arguments, AssistantTurn, ChatModel, EchoChatModel, FlattenChatModel,
    KeywordToolModel, ModelToolCall, PingPongChatModel, ScriptedChatModel,
};

use crate::errors::AgentError;

/// A text-generation model (LLM or similar).
pub trait Model: Send + Sync {
    fn generate(&self, prompt: &str) -> Result<String, AgentError>;
}

/// Placeholder model for wiring. Always returns an error.
#[derive(Debug, Default, Clone)]
pub struct UnconfiguredModel;

impl Model for UnconfiguredModel {
    fn generate(&self, _prompt: &str) -> Result<String, AgentError> {
        Err(AgentError::msg("model is not configured"))
    }
}

