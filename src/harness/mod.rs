//! Harness stack: chat session state, model/tool loop, protobuf wire types, and gRPC service.
//!
//! These pieces exist to run the remote harness; local `Agent` / CLI flows use [`crate::core`]
//! and [`crate::runtime`] instead. For a stable import path, the crate root re-exports
//! [`agent`], [`session`], [`proto`], and [`server`] as `llmos::agent`, `llmos::session`, etc.

pub mod agent;
pub mod proto;
pub mod server;
pub mod session;
