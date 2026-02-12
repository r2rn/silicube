//! Meta file parsing for isolate
//!
//! Parses the meta file produced by isolate after execution to extract
//! execution results like time used, memory used, and exit status.

use std::collections::HashMap;
use std::path::Path;

use thiserror::Error;

use crate::isolate::IsolateError;
use crate::types::{ExecutionResult, ExecutionStatus, LimitExceeded};

/// Error that occurs during meta file parsing
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("meta file parse error at line {line_number}: {message} (line: {line:?})")]
pub struct MetaParseError {
    /// Line number (1-indexed) where the error occurred
    pub line_number: usize,
    /// The problematic line content
    pub line: String,
    /// Description of the error
    pub message: String,
}

/// Parsed meta file from Isolate
#[derive(Debug, Clone, Default)]
pub struct MetaFile {
    /// Raw key-value pairs from the meta file
    pub entries: HashMap<String, String>,
}

impl MetaFile {
    /// Parse meta file content from a string
    ///
    /// This is a lenient parser that skips malformed lines. For strict parsing
    /// that reports errors, use [`try_parse`](Self::try_parse).
    pub fn parse(content: &str) -> Self {
        let mut entries = HashMap::new();

        // Meta-file entries are key-value pairs separated by colons
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                // Handle values that may contain colons (e.g., timestamps)
                // The value is everything after the first colon
                let value = value.trim();
                if !key.is_empty() {
                    entries.insert(key.to_string(), value.to_string());
                }
            }
        }

        Self { entries }
    }

    /// Parse meta file content with strict error handling
    ///
    /// Returns an error if any line is malformed (non-empty but missing colon).
    /// Empty lines are ignored.
    pub fn try_parse(content: &str) -> Result<Self, MetaParseError> {
        let mut entries = HashMap::new();

        for (line_idx, line) in content.lines().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            match trimmed.split_once(':') {
                Some((key, value)) => {
                    let key = key.trim();
                    let value = value.trim();

                    if key.is_empty() {
                        return Err(MetaParseError {
                            line_number,
                            line: line.to_string(),
                            message: "empty key before colon".to_string(),
                        });
                    }

                    entries.insert(key.to_string(), value.to_string());
                }
                None => {
                    return Err(MetaParseError {
                        line_number,
                        line: line.to_string(),
                        message: "missing colon separator".to_string(),
                    });
                }
            }
        }

        Ok(Self { entries })
    }

    /// Load and parse a meta file from disk
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, IsolateError> {
        let content = tokio::fs::read_to_string(path.as_ref()).await?;
        Ok(Self::parse(&content))
    }

    /// Load and parse a meta file from disk with strict error handling
    pub async fn try_load(path: impl AsRef<Path>) -> Result<Self, IsolateError> {
        let content = tokio::fs::read_to_string(path.as_ref()).await?;
        Self::try_parse(&content).map_err(|e| IsolateError::MetaParseFailed(e.to_string()))
    }

    /// Get a string value
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Get a float value
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Get an integer value
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Get an unsigned integer value
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Get the execution status
    pub fn status(&self) -> ExecutionStatus {
        self.get("status")
            .map(ExecutionStatus::from_isolate_status)
            .unwrap_or(ExecutionStatus::Ok)
    }

    /// Get the CPU time used in seconds
    pub fn time(&self) -> f64 {
        self.get_f64("time").unwrap_or(0.0)
    }

    /// Get the wall clock time used in seconds
    pub fn wall_time(&self) -> f64 {
        self.get_f64("time-wall").unwrap_or(0.0)
    }

    /// Get the peak memory usage in kilobytes
    pub fn memory(&self) -> u64 {
        // Try cg-mem first (cgroup memory), then max-rss
        self.get_u64("cg-mem")
            .or_else(|| self.get_u64("max-rss"))
            .unwrap_or(0)
    }

    /// Get cgroup memory usage in kilobytes (cg-mem from isolate meta).
    /// Includes RSS + page cache + file-mapped memory for the entire cgroup.
    pub fn cg_memory(&self) -> Option<u64> {
        self.get_u64("cg-mem")
    }

    /// Get peak resident set size in kilobytes (max-rss from isolate meta).
    /// Measures only the process's own resident memory.
    pub fn max_rss(&self) -> Option<u64> {
        self.get_u64("max-rss")
    }

    /// Get the exit code
    pub fn exit_code(&self) -> Option<i32> {
        self.get_i32("exitcode")
    }

    /// Get the signal that killed the process
    pub fn signal(&self) -> Option<i32> {
        self.get_i32("exitsig")
    }

    /// Get the message from isolate
    pub fn message(&self) -> Option<String> {
        self.get("message").map(String::from)
    }

    /// Get whether the process was killed
    pub fn killed(&self) -> bool {
        self.get("killed").is_some()
    }

    /// Determine which limit was exceeded based on status and message
    pub fn limit_exceeded(&self) -> LimitExceeded {
        let status = self.status();
        let message = self.message();

        // First try to infer from the message
        let from_message = LimitExceeded::from_message(message.as_deref());
        if from_message.is_exceeded() {
            return from_message;
        }

        // If status is TO (timeout), it's a time limit issue
        if status == ExecutionStatus::TimeLimitExceeded {
            return LimitExceeded::Time;
        }

        LimitExceeded::NotExceeded
    }

    /// Convert to an ExecutionResult
    pub fn to_execution_result(&self) -> ExecutionResult {
        ExecutionResult {
            status: self.status(),
            limit_exceeded: self.limit_exceeded(),
            time: self.time(),
            wall_time: self.wall_time(),
            memory: self.memory(),
            cg_memory: self.cg_memory(),
            max_rss: self.max_rss(),
            exit_code: self.exit_code(),
            signal: self.signal(),
            message: self.message(),
            stdout: None,
            stderr: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_success_meta() {
        let content = r#"
time:0.042
time-wall:0.050
max-rss:3456
exitcode:0
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::Ok);
        assert!((meta.time() - 0.042).abs() < 0.001);
        assert!((meta.wall_time() - 0.050).abs() < 0.001);
        assert_eq!(meta.memory(), 3456);
        assert_eq!(meta.exit_code(), Some(0));
        assert_eq!(meta.signal(), None);
    }

    #[test]
    fn test_parse_tle_meta() {
        let content = r#"
time:2.001
time-wall:2.500
max-rss:1234
status:TO
message:Time limit exceeded
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::TimeLimitExceeded);
        assert_eq!(meta.message(), Some("Time limit exceeded".to_string()));
        assert_eq!(meta.limit_exceeded(), LimitExceeded::Time);
    }

    #[test]
    fn test_limit_exceeded_wall_time() {
        let content = r#"
time:1.000
time-wall:5.001
status:TO
message:Wall time limit exceeded
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::TimeLimitExceeded);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::WallTime);
    }

    #[test]
    fn test_limit_exceeded_memory() {
        let content = r#"
time:0.100
cg-mem:262144
status:SG
exitsig:9
message:Out of memory
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::Signaled);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::Memory);
    }

    #[test]
    fn test_limit_exceeded_output() {
        let content = r#"
time:0.050
status:SG
message:Output limit exceeded
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::Signaled);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::Output);
    }

    #[test]
    fn test_limit_exceeded_none_for_success() {
        let content = r#"
time:0.042
exitcode:0
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::Ok);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::NotExceeded);
    }

    #[test]
    fn test_limit_exceeded_fallback_to_status() {
        // When status is TO but no message, should infer Time limit
        let content = r#"
time:2.001
status:TO
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::TimeLimitExceeded);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::Time);
    }

    #[test]
    fn test_parse_signal_meta() {
        let content = r#"
time:0.010
time-wall:0.020
max-rss:1000
exitsig:11
status:SG
message:Caught fatal signal 11
"#;
        let meta = MetaFile::parse(content);

        assert_eq!(meta.status(), ExecutionStatus::Signaled);
        assert_eq!(meta.signal(), Some(11));
    }

    #[test]
    fn test_parse_cgroup_mem() {
        let content = r#"
time:0.100
cg-mem:524288
max-rss:512000
"#;
        let meta = MetaFile::parse(content);

        // Should prefer cg-mem over max-rss
        assert_eq!(meta.memory(), 524288);
    }

    #[test]
    fn test_to_execution_result() {
        let content = r#"
time:1.234
time-wall:1.500
max-rss:65536
exitcode:0
"#;
        let meta = MetaFile::parse(content);
        let result = meta.to_execution_result();

        assert_eq!(result.status, ExecutionStatus::Ok);
        assert_eq!(result.limit_exceeded, LimitExceeded::NotExceeded);
        assert!((result.time - 1.234).abs() < 0.001);
        assert_eq!(result.memory, 65536);
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_to_execution_result_with_limit() {
        let content = r#"
time:2.001
time-wall:2.500
status:TO
message:Time limit exceeded
"#;
        let meta = MetaFile::parse(content);
        let result = meta.to_execution_result();

        assert_eq!(result.status, ExecutionStatus::TimeLimitExceeded);
        assert_eq!(result.limit_exceeded, LimitExceeded::Time);
    }

    #[test]
    fn test_try_parse_success() {
        let content = "time:0.042\ntime-wall:0.050";
        let meta = MetaFile::try_parse(content).unwrap();
        assert!((meta.time() - 0.042).abs() < 0.001);
    }

    #[test]
    fn test_try_parse_empty_lines() {
        let content = "\n\ntime:0.042\n\n";
        let meta = MetaFile::try_parse(content).unwrap();
        assert!((meta.time() - 0.042).abs() < 0.001);
    }

    #[test]
    fn test_try_parse_whitespace_handling() {
        let content = "  time  :  0.042  ";
        let meta = MetaFile::try_parse(content).unwrap();
        assert!((meta.time() - 0.042).abs() < 0.001);
    }

    #[test]
    fn test_try_parse_value_with_colon() {
        // Values can contain colons (e.g., timestamps or messages)
        let content = "message:Error at 12:30:45";
        let meta = MetaFile::try_parse(content).unwrap();
        assert_eq!(meta.message(), Some("Error at 12:30:45".to_string()));
    }

    #[test]
    fn test_try_parse_missing_colon() {
        let content = "time:0.042\ninvalid line\nexitcode:0";
        let err = MetaFile::try_parse(content).unwrap_err();
        assert_eq!(err.line_number, 2);
        assert_eq!(err.line, "invalid line");
        assert!(err.message.contains("missing colon"));
    }

    #[test]
    fn test_try_parse_empty_key() {
        let content = ":value";
        let err = MetaFile::try_parse(content).unwrap_err();
        assert_eq!(err.line_number, 1);
        assert!(err.message.contains("empty key"));
    }

    #[test]
    fn test_parse_lenient_skips_invalid() {
        // parse() should skip invalid lines without error
        let content = "time:0.042\ninvalid line\nexitcode:0";
        let meta = MetaFile::parse(content);
        assert!((meta.time() - 0.042).abs() < 0.001);
        assert_eq!(meta.exit_code(), Some(0));
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn parse_does_not_panic(content in ".*") {
            // MetaFile::parse should never panic on any input
            let _ = MetaFile::parse(&content);
        }

        #[test]
        fn parse_valid_key_value_pairs(
            key in "[a-z_-]+",
            value in "[a-zA-Z0-9._-]*"
        ) {
            let content = format!("{}:{}", key, value);
            let meta = MetaFile::parse(&content);
            assert_eq!(meta.get(&key), Some(value.as_str()));
        }

        #[test]
        fn parse_preserves_numeric_values(time in 0.0f64..1000.0f64) {
            let content = format!("time:{:.3}", time);
            let meta = MetaFile::parse(&content);
            // Allow for floating point formatting differences
            if let Some(parsed) = meta.get_f64("time") {
                prop_assert!((parsed - time).abs() < 0.001);
            }
        }
    }
}
