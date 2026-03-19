//! Agent turn loop (model ↔ tools), used by the gRPC harness driver.

mod loop_;

pub use loop_::{run_turn_with_loop, AgentLoopConfig, LoopEmit};
