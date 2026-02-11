//! A library for sandboxed code execution.
//!
//! Silicube provides an async Rust API for running untrusted code in isolated
//! sandboxes using IOI Isolate. It supports flexible language configuration,
//! batch and interactive program execution, and resource limit enforcement.
//!
//! # Features
//!
//! - **Sandboxed execution** — Pool-based lifecycle for running untrusted code safely using Isolate.
//! - **Multi-language** — Supports both compiled and interpreted languages.
//! - **TOML configuration** — Flexible per-language compiler/runtime settings.
//! - **Interactive execution** — FIFO-based sessions for interactive programs.
//! - **Resource limits** — Enforce CPU time, memory, wall time, processes, and output constraints.
//! - **cgroup v2 support** — Memory limiting in container environments.

pub use config::{Config, EXAMPLE_CONFIG};
pub use isolate::{BoxPool, prepare_cgroup};
pub use runner::Runner;
pub use types::ResourceLimits;

pub mod config;
pub mod isolate;
pub mod runner;
pub mod types;
