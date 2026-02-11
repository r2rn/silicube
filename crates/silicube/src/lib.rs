//! A library for sandboxed code execution.
//!
//! Silicube provides an async Rust API for running untrusted code in isolated
//! sandboxes using IOI Isolate. It supports flexible language configuration,
//! batch and interactive program execution, and resource limit enforcement.
//!
//! # Features
//!
//! - Sandboxed execution for running untrusted code safely using Isolate.
//! - Support for multiple languages, both compiled and interpreted.
//! - Flexible configuration with a TOML-based configuration file.
//! - Interactive code execution with FIFO-based sessions for interactive programs.
//! - Enforce resource limits such as CPU time, memory, and other resource constraints.

pub use config::{Config, EXAMPLE_CONFIG};
pub use isolate::{BoxPool, prepare_cgroup};
pub use runner::Runner;
pub use types::ResourceLimits;

pub mod config;
pub mod isolate;
pub mod runner;
pub mod types;
