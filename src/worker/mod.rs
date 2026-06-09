//! Worker subsystem.
//!
//! A [`Worker`] is the backend that executes agent tool calls. Three concrete
//! implementations are provided:
//!
//! - [`LocalWorker`] — the "no worker" sandbox, running tools on the host.
//! - [`RemoteWorker`] — talks to a standalone worker server over HTTP.
//! - [`DockerWorker`] — deploys and drives a Kali container as a worker.
//!
//! Supporting pieces live alongside them: [`client`] (HTTP client to a worker
//! server), [`discovery`] (LAN auto-discovery), and [`server`] (the daemon side
//! started by `minipwn worker`).

pub mod client;
pub mod discovery;
pub mod docker;
pub mod local;
pub mod remote;
pub mod server;
mod traits;

pub use docker::{DeployedContainer, DockerWorker, deploy_kali_worker};
pub use local::LocalWorker;
pub use remote::RemoteWorker;
pub use traits::{Worker, WorkerKind};
