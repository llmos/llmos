//! Assemble system prompt + history for the harness (nanobot-style context builder).

use std::path::PathBuf;

use crate::errors::AgentError;
use crate::harness::session::{ChatMessage, Session};

const BOOTSTRAP_FILES: &[&str] = &["AGENTS.md", "SOUL.md", "USER.md", "TOOLS.md"];

/// Result of [`ContextBuilder::build`].
#[derive(Debug, Clone)]
pub struct BuiltContext {
    pub system: String,
    pub history: Vec<ChatMessage>,
}

/// Builds system text from optional workspace bootstrap files and trims session history.
#[derive(Debug, Clone)]
pub struct ContextBuilder {
    workspace: Option<PathBuf>,
    max_history_messages: usize,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self {
            workspace: None,
            max_history_messages: 64,
        }
    }
}

impl ContextBuilder {
    pub fn new(workspace: Option<PathBuf>, max_history_messages: usize) -> Self {
        Self {
            workspace,
            max_history_messages,
        }
    }

    pub fn with_workspace_dir(root: impl Into<PathBuf>) -> Self {
        Self {
            workspace: Some(root.into()),
            max_history_messages: 64,
        }
    }

    fn load_bootstrap_files(&self) -> String {
        let Some(ref root) = self.workspace else {
            return String::new();
        };
        let mut parts = Vec::new();
        for name in BOOTSTRAP_FILES {
            let path: PathBuf = root.join(name);
            if path.is_file() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    parts.push(format!("## {name}\n\n{text}"));
                }
            }
        }
        parts.join("\n\n")
    }

    fn base_identity(&self) -> String {
        let ws_note = self
            .workspace
            .as_ref()
            .map(|p| format!("Workspace: {}", p.display()))
            .unwrap_or_else(|| "No workspace configured.".to_string());
        format!(
            "# llmos harness\n\nYou are an assistant running on the llmos server.\n\n{ws_note}\n"
        )
    }

    pub fn build_system_prompt(&self) -> String {
        let mut sections = vec![self.base_identity()];
        let boot = self.load_bootstrap_files();
        if !boot.is_empty() {
            sections.push(boot);
        }
        sections.join("\n\n---\n\n")
    }

    /// Build system + model history from the session (excludes the pending user line if not yet appended).
    pub fn build_for_session(&self, session: &Session) -> Result<BuiltContext, AgentError> {
        let system = self.build_system_prompt();
        let history = session.get_history_for_model(self.max_history_messages);
        Ok(BuiltContext { system, history })
    }
}

impl BuiltContext {
    pub fn inject_runtime_note(&mut self, note: &str) {
        if note.trim().is_empty() {
            return;
        }
        self.system.push_str("\n\n---\n\n## Runtime (metadata)\n\n");
        self.system.push_str(note.trim());
    }
}
