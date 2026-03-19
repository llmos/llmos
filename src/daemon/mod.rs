//! `llmosd` process: shared harness driver, gRPC (streaming), and HTTP (REST).

mod http;

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

pub use http::{AppState, TurnRequest, TurnResponse};

use crate::agent::AgentLoopConfig;
use crate::background::BackgroundHub;
use crate::model::{ChatModel, KeywordToolModel, PingPongChatModel};
use crate::policy::ToolPolicy;
use crate::prompting::ContextBuilder;
use crate::scheduler::{HeartbeatTask, Scheduler};
use crate::server::{HarnessDriver, HarnessGrpc};
use crate::tools::{
    BackgroundSpawnTool, BrowserAutomationTool, PingTool, ToolRegistry,
};
use tokio::net::TcpListener;
use tonic::transport::Server;

/// Addresses for the two front-ends on `llmosd`.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub grpc_listen: SocketAddr,
    pub http_listen: SocketAddr,
}

fn env_truthy(name: &str) -> bool {
    env::var(name)
        .map(|v| {
            let v = v.trim();
            matches!(v, "1" | "true" | "yes" | "on") || v.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

/// Build driver + background services, then run gRPC and HTTP until one exits or errors.
///
/// Honors `LLMOS_*` env vars (see previous `llmos server` behavior). `LLMOS_FULL_HARNESS=1`
/// enables tools, browser bridge, background jobs, and keyword-stub model.
pub async fn run(config: DaemonConfig) -> Result<(), String> {
    let max_bg = env::var("LLMOS_BG_MAX_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);
    let hub = Arc::new(BackgroundHub::new(max_bg));

    let mut sched_builder = Scheduler::builder(hub.clone());
    if let Ok(expr) = env::var("LLMOS_CRON_HEARTBEAT") {
        let expr = expr.trim();
        if !expr.is_empty() {
            sched_builder = sched_builder.try_add("heartbeat", expr, Arc::new(HeartbeatTask))?;
        }
    }
    sched_builder.build().start();

    let workspace = env::var("LLMOS_WORKSPACE").ok().map(std::path::PathBuf::from);
    let full = env_truthy("LLMOS_FULL_HARNESS");

    let (registry, chat, policy): (ToolRegistry, Arc<dyn ChatModel + Send + Sync>, ToolPolicy) =
        if full {
            tracing::info!("full harness: tools + KeywordToolModel (LLMOS_FULL_HARNESS)");
            let mut registry = ToolRegistry::new();
            registry.register(Arc::new(PingTool));
            registry.register(Arc::new(BackgroundSpawnTool::new(hub.clone())));
            registry.register(Arc::new(BrowserAutomationTool::new(workspace.clone())));
            (
                registry,
                Arc::new(KeywordToolModel),
                ToolPolicy::default(),
            )
        } else {
            tracing::info!(
                "simple harness: PingPongChatModel, no tools (set LLMOS_FULL_HARNESS=1 for more)"
            );
            (
                ToolRegistry::new(),
                Arc::new(PingPongChatModel),
                ToolPolicy::new(vec![]),
            )
        };

    let ctx = match workspace {
        Some(p) => ContextBuilder::with_workspace_dir(p),
        None => ContextBuilder::default(),
    };

    let driver = Arc::new(HarnessDriver::new(
        registry,
        chat,
        policy,
        ctx,
        AgentLoopConfig::default(),
    ));

    let grpc_svc = HarnessGrpc::shared(driver.clone()).into_service();
    let grpc_addr = config.grpc_listen;
    let grpc_server = Server::builder()
        .add_service(grpc_svc)
        .serve(grpc_addr);

    let http_listener = TcpListener::bind(config.http_listen).await?;
    let http_addr = http_listener.local_addr()?;
    let app = http::router(AppState {
        driver: driver.clone(),
    });
    let http_server = axum::serve(http_listener, app);

    tracing::info!(%grpc_addr, %http_addr, "llmosd listening (gRPC + HTTP)");
    if env::var("LLMOS_CRON_HEARTBEAT")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        tracing::info!("cron heartbeat enabled (LLMOS_CRON_HEARTBEAT)");
    }
    if full {
        tracing::info!(max_bg, "background hub max concurrency");
        tracing::info!("browser tool: set LLMOS_BROWSER_SCRIPT (+ optional LLMOS_BROWSER_NODE)");
    }

    tokio::try_join!(
        async move { grpc_server.await.map_err(|e| e.to_string()) },
        async move { http_server.await.map_err(|e| e.to_string()) },
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
