use super::Command;
use crate::config::{UsageWindows, chat_usage, global_usage};
use crate::tui::app::App;
use anyhow::Result;
use async_trait::async_trait;

pub struct UsageCommand {}

#[async_trait]
impl Command for UsageCommand {
    fn name(&self) -> &str {
        "usage"
    }
    fn description(&self) -> &str {
        "Show token usage over 24h / 7d / 30d"
    }
    fn usage(&self) -> &str {
        "/usage"
    }

    async fn execute(&self, app: &mut App, _name: &str, _args: &[&str]) -> Result<String> {
        let chat = chat_usage(&app.chat_id).unwrap_or_default();
        let global = global_usage().unwrap_or_default();

        Ok(format!(
            "Token usage (this chat: {}):\n{}\n\nToken usage (global):\n{}",
            app.chat_id,
            format_windows(&chat),
            format_windows(&global),
        ))
    }
}

fn format_windows(w: &UsageWindows) -> String {
    format!(
        "  24h: {}   7d: {}   30d: {}",
        fmt_tokens(w.last_24h),
        fmt_tokens(w.last_7d),
        fmt_tokens(w.last_30d),
    )
}

/// Format a token count with thousands separators (e.g. 12,345).
fn fmt_tokens(n: u64) -> String {
    let digits = n.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}
