//! Multi-turn chat + tool calling surface for harness models.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::errors::AgentError;
use crate::model::Model;
use crate::prompting::context::BuiltContext;
use crate::harness::session::ChatRole;
use crate::tools::ToolDefinition;

/// One model-emitted tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments_json: String,
}

/// One assistant generation from the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantTurn {
    pub content: Option<String>,
    pub tool_calls: Vec<ModelToolCall>,
}

/// Chat models used by the server-side agent loop.
pub trait ChatModel: Send + Sync {
    fn chat(
        &self,
        context: &BuiltContext,
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn, AgentError>;
}

/// Echo the latest user message (no tool calls).
#[derive(Debug, Default, Clone)]
pub struct EchoChatModel;

impl ChatModel for EchoChatModel {
    fn chat(
        &self,
        context: &BuiltContext,
        _tools: &[ToolDefinition],
    ) -> Result<AssistantTurn, AgentError> {
        let last_user = context
            .history
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();
        Ok(AssistantTurn {
            content: Some(format!("[echo] {last_user}")),
            tool_calls: Vec::new(),
        })
    }
}

/// Minimal server reply: `ping` → `pong`, anything else → `echo: <text>`.
#[derive(Debug, Default, Clone)]
pub struct PingPongChatModel;

impl ChatModel for PingPongChatModel {
    fn chat(
        &self,
        context: &BuiltContext,
        _tools: &[ToolDefinition],
    ) -> Result<AssistantTurn, AgentError> {
        let last = context
            .history
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.trim().to_string())
            .unwrap_or_default();
        let out = if last.eq_ignore_ascii_case("ping") {
            "pong".to_string()
        } else {
            format!("echo: {last}")
        };
        Ok(AssistantTurn {
            content: Some(out),
            tool_calls: Vec::new(),
        })
    }
}

/// Wrap a [`Model`] by flattening system + history + tool schemas into one prompt (no native tool protocol).
#[derive(Clone)]
pub struct FlattenChatModel {
    inner: Arc<dyn Model + Send + Sync>,
}

impl FlattenChatModel {
    pub fn new(inner: Arc<dyn Model + Send + Sync>) -> Self {
        Self { inner }
    }

    fn flatten_prompt(context: &BuiltContext, tools: &[ToolDefinition]) -> String {
        let mut s = String::new();
        s.push_str(&context.system);
        s.push_str("\n\n---\n\n");
        for m in &context.history {
            let role = match m.role {
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
                ChatRole::Tool => "tool",
            };
            s.push_str(role);
            s.push_str(": ");
            s.push_str(&m.content);
            s.push('\n');
        }
        if !tools.is_empty() {
            s.push_str("\nAvailable tools (JSON function schema):\n");
            for t in tools {
                let line = serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                });
                if let Ok(txt) = serde_json::to_string(&line) {
                    s.push_str(&txt);
                    s.push('\n');
                }
            }
        }
        s
    }
}

impl ChatModel for FlattenChatModel {
    fn chat(
        &self,
        context: &BuiltContext,
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn, AgentError> {
        let prompt = Self::flatten_prompt(context, tools);
        let text = self.inner.generate(&prompt)?;
        Ok(AssistantTurn {
            content: Some(text),
            tool_calls: Vec::new(),
        })
    }
}

/// Dev model: message `ping` triggers a `ping` tool call if that tool is offered.
#[derive(Debug, Default, Clone)]
pub struct KeywordToolModel;

impl ChatModel for KeywordToolModel {
    fn chat(
        &self,
        context: &BuiltContext,
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn, AgentError> {
        let last_user = context
            .history
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.trim().to_string())
            .unwrap_or_default();

        if last_user == "ping" && tools.iter().any(|t| t.name == "ping") {
            return Ok(AssistantTurn {
                content: None,
                tool_calls: vec![ModelToolCall {
                    id: "kw_ping_1".to_string(),
                    name: "ping".to_string(),
                    arguments_json: "{}".to_string(),
                }],
            });
        }

        Ok(AssistantTurn {
            content: Some(format!("[llmos] {last_user}")),
            tool_calls: Vec::new(),
        })
    }
}

/// Test double: pop canned assistant turns in order.
#[derive(Clone)]
pub struct ScriptedChatModel {
    turns: Arc<Mutex<Vec<AssistantTurn>>>,
}

impl ScriptedChatModel {
    pub fn new(turns: Vec<AssistantTurn>) -> Self {
        Self {
            turns: Arc::new(Mutex::new(turns)),
        }
    }
}

impl ChatModel for ScriptedChatModel {
    fn chat(
        &self,
        _context: &BuiltContext,
        _tools: &[ToolDefinition],
    ) -> Result<AssistantTurn, AgentError> {
        let mut g = self
            .turns
            .lock()
            .map_err(|_| AgentError::msg("scripted model lock poisoned"))?;
        if g.is_empty() {
            return Ok(AssistantTurn {
                content: Some("(no more scripted turns)".into()),
                tool_calls: Vec::new(),
            });
        }
        Ok(g.remove(0))
    }
}

/// Parse `arguments_json` into a [`Value`] for tool dispatch.
pub fn parse_tool_arguments(json_str: &str) -> Result<Value, AgentError> {
    if json_str.trim().is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    serde_json::from_str(json_str).map_err(|e| AgentError::msg(format!("invalid tool JSON: {e}")))
}
