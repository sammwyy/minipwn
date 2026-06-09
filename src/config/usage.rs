//! Token-usage logging and time-windowed aggregation.
//!
//! Each token-spending event is appended (with a timestamp) to two logs:
//! a global one in the user config dir and a per-chat one in the workspace.
//! `/usage` reads them back and sums tokens over rolling 24h / 7d / 30d windows.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

const USAGE_DIR: &str = "usage";
const GLOBAL_USAGE_FILE: &str = "usage.log";

/// Token totals over rolling time windows.
#[derive(Debug, Clone, Copy, Default)]
pub struct UsageWindows {
    pub last_24h: u64,
    pub last_7d: u64,
    pub last_30d: u64,
}

/// Record `tokens` against the current time, in both the global and per-chat logs.
pub fn record_usage(chat_id: &str, tokens: u64) -> Result<()> {
    let now = Utc::now();
    append_entry(&global_usage_path()?, now, tokens)?;
    append_entry(&chat_usage_path(chat_id)?, now, tokens)?;
    Ok(())
}

/// Aggregate global usage across all chats.
pub fn global_usage() -> Result<UsageWindows> {
    Ok(read_windows(&global_usage_path()?))
}

/// Aggregate usage for a single chat.
pub fn chat_usage(chat_id: &str) -> Result<UsageWindows> {
    Ok(read_windows(&chat_usage_path(chat_id)?))
}

fn global_usage_path() -> Result<PathBuf> {
    Ok(super::config_dir()?.join(GLOBAL_USAGE_FILE))
}

fn chat_usage_path(chat_id: &str) -> Result<PathBuf> {
    let dir = super::workspace_dir()?.join(USAGE_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.log", chat_id)))
}

fn append_entry(path: &PathBuf, ts: DateTime<Utc>, tokens: u64) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{} {}", ts.to_rfc3339(), tokens)?;
    Ok(())
}

/// Parse a usage log and total tokens within each rolling window.
fn read_windows(path: &PathBuf) -> UsageWindows {
    let Ok(content) = std::fs::read_to_string(path) else {
        return UsageWindows::default();
    };

    let now = Utc::now();
    let cutoff_24h = now - Duration::hours(24);
    let cutoff_7d = now - Duration::days(7);
    let cutoff_30d = now - Duration::days(30);

    let mut windows = UsageWindows::default();
    for line in content.lines() {
        let Some((ts_str, tokens_str)) = line.split_once(' ') else {
            continue;
        };
        let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) else {
            continue;
        };
        let Ok(tokens) = tokens_str.trim().parse::<u64>() else {
            continue;
        };
        let ts = ts.with_timezone(&Utc);

        if ts >= cutoff_30d {
            windows.last_30d += tokens;
            if ts >= cutoff_7d {
                windows.last_7d += tokens;
            }
            if ts >= cutoff_24h {
                windows.last_24h += tokens;
            }
        }
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_windows_buckets_by_age() {
        let dir = std::env::temp_dir().join(format!("minipwn-usage-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("u.log");

        let now = Utc::now();
        let mut content = String::new();
        content.push_str(&format!("{} {}\n", (now - Duration::hours(1)).to_rfc3339(), 100));
        content.push_str(&format!("{} {}\n", (now - Duration::days(3)).to_rfc3339(), 200));
        content.push_str(&format!("{} {}\n", (now - Duration::days(20)).to_rfc3339(), 400));
        content.push_str(&format!("{} {}\n", (now - Duration::days(40)).to_rfc3339(), 800));
        content.push_str("garbage line that should be skipped\n");
        std::fs::write(&path, content).unwrap();

        let w = read_windows(&path);
        assert_eq!(w.last_24h, 100);
        assert_eq!(w.last_7d, 300);
        assert_eq!(w.last_30d, 700);

        std::fs::remove_dir_all(&dir).ok();
    }
}
