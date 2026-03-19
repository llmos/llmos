//! Tool interfaces for agents that can call external capabilities.

mod background_spawn;
mod browser;
mod registry;
mod schema;

pub use background_spawn::BackgroundSpawnTool;
pub use browser::BrowserAutomationTool;
pub use registry::{ToolDefinition, ToolRegistry};
pub use schema::SchemaTool;

use serde_json::{json, Value};

use crate::errors::AgentError;

/// A callable tool (legacy string API for simple agents).
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn call(&self, input: &str) -> Result<String, AgentError>;
}

/// Empty toolset placeholder (useful while wiring the runtime).
#[derive(Debug, Default, Clone)]
pub struct NoTools;

/// Demo tool: JSON args ignored, returns `"pong"`.
#[derive(Debug, Default, Clone)]
pub struct PingTool;

impl SchemaTool for PingTool {
    fn name(&self) -> &str {
        "ping"
    }

    fn description(&self) -> &str {
        "Health check. No parameters."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn call_json(&self, _args: &Value) -> Result<String, AgentError> {
        Ok("pong".to_string())
    }
}

impl Tool for PingTool {
    fn name(&self) -> &str {
        "ping"
    }

    fn call(&self, input: &str) -> Result<String, AgentError> {
        let _ = input;
        self.call_json(&json!({}))
    }
}
