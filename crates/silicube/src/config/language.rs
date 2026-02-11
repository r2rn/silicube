use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::config::ConfigError;
use crate::types::{MountConfig, ResourceLimits};

const INVALID_FILE_EXT_CHARS: [char; 2] = ['/', '.'];

/// Configuration for a programming language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    /// Human-readable name for the language (e.g., "C++20 (GCC)")
    pub name: String,

    /// File extension
    pub extension: FileExtension,

    /// Compilation configuration (None for interpreted languages)
    #[serde(default)]
    pub compile: Option<CompileConfig>,

    /// Execution configuration
    pub run: RunConfig,
}

impl Language {
    /// Check if the language is compiled
    pub fn is_compiled(&self) -> bool {
        self.compile.is_some()
    }

    /// Get the source file name for this language
    pub fn source_name(&self) -> String {
        if let Some(ref compile) = self.compile {
            compile.source_name.clone()
        } else {
            format!("main.{}", self.extension)
        }
    }

    /// Expand placeholders in the given command
    pub fn expand_command(command: &[String], source: &str, binary: &str) -> Vec<String> {
        command
            .iter()
            .map(|arg| {
                arg.replace("{source}", source)
                    .replace("{output}", binary)
                    .replace("{binary}", binary)
            })
            .collect()
    }
}

/// File extension without dot (e.g., "cpp")
#[derive(Debug, Clone, Serialize)]
pub struct FileExtension(String);

impl FileExtension {
    pub fn new(extension: &str) -> Result<Self, ConfigError> {
        let contains_invalid = extension
            .chars()
            .any(|c| INVALID_FILE_EXT_CHARS.contains(&c));
        if contains_invalid {
            return Err(ConfigError::InvalidFileExtChars);
        }
        Ok(Self(extension.to_owned()))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for FileExtension {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FileExtension::new(&s).map_err(|_| {
            de::Error::invalid_value(
                de::Unexpected::Str(&s),
                &"a file extension without '/' or '.' characters",
            )
        })
    }
}

impl std::fmt::Display for FileExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for the compilation step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileConfig {
    /// Command and arguments with placeholders
    /// Placeholders: {source}, {binary}
    pub command: Vec<String>,

    /// Source file name in the sandbox (e.g., "main.cpp")
    pub source_name: String,

    /// Output binary name (e.g., "main")
    pub output_name: String,

    /// Environment variables to set during compilation
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Resource limits for compilation (overrides defaults)
    #[serde(default)]
    pub limits: Option<ResourceLimits>,
}

/// Default PATH for sandbox execution
pub const DEFAULT_SANDBOX_PATH: &str = "/usr/bin:/bin";

/// Configuration for the execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    /// Command and arguments with placeholders
    /// Placeholders: {source}, {binary}
    pub command: Vec<String>,

    /// Environment Variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Directory mounts
    #[serde(default)]
    pub mounts: Vec<MountConfig>,

    /// PATH environment variable for the sandbox
    ///
    /// Defaults to "/usr/bin:/bin" if not specified.
    #[serde(default = "default_sandbox_path")]
    pub path: String,

    /// Resource limits for execution (overrides defaults)
    #[serde(default)]
    pub limits: Option<ResourceLimits>,
}

fn default_sandbox_path() -> String {
    DEFAULT_SANDBOX_PATH.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_extension_new_valid() {
        let ext = FileExtension::new("cpp").unwrap();
        assert_eq!(ext.to_string(), "cpp");
    }

    #[test]
    fn file_extension_new_valid_with_numbers() {
        let ext = FileExtension::new("f90").unwrap();
        assert_eq!(ext.to_string(), "f90");
    }

    #[test]
    fn file_extension_new_empty() {
        let ext = FileExtension::new("").unwrap();
        assert!(ext.is_empty());
    }

    #[test]
    fn file_extension_new_rejects_slash() {
        let result = FileExtension::new("path/ext");
        assert!(result.is_err());
    }

    #[test]
    fn file_extension_new_rejects_dot() {
        let result = FileExtension::new(".cpp");
        assert!(result.is_err());
    }

    #[test]
    fn file_extension_new_rejects_multiple_dots() {
        let result = FileExtension::new(".tar.gz");
        assert!(result.is_err());
    }

    #[test]
    fn file_extension_is_empty() {
        let empty = FileExtension::new("").unwrap();
        let non_empty = FileExtension::new("rs").unwrap();
        assert!(empty.is_empty());
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn file_extension_display() {
        let ext = FileExtension::new("py").unwrap();
        assert_eq!(format!("{ext}"), "py");
    }

    #[test]
    fn expand_command_source_placeholder() {
        let cmd = vec![
            "gcc".to_owned(),
            "-o".to_owned(),
            "out".to_owned(),
            "{source}".to_owned(),
        ];
        let result = Language::expand_command(&cmd, "main.c", "main");
        assert_eq!(result, vec!["gcc", "-o", "out", "main.c"]);
    }

    #[test]
    fn expand_command_output_placeholder() {
        let cmd = vec![
            "gcc".to_owned(),
            "-o".to_owned(),
            "{output}".to_owned(),
            "main.c".to_owned(),
        ];
        let result = Language::expand_command(&cmd, "main.c", "main");
        assert_eq!(result, vec!["gcc", "-o", "main", "main.c"]);
    }

    #[test]
    fn expand_command_binary_placeholder() {
        let cmd = vec!["./{binary}".to_owned()];
        let result = Language::expand_command(&cmd, "main.cpp", "main");
        assert_eq!(result, vec!["./main"]);
    }

    #[test]
    fn expand_command_multiple_placeholders() {
        let cmd = vec![
            "gcc".to_owned(),
            "{source}".to_owned(),
            "-o".to_owned(),
            "{output}".to_owned(),
        ];
        let result = Language::expand_command(&cmd, "test.c", "test");
        assert_eq!(result, vec!["gcc", "test.c", "-o", "test"]);
    }

    #[test]
    fn expand_command_no_placeholders() {
        let cmd = vec!["echo".to_owned(), "hello".to_owned()];
        let result = Language::expand_command(&cmd, "main.c", "main");
        assert_eq!(result, vec!["echo", "hello"]);
    }

    #[test]
    fn expand_command_empty() {
        let cmd: Vec<String> = vec![];
        let result = Language::expand_command(&cmd, "main.c", "main");
        assert!(result.is_empty());
    }

    #[test]
    fn expand_command_placeholder_in_middle() {
        let cmd = vec!["prefix-{source}-suffix".to_owned()];
        let result = Language::expand_command(&cmd, "main.c", "main");
        assert_eq!(result, vec!["prefix-main.c-suffix"]);
    }

    #[test]
    fn language_is_compiled_true() {
        let lang = Language {
            name: "C++".to_owned(),
            extension: FileExtension::new("cpp").unwrap(),
            compile: Some(CompileConfig {
                command: vec!["g++".to_owned()],
                source_name: "main.cpp".to_owned(),
                output_name: "main".to_owned(),
                env: std::collections::HashMap::new(),
                limits: None,
            }),
            run: RunConfig {
                command: vec!["./{binary}".to_owned()],
                env: std::collections::HashMap::new(),
                mounts: vec![],
                path: DEFAULT_SANDBOX_PATH.to_owned(),
                limits: None,
            },
        };
        assert!(lang.is_compiled());
    }

    #[test]
    fn language_is_compiled_false() {
        let lang = Language {
            name: "Python".to_owned(),
            extension: FileExtension::new("py").unwrap(),
            compile: None,
            run: RunConfig {
                command: vec!["python3".to_owned(), "{source}".to_owned()],
                env: std::collections::HashMap::new(),
                mounts: vec![],
                path: DEFAULT_SANDBOX_PATH.to_owned(),
                limits: None,
            },
        };
        assert!(!lang.is_compiled());
    }

    #[test]
    fn language_source_name_compiled() {
        let lang = Language {
            name: "C++".to_owned(),
            extension: FileExtension::new("cpp").unwrap(),
            compile: Some(CompileConfig {
                command: vec!["g++".to_owned()],
                source_name: "solution.cpp".to_owned(),
                output_name: "solution".to_owned(),
                env: std::collections::HashMap::new(),
                limits: None,
            }),
            run: RunConfig {
                command: vec!["./{binary}".to_owned()],
                env: std::collections::HashMap::new(),
                mounts: vec![],
                path: DEFAULT_SANDBOX_PATH.to_owned(),
                limits: None,
            },
        };
        assert_eq!(lang.source_name(), "solution.cpp");
    }

    #[test]
    fn language_source_name_interpreted() {
        let lang = Language {
            name: "Python".to_owned(),
            extension: FileExtension::new("py").unwrap(),
            compile: None,
            run: RunConfig {
                command: vec!["python3".to_owned(), "{source}".to_owned()],
                env: std::collections::HashMap::new(),
                mounts: vec![],
                path: DEFAULT_SANDBOX_PATH.to_owned(),
                limits: None,
            },
        };
        assert_eq!(lang.source_name(), "main.py");
    }

    #[test]
    fn run_config_default_path() {
        assert_eq!(DEFAULT_SANDBOX_PATH, "/usr/bin:/bin");
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn file_extension_rejects_all_strings_with_slash(s in ".*/.*.") {
            // Any string containing a slash should be rejected
            let result = FileExtension::new(&s);
            prop_assert!(result.is_err());
        }

        #[test]
        fn file_extension_rejects_all_strings_with_dot(s in ".*\\..*.") {
            // Any string containing a dot should be rejected
            let result = FileExtension::new(&s);
            prop_assert!(result.is_err());
        }

        #[test]
        fn file_extension_accepts_alphanumeric(s in "[a-zA-Z0-9_-]+") {
            // Alphanumeric strings without dots or slashes should be accepted
            let result = FileExtension::new(&s);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn expand_command_preserves_args_without_placeholders(
            arg1 in "[a-z]+",
            arg2 in "[a-z]+",
            arg3 in "[a-z]+"
        ) {
            let cmd = vec![arg1.clone(), arg2.clone(), arg3.clone()];
            let result = Language::expand_command(&cmd, "source.c", "binary");
            prop_assert_eq!(&result[0], &arg1);
            prop_assert_eq!(&result[1], &arg2);
            prop_assert_eq!(&result[2], &arg3);
        }

        #[test]
        fn expand_command_length_preserved(cmd_len in 1usize..10) {
            let cmd: Vec<String> = (0..cmd_len).map(|i| format!("arg{i}")).collect();
            let result = Language::expand_command(&cmd, "source", "binary");
            prop_assert_eq!(result.len(), cmd_len);
        }
    }
}
