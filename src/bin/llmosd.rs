//! llmosd — daemon: gRPC harness (streaming) + HTTP (REST).

use std::net::SocketAddr;

use clap::Parser;
use llmos::daemon::{self, DaemonConfig};

#[derive(Parser)]
#[command(name = "llmosd", version, about = "llmos daemon: gRPC harness + HTTP API")]
struct Opt {
    /// gRPC listen address (Harness service).
    #[arg(long, env = "LLMOS_GRPC_LISTEN", default_value = "127.0.0.1:50051")]
    grpc_listen: String,
    /// HTTP listen address (REST: `/health`, `/v1/turn`, …).
    #[arg(long, env = "LLMOS_HTTP_LISTEN", default_value = "127.0.0.1:8080")]
    http_listen: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _otel = match llmos::telemetry::init_subscriber() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("llmosd: OpenTelemetry init failed ({e}); logging to stdout only (see OTEL_* env).");
            let _ = llmos::telemetry::init_stdout_only();
            None
        }
    };

    let opt = Opt::parse();
    let grpc_listen: SocketAddr = opt.grpc_listen.parse()?;
    let http_listen: SocketAddr = opt.http_listen.parse()?;

    daemon::run(DaemonConfig {
        grpc_listen,
        http_listen,
    })
    .await
    .map_err(|e| e.into())
}
