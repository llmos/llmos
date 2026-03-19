//! Queue asynchronous work on [`crate::background::BackgroundHub`] from a tool call.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::background::BackgroundHub;
use crate::errors::AgentError;

use super::schema::SchemaTool;

/// Enqueues background jobs (sleep+log, etc.) without blocking the agent turn longer than a quick spawn.
#[derive(Clone)]
pub struct BackgroundSpawnTool {
    hub: Arc<BackgroundHub>,
}

impl BackgroundSpawnTool {
    pub fn new(hub: Arc<BackgroundHub>) -> Self {
        Self { hub }
    }
}

impl SchemaTool for BackgroundSpawnTool {
    fn name(&self) -> &str {
        "background_job"
    }

    fn description(&self) -> &str {
        "Run work asynchronously on the server (does not block the chat turn). Ops: sleep_log."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "op": { "type": "string", "enum": ["sleep_log"] },
                "seconds": { "type": "integer", "minimum": 0, "maximum": 600 },
                "message": { "type": "string" }
            },
            "required": ["op"],
            "additionalProperties": false
        })
    }

    fn call_json(&self, args: &Value) -> Result<String, AgentError> {
        let op = args
            .get("op")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::msg("missing op"))?;
        match op {
            "sleep_log" => {
                let seconds = args
                    .get("seconds")
                    .and_then(|v| v.as_u64())
                    .or_else(|| args.get("seconds").and_then(|v| v.as_i64()).map(|n| n.max(0) as u64))
                    .unwrap_or(0)
                    .min(600);
                let message = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("background_job")
                    .to_string();
                self.hub.spawn(
                    format!("sleep_log:{seconds}s"),
                    async move {
                        tokio::time::sleep(Duration::from_secs(seconds)).await;
                        tracing::info!(%message, seconds, "background_job sleep_log done");
                    },
                );
                Ok(r#"{"queued":true,"op":"sleep_log"}"#.to_string())
            }
            _ => Err(AgentError::msg(format!("unknown op: {op}"))),
        }
    }
}
