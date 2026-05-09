//! Path sanitization: prevent directory traversal and absolute paths.
//! All file tool paths are restricted to the workspace directory.

use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};

/// Sanitize a user/AI-provided path so it cannot escape the workspace.
/// - Rejects absolute paths
/// - Rejects paths containing `..` components
/// - Rejects `.minipwn` access
/// Returns a cleaned relative path on success.
pub fn sanitize_path(input: &str) -> Result<PathBuf> {
    let path = Path::new(input);

    // Reject absolute paths
    if path.is_absolute() {
        bail!("Absolute paths are not allowed: {}", input);
    }

    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part_str = part.to_string_lossy();
                // Block access to the hidden workspace dir
                if part_str == ".minipwn" {
                    bail!("Access to .minipwn directory is not permitted");
                }
                cleaned.push(part);
            }
            Component::CurDir => {} // skip `.`
            Component::ParentDir => {
                bail!("Directory traversal (../) is not allowed in: {}", input);
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("Absolute paths are not allowed: {}", input);
            }
        }
    }

    if cleaned.as_os_str().is_empty() {
        return Ok(PathBuf::from("."));
    }

    Ok(cleaned)
}

/// Sanitize and resolve a path relative to the workspace root.
pub fn resolve_workspace_path(workspace: &Path, input: &str) -> Result<PathBuf> {
    let relative = sanitize_path(input)?;
    let full = workspace.join(relative);

    // Double-check the resolved path is still inside the workspace
    let canonical_workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let canonical_full = full
        .canonicalize()
        .unwrap_or_else(|_| workspace.join(sanitize_path(input).unwrap_or_default()));

    if !canonical_full.starts_with(&canonical_workspace) {
        bail!("Path escapes workspace boundaries: {}", input);
    }

    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_absolute() {
        assert!(sanitize_path("/etc/passwd").is_err());
    }

    #[test]
    fn test_reject_traversal() {
        assert!(sanitize_path("../../etc/passwd").is_err());
        assert!(sanitize_path("foo/../../../etc").is_err());
    }

    #[test]
    fn test_reject_minipwn() {
        assert!(sanitize_path(".minipwn/secrets").is_err());
    }

    #[test]
    fn test_allow_normal() {
        assert!(sanitize_path("src/main.rs").is_ok());
        assert!(sanitize_path("./notes.txt").is_ok());
        assert!(sanitize_path("a/b/c").is_ok());
    }
}
