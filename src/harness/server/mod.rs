//! gRPC harness server: orchestration (sessions, tools, model) stays here; thin clients send turns and consume events.

mod driver;
mod grpc;

pub use crate::policy::ToolPolicy;
pub use driver::{HarnessDriver, RunTurnParams};
pub use grpc::HarnessGrpc;
