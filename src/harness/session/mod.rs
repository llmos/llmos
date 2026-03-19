//! Conversation sessions: chat-shaped history with safe windowing for tool turns.
//!
//! The gRPC harness stores transcripts here. For a minimal [`crate::core::Agent`] turn log of
//! [`crate::protocol::AgentTurn`], see [`crate::protocol::Memory`] instead.

use serde::{Deserialize, Serialize};

/// OpenAI-style roles for multi-turn + tool workflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    Tool,
}

/// One assistant-emitted tool invocation (model output).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub id: String,
    pub name: String,
    /// JSON object string (model output).
    pub arguments_json: String,
}

/// A single message in the session transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant_text(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant_tools(tool_calls: Vec<ToolCallRecord>, content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
        }
    }
}

/// In-memory session transcript (append-only for cache-friendly reuse).
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub messages: Vec<ChatMessage>,
}

impl Session {
    pub fn push(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
    }

    /// History slice suitable for the chat model: cap size, start at a user turn, align tool boundaries.
    pub fn get_history_for_model(&self, max_messages: usize) -> Vec<ChatMessage> {
        if max_messages == 0 || self.messages.is_empty() {
            return Vec::new();
        }
        let n = self.messages.len().min(max_messages);
        let mut sliced: Vec<ChatMessage> = self.messages[self.messages.len() - n..].to_vec();

        for (i, m) in sliced.iter().enumerate() {
            if m.role == ChatRole::User {
                sliced = sliced[i..].to_vec();
                break;
            }
        }

        let start = find_legal_tool_start(&sliced);
        if start > 0 && start < sliced.len() {
            sliced = sliced[start..].to_vec();
        }
        sliced
    }
}

/// Port of nanobot `Session._find_legal_start`: drop a prefix if a tool result has no matching assistant `tool_calls` id in-window.
fn find_legal_tool_start(messages: &[ChatMessage]) -> usize {
    use std::collections::HashSet;

    let mut declared: HashSet<String> = HashSet::new();
    let mut start: usize = 0;

    for (i, msg) in messages.iter().enumerate() {
        match msg.role {
            ChatRole::Assistant => {
                if let Some(ref tcs) = msg.tool_calls {
                    for tc in tcs {
                        if !tc.id.is_empty() {
                            declared.insert(tc.id.clone());
                        }
                    }
                }
            }
            ChatRole::Tool => {
                let tid = msg.tool_call_id.as_deref().unwrap_or("");
                if !tid.is_empty() && !declared.contains(tid) {
                    start = i + 1;
                    declared.clear();
                    if start <= i {
                        for prev in &messages[start..=i] {
                            if prev.role == ChatRole::Assistant {
                                if let Some(ref tcs) = prev.tool_calls {
                                    for tc in tcs {
                                        if !tc.id.is_empty() {
                                            declared.insert(tc.id.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ChatRole::User => {}
        }
    }
    start
}

/// Holds sessions keyed by client id (e.g. gRPC `session_id`).
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: std::collections::HashMap<String, Session>,
}

impl SessionManager {
    pub fn get_or_create(&mut self, key: impl Into<String>) -> &mut Session {
        let key = key.into();
        self.sessions.entry(key).or_default()
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Session> {
        self.sessions.get_mut(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_start_drops_orphan_tool() {
        let messages = vec![
            ChatMessage::user("hi"),
            ChatMessage::assistant_text("calling"),
            ChatMessage::tool_result("missing-id", "x", "oops"),
            ChatMessage::user("again"),
        ];
        let start = find_legal_tool_start(&messages);
        assert_eq!(start, 3);
    }

    #[test]
    fn get_history_keeps_matched_tool_pair() {
        let mut s = Session::default();
        s.push(ChatMessage::user("u1"));
        s.push(ChatMessage::assistant_tools(
            vec![ToolCallRecord {
                id: "t1".into(),
                name: "ping".into(),
                arguments_json: "{}".into(),
            }],
            "",
        ));
        s.push(ChatMessage::tool_result("t1", "ping", "pong"));
        let h = s.get_history_for_model(50);
        assert_eq!(h.len(), 3);
    }
}
