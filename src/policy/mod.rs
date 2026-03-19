//! Authorization policy for which tools may run on a turn.

/// Which tool names may execute this turn. Empty internal list denies all tools.
#[derive(Debug, Clone)]
pub struct ToolPolicy {
    allowed: Vec<String>,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            allowed: vec![
                "ping".to_string(),
                "background_job".to_string(),
                "browser".to_string(),
            ],
        }
    }
}

impl ToolPolicy {
    pub fn new(allowed: Vec<String>) -> Self {
        Self { allowed }
    }

    pub fn allowed_names(&self) -> &[String] {
        &self.allowed
    }

    pub fn allows(&self, name: &str) -> bool {
        self.allowed.iter().any(|n| n == name)
    }

    /// If `client` is empty, use `server_default`. Otherwise use the client list (server should still intersect with registered tools when building definitions).
    pub fn for_request(client: &[String], server_default: &ToolPolicy) -> Self {
        if client.is_empty() {
            return server_default.clone();
        }
        Self::new(client.to_vec())
    }
}
