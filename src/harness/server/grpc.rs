//! gRPC [`harness_server::Harness`] implementation.

use std::pin::Pin;

use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::Instrument;

use super::driver::{HarnessDriver, RunTurnParams};
use crate::harness::proto::harness_server::{Harness, HarnessServer};
use crate::harness::proto::{HarnessEvent, RunTurnRequest};

/// Type alias for the server streaming response.
pub type RunTurnStream =
    Pin<Box<dyn Stream<Item = Result<HarnessEvent, Status>> + Send + 'static>>;

#[derive(Clone)]
pub struct HarnessGrpc {
    inner: std::sync::Arc<HarnessDriver>,
}

impl HarnessGrpc {
    pub fn new(driver: HarnessDriver) -> Self {
        Self {
            inner: std::sync::Arc::new(driver),
        }
    }

    /// Share an existing driver (e.g. `llmosd` serves gRPC and HTTP from one [`HarnessDriver`]).
    pub fn shared(driver: std::sync::Arc<HarnessDriver>) -> Self {
        Self { inner: driver }
    }

    pub fn into_service(self) -> HarnessServer<Self> {
        HarnessServer::new(self)
    }
}

#[tonic::async_trait]
impl Harness for HarnessGrpc {
    type RunTurnStream = RunTurnStream;

    async fn run_turn(
        &self,
        request: Request<RunTurnRequest>,
    ) -> Result<Response<Self::RunTurnStream>, Status> {
        let r = request.into_inner();
        if r.session_id.is_empty() {
            return Err(Status::invalid_argument("session_id is required"));
        }
        if r.agent_id.is_empty() {
            return Err(Status::invalid_argument("agent_id is required"));
        }

        let session_id = r.session_id.clone();
        let agent_id = r.agent_id.clone();
        let msg_len = r.user_message.len();
        tracing::info!(%session_id, %agent_id, msg_len, "RunTurn");

        let params = RunTurnParams {
            session_id: r.session_id,
            agent_id: r.agent_id,
            user_message: r.user_message,
            client_tool_allowlist: r.allowed_tool_names,
        };

        let driver = std::sync::Arc::clone(&self.inner);
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<HarnessEvent, Status>>(32);

        let span = tracing::info_span!("run_turn_stream", %session_id, %agent_id);
        tokio::spawn(
            async move {
                if let Err(e) = driver.run_turn_stream(params, tx.clone()).await {
                    tracing::warn!(error = %e, "run_turn_stream failed");
                    let _ = tx.send(Err(e)).await;
                }
            }
            .instrument(span),
        );

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}
