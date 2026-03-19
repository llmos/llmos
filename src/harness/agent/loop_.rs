//! Multi-iteration harness: call the chat model, execute tool calls, repeat.

use crate::errors::AgentError;
use crate::model::{parse_tool_arguments, AssistantTurn, ChatModel, ModelToolCall};
use crate::policy::ToolPolicy;
use crate::prompting::ContextBuilder;
use crate::harness::session::{ChatMessage, Session, ToolCallRecord};
use crate::tools::ToolRegistry;

/// Upper bound on model↔tool rounds for one user message.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_iterations: u32,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 16,
        }
    }
}

/// Events streamed to the RPC layer (mapped to protobuf in the driver).
#[derive(Debug, Clone)]
pub enum LoopEmit {
    Phase { name: String, detail: String },
    Iteration { n: u32 },
    ToolInvoked {
        name: String,
        request: String,
        response: String,
    },
    AssistantText { text: String },
    Log { level: String, message: String },
}

/// Append `user_message`, run model/tool iterations until assistant returns text without tools.
pub fn run_turn_with_loop(
    session: &mut Session,
    user_message: String,
    registry: &ToolRegistry,
    policy: &ToolPolicy,
    chat_model: &dyn ChatModel,
    ctx: &ContextBuilder,
    config: &AgentLoopConfig,
    runtime_note: Option<&str>,
    mut emit: impl FnMut(LoopEmit),
) -> Result<String, AgentError> {
    session.push(ChatMessage::user(user_message));

    for iteration in 0..config.max_iterations {
        emit(LoopEmit::Iteration { n: iteration + 1 });

        let mut built = ctx.build_for_session(session)?;
        if let Some(note) = runtime_note {
            built.inject_runtime_note(note);
        }

        let defs = registry.definitions_for_policy(policy);
        let turn = chat_model.chat(&built, &defs)?;

        if turn.tool_calls.is_empty() {
            let text = turn.content.unwrap_or_default();
            session.push(ChatMessage::assistant_text(text.clone()));
            emit(LoopEmit::AssistantText { text: text.clone() });
            return Ok(text);
        }

        push_assistant_tool_turn(session, &turn);
        execute_tool_calls(session, registry, policy, &turn.tool_calls, &mut emit)?;
    }

    Err(AgentError::msg(format!(
        "agent loop exceeded max_iterations ({})",
        config.max_iterations
    )))
}

fn push_assistant_tool_turn(session: &mut Session, turn: &AssistantTurn) {
    let records: Vec<ToolCallRecord> = turn
        .tool_calls
        .iter()
        .map(|tc| ToolCallRecord {
            id: tc.id.clone(),
            name: tc.name.clone(),
            arguments_json: tc.arguments_json.clone(),
        })
        .collect();
    let content = turn.content.clone().unwrap_or_default();
    session.push(ChatMessage::assistant_tools(records, content));
}

fn execute_tool_calls(
    session: &mut Session,
    registry: &ToolRegistry,
    policy: &ToolPolicy,
    calls: &[ModelToolCall],
    emit: &mut impl FnMut(LoopEmit),
) -> Result<(), AgentError> {
    for tc in calls {
        if !policy.allows(&tc.name) {
            return Err(AgentError::msg(format!(
                "tool {} is not authorized for this turn",
                tc.name
            )));
        }

        emit(LoopEmit::Phase {
            name: "tool_execute".into(),
            detail: tc.name.clone(),
        });

        let args = parse_tool_arguments(&tc.arguments_json)?;
        let request = tc.arguments_json.clone();
        let response = registry.execute_json(&tc.name, &args)?;

        emit(LoopEmit::ToolInvoked {
            name: tc.name.clone(),
            request,
            response: response.clone(),
        });

        session.push(ChatMessage::tool_result(tc.id.clone(), tc.name.clone(), response));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AssistantTurn, ModelToolCall, ScriptedChatModel};
    use crate::tools::{PingTool, ToolRegistry};
    use std::sync::Arc;

    #[test]
    fn loop_runs_tool_then_finishes() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(PingTool));
        let policy = ToolPolicy::default();
        let model = ScriptedChatModel::new(vec![
            AssistantTurn {
                content: None,
                tool_calls: vec![ModelToolCall {
                    id: "1".into(),
                    name: "ping".into(),
                    arguments_json: "{}".into(),
                }],
            },
            AssistantTurn {
                content: Some("all good".into()),
                tool_calls: vec![],
            },
        ]);
        let ctx = ContextBuilder::default();
        let cfg = AgentLoopConfig {
            max_iterations: 8,
        };
        let mut session = Session::default();
        let mut events = Vec::new();
        let out = run_turn_with_loop(
            &mut session,
            "hi".into(),
            &reg,
            &policy,
            &model,
            &ctx,
            &cfg,
            None,
            |e| events.push(e),
        )
        .unwrap();
        assert_eq!(out, "all good");
        assert!(events.iter().any(|e| matches!(e, LoopEmit::ToolInvoked { name, .. } if name == "ping")));
    }
}
