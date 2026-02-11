use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

pub use crate::config::language::{
    CompileConfig, DEFAULT_SANDBOX_PATH, FileExtension, Language, RunConfig,
};
use crate::types::{MountConfig, ResourceLimits};

pub mod language;
mod loader;

/// Example configuration embedded at compile time.
///
/// Library users can access this to generate a starter config file.
pub const EXAMPLE_CONFIG: &str = include_str!("../../silicube.example.toml");

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid characters in file extension")]
    InvalidFileExtChars,

    #[error("failed to read config file at {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config: {0}")]
    Parse(#[from] config::ConfigError),

    #[error("language '{0}' not found in configuration")]
    LanguageNotFound(String),

    #[error("invalid config: {0}")]
    Invalid(String),
}

/// Config for Silicube
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Path to the Isolate binary (uses PATH if not specified).
    #[serde(default)]
    pub isolate_path: Option<PathBuf>,

    /// Use cgroup memory limiting instead of RLIMIT_AS.
    ///
    /// When enabled, isolate uses `--cg` and `--cg-mem` which limit actual memory
    /// usage (RSS) rather than virtual address space. This is required for runtimes
    /// like the JVM and Go that map large amounts of virtual memory.
    #[serde(default)]
    pub cgroup: bool,

    /// Cgroup root path for isolate. Must match isolate's `cg_root` config value.
    ///
    /// When `cgroup = true`, silicube will attempt to create this cgroup directory
    /// and enable the memory controller before invoking isolate. This replaces the
    /// need for `isolate-cg-keeper` / systemd in container environments.
    #[serde(default = "default_cg_root")]
    pub cg_root: PathBuf,

    /// Global directory mounts applied to all sandbox invocations
    /// (both compilation and execution).
    #[serde(default)]
    pub sandbox_mounts: Vec<MountConfig>,

    /// Default resource limits applied to all executions.
    /// This will be overridden if the code execution request specifies different limits
    #[serde(default)]
    pub default_limits: ResourceLimits,

    /// Language configurations keyed by language ID
    #[serde(default)]
    pub languages: HashMap<String, Language>,
}

impl Config {
    /// Create a new config with embedded default languages
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an empty config with no languages
    pub fn empty() -> Self {
        Self {
            isolate_path: None,
            cgroup: false,
            cg_root: default_cg_root(),
            sandbox_mounts: Vec::new(),
            default_limits: ResourceLimits::default(),
            languages: HashMap::new(),
        }
    }

    /// Get a language by ID
    pub fn get_language(&self, id: &str) -> Result<&Language, ConfigError> {
        self.languages
            .get(id)
            .ok_or_else(|| ConfigError::LanguageNotFound(id.to_string()))
    }

    /// Get the path to the isolate binary
    pub fn isolate_binary(&self) -> PathBuf {
        self.isolate_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("isolate"))
    }

    /// Merge resource limits with defaults
    pub fn effective_limits(&self, overrides: Option<&ResourceLimits>) -> ResourceLimits {
        match overrides {
            Some(limits) => self.default_limits.with_overrides(limits),
            None => self.default_limits.clone(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::parse_toml(EXAMPLE_CONFIG).expect("embedded default config should be valid")
    }
}

fn default_cg_root() -> PathBuf {
    PathBuf::from("/sys/fs/cgroup/isolate")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_language_found() {
        let config = Config::default();
        let result = config.get_language("cpp17");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "C++ 17 (GCC)");
    }

    #[test]
    fn get_language_not_found() {
        let config = Config::default();
        let result = config.get_language("nonexistent");
        assert!(result.is_err());
        match result {
            Err(ConfigError::LanguageNotFound(name)) => assert_eq!(name, "nonexistent"),
            _ => panic!("expected LanguageNotFound error"),
        }
    }

    #[test]
    fn get_language_empty_config() {
        let config = Config::empty();
        let result = config.get_language("cpp17");
        assert!(result.is_err());
    }

    #[test]
    fn isolate_binary_default() {
        let config = Config::empty();
        assert_eq!(config.isolate_binary(), PathBuf::from("isolate"));
    }

    #[test]
    fn isolate_binary_custom_path() {
        let config = Config {
            isolate_path: Some(PathBuf::from("/usr/local/bin/isolate")),
            cgroup: false,
            cg_root: default_cg_root(),
            sandbox_mounts: Vec::new(),
            default_limits: ResourceLimits::default(),
            languages: std::collections::HashMap::new(),
        };
        assert_eq!(
            config.isolate_binary(),
            PathBuf::from("/usr/local/bin/isolate")
        );
    }

    #[test]
    fn effective_limits_no_override() {
        let config = Config::default();
        let result = config.effective_limits(None);
        assert_eq!(result.time_limit, config.default_limits.time_limit);
        assert_eq!(result.memory_limit, config.default_limits.memory_limit);
    }

    #[test]
    fn effective_limits_with_override() {
        let config = Config::default();
        let overrides = ResourceLimits {
            time_limit: Some(10.0),
            memory_limit: Some(512 * 1024),
            ..Default::default()
        };
        let result = config.effective_limits(Some(&overrides));
        assert_eq!(result.time_limit, Some(10.0));
        assert_eq!(result.memory_limit, Some(512 * 1024));
    }

    #[test]
    fn effective_limits_partial_override() {
        let config = Config::default();
        let overrides = ResourceLimits {
            time_limit: Some(10.0),
            memory_limit: None,
            ..Default::default()
        };
        let result = config.effective_limits(Some(&overrides));
        assert_eq!(result.time_limit, Some(10.0));
        // Memory should come from default
        assert_eq!(result.memory_limit, config.default_limits.memory_limit);
    }

    #[test]
    fn config_new_has_languages() {
        let config = Config::new();
        assert!(!config.languages.is_empty());
    }

    #[test]
    fn config_empty_has_no_languages() {
        let config = Config::empty();
        assert!(config.languages.is_empty());
    }

    #[test]
    fn config_empty_has_default_limits() {
        let config = Config::empty();
        // Default limits should still be populated
        assert!(config.default_limits.time_limit.is_some());
    }
}
