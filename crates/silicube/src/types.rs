use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU time limit in seconds
    #[serde(default)]
    pub time_limit: Option<f64>,

    /// Wall clock time limit in seconds
    #[serde(default)]
    pub wall_time_limit: Option<f64>,

    /// Memory limit in kilobytes
    #[serde(default)]
    pub memory_limit: Option<u64>,

    /// Stack size limit in kilobytes
    #[serde(default)]
    pub stack_limit: Option<u64>,

    /// Maximum number of processes/threads
    #[serde(default)]
    pub max_processes: Option<u32>,

    /// Maximum output size in kilobytes
    #[serde(default)]
    pub max_output: Option<u64>,

    /// Maximum open files
    #[serde(default)]
    pub max_open_files: Option<u32>,

    /// Extra time before killing (grace period) in seconds
    #[serde(default)]
    pub extra_time: Option<f64>,
}

impl ResourceLimits {
    /// 1 kilobyte in bytes
    pub const KB: u64 = 1;
    /// 1 megabyte in kilobytes
    pub const MB: u64 = 1024;
    /// 1 gigabyte in kilobytes
    pub const GB: u64 = 1024 * 1024;

    /// Create new resource limits with all fields set to None
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the CPU time limit in seconds
    pub fn with_time_limit(mut self, seconds: f64) -> Self {
        self.time_limit = Some(seconds);
        self
    }

    /// Set the wall clock time limit in seconds
    pub fn with_wall_time_limit(mut self, seconds: f64) -> Self {
        self.wall_time_limit = Some(seconds);
        self
    }

    /// Set the memory limit in kilobytes
    pub fn with_memory_limit(mut self, kb: u64) -> Self {
        self.memory_limit = Some(kb);
        self
    }

    /// Set the stack size limit in kilobytes
    pub fn with_stack_limit(mut self, kb: u64) -> Self {
        self.stack_limit = Some(kb);
        self
    }

    /// Set the maximum number of processes
    pub fn with_max_processes(mut self, count: u32) -> Self {
        self.max_processes = Some(count);
        self
    }

    /// Set the maximum output size in kilobytes
    pub fn with_max_output(mut self, kb: u64) -> Self {
        self.max_output = Some(kb);
        self
    }

    /// Apply overrides from another ResourceLimits, preferring values from `overrides`
    ///
    /// Returns a new ResourceLimits with values from `overrides` taking precedence
    /// over values from `self` when both are present.
    pub fn with_overrides(&self, overrides: &ResourceLimits) -> ResourceLimits {
        ResourceLimits {
            time_limit: overrides.time_limit.or(self.time_limit),
            wall_time_limit: overrides.wall_time_limit.or(self.wall_time_limit),
            memory_limit: overrides.memory_limit.or(self.memory_limit),
            stack_limit: overrides.stack_limit.or(self.stack_limit),
            max_processes: overrides.max_processes.or(self.max_processes),
            max_output: overrides.max_output.or(self.max_output),
            max_open_files: overrides.max_open_files.or(self.max_open_files),
            extra_time: overrides.extra_time.or(self.extra_time),
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            time_limit: Some(2.0),
            wall_time_limit: Some(5.0),
            memory_limit: Some(262144), // 256 MB
            stack_limit: Some(262144),  // 256 MB
            max_processes: Some(1),
            max_output: Some(65536), // 64 MB
            max_open_files: Some(64),
            extra_time: Some(0.5),
        }
    }
}

/// Result of an execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Execution status (matches IOI Isolate status codes)
    pub status: ExecutionStatus,

    /// Secondary status indicating which limit was exceeded (if any)
    pub limit_exceeded: LimitExceeded,

    /// CPU time used in seconds
    pub time: f64,

    /// Wall clock time used in seconds
    pub wall_time: f64,

    /// Peak memory usage in kilobytes (cg-mem preferred, fallback to max-rss)
    pub memory: u64,

    /// cgroup memory in kilobytes (includes page cache).
    /// None if isolate didn't report cg-mem.
    pub cg_memory: Option<u64>,

    /// Peak resident set size in kilobytes (process-only).
    /// None if isolate didn't report max-rss.
    pub max_rss: Option<u64>,

    /// Exit code if the program exited normally
    pub exit_code: Option<i32>,

    /// Signal number if the program was killed by a signal
    pub signal: Option<i32>,

    /// Additional message from isolate
    pub message: Option<String>,

    /// Standard output (if captured)
    pub stdout: Option<Vec<u8>>,

    /// Standard error (if captured)
    pub stderr: Option<Vec<u8>>,
}

impl ExecutionResult {
    /// Check if the execution was successful (exited with code 0)
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self.status, ExecutionStatus::Ok) && self.exit_code == Some(0)
    }
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            status: ExecutionStatus::Ok,
            limit_exceeded: LimitExceeded::NotExceeded,
            time: 0.0,
            wall_time: 0.0,
            memory: 0,
            cg_memory: None,
            max_rss: None,
            exit_code: None,
            signal: None,
            message: None,
            stdout: None,
            stderr: None,
        }
    }
}

/// Status of an execution
/// Corresponds to IOI Isolate two-letter status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Program exited normally
    #[serde(rename = "OK")]
    Ok,

    /// Runtime error (non-zero exit code)
    #[serde(rename = "RE")]
    RuntimeError,

    /// Time limit exceeded
    #[serde(rename = "TO")]
    TimeLimitExceeded,

    /// Program was killed by a signal
    #[serde(rename = "SG")]
    Signaled,

    /// Internal error in Isolate
    #[serde(rename = "XX")]
    InternalError,
}

impl ExecutionStatus {
    /// Parse status from isolate meta file status string
    pub fn from_isolate_status(status: &str) -> Self {
        match status {
            "OK" => ExecutionStatus::Ok,
            "RE" => ExecutionStatus::RuntimeError,
            "TO" => ExecutionStatus::TimeLimitExceeded,
            "SG" => ExecutionStatus::Signaled,
            "XX" => ExecutionStatus::InternalError,
            _ => ExecutionStatus::InternalError,
        }
    }
}

/// Secondary status indicating which resource limit was exceeded.
/// This provides more detail beyond the basic ExecutionStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LimitExceeded {
    /// No limit was exceeded
    #[default]
    #[serde(rename = "none")]
    NotExceeded,

    /// CPU time limit exceeded (TLE)
    #[serde(rename = "time")]
    Time,

    /// Wall clock time limit exceeded
    #[serde(rename = "wall_time")]
    WallTime,

    /// Memory limit exceeded (MLE)
    #[serde(rename = "memory")]
    Memory,

    /// Output limit exceeded (OLE)
    #[serde(rename = "output")]
    Output,
}

impl LimitExceeded {
    /// Infer which limit was exceeded from isolate's message field
    pub fn from_message(message: Option<&str>) -> Self {
        let Some(msg) = message else {
            return LimitExceeded::NotExceeded;
        };

        let msg_lower = msg.to_lowercase();

        if msg_lower.contains("time limit") {
            if msg_lower.contains("wall") {
                LimitExceeded::WallTime
            } else {
                LimitExceeded::Time
            }
        } else if msg_lower.contains("memory") || msg_lower.contains("out of memory") {
            LimitExceeded::Memory
        } else if msg_lower.contains("output") {
            LimitExceeded::Output
        } else {
            LimitExceeded::NotExceeded
        }
    }

    /// Check if any limit was exceeded
    #[must_use]
    pub fn is_exceeded(&self) -> bool {
        !matches!(self, LimitExceeded::NotExceeded)
    }
}

/// Configuration for a directory mount in Isolate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    /// Source path on the host
    pub source: String,

    /// Target path in the sandbox
    pub target: String,

    /// Whether the mount is read-write (default: read-only)
    #[serde(default)]
    pub writable: bool,

    /// Whether this mount is optional (don't fail if source doesn't exist)
    /// Maps to isolate's `:maybe` flag
    #[serde(default)]
    pub optional: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ResourceLimits tests

    #[test]
    fn resource_limits_default_has_all_fields() {
        let limits = ResourceLimits::default();
        assert!(limits.time_limit.is_some());
        assert!(limits.wall_time_limit.is_some());
        assert!(limits.memory_limit.is_some());
        assert!(limits.stack_limit.is_some());
        assert!(limits.max_processes.is_some());
        assert!(limits.max_output.is_some());
        assert!(limits.max_open_files.is_some());
        assert!(limits.extra_time.is_some());
    }

    #[test]
    fn resource_limits_new_equals_default() {
        let new = ResourceLimits::new();
        let default = ResourceLimits::default();
        assert_eq!(new.time_limit, default.time_limit);
        assert_eq!(new.memory_limit, default.memory_limit);
    }

    #[test]
    fn resource_limits_builder_methods() {
        let limits = ResourceLimits::new()
            .with_time_limit(5.0)
            .with_wall_time_limit(10.0)
            .with_memory_limit(1024)
            .with_stack_limit(512)
            .with_max_processes(4)
            .with_max_output(2048);

        assert_eq!(limits.time_limit, Some(5.0));
        assert_eq!(limits.wall_time_limit, Some(10.0));
        assert_eq!(limits.memory_limit, Some(1024));
        assert_eq!(limits.stack_limit, Some(512));
        assert_eq!(limits.max_processes, Some(4));
        assert_eq!(limits.max_output, Some(2048));
    }

    #[test]
    fn with_overrides_empty_preserves_base() {
        let base = ResourceLimits::default();
        let empty = ResourceLimits {
            time_limit: None,
            wall_time_limit: None,
            memory_limit: None,
            stack_limit: None,
            max_processes: None,
            max_output: None,
            max_open_files: None,
            extra_time: None,
        };

        let result = base.with_overrides(&empty);
        assert_eq!(result.time_limit, base.time_limit);
        assert_eq!(result.wall_time_limit, base.wall_time_limit);
        assert_eq!(result.memory_limit, base.memory_limit);
        assert_eq!(result.stack_limit, base.stack_limit);
        assert_eq!(result.max_processes, base.max_processes);
        assert_eq!(result.max_output, base.max_output);
        assert_eq!(result.max_open_files, base.max_open_files);
        assert_eq!(result.extra_time, base.extra_time);
    }

    #[test]
    fn with_overrides_replaces_values() {
        let base = ResourceLimits::default();
        let overrides = ResourceLimits {
            time_limit: Some(10.0),
            memory_limit: Some(512 * ResourceLimits::MB),
            ..Default::default()
        };

        let result = base.with_overrides(&overrides);
        assert_eq!(result.time_limit, Some(10.0));
        assert_eq!(result.memory_limit, Some(512 * ResourceLimits::MB));
        // Other fields should come from base (or be base defaults)
        assert_eq!(result.wall_time_limit, base.wall_time_limit);
    }

    #[test]
    fn with_overrides_partial_override() {
        let base = ResourceLimits {
            time_limit: Some(2.0),
            memory_limit: Some(256 * ResourceLimits::MB),
            max_processes: None,
            ..Default::default()
        };
        let overrides = ResourceLimits {
            time_limit: Some(5.0),
            max_processes: Some(4),
            ..Default::default()
        };

        let result = base.with_overrides(&overrides);
        assert_eq!(result.time_limit, Some(5.0)); // Overridden
        assert_eq!(result.memory_limit, Some(256 * ResourceLimits::MB)); // From base
        assert_eq!(result.max_processes, Some(4)); // Overridden (was None in base)
    }

    // ExecutionStatus tests

    #[test]
    fn execution_status_from_isolate_status_ok() {
        assert_eq!(
            ExecutionStatus::from_isolate_status("OK"),
            ExecutionStatus::Ok
        );
    }

    #[test]
    fn execution_status_from_isolate_status_re() {
        assert_eq!(
            ExecutionStatus::from_isolate_status("RE"),
            ExecutionStatus::RuntimeError
        );
    }

    #[test]
    fn execution_status_from_isolate_status_to() {
        assert_eq!(
            ExecutionStatus::from_isolate_status("TO"),
            ExecutionStatus::TimeLimitExceeded
        );
    }

    #[test]
    fn execution_status_from_isolate_status_sg() {
        assert_eq!(
            ExecutionStatus::from_isolate_status("SG"),
            ExecutionStatus::Signaled
        );
    }

    #[test]
    fn execution_status_from_isolate_status_xx() {
        assert_eq!(
            ExecutionStatus::from_isolate_status("XX"),
            ExecutionStatus::InternalError
        );
    }

    #[test]
    fn execution_status_from_isolate_status_unknown_defaults_to_internal_error() {
        assert_eq!(
            ExecutionStatus::from_isolate_status("UNKNOWN"),
            ExecutionStatus::InternalError
        );
        assert_eq!(
            ExecutionStatus::from_isolate_status(""),
            ExecutionStatus::InternalError
        );
        assert_eq!(
            ExecutionStatus::from_isolate_status("ok"),
            ExecutionStatus::InternalError
        );
    }

    // LimitExceeded tests

    #[test]
    fn limit_exceeded_from_message_none() {
        assert_eq!(
            LimitExceeded::from_message(None),
            LimitExceeded::NotExceeded
        );
    }

    #[test]
    fn limit_exceeded_from_message_time_limit() {
        assert_eq!(
            LimitExceeded::from_message(Some("Time limit exceeded")),
            LimitExceeded::Time
        );
        assert_eq!(
            LimitExceeded::from_message(Some("time limit exceeded")),
            LimitExceeded::Time
        );
        assert_eq!(
            LimitExceeded::from_message(Some("TIME LIMIT EXCEEDED")),
            LimitExceeded::Time
        );
    }

    #[test]
    fn limit_exceeded_from_message_wall_time() {
        assert_eq!(
            LimitExceeded::from_message(Some("Wall time limit exceeded")),
            LimitExceeded::WallTime
        );
        assert_eq!(
            LimitExceeded::from_message(Some("wall time limit exceeded")),
            LimitExceeded::WallTime
        );
    }

    #[test]
    fn limit_exceeded_from_message_memory() {
        assert_eq!(
            LimitExceeded::from_message(Some("Memory limit exceeded")),
            LimitExceeded::Memory
        );
        assert_eq!(
            LimitExceeded::from_message(Some("Out of memory")),
            LimitExceeded::Memory
        );
        assert_eq!(
            LimitExceeded::from_message(Some("out of memory")),
            LimitExceeded::Memory
        );
    }

    #[test]
    fn limit_exceeded_from_message_output() {
        assert_eq!(
            LimitExceeded::from_message(Some("Output limit exceeded")),
            LimitExceeded::Output
        );
        assert_eq!(
            LimitExceeded::from_message(Some("output limit")),
            LimitExceeded::Output
        );
    }

    #[test]
    fn limit_exceeded_from_message_unknown() {
        assert_eq!(
            LimitExceeded::from_message(Some("Some other error")),
            LimitExceeded::NotExceeded
        );
        assert_eq!(
            LimitExceeded::from_message(Some("")),
            LimitExceeded::NotExceeded
        );
    }

    #[test]
    fn limit_exceeded_is_exceeded() {
        assert!(!LimitExceeded::NotExceeded.is_exceeded());
        assert!(LimitExceeded::Time.is_exceeded());
        assert!(LimitExceeded::WallTime.is_exceeded());
        assert!(LimitExceeded::Memory.is_exceeded());
        assert!(LimitExceeded::Output.is_exceeded());
    }

    // ExecutionResult tests

    #[test]
    fn execution_result_is_success_true() {
        let result = ExecutionResult {
            status: ExecutionStatus::Ok,
            exit_code: Some(0),
            ..Default::default()
        };
        assert!(result.is_success());
    }

    #[test]
    fn execution_result_is_success_false_non_zero_exit() {
        let result = ExecutionResult {
            status: ExecutionStatus::Ok,
            exit_code: Some(1),
            ..Default::default()
        };
        assert!(!result.is_success());
    }

    #[test]
    fn execution_result_is_success_false_bad_status() {
        let result = ExecutionResult {
            status: ExecutionStatus::RuntimeError,
            exit_code: Some(0),
            ..Default::default()
        };
        assert!(!result.is_success());
    }

    #[test]
    fn execution_result_is_success_false_no_exit_code() {
        let result = ExecutionResult {
            status: ExecutionStatus::Ok,
            exit_code: None,
            ..Default::default()
        };
        assert!(!result.is_success());
    }

    #[test]
    fn execution_result_default() {
        let result = ExecutionResult::default();
        assert_eq!(result.status, ExecutionStatus::Ok);
        assert_eq!(result.limit_exceeded, LimitExceeded::NotExceeded);
        assert_eq!(result.time, 0.0);
        assert_eq!(result.wall_time, 0.0);
        assert_eq!(result.memory, 0);
        assert!(result.exit_code.is_none());
        assert!(result.signal.is_none());
        assert!(result.message.is_none());
        assert!(result.stdout.is_none());
        assert!(result.stderr.is_none());
    }

    // MountConfig tests

    #[test]
    fn mount_config_default_read_only() {
        let mount = MountConfig {
            source: "/src".to_string(),
            target: "/dest".to_string(),
            writable: false,
            optional: false,
        };
        assert!(!mount.writable);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn with_overrides_identity(
            time in proptest::option::of(0.0f64..1000.0),
            wall_time in proptest::option::of(0.0f64..1000.0),
            memory in proptest::option::of(0u64..1_000_000),
            stack in proptest::option::of(0u64..1_000_000),
            procs in proptest::option::of(0u32..100),
            output in proptest::option::of(0u64..1_000_000),
            open_files in proptest::option::of(0u32..1000),
            extra in proptest::option::of(0.0f64..10.0),
        ) {
            let base = ResourceLimits {
                time_limit: time,
                wall_time_limit: wall_time,
                memory_limit: memory,
                stack_limit: stack,
                max_processes: procs,
                max_output: output,
                max_open_files: open_files,
                extra_time: extra,
            };
            let empty = ResourceLimits {
                time_limit: None,
                wall_time_limit: None,
                memory_limit: None,
                stack_limit: None,
                max_processes: None,
                max_output: None,
                max_open_files: None,
                extra_time: None,
            };

            let result = base.with_overrides(&empty);
            prop_assert_eq!(result.time_limit, base.time_limit);
            prop_assert_eq!(result.wall_time_limit, base.wall_time_limit);
            prop_assert_eq!(result.memory_limit, base.memory_limit);
            prop_assert_eq!(result.stack_limit, base.stack_limit);
            prop_assert_eq!(result.max_processes, base.max_processes);
            prop_assert_eq!(result.max_output, base.max_output);
            prop_assert_eq!(result.max_open_files, base.max_open_files);
            prop_assert_eq!(result.extra_time, base.extra_time);
        }

        #[test]
        fn with_overrides_full_override(
            base_time in proptest::option::of(0.0f64..1000.0),
            override_time in 0.0f64..1000.0,
        ) {
            let base = ResourceLimits {
                time_limit: base_time,
                ..Default::default()
            };
            let overrides = ResourceLimits {
                time_limit: Some(override_time),
                ..Default::default()
            };

            let result = base.with_overrides(&overrides);
            prop_assert_eq!(result.time_limit, Some(override_time));
        }

        #[test]
        fn limit_exceeded_from_message_never_panics(msg in ".*") {
            // Should never panic on any input
            let _ = LimitExceeded::from_message(Some(&msg));
        }

        #[test]
        fn execution_status_from_isolate_never_panics(status in ".*") {
            // Should never panic on any input
            let _ = ExecutionStatus::from_isolate_status(&status);
        }
    }
}
