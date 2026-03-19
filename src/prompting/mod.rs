//! Prompt building utilities.

pub mod context;

pub use context::{BuiltContext, ContextBuilder};

/// A prompt template that can be filled with the current input.
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub system: String,
    pub user_prefix: String,
}

/// Build a simple prompt by concatenating template parts.
pub fn build_prompt(template: &PromptTemplate, input: &str) -> String {
    format!(
        "{}\n{}{}",
        template.system.trim_end(),
        template.user_prefix,
        input
    )
}

