//! Integration tests for silicube
//!
//! These tests require the isolate binary to be installed and accessible.
//! Run with: cargo test -p silicube --features integration-tests
//!
//! Tests that require root are marked `#[ignore]`. To include them:
//!    cargo test -p silicube --features integration-tests -- --include-ignored

#![cfg(feature = "integration-tests")]

use std::fs;

use silicube::config::Config;

mod compilation;
mod compile_and_run;
mod config_loading;
mod execution;
mod interactive_execution;
mod meta_file_fixtures;
mod resource_limits;
mod sandbox_lifecycle;

const FIXTURES_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// Helper to get fixture file content
pub(crate) fn fixture_source(name: &str) -> Vec<u8> {
    let path = format!("{FIXTURES_PATH}/sources/{name}");
    fs::read(&path).unwrap_or_else(|e| panic!("Failed to read fixture {path}: {e}"))
}

/// Create a test config with cgroup support if available, falling back to non-cgroup mode.
pub(crate) fn test_config() -> Config {
    let mut config = Config::default();
    if config.cgroup {
        match silicube::prepare_cgroup(&config.cg_root) {
            Ok(true) => {}              // cgroups ready
            _ => config.cgroup = false, // not available, fall back
        }
    }
    config
}
