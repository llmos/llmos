//! `llmos client` — call the harness gRPC server (ping/pong smoke test).

use llmos::proto::harness_client::HarnessClient;
use llmos::proto::harness_event::Payload;
use llmos::proto::RunTurnRequest;
use tonic::Request;

/// Send one user message; print the final assistant line from `TurnComplete` (and log stream at debug).
pub async fn run(
    url: String,
    session_id: String,
    agent_id: String,
    message: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = HarnessClient::connect(url.clone())
        .await
        .map_err(|e| format!("connect {url}: {e}"))?;
    tracing::info!(%url, %session_id, %agent_id, len = message.len(), "client RunTurn");

    let req = RunTurnRequest {
        session_id: session_id.clone(),
        agent_id: agent_id.clone(),
        user_message: message,
        allowed_tool_names: vec![],
    };

    let mut stream = client
        .run_turn(Request::new(req))
        .await
        .map_err(|e| format!("RunTurn: {e}"))?
        .into_inner();

    let mut final_output: Option<String> = None;

    while let Some(ev) = stream.message().await? {
        tracing::debug!(seq = ev.seq, correlation_id = %ev.correlation_id, "event");
        match ev.payload {
            Some(Payload::Complete(c)) => {
                final_output = Some(c.output);
            }
            Some(Payload::Error(e)) => {
                return Err(format!("server error {}: {}", e.code, e.message).into());
            }
            _ => {}
        }
    }

    match final_output {
        Some(text) => {
            println!("{text}");
            Ok(())
        }
        None => Err("stream ended without TurnComplete".into()),
    }
}

/// Resolve body: literal string, or stdin when `message == "-"`.
pub fn resolve_user_message(message: String) -> Result<String, Box<dyn std::error::Error>> {
    if message == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
        return Ok(buf.trim().to_string());
    }
    Ok(message)
}
