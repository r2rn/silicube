//! Code runner for Silicube
//!
//! Provides high-level APIs for compiling and executing code in isolate sandboxes.

use thiserror::Error;

pub use crate::runner::compile::{CompileResult, compile};
pub use crate::runner::execute::{execute, execute_interpreted};
pub use crate::runner::interactive::{
    InteractiveEvent, InteractiveEventStream, InteractiveSession, InteractiveSessionHandle,
};

mod compile;
mod execute;
mod interactive;

use crate::{
    config::{Config, Language},
    isolate::{IsolateBox, IsolateError},
    types::{ExecutionResult, ResourceLimits},
};

/// Request for compiling and running code in one step
#[derive(Debug)]
pub struct CompileAndRunRequest<'a> {
    /// The isolate sandbox to use
    pub sandbox: &'a IsolateBox,
    /// Source code to compile
    pub source: &'a [u8],
    /// Optional input to provide to the program
    pub input: Option<&'a [u8]>,
    /// Language configuration
    pub language: &'a Language,
    /// Optional resource limits for compilation
    pub compile_limits: Option<&'a ResourceLimits>,
    /// Optional resource limits for execution
    pub run_limits: Option<&'a ResourceLimits>,
}

/// Errors that occur during compilation
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("compilation failed with exit code {exit_code}: {stderr}")]
    Failed { exit_code: i32, stderr: String },

    #[error("compilation timed out")]
    Timeout,

    #[error("language '{0}' does not support compilation")]
    NotCompiled(String),

    #[error("isolate error: {0}")]
    Isolate(#[from] IsolateError),
}

/// Errors that occur during execution
#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("execution not started: {0}")]
    NotStarted(String),

    #[error("isolate error: {0}")]
    Isolate(#[from] IsolateError),
}

/// Errors that occur during interactive sessions
#[derive(Debug, Error)]
pub enum InteractiveError {
    #[error("session not started")]
    NotStarted,

    #[error("session already terminated")]
    Terminated,

    #[error("failed to create FIFO: {0}")]
    FifoCreation(#[source] std::io::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("isolate error: {0}")]
    Isolate(#[from] IsolateError),
}

/// Errors that occur during compile-and-run operations
///
/// This error type preserves the full context of whether the error
/// occurred during compilation or execution.
#[derive(Debug, Error)]
pub enum CompileAndRunError {
    /// Error during compilation phase
    #[error("compilation error: {0}")]
    Compile(#[from] CompileError),

    /// Error during execution phase (compilation succeeded)
    #[error("execution error: {0}")]
    Execute(#[from] ExecuteError),
}

/// High-level runner for code execution
#[derive(Debug, Clone)]
pub struct Runner {
    config: Config,
}

impl Runner {
    /// Create a new runner with the given configuration
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Create a new runner with default configuration
    pub fn with_defaults() -> Self {
        Self {
            config: Config::default(),
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Compile source code
    pub async fn compile(
        &self,
        sandbox: &IsolateBox,
        source: &[u8],
        language: &Language,
        limits: Option<&ResourceLimits>,
    ) -> Result<CompileResult, CompileError> {
        compile::compile(sandbox, &self.config, language, source, limits).await
    }

    /// Run a program with batch I/O
    pub async fn run(
        &self,
        sandbox: &IsolateBox,
        input: Option<&[u8]>,
        language: &Language,
        limits: Option<&ResourceLimits>,
    ) -> Result<ExecutionResult, ExecuteError> {
        execute::execute(sandbox, &self.config, language, input, limits).await
    }

    /// Run an interpreted program (writes source and executes)
    pub async fn run_interpreted(
        &self,
        sandbox: &IsolateBox,
        source: &[u8],
        input: Option<&[u8]>,
        language: &Language,
        limits: Option<&ResourceLimits>,
    ) -> Result<ExecutionResult, ExecuteError> {
        execute::execute_interpreted(sandbox, &self.config, language, source, input, limits).await
    }

    /// Start an interactive session
    pub async fn run_interactive(
        &self,
        sandbox: &IsolateBox,
        language: &Language,
        limits: Option<&ResourceLimits>,
    ) -> Result<InteractiveSession, InteractiveError> {
        InteractiveSession::start(sandbox, &self.config, language, limits).await
    }

    /// Compile and run in one step (for compiled languages)
    ///
    /// Returns a tuple of (compile_result, optional_run_result). If compilation
    /// fails, the run result will be `None`.
    ///
    /// # Errors
    ///
    /// Returns [`CompileAndRunError::Compile`] if compilation fails, or
    /// [`CompileAndRunError::Execute`] if compilation succeeds but execution fails.
    /// The error type preserves full context about which phase failed.
    pub async fn compile_and_run(
        &self,
        request: CompileAndRunRequest<'_>,
    ) -> Result<(CompileResult, Option<ExecutionResult>), CompileAndRunError> {
        // Compile first
        let compile_result = self
            .compile(
                request.sandbox,
                request.source,
                request.language,
                request.compile_limits,
            )
            .await?;

        // Only run if compilation succeeded
        if compile_result.success {
            let run_result = self
                .run(
                    request.sandbox,
                    request.input,
                    request.language,
                    request.run_limits,
                )
                .await?;
            Ok((compile_result, Some(run_result)))
        } else {
            Ok((compile_result, None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_creation() {
        let runner = Runner::with_defaults();
        // Default config includes languages from embedded silicube.example.toml
        assert!(runner.config().languages.contains_key("cpp17"));
        assert!(runner.config().languages.contains_key("python3"));
    }
}
