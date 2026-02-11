//! Compilation step for code execution
//!
//! Handles compiling source code using language-specific compilers.

use tracing::{debug, instrument};

use crate::config::language::DEFAULT_SANDBOX_PATH;
use crate::config::{Config, Language};
use crate::isolate::{IsolateAction, IsolateBox, IsolateCommand, resolve_command, run_with_output};
use crate::runner::CompileError;
use crate::types::{ExecutionResult, ResourceLimits};

/// Result of a compilation
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// Whether compilation succeeded
    pub success: bool,

    /// Execution result from the compilation process
    pub execution: ExecutionResult,

    /// Compiler output (usually stderr for error messages)
    pub output: String,
}

impl CompileResult {
    /// Check if compilation was successful
    pub fn is_success(&self) -> bool {
        self.success && self.execution.exit_code == Some(0)
    }
}

/// Default compilation limits
fn default_compile_limits() -> ResourceLimits {
    ResourceLimits {
        time_limit: Some(30.0),      // 30 seconds
        wall_time_limit: Some(60.0), // 60 seconds wall time
        memory_limit: Some(524288),  // 512 MB
        max_processes: Some(10),     // Allow multiple processes for compilers
        max_output: Some(65536),     // 64 MB output
        ..Default::default()
    }
}

/// Compile source code in an isolate box
#[instrument(skip(sandbox, config, source))]
pub async fn compile(
    sandbox: &IsolateBox,
    config: &Config,
    language: &Language,
    source: &[u8],
    limits: Option<&ResourceLimits>,
) -> Result<CompileResult, CompileError> {
    // Check if language requires compilation
    let compile_config = language
        .compile
        .as_ref()
        .ok_or_else(|| CompileError::NotCompiled(language.name.clone()))?;

    // Write source file to sandbox
    let source_name = &compile_config.source_name;
    sandbox
        .write_file(source_name, source)
        .await
        .map_err(CompileError::Isolate)?;

    debug!(source_name, "wrote source file");

    // Determine limits
    let base_limits = default_compile_limits();
    let lang_limits = compile_config.limits.as_ref();
    let effective_limits = match (limits, lang_limits) {
        (Some(user), Some(lang)) => base_limits.with_overrides(lang).with_overrides(user),
        (Some(user), None) => base_limits.with_overrides(user),
        (None, Some(lang)) => base_limits.with_overrides(lang),
        (None, None) => base_limits,
    };

    // Build compile command with resolved path (isolate uses execve, not execvp)
    let mut expanded_cmd = Language::expand_command(
        &compile_config.command,
        source_name,
        &compile_config.output_name,
    );
    resolve_command(&mut expanded_cmd).map_err(CompileError::Isolate)?;

    let mut command = IsolateCommand::new(config.isolate_binary(), sandbox.id())
        .action(IsolateAction::Run)
        .cgroup(config.cgroup)
        .limits(effective_limits)
        .working_dir("/box")
        .env("PATH", DEFAULT_SANDBOX_PATH)
        .mounts(config.sandbox_mounts.iter().cloned())
        .command(expanded_cmd);

    // Add environment variables from compile config
    for (key, value) in &compile_config.env {
        command = command.env(key, value);
    }

    // Run compilation
    let (result, mut output) = run_with_output(sandbox, command)
        .await
        .map_err(CompileError::Isolate)?;

    let success = result.exit_code == Some(0);

    debug!(
        success,
        exit_code = ?result.exit_code,
        status = ?result.status,
        message = ?result.message,
        "compilation complete"
    );

    // Include isolate's error message in output if the sandboxed process produced nothing
    if output.is_empty()
        && let Some(ref msg) = result.message
    {
        output = msg.clone();
    }

    Ok(CompileResult {
        success,
        execution: result,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_compile_limits() {
        let limits = default_compile_limits();
        assert_eq!(limits.time_limit, Some(30.0));
        assert_eq!(limits.memory_limit, Some(524288));
    }
}
