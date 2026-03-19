//! JSON-schema-shaped tools for model function-calling.

use serde_json::Value;

use crate::errors::AgentError;

/// A tool the model can call: name, description, and JSON Schema parameters object.
pub trait SchemaTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema for the `parameters` field of an OpenAI-style function tool.
    fn parameters_schema(&self) -> Value;
    fn call_json(&self, args: &Value) -> Result<String, AgentError>;
}
