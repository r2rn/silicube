//! Execution step for code running
//!
//! Handles running compiled or interpreted programs with input/output.

use tracing::{debug, instrument};

use crate::config::{Config, Language};
use crate::isolate::{
    IsolateAction, IsolateBox, IsolateCommand, resolve_command, run_batch, validate_mounts,
};
use crate::runner::ExecuteError;
use crate::types::{ExecutionResult, ResourceLimits};

/// Execute a program in an Isolate box with batch I/O
#[instrument(skip(sandbox, config, input))]
pub async fn execute(
    sandbox: &IsolateBox,
    config: &Config,
    language: &Language,
    input: Option<&[u8]>,
    limits: Option<&ResourceLimits>,
) -> Result<ExecutionResult, ExecuteError> {
    // Determine effective limits: config defaults → language run limits → user overrides
    let mut effective_limits = config.default_limits.clone();
    if let Some(ref lang_limits) = language.run.limits {
        effective_limits = effective_limits.with_overrides(lang_limits);
    }
    if let Some(user_limits) = limits {
        effective_limits = effective_limits.with_overrides(user_limits);
    }

    // Determine the command based on whether it's compiled or interpreted
    let mut run_cmd = if let Some(ref compile_config) = language.compile {
        // Compiled language - use the binary
        let binary = &compile_config.output_name;

        // Check if binary exists
        if !sandbox.file_exists(binary).await? {
            return Err(ExecuteError::NotStarted(format!(
                "binary '{}' not found in sandbox - was compilation run?",
                binary
            )));
        }

        Language::expand_command(&language.run.command, &compile_config.source_name, binary)
    } else {
        // Interpreted language - source should already be in sandbox
        let source_name = language.source_name();

        // Check if source exists
        if !sandbox.file_exists(&source_name).await? {
            return Err(ExecuteError::NotStarted(format!(
                "source '{}' not found in sandbox - write source first",
                source_name
            )));
        }

        Language::expand_command(&language.run.command, &source_name, &source_name)
    };

    // Resolve command path (isolate uses execve, not execvp)
    resolve_command(&mut run_cmd).map_err(ExecuteError::Isolate)?;

    debug!(?run_cmd, "executing program");

    // Validate mount source paths exist before running
    validate_mounts(&language.run.mounts).map_err(ExecuteError::Isolate)?;

    // Save memory limit before effective_limits is moved
    let memory_limit = effective_limits.memory_limit;

    // Build execute command
    let mut command = IsolateCommand::new(config.isolate_binary(), sandbox.id())
        .action(IsolateAction::Run)
        .cgroup(config.cgroup)
        .limits(effective_limits)
        .working_dir("/box")
        .env("PATH", &language.run.path)
        .mounts(config.sandbox_mounts.iter().cloned())
        .mounts(language.run.mounts.iter().cloned())
        .command(run_cmd);

    // Add environment variables from language config
    for (key, value) in &language.run.env {
        command = command.env(key, value);
    }

    // Run the program
    let mut result = run_batch(sandbox, command, input)
        .await
        .map_err(ExecuteError::Isolate)?;

    if let Some(mem_limit) = memory_limit {
        result.detect_memory_limit(mem_limit);
    }

    debug!(
        status = ?result.status,
        time = result.time,
        memory = result.memory,
        exit_code = ?result.exit_code,
        "execution complete"
    );

    Ok(result)
}

/// Execute an interpreted program by writing source and running
#[instrument(skip(sandbox, config, source, input))]
pub async fn execute_interpreted(
    sandbox: &IsolateBox,
    config: &Config,
    language: &Language,
    source: &[u8],
    input: Option<&[u8]>,
    limits: Option<&ResourceLimits>,
) -> Result<ExecutionResult, ExecuteError> {
    // Write source file
    let source_name = language.source_name();
    sandbox
        .write_file(&source_name, source)
        .await
        .map_err(ExecuteError::Isolate)?;

    debug!(source_name, "wrote source file for interpreted execution");

    // Execute
    execute(sandbox, config, language, input, limits).await
}
