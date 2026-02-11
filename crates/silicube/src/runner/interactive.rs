//! Interactive I/O handling for code execution
//!
//! Provides FIFO-based interactive sessions for programs that require
//! back-and-forth communication (e.g., interactive problems, REPLs).

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout};
use tokio::sync::{Notify, mpsc};
use tracing::{debug, instrument, warn};

use crate::config::{Config, Language};
use crate::isolate::{
    IsolateAction, IsolateBox, IsolateCommand, IsolateProcess, resolve_command, validate_mounts,
};
use crate::runner::InteractiveError;
use crate::types::{ExecutionResult, ResourceLimits};

/// Event from an interactive session
#[derive(Debug, Clone)]
pub enum InteractiveEvent {
    /// Data received on stdout
    Stdout(Vec<u8>),

    /// Data received on stderr
    Stderr(Vec<u8>),

    /// A complete line was received on stdout
    StdoutLine(String),

    /// A complete line was received on stderr
    StderrLine(String),

    /// The process exited
    Exited(ExecutionResult),
}

/// An interactive execution session
pub struct InteractiveSession {
    process: IsolateProcess,
    /// Buffered reader for stdout - stored to preserve buffered data between reads
    stdout_reader: Option<BufReader<ChildStdout>>,
    /// Buffered reader for stderr - stored to preserve buffered data between reads
    stderr_reader: Option<BufReader<ChildStderr>>,
    terminated: bool,
}

impl InteractiveSession {
    /// Start a new interactive session
    #[instrument(skip(sandbox, config))]
    pub async fn start(
        sandbox: &IsolateBox,
        config: &Config,
        language: &Language,
        limits: Option<&ResourceLimits>,
    ) -> Result<Self, InteractiveError> {
        // Determine effective limits: config defaults → language run limits → user overrides
        let mut effective_limits = config.default_limits.clone();
        if let Some(ref lang_limits) = language.run.limits {
            effective_limits = effective_limits.with_overrides(lang_limits);
        }
        if let Some(user_limits) = limits {
            effective_limits = effective_limits.with_overrides(user_limits);
        }

        // Determine command
        let (mut run_cmd, _source_name) = if let Some(ref compile_config) = language.compile {
            let binary = &compile_config.output_name;
            (
                Language::expand_command(
                    &language.run.command,
                    &compile_config.source_name,
                    binary,
                ),
                compile_config.source_name.clone(),
            )
        } else {
            let source_name = language.source_name();
            (
                Language::expand_command(&language.run.command, &source_name, &source_name),
                source_name,
            )
        };

        // Resolve command path (isolate uses execve, not execvp)
        resolve_command(&mut run_cmd).map_err(InteractiveError::Isolate)?;

        debug!(?run_cmd, "starting interactive session");

        // Validate mount source paths exist before running
        validate_mounts(&language.run.mounts).map_err(InteractiveError::Isolate)?;

        // Build command
        let mut command = IsolateCommand::new(config.isolate_binary(), sandbox.id())
            .action(IsolateAction::Run)
            .cgroup(config.cgroup)
            .limits(effective_limits)
            .working_dir("/box")
            .env("PATH", &language.run.path)
            .mounts(config.sandbox_mounts.iter().cloned())
            .mounts(language.run.mounts.iter().cloned())
            .command(run_cmd);

        for (key, value) in &language.run.env {
            command = command.env(key, value);
        }

        // Spawn process
        let mut process = IsolateProcess::spawn(sandbox, command)
            .await
            .map_err(InteractiveError::Isolate)?;

        // Take ownership of stdout/stderr and wrap in buffered readers
        let stdout_reader = process.take_stdout().map(BufReader::new);
        let stderr_reader = process.take_stderr().map(BufReader::new);

        Ok(Self {
            process,
            stdout_reader,
            stderr_reader,
            terminated: false,
        })
    }

    /// Write data to the process stdin
    pub async fn write(&mut self, data: &[u8]) -> Result<(), InteractiveError> {
        if self.terminated {
            return Err(InteractiveError::Terminated);
        }

        self.process
            .write(data)
            .await
            .map_err(InteractiveError::Isolate)?;

        debug!(len = data.len(), "wrote to stdin");
        Ok(())
    }

    /// Write a line to the process stdin (adds newline)
    pub async fn write_line(&mut self, line: &str) -> Result<(), InteractiveError> {
        let mut data = line.as_bytes().to_vec();
        data.push(b'\n');
        self.write(&data).await
    }

    /// Close stdin to signal EOF
    pub fn close_stdin(&mut self) {
        self.process.close_stdin();
        debug!("closed stdin");
    }

    /// Read available data from stdout
    pub async fn read_stdout(&mut self, buf: &mut [u8]) -> Result<usize, InteractiveError> {
        if self.terminated {
            return Ok(0);
        }

        if let Some(ref mut reader) = self.stdout_reader {
            let n = reader.read(buf).await?;
            Ok(n)
        } else {
            Ok(0)
        }
    }

    /// Read available data from stderr
    pub async fn read_stderr(&mut self, buf: &mut [u8]) -> Result<usize, InteractiveError> {
        if self.terminated {
            return Ok(0);
        }

        if let Some(ref mut reader) = self.stderr_reader {
            let n = reader.read(buf).await?;
            Ok(n)
        } else {
            Ok(0)
        }
    }

    /// Read a line from stdout
    ///
    /// The internal BufReader is preserved between calls, so buffered data
    /// is not lost.
    pub async fn read_line(&mut self) -> Result<Option<String>, InteractiveError> {
        if self.terminated {
            return Ok(None);
        }

        if let Some(ref mut reader) = self.stdout_reader {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => Ok(None),
                Ok(_) => {
                    // Remove trailing newline
                    if line.ends_with('\n') {
                        line.pop();
                        if line.ends_with('\r') {
                            line.pop();
                        }
                    }
                    Ok(Some(line))
                }
                Err(e) => Err(InteractiveError::Io(e)),
            }
        } else {
            Ok(None)
        }
    }

    /// Check if the process has terminated
    pub fn is_terminated(&mut self) -> bool {
        if self.terminated {
            return true;
        }

        match self.process.try_wait() {
            Ok(Some(())) => {
                self.terminated = true;
                true
            }
            Ok(None) => false,
            Err(_) => {
                self.terminated = true;
                true
            }
        }
    }

    /// Wait for the process to exit and get the result
    pub async fn wait(mut self) -> Result<ExecutionResult, InteractiveError> {
        if self.terminated {
            return Err(InteractiveError::Terminated);
        }

        self.terminated = true;
        self.process.wait().await.map_err(InteractiveError::Isolate)
    }

    /// Kill the process
    pub async fn kill(&mut self) -> Result<(), InteractiveError> {
        if !self.terminated {
            self.process
                .kill()
                .await
                .map_err(InteractiveError::Isolate)?;
            self.terminated = true;
        }
        Ok(())
    }

    /// Wait for the process with a timeout
    pub async fn wait_timeout(
        self,
        timeout: Duration,
    ) -> Result<ExecutionResult, InteractiveError> {
        match tokio::time::timeout(timeout, self.wait()).await {
            Ok(result) => result,
            Err(_) => Err(InteractiveError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "wait timed out",
            ))),
        }
    }
}

/// Stream events from an interactive session
pub struct InteractiveEventStream {
    rx: mpsc::Receiver<InteractiveEvent>,
    _handle: tokio::task::JoinHandle<()>,
}

impl InteractiveEventStream {
    /// Create an event stream from a session
    ///
    /// The event stream spawns a background task that reads from stdout and
    /// signals when the process terminates. Uses `Notify` for efficient
    /// termination detection instead of polling.
    pub fn new(mut session: InteractiveSession) -> (Self, InteractiveSessionHandle) {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<Vec<u8>>(100);

        // Notify for signaling termination - more efficient than polling
        let termination_notify = Arc::new(Notify::new());
        let termination_notify_clone = termination_notify.clone();

        let handle = tokio::spawn(async move {
            let mut stdout_buf = vec![0u8; 4096];
            let mut stdout_closed = false;

            loop {
                tokio::select! {
                    biased;

                    // Handle stdin writes - prioritize writes
                    Some(data) = stdin_rx.recv() => {
                        if let Err(e) = session.write(&data).await {
                            warn!(?e, "failed to write to stdin");
                            break;
                        }
                    }

                    // Read stdout (only if not closed)
                    result = session.read_stdout(&mut stdout_buf), if !stdout_closed => {
                        match result {
                            Ok(0) => {
                                // EOF - stdout closed, process likely terminating
                                stdout_closed = true;
                                termination_notify_clone.notify_one();
                            }
                            Ok(n) => {
                                let _ = event_tx.send(InteractiveEvent::Stdout(
                                    stdout_buf[..n].to_vec()
                                )).await;
                            }
                            Err(e) => {
                                warn!(?e, "stdout read error");
                                stdout_closed = true;
                                termination_notify_clone.notify_one();
                            }
                        }
                    }

                    // Wait for termination signal
                    _ = termination_notify.notified(), if stdout_closed => {
                        // Check if process terminated
                        if session.is_terminated() {
                            match session.wait().await {
                                Ok(result) => {
                                    let _ = event_tx.send(InteractiveEvent::Exited(result)).await;
                                }
                                Err(e) => {
                                    warn!(?e, "failed to get exit result");
                                }
                            }
                            break;
                        }
                    }
                }
            }
        });

        let stream = Self {
            rx: event_rx,
            _handle: handle,
        };

        let session_handle = InteractiveSessionHandle { stdin_tx };

        (stream, session_handle)
    }

    /// Receive the next event
    pub async fn recv(&mut self) -> Option<InteractiveEvent> {
        self.rx.recv().await
    }
}

/// Handle for writing to an interactive session
#[derive(Clone)]
pub struct InteractiveSessionHandle {
    stdin_tx: mpsc::Sender<Vec<u8>>,
}

impl InteractiveSessionHandle {
    /// Write data to stdin
    pub async fn write(&self, data: &[u8]) -> Result<(), InteractiveError> {
        self.stdin_tx
            .send(data.to_vec())
            .await
            .map_err(|_| InteractiveError::Terminated)
    }

    /// Write a line to stdin
    pub async fn write_line(&self, line: &str) -> Result<(), InteractiveError> {
        let mut data = line.as_bytes().to_vec();
        data.push(b'\n');
        self.write(&data).await
    }
}
