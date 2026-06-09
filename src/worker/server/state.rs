//! Shared worker state: config, active shell sessions.

use std::collections::HashMap;
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::{Arc, Mutex};

use crate::config::WorkerConfig;

/// A running interactive shell session managed by the worker.
pub struct ShellSession {
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub child: Child,
}

/// Thread-safe worker application state.
#[derive(Clone)]
pub struct AppState {
    pub config: WorkerConfig,
    pub shells: Arc<Mutex<HashMap<String, Arc<Mutex<ShellSession>>>>>,
}

impl AppState {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            config,
            shells: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
