//! Shared contract for the Vise background worker.
//!
//! The desktop application never runs the transcode pipeline in-process. It
//! launches a separate worker binary that exposes a loopback-only,
//! token-authenticated JSON API, and talks to it through [`WorkerClient`].
//!
//! Module map:
//!
//! - [`contract`] — request/response payloads shared by both sides.
//! - [`http`] — the minimal HTTP/1.1 message layer both sides agree on.
//! - [`token`] — token generation and authentication checks.
//! - [`timespan`] — strict duration parsing used by the scan contract.
//! - [`validation`] — pre-flight job validation.
//! - [`client`] — loopback HTTP client with typed command wrappers.
//! - [`supervisor`] — worker process lifecycle, handshake, and restart.
//! - [`portable`] — portable-mode switch that can disable process isolation.

pub mod api;
pub mod base64;
pub mod client;
pub mod contract;
pub mod http;
pub mod portable;
pub mod route;
pub mod supervisor;
pub mod timespan;
pub mod token;
pub mod validation;

pub use client::{ClientError, WorkerClient};
pub use contract::*;
pub use portable::{PortableError, PortableSettings};
pub use route::{Route, TOKEN_HEADER};
pub use supervisor::{SupervisorError, WorkerConfig, WorkerConnection, WorkerSupervisor};
pub use timespan::{TimeSpan, TimeSpanError};
pub use token::{generate_token, TokenGuard};

/// Protocol revision reported by the worker and required by the client.
pub const PROTOCOL_VERSION: u32 = 1;

/// Name of the request header that carries the worker token.
pub const WORKER_NAME: &str = "vise-worker";

/// Loopback address the worker binds. Never a routable interface.
pub const LOOPBACK_HOST: &str = "127.0.0.1";
