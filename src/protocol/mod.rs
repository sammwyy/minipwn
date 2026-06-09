//! Wire protocol shared between the worker server and its clients.
//!
//! Defining these request/response types once (instead of mirroring them on
//! each side) prevents silent drift: if the server changes a field, the client
//! fails to compile rather than deserializing garbage at runtime.

mod worker;

pub use worker::*;
