//! Full-screen flows shown before the main chat UI.
//!
//! - [`select`] — pick the execution environment (local / docker / a worker).
//! - [`deploy`] — live log while a Kali Docker worker is provisioned.
//! - [`forms`] — text-entry dialogs (add / rename a worker).

mod deploy;
mod forms;
mod select;

pub use deploy::docker_deploy_screen;
pub use select::worker_select_screen;

/// Result of the worker selection screen.
pub enum WorkerChoice {
    NoWorker,
    Saved(usize),
    DockerKali,
    New {
        url: String,
        secret: String,
        name: String,
    },
}
