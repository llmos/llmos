//! `llmos`: agent primitives, harness (gRPC + HTTP on `llmosd`), protocol types, and runtime.
//!
//! Binaries: **`llmosd`** (daemon: heavy lifting), **`llmos-cli`** (talks to the daemon).

pub mod background;
pub mod daemon;
pub mod bus;
pub mod core;
pub mod errors;
pub mod harness;
pub mod model;
pub mod policy;
pub mod prompting;
pub mod protocol;
pub mod runtime;
pub mod scheduler;
pub mod telemetry;
pub mod tools;

// Stable paths: harness submodules are also available at the crate root.
pub use harness::agent as agent;
pub use harness::proto as proto;
pub use harness::server as server;
pub use harness::session as session;

pub use background::BackgroundHub;
pub use core::{Agent, EchoAgent};
pub use errors::AgentError;
pub use protocol::{AgentReply, AgentTurn, InMemoryMemory, Memory};
pub use runtime::AgentRuntime;
pub use scheduler::{HeartbeatTask, ScheduledTask, Scheduler, SchedulerBuilder};
pub use telemetry::{harness_metrics, init_stdout_only, init_subscriber, OtelHandle};
