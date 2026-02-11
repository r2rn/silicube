//! Process spawning and I/O for Isolate
//!
//! Handles running commands inside Isolate and capturing output.

use std::path::Path;
use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, instrument};

use crate::isolate::IsolateError;
use crate::isolate::box_manager::IsolateBox;
use crate::isolate::command::IsolateCommand;
use crate::isolate::meta::MetaFile;
use crate::types::ExecutionResult;

/// Run an isolate command and parse the meta file result
async fn run_isolate_command(
    args: Vec<String>,
    meta_path: &Path,
) -> Result<(std::process::Output, MetaFile), IsolateError> {
    let program = args
        .first()
        .ok_or_else(|| IsolateError::CommandFailed("empty command arguments".to_string()))?;

    let output = Command::new(program)
        .args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(IsolateError::SpawnFailed)?;

    // Parse meta file
    let meta = if meta_path.exists() {
        MetaFile::load(meta_path).await?
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(IsolateError::CommandFailed(stderr.to_string()));
    };

    Ok((output, meta))
}

/// Run a command in an Isolate box with batch I/O
///
/// Runs the command with non-interactive I/O. The input is given once via
/// stdin.txt and the result from stdout and stderr is captured into their
/// respective files.
#[instrument(skip(sandbox, stdin_data))]
pub async fn run_batch(
    sandbox: &IsolateBox,
    command: IsolateCommand,
    stdin_data: Option<&[u8]>,
) -> Result<ExecutionResult, IsolateError> {
    // Host paths (for meta file and reading back results)
    let meta_path = sandbox.file_path("meta.txt")?;
    let stdout_host_path = sandbox.file_path("stdout.txt")?;
    let stderr_host_path = sandbox.file_path("stderr.txt")?;

    // Sandbox-internal paths (for isolate --stdin/--stdout/--stderr, opened inside the sandbox)
    let stdin_sandbox_path = sandbox.sandbox_path("stdin.txt")?;
    let stdout_sandbox_path = sandbox.sandbox_path("stdout.txt")?;
    let stderr_sandbox_path = sandbox.sandbox_path("stderr.txt")?;

    // Write stdin data if provided.
    // Isolate requires a stdin file even if empty - it cannot read from /dev/null
    // when --stdin is specified, so we always create the file.
    if let Some(data) = stdin_data {
        sandbox.write_file("stdin.txt", data).await?;
    } else {
        sandbox.write_file("stdin.txt", b"").await?;
    }

    // Configure command with I/O files
    let command = command
        .meta_file(&meta_path)
        .stdin(&stdin_sandbox_path)
        .stdout(&stdout_sandbox_path)
        .stderr(&stderr_sandbox_path);

    let args = command.build();
    debug!(?args, "running isolate command");

    // Run the command
    let (_output, meta) = run_isolate_command(args, &meta_path).await?;

    let mut result = meta.to_execution_result();

    // Read stdout/stderr via host paths
    if stdout_host_path.exists() {
        result.stdout = Some(tokio::fs::read(&stdout_host_path).await?);
    }
    if stderr_host_path.exists() {
        result.stderr = Some(tokio::fs::read(&stderr_host_path).await?);
    }

    debug!(
        status = ?result.status,
        time = result.time,
        memory = result.memory,
        "execution complete"
    );

    Ok(result)
}

/// Run a command and capture output (for compilation feedback)
///
/// Used for compiling programs. Writes stdout and stderr outputs to
/// compilation-specific output files.
#[instrument(skip(sandbox))]
pub async fn run_with_output(
    sandbox: &IsolateBox,
    command: IsolateCommand,
) -> Result<(ExecutionResult, String), IsolateError> {
    // Host paths (for meta file and reading back results)
    let meta_path = sandbox.file_path("meta.txt")?;
    let stdout_host_path = sandbox.file_path("compile_stdout.txt")?;
    let stderr_host_path = sandbox.file_path("compile_stderr.txt")?;

    // Sandbox-internal paths (for isolate --stdin/--stdout/--stderr, opened inside the sandbox)
    let stdin_sandbox_path = sandbox.sandbox_path("compile_stdin.txt")?;
    let stdout_sandbox_path = sandbox.sandbox_path("compile_stdout.txt")?;
    let stderr_sandbox_path = sandbox.sandbox_path("compile_stderr.txt")?;

    // Write empty stdin - isolate requires a stdin file when --stdin is specified
    sandbox.write_file("compile_stdin.txt", b"").await?;

    let command = command
        .meta_file(&meta_path)
        .stdin(&stdin_sandbox_path)
        .stdout(&stdout_sandbox_path)
        .stderr(&stderr_sandbox_path);

    let args = command.build();
    debug!(?args, "running compile command");

    let (_output, meta) = run_isolate_command(args, &meta_path).await?;

    let result = meta.to_execution_result();

    // Combine stdout and stderr for compiler output (read via host paths)
    let mut compiler_output = String::new();
    if stdout_host_path.exists() {
        let stdout = tokio::fs::read_to_string(&stdout_host_path).await?;
        compiler_output.push_str(&stdout);
    }
    if stderr_host_path.exists() {
        let stderr = tokio::fs::read_to_string(&stderr_host_path).await?;
        if !compiler_output.is_empty() && !stderr.is_empty() {
            compiler_output.push('\n');
        }
        compiler_output.push_str(&stderr);
    }

    Ok((result, compiler_output))
}

/// Process handle for interactive execution
#[derive(Debug)]
pub struct IsolateProcess {
    child: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    meta_path: std::path::PathBuf,
}

impl IsolateProcess {
    /// Spawn a new isolate process
    #[instrument(skip(sandbox))]
    pub async fn spawn(
        sandbox: &IsolateBox,
        command: IsolateCommand,
    ) -> Result<Self, IsolateError> {
        let meta_path = sandbox.file_path("interactive_meta.txt")?;

        let command = command.meta_file(&meta_path);
        let args = command.build();

        debug!(?args, "spawning interactive isolate process");

        let program = args
            .first()
            .ok_or_else(|| IsolateError::CommandFailed("empty command arguments".to_string()))?;
        let mut child = Command::new(program)
            .args(&args[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(IsolateError::SpawnFailed)?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        Ok(Self {
            child,
            stdin,
            stdout,
            stderr,
            meta_path,
        })
    }

    /// Write to the process stdin
    pub async fn write(&mut self, data: &[u8]) -> Result<(), IsolateError> {
        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(data).await?;
            stdin.flush().await?;
            Ok(())
        } else {
            Err(IsolateError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stdin closed",
            )))
        }
    }

    /// Close stdin
    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Get the stdout handle
    pub fn stdout(&mut self) -> Option<&mut tokio::process::ChildStdout> {
        self.stdout.as_mut()
    }

    /// Get the stderr handle
    pub fn stderr(&mut self) -> Option<&mut tokio::process::ChildStderr> {
        self.stderr.as_mut()
    }

    /// Take ownership of stdout
    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.stdout.take()
    }

    /// Take ownership of stderr
    pub fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.stderr.take()
    }

    /// Wait for the process to exit and get the result
    pub async fn wait(mut self) -> Result<ExecutionResult, IsolateError> {
        // Close stdin to signal EOF
        self.stdin = None;

        // Wait for process
        let _ = self.child.wait().await?;

        // Parse meta file
        let meta = if self.meta_path.exists() {
            MetaFile::load(&self.meta_path).await?
        } else {
            return Err(IsolateError::CommandFailed(
                "no meta file produced".to_string(),
            ));
        };

        Ok(meta.to_execution_result())
    }

    /// Kill the process
    pub async fn kill(&mut self) -> Result<(), IsolateError> {
        self.child.kill().await?;
        Ok(())
    }

    /// Try to get the result without waiting (non-blocking)
    pub fn try_wait(&mut self) -> Result<Option<()>, IsolateError> {
        match self.child.try_wait()? {
            Some(_) => Ok(Some(())),
            None => Ok(None),
        }
    }
}
