//! Register tools by name and execute with JSON arguments.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::errors::AgentError;
use crate::policy::ToolPolicy;

use super::schema::SchemaTool;

/// OpenAI-style function definition for the chat API layer.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Dynamic tool registry (nanobot-style).
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn SchemaTool + Send + Sync>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn SchemaTool + Send + Sync>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn register_boxed(&mut self, tool: Box<dyn SchemaTool + Send + Sync>) {
        let t = Arc::from(tool);
        self.register(t);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn SchemaTool + Send + Sync>> {
        self.tools.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Tool definitions visible to the model this turn (policy ∩ registered).
    pub fn definitions_for_policy(&self, policy: &ToolPolicy) -> Vec<ToolDefinition> {
        let mut out: Vec<ToolDefinition> = self
            .tools
            .values()
            .filter(|t| policy.allows(t.name()))
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters_schema(),
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn execute_json(&self, name: &str, args: &Value) -> Result<String, AgentError> {
        let Some(t) = self.get(name) else {
            return Err(AgentError::msg(format!("unknown tool: {name}")));
        };
        t.call_json(args)
    }
}
