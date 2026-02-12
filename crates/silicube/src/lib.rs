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

pub use config::{Config, ConfigError, EXAMPLE_CONFIG, Language};
pub use isolate::{BoxPool, IsolateBox, IsolateError, prepare_cgroup};
pub use runner::{
    CompileAndRunError, CompileAndRunRequest, CompileError, CompileResult, ExecuteError,
    InteractiveError, InteractiveEvent, InteractiveEventStream, InteractiveSession,
    InteractiveSessionHandle, Runner,
};
pub use types::{ExecutionResult, ExecutionStatus, LimitExceeded, MountConfig, ResourceLimits};

pub mod config;
pub mod isolate;
pub mod runner;
pub mod types;
