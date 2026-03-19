//! Optional browser automation via an external Node/Playwright driver script (stdin JSON → stdout).

use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Value};

use crate::errors::AgentError;

use super::schema::SchemaTool;

/// Drives a user-supplied script; intended for Playwright/puppeteer-style hooks.
///
/// Configure with `LLMOS_BROWSER_NODE` (default `node`) and `LLMOS_BROWSER_SCRIPT` (path to `.js`/`.mjs`).
/// The script receives one JSON line on stdin: `{"action":"goto","url":"https://..."}` and should print a single-line JSON result to stdout.
#[derive(Clone)]
pub struct BrowserAutomationTool {
    workspace: Option<PathBuf>,
}

impl BrowserAutomationTool {
    pub fn new(workspace: Option<PathBuf>) -> Self {
        Self { workspace }
    }

    fn resolve_script_path(&self, raw: &str) -> Result<PathBuf, AgentError> {
        let p = PathBuf::from(raw);
        let abs = if p.is_absolute() {
            p
        } else if let Some(ws) = &self.workspace {
            ws.join(&p)
        } else {
            return Err(AgentError::msg(
                "relative LLMOS_BROWSER_SCRIPT requires LLMOS_WORKSPACE",
            ));
        };

        let can = abs.canonicalize().map_err(|e| {
            AgentError::msg(format!("browser script path error: {e}"))
        })?;

        if let Some(ws) = &self.workspace {
            let base = ws.canonicalize().map_err(|e| {
                AgentError::msg(format!("workspace path error: {e}"))
            })?;
            if !can.starts_with(&base) {
                return Err(AgentError::msg(
                    "browser script must live under LLMOS_WORKSPACE",
                ));
            }
        }

        Ok(can)
    }
}

fn is_allowed_url(url: &str) -> bool {
    let u = url.trim();
    u.starts_with("https://") || u.starts_with("http://localhost") || u.starts_with("http://127.0.0.1")
}

impl SchemaTool for BrowserAutomationTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Run a configured browser driver script (Playwright/Node). Requires LLMOS_BROWSER_SCRIPT; only http(s) URLs allowed."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["goto"] },
                "url": { "type": "string" }
            },
            "required": ["action", "url"],
            "additionalProperties": false
        })
    }

    fn call_json(&self, args: &Value) -> Result<String, AgentError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::msg("missing action"))?;
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::msg("missing url"))?;

        if !is_allowed_url(url) {
            return Err(AgentError::msg(
                "url must be https:// or http://localhost / 127.0.0.1",
            ));
        }

        let script_raw = std::env::var("LLMOS_BROWSER_SCRIPT").map_err(|_| {
            AgentError::msg("browser automation disabled: set LLMOS_BROWSER_SCRIPT")
        })?;
        let script = self.resolve_script_path(&script_raw)?;

        let node = std::env::var("LLMOS_BROWSER_NODE").unwrap_or_else(|_| "node".into());

        let payload = json!({ "action": action, "url": url });
        let stdin = serde_json::to_string(&payload)
            .map_err(|e| AgentError::msg(format!("browser payload: {e}")))?;

        let out = Command::new(&node)
            .arg(script.as_os_str())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut sin) = child.stdin.take() {
                    let _ = sin.write_all(stdin.as_bytes());
                }
                child.wait_with_output()
            })
            .map_err(|e| AgentError::msg(format!("browser process: {e}")))?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(AgentError::msg(format!(
                "browser script failed (status {:?}): {err}",
                out.status.code()
            )));
        }

        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.is_empty() {
            Ok(r#"{"ok":true}"#.to_string())
        } else {
            Ok(text)
        }
    }
}
