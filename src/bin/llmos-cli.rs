//! llmos-cli — client for llmosd (gRPC + HTTP).

#[path = "../cli/mod.rs"]
mod cli;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "llmos-cli", version, about = "Talk to llmosd over gRPC (turns) and HTTP (health, REST).")]
struct Opt {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run one agent turn via gRPC (prints the final assistant line).
    Turn {
        #[arg(long, env = "LLMOS_GRPC_URL", default_value = "http://127.0.0.1:50051")]
        grpc_url: String,
        #[arg(long, default_value = "default")]
        session: String,
        #[arg(long, default_value = "cli")]
        agent: String,
        /// User message (`-` reads stdin). Default `ping` for a quick pong check.
        #[arg(short, long, default_value = "ping")]
        message: String,
    },
    /// GET /health on the daemon HTTP API.
    Health {
        #[arg(long, env = "LLMOS_HTTP_URL", default_value = "http://127.0.0.1:8080")]
        http_url: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _otel = cli::logging::init();
    let opt = Opt::parse();
    match opt.cmd {
        Cmd::Turn {
            grpc_url,
            session,
            agent,
            message,
        } => {
            let msg = cli::client::resolve_user_message(message)?;
            cli::client::run(grpc_url, session, agent, msg).await?;
        }
        Cmd::Health { http_url } => {
            let base = http_url.trim_end_matches('/');
            let url = format!("{base}/health");
            let body = reqwest::Client::new()
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;
            println!("{body}");
        }
    }
    Ok(())
}
