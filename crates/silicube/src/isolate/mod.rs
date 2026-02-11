//! IOI Isolate wrapper
//!
//! This module provides a Rust interface to Isolate, handling command bulding,
//! box lifecycle management, and result parsing.
//!
//! References for Isolate's CLI arguments and meta-files:
//! - https://www.ucw.cz/isolate/isolate.1.html
//! - https://github.com/ioi/isolate

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub use crate::isolate::box_manager::{BoxPool, IsolateBox};
pub use crate::isolate::command::{IsolateAction, IsolateCommand};
pub use crate::isolate::meta::{MetaFile, MetaParseError};
pub use crate::isolate::process::{IsolateProcess, run_batch, run_with_output};
use crate::types::MountConfig;

mod box_manager;
mod command;
mod meta;
mod process;

/// Errors that occur during isolate sandbox operations
#[derive(Debug, Error)]
pub enum IsolateError {
    #[error("failed to initialize box {id}: {message}")]
    InitFailed { id: u32, message: String },

    #[error("failed to cleanup box {id}: {message}")]
    CleanupFailed { id: u32, message: String },

    #[error("isolate command failed: {0}")]
    CommandFailed(String),

    #[error("failed to spawn isolate process: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("failed to parse meta file: {0}")]
    MetaParseFailed(String),

    #[error("box {0} not found or not initialized")]
    BoxNotFound(u32),

    #[error("no available boxes in pool")]
    PoolExhausted,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("isolate binary not found at {0}")]
    BinaryNotFound(PathBuf),

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("mount source path does not exist: {0}")]
    MountSourceNotFound(String),

    #[error("stdin is closed")]
    StdinClosed,
}

/// Attempt to set up the cgroup v2 hierarchy for isolate.
///
/// In container environments, `isolate-cg-keeper` (the systemd service that
/// normally manages isolate's cgroup) is not available. This function replicates
/// its job: creating the cgroup directory at `cg_root` and enabling the memory
/// and pids controllers so that per-box child cgroups work.
///
/// Returns `Ok(true)` if cgroups are ready, `Ok(false)` if setup failed and the
/// caller should fall back to non-cgroup mode (RLIMIT_AS).
pub fn prepare_cgroup(cg_root: &Path) -> Result<bool, IsolateError> {
    let cg_base = Path::new("/sys/fs/cgroup");

    // Check if cgroup v2 is available
    let controllers_path = cg_base.join("cgroup.controllers");
    if !controllers_path.exists() {
        return Ok(false);
    }

    // Check if the memory controller is available in this namespace
    let controllers = fs::read_to_string(&controllers_path)?;
    if !controllers.split_whitespace().any(|c| c == "memory") {
        return Ok(false);
    }

    // If cg_root already has the memory controller enabled, nothing to do
    if cg_root.exists() {
        let subtree = cg_root.join("cgroup.subtree_control");
        if let Ok(content) = fs::read_to_string(&subtree)
            && content.split_whitespace().any(|c| c == "memory")
        {
            return Ok(true);
        }
    }

    // Move our process out of the root cgroup into a leaf cgroup.
    // cgroup v2's "no internal process" rule prevents enabling controllers
    // in a cgroup that has processes directly in it.
    let init_cg = cg_base.join("init");
    if !init_cg.exists() {
        fs::create_dir(&init_cg)?;
    }
    fs::write(init_cg.join("cgroup.procs"), std::process::id().to_string())?;

    // Enable memory and pids controllers at the root
    fs::write(cg_base.join("cgroup.subtree_control"), "+memory +pids")?;

    // Create the isolate cgroup directory
    if !cg_root.exists() {
        fs::create_dir(cg_root)?;
    }

    // Enable controllers for per-box children
    fs::write(cg_root.join("cgroup.subtree_control"), "+memory +pids")?;

    Ok(true)
}

/// Validate that all mount source paths exist
///
/// Returns an error if any non-optional mount source path does not exist on the host filesystem.
/// Optional mounts (with `optional: true`) are silently skipped if the source doesn't exist.
pub fn validate_mounts(mounts: &[MountConfig]) -> Result<(), IsolateError> {
    for mount in mounts {
        if mount.optional {
            continue;
        }
        let path = Path::new(&mount.source);
        if !path.exists() {
            return Err(IsolateError::MountSourceNotFound(mount.source.clone()));
        }
    }
    Ok(())
}

/// Resolve the program in a command to an absolute path using the host's PATH.
///
/// Isolate uses `execve` which does not search PATH, so commands must be
/// absolute paths or contain a `/`. This function resolves bare command names
/// (like `g++`) to their full path (like `/bin/g++`) using the host's PATH
/// environment variable.
///
/// Commands that already contain a `/` (like `./main` or `/usr/bin/g++`) are
/// left unchanged.
pub fn resolve_command(command: &mut [String]) -> Result<(), IsolateError> {
    let first = match command.first_mut() {
        Some(first) => first,
        None => return Ok(()),
    };

    // Already an absolute or relative path
    if first.contains('/') {
        return Ok(());
    }

    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in path_var.split(':') {
        let candidate = std::path::Path::new(dir).join(&*first);
        if candidate.exists() {
            // Canonicalize to resolve symlinks (e.g., /bin/go -> /nix/store/.../bin/go).
            // This ensures the resolved path is directly accessible inside the sandbox
            // without relying on symlink resolution across bind-mount boundaries.
            *first = std::fs::canonicalize(&candidate)
                .unwrap_or(candidate)
                .to_string_lossy()
                .into_owned();
            return Ok(());
        }
    }

    Err(IsolateError::CommandFailed(format!(
        "command '{first}' not found in PATH",
    )))
}
