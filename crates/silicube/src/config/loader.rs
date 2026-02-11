//! Configuration file loading for Silicube
//!
//! Handles loading and parsing configuration files using the config crate.

use std::path::Path;

use config::{Config as ConfigBuilder, File, FileFormat};

use crate::config::{Config, ConfigError};

impl Config {
    /// Load configuration from a file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let config = ConfigBuilder::builder()
            .add_source(File::from(path))
            .build()?;

        let config: Config = config.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }

    /// Parse configuration from a TOML string
    pub fn parse_toml(content: &str) -> Result<Self, ConfigError> {
        let config = ConfigBuilder::builder()
            .add_source(File::from_str(content, FileFormat::Toml))
            .build()?;

        let config: Config = config.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate all languages have required fields
        for (id, lang) in &self.languages {
            if lang.name.is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "language '{id}' has empty name"
                )));
            }
            if lang.extension.is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "language '{id}' has empty extension"
                )));
            }
            if lang.run.command.is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "language '{id}' has empty run command"
                )));
            }
            if let Some(ref compile) = lang.compile
                && compile.command.is_empty()
            {
                return Err(ConfigError::Invalid(format!(
                    "language '{id}' has empty compile command"
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[languages.test]
name = "Test Language"
extension = "test"

[languages.test.run]
command = ["./test"]
"#;

        let config = Config::parse_toml(toml).unwrap();
        assert!(config.languages.contains_key("test"));
        assert_eq!(config.languages["test"].name, "Test Language");
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
isolate_path = "/usr/local/bin/isolate"

[default_limits]
time_limit = 2.0
memory_limit = 262144

[languages.cpp17]
name = "C++ 17 (GCC)"
extension = "cpp"

[languages.cpp17.compile]
command = ["g++", "-std=c++17", "-O2", "{source}", "-o", "{output}"]
source_name = "main.cpp"
output_name = "main"

[languages.cpp17.run]
command = ["./{binary}"]
"#;

        let config = Config::parse_toml(toml).unwrap();
        assert_eq!(
            config.isolate_path,
            Some(std::path::PathBuf::from("/usr/local/bin/isolate"))
        );
        assert_eq!(config.default_limits.time_limit, Some(2.0));
        assert_eq!(config.default_limits.memory_limit, Some(262144));
        assert!(config.languages["cpp17"].compile.is_some());
    }

    #[test]
    fn test_default_languages_included() {
        let config = Config::default();
        // Default config includes languages from embedded silicube.example.toml
        assert!(config.languages.contains_key("cpp17"));
        assert!(config.languages.contains_key("cpp20"));
        assert!(config.languages.contains_key("python3"));
        assert!(config.languages.contains_key("java"));
        assert!(config.languages.contains_key("rust"));
        assert!(config.languages.contains_key("go"));
        assert!(config.languages.contains_key("javascript"));
    }

    #[test]
    fn test_partial_limits_dont_override_unspecified_fields() {
        let toml = r#"
[languages.go]
name = "Go"
extension = "go"

[languages.go.compile]
command = ["go", "build", "-o", "{output}", "{source}"]
source_name = "main.go"
output_name = "main"

[languages.go.compile.limits]
max_processes = 50

[languages.go.run]
command = ["./{binary}"]
"#;

        let config = Config::parse_toml(toml).unwrap();
        let compile_limits = config.languages["go"]
            .compile
            .as_ref()
            .unwrap()
            .limits
            .as_ref()
            .unwrap();

        // Only max_processes was specified; other fields should be None
        // so they don't override compile-time base limits via with_overrides
        assert_eq!(compile_limits.max_processes, Some(50));
        assert_eq!(compile_limits.time_limit, None);
        assert_eq!(compile_limits.memory_limit, None);
        assert_eq!(compile_limits.wall_time_limit, None);
    }

    #[test]
    fn test_invalid_empty_name() {
        let toml = r#"
[languages.test]
name = ""
extension = "test"

[languages.test.run]
command = ["./test"]
"#;

        let result = Config::parse_toml(toml);
        assert!(result.is_err());
    }
}
