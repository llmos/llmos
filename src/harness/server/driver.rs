//! Server-side harness: sessions, tool registry, chat loop, streamed protobuf events.

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tonic::Status;

use crate::harness::agent::{run_turn_with_loop, AgentLoopConfig, LoopEmit};
use crate::errors::AgentError;
use crate::model::ChatModel;
use crate::policy::ToolPolicy;
use crate::prompting::ContextBuilder;
use crate::harness::proto::harness_event::Payload;
use crate::harness::proto::{HarnessEvent, Log, MemoryContext, PartialOutput, Phase, ToolCall, TurnComplete};
use crate::harness::session::{ChatRole, SessionManager};
use crate::tools::ToolRegistry;

/// Parameters for one streamed turn (mapped from gRPC request).
#[derive(Debug, Clone)]
pub struct RunTurnParams {
    pub session_id: String,
    pub agent_id: String,
    pub user_message: String,
    /// Empty slice ⇒ use server default policy.
    pub client_tool_allowlist: Vec<String>,
}

/// Owns sessions, tools, and chat model; emits [`HarnessEvent`]s for the RPC layer.
pub struct HarnessDriver {
    sessions: Arc<Mutex<SessionManager>>,
    registry: Arc<ToolRegistry>,
    chat_model: Arc<dyn ChatModel + Send + Sync>,
    default_tool_policy: ToolPolicy,
    context_builder: ContextBuilder,
    loop_config: AgentLoopConfig,
}

impl HarnessDriver {
    pub fn new(
        registry: ToolRegistry,
        chat_model: Arc<dyn ChatModel + Send + Sync>,
        default_tool_policy: ToolPolicy,
        context_builder: ContextBuilder,
        loop_config: AgentLoopConfig,
    ) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(SessionManager::default())),
            registry: Arc::new(registry),
            chat_model,
            default_tool_policy,
            context_builder,
            loop_config,
        }
    }

    /// Run one turn and send protobuf events to `tx` until finished or error.
    pub async fn run_turn_stream(
        &self,
        params: RunTurnParams,
        tx: mpsc::Sender<Result<HarnessEvent, Status>>,
    ) -> Result<(), Status> {
        let t0 = std::time::Instant::now();
        let correlation_id = format!("{}-{}", params.session_id, params.agent_id);
        let mut seq: u64 = 0;

        let policy = ToolPolicy::for_request(
            &params.client_tool_allowlist,
            &self.default_tool_policy,
        );

        send_event(
            &tx,
            &correlation_id,
            &mut seq,
            Payload::Phase(Phase {
                name: "authorize_tools".into(),
                detail: format!("allowed_tools={:?}", policy.allowed_names()),
            }),
        )
        .await?;

        send_event(
            &tx,
            &correlation_id,
            &mut seq,
            Payload::Phase(Phase {
                name: "memory_retrieve".into(),
                detail: "load session transcript".into(),
            }),
        )
        .await?;

        let (prior_turns, summary) = {
            let mut mgr = self.sessions.lock().map_err(|e| {
                Status::internal(format!("session store lock: {e}"))
            })?;
            let session = mgr.get_or_create(params.session_id.clone());
            (
                session.messages.len() as u32,
                summarize_session(session),
            )
        };

        send_event(
            &tx,
            &correlation_id,
            &mut seq,
            Payload::Memory(MemoryContext {
                summary,
                prior_turns,
            }),
        )
        .await?;

        send_event(
            &tx,
            &correlation_id,
            &mut seq,
            Payload::Phase(Phase {
                name: "agent_loop".into(),
                detail: "model iterations and tool execution".into(),
            }),
        )
        .await?;

        let runtime_note = format!(
            "session_id={}\nagent_id={}",
            params.session_id, params.agent_id
        );

        let (loop_emits, turn_result) = {
            let mut mgr = self.sessions.lock().map_err(|e| {
                Status::internal(format!("session store lock: {e}"))
            })?;
            let session = mgr.get_or_create(params.session_id.clone());
            let mut loop_emits: Vec<LoopEmit> = Vec::new();
            let turn_result = run_turn_with_loop(
                session,
                params.user_message.clone(),
                self.registry.as_ref(),
                &policy,
                self.chat_model.as_ref(),
                &self.context_builder,
                &self.loop_config,
                Some(runtime_note.as_str()),
                |e| loop_emits.push(e),
            );
            (loop_emits, turn_result)
        };

        let final_text = turn_result.map_err(agent_error_to_status)?;

        for ev in loop_emits {
            map_loop_emit(ev, &tx, &correlation_id, &mut seq).await?;
        }

        send_event(
            &tx,
            &correlation_id,
            &mut seq,
            Payload::Complete(TurnComplete {
                agent_id: params.agent_id.clone(),
                output: final_text.clone(),
            }),
        )
        .await?;

        tracing::info!(
            %correlation_id,
            out_len = final_text.len(),
            "turn completed"
        );

        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let m = crate::telemetry::harness_metrics();
        m.run_turn_total.add(1, &[]);
        m.run_turn_duration_ms.record(ms, &[]);

        Ok(())
    }
}

fn summarize_session(session: &crate::harness::session::Session) -> String {
    let m = &session.messages;
    if m.is_empty() {
        return String::new();
    }
    let tail: Vec<_> = m.iter().rev().take(3).rev().collect();
    tail
        .iter()
        .map(|msg| {
            let role = match msg.role {
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
                ChatRole::Tool => "tool",
            };
            format!("{role}: {}", truncate(&msg.content, 80))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    format!("{}…", &s[..max.saturating_sub(1)])
}

async fn map_loop_emit(
    ev: LoopEmit,
    tx: &mpsc::Sender<Result<HarnessEvent, Status>>,
    correlation_id: &str,
    seq: &mut u64,
) -> Result<(), Status> {
    match ev {
        LoopEmit::Phase { name, detail } => {
            send_event(
                tx,
                correlation_id,
                seq,
                Payload::Phase(Phase { name, detail }),
            )
            .await
        }
        LoopEmit::Iteration { n } => {
            send_event(
                tx,
                correlation_id,
                seq,
                Payload::Log(Log {
                    level: "info".into(),
                    message: format!("model iteration {n}"),
                }),
            )
            .await
        }
        LoopEmit::ToolInvoked { name, request, response } => {
            send_event(
                tx,
                correlation_id,
                seq,
                Payload::Tool(ToolCall {
                    tool_name: name,
                    request,
                    response,
                }),
            )
            .await
        }
        LoopEmit::AssistantText { text } => {
            send_event(
                tx,
                correlation_id,
                seq,
                Payload::Partial(PartialOutput { chunk: text }),
            )
            .await
        }
        LoopEmit::Log { level, message } => {
            send_event(
                tx,
                correlation_id,
                seq,
                Payload::Log(Log { level, message }),
            )
            .await
        }
    }
}

async fn send_event(
    tx: &mpsc::Sender<Result<HarnessEvent, Status>>,
    correlation_id: &str,
    seq: &mut u64,
    payload: Payload,
) -> Result<(), Status> {
    *seq += 1;
    let ev = HarnessEvent {
        correlation_id: correlation_id.to_string(),
        seq: *seq,
        payload: Some(payload),
    };
    tx.send(Ok(ev))
        .await
        .map_err(|_| Status::internal("client disconnected"))
}

fn agent_error_to_status(e: AgentError) -> Status {
    let m = e.to_string();
    if m.contains("not authorized") {
        return Status::permission_denied(m);
    }
    Status::internal(m)
}
