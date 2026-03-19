//! REST surface on `llmosd` (JSON). gRPC remains the primary streaming protocol.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::harness::proto::harness_event::Payload;
use crate::harness::proto::HarnessEvent;
use crate::harness::server::{HarnessDriver, RunTurnParams};

#[derive(Clone)]
pub struct AppState {
    pub driver: Arc<HarnessDriver>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/turn", post(turn))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "llmosd" }))
}

#[derive(Debug, Deserialize)]
pub struct TurnRequest {
    pub session_id: String,
    pub agent_id: String,
    pub message: String,
    #[serde(default)]
    pub allowed_tool_names: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TurnResponse {
    pub output: String,
}

async fn turn(
    State(state): State<AppState>,
    Json(body): Json<TurnRequest>,
) -> Result<Json<TurnResponse>, (StatusCode, String)> {
    if body.session_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "session_id is required".into()));
    }
    if body.agent_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "agent_id is required".into()));
    }

    let params = RunTurnParams {
        session_id: body.session_id,
        agent_id: body.agent_id,
        user_message: body.message,
        client_tool_allowlist: body.allowed_tool_names,
    };

    let output = run_turn_to_output(state.driver.clone(), params)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(TurnResponse { output }))
}

async fn run_turn_to_output(
    driver: Arc<HarnessDriver>,
    params: RunTurnParams,
) -> Result<String, String> {
    let (tx, mut rx) = mpsc::channel::<Result<HarnessEvent, tonic::Status>>(32);
    let worker = tokio::spawn(async move {
        driver.run_turn_stream(params, tx).await
    });

    let mut final_output: Option<String> = None;
    while let Some(item) = rx.recv().await {
        match item {
            Ok(ev) => match ev.payload {
                Some(Payload::Complete(c)) => final_output = Some(c.output),
                Some(Payload::Error(e)) => {
                    return Err(format!("{}: {}", e.code, e.message));
                }
                _ => {}
            },
            Err(s) => return Err(s.to_string()),
        }
    }

    worker
        .await
        .map_err(|e| format!("turn task join: {e}"))?
        .map_err(|s| s.to_string())?;

    final_output.ok_or_else(|| "stream ended without TurnComplete".to_string())
}
