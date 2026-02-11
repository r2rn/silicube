//! Box lifecycle management for isolate
//!
//! Manages the initialization, use, and cleanup of Isolate sandbox boxes.

use std::path::{Path, PathBuf};

use tokio::process::Command;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, instrument, warn};

use crate::isolate::IsolateError;
use crate::isolate::command::{IsolateAction, IsolateCommand};

/// An Isolate sandbox
///
/// Represents an initialized isolate box that can be used to run sandboxed code.
///
/// # Cleanup
///
/// **Important:** Always call [`cleanup()`](Self::cleanup) explicitly before dropping
/// the box. The `Drop` implementation attempts best-effort cleanup via a spawned
/// thread, but this is unreliable and may not complete before process exit.
///
/// ```rust,ignore
/// let mut sandbox = IsolateBox::init(0, "isolate").await?;
/// // ... use the sandbox ...
/// sandbox.cleanup().await?; // Always cleanup explicitly!
/// ```
///
/// If you want RAII-style cleanup, consider using [`into_guard()`](Self::into_guard)
/// which returns a guard that will log a warning if dropped without explicit cleanup.
#[derive(Debug)]
pub struct IsolateBox {
    /// Box ID
    id: u32,

    /// Path to the box directory
    box_path: PathBuf,

    /// Path to the isolate binary
    isolate_path: PathBuf,

    /// Whether the box is initialized
    initialized: bool,

    /// Whether cgroup support is enabled
    cgroup: bool,

    /// Pool permit (if acquired from a pool)
    _permit: Option<OwnedSemaphorePermit>,
}

impl IsolateBox {
    /// Initialize a new isolate box
    #[instrument(skip(isolate_path))]
    pub async fn init(
        id: u32,
        isolate_path: impl Into<PathBuf>,
        cgroup: bool,
    ) -> Result<Self, IsolateError> {
        let isolate_path = isolate_path.into();

        // Run `isolate --init`
        let cmd = IsolateCommand::new(&isolate_path, id)
            .action(IsolateAction::Init)
            .cgroup(cgroup);
        let args = cmd.build();

        debug!(?args, "initializing isolate box");

        let program = args
            .first()
            .ok_or_else(|| IsolateError::CommandFailed("empty command arguments".to_string()))?;
        let output = Command::new(program)
            .args(&args[1..])
            .output()
            .await
            .map_err(IsolateError::SpawnFailed)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IsolateError::InitFailed {
                id,
                message: stderr.to_string(),
            });
        }

        // Parse box path from stdout
        let stdout = String::from_utf8_lossy(&output.stdout);
        let box_path = PathBuf::from(stdout.trim());

        if !box_path.exists() {
            return Err(IsolateError::InitFailed {
                id,
                message: format!("box path does not exist: {}", box_path.display()),
            });
        }

        debug!(?box_path, "box initialized");

        Ok(Self {
            id,
            box_path,
            isolate_path,
            initialized: true,
            cgroup,
            _permit: None,
        })
    }

    /// Get the box ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get the path to the box directory (where files can be placed)
    pub fn path(&self) -> &Path {
        &self.box_path
    }

    /// Get the host path to a file inside the box
    ///
    /// Returns an error if the path contains path traversal attempts.
    pub fn file_path(&self, name: &str) -> Result<PathBuf, IsolateError> {
        // Reject path traversal attempts
        if name.contains("..") || name.starts_with('/') {
            return Err(IsolateError::InvalidPath(format!(
                "path traversal not allowed: {}",
                name
            )));
        }
        Ok(self.box_path.join("box").join(name))
    }

    /// Get the sandbox-internal path for a file inside the box
    ///
    /// Returns the path as seen from inside the isolate sandbox, where the box
    /// directory is mounted at `/box/`. Use this for isolate `--stdin`,
    /// `--stdout`, and `--stderr` flags which are opened inside the sandbox.
    pub fn sandbox_path(&self, name: &str) -> Result<PathBuf, IsolateError> {
        if name.contains("..") || name.starts_with('/') {
            return Err(IsolateError::InvalidPath(format!(
                "path traversal not allowed: {}",
                name
            )));
        }
        Ok(PathBuf::from("/box").join(name))
    }

    /// Get the path to the isolate binary
    pub fn isolate_path(&self) -> &Path {
        &self.isolate_path
    }

    /// Write a file into the box
    #[instrument(skip(self, content))]
    pub async fn write_file(&self, name: &str, content: &[u8]) -> Result<(), IsolateError> {
        let path = self.file_path(name)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&path, content).await?;
        debug!(?path, len = content.len(), "wrote file to box");
        Ok(())
    }

    /// Read a file from the box
    #[instrument(skip(self))]
    pub async fn read_file(&self, name: &str) -> Result<Vec<u8>, IsolateError> {
        let path = self.file_path(name)?;
        let content = tokio::fs::read(&path).await?;
        debug!(?path, len = content.len(), "read file from box");
        Ok(content)
    }

    /// Check if a file exists in the box
    pub async fn file_exists(&self, name: &str) -> Result<bool, IsolateError> {
        let path = self.file_path(name)?;
        Ok(tokio::fs::metadata(&path).await.is_ok())
    }

    /// Clean up the box
    ///
    /// This method should always be called before dropping the box to ensure
    /// proper resource cleanup. The return value indicates whether cleanup
    /// succeeded and should be checked.
    ///
    /// # Errors
    ///
    /// Returns an error if the isolate cleanup command fails.
    #[must_use = "cleanup errors should be handled"]
    #[instrument(skip(self))]
    pub async fn cleanup(&mut self) -> Result<(), IsolateError> {
        if !self.initialized {
            return Ok(());
        }

        let cmd = IsolateCommand::new(&self.isolate_path, self.id)
            .action(IsolateAction::Cleanup)
            .cgroup(self.cgroup);
        let args = cmd.build();

        debug!(?args, "cleaning up isolate box");

        let program = args
            .first()
            .ok_or_else(|| IsolateError::CommandFailed("empty command arguments".to_string()))?;
        let output = Command::new(program)
            .args(&args[1..])
            .output()
            .await
            .map_err(IsolateError::SpawnFailed)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(id = self.id, stderr = %stderr, "cleanup failed");
            return Err(IsolateError::CleanupFailed {
                id: self.id,
                message: stderr.to_string(),
            });
        }

        self.initialized = false;
        debug!("box cleaned up");
        Ok(())
    }

    /// Attach a pool permit to this box
    pub(crate) fn with_permit(mut self, permit: OwnedSemaphorePermit) -> Self {
        self._permit = Some(permit);
        self
    }

    /// Check if the box is still initialized (not yet cleaned up)
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Drop for IsolateBox {
    fn drop(&mut self) {
        if self.initialized {
            // Log a warning - best-effort cleanup in Drop is unreliable
            // Callers should explicitly call cleanup() before dropping
            warn!(
                box_id = self.id,
                box_path = %self.box_path.display(),
                "IsolateBox dropped without explicit cleanup! \
                 Call cleanup() before dropping to ensure proper resource release. \
                 Attempting best-effort cleanup via spawned thread (may not complete)."
            );

            // Try to cleanup synchronously via a blocking thread
            // This is best-effort cleanup on drop - the thread may not complete
            // before process exit, leading to leaked sandbox resources
            let isolate_path = self.isolate_path.clone();
            let id = self.id;
            let cgroup = self.cgroup;

            std::thread::spawn(move || {
                let cmd = IsolateCommand::new(&isolate_path, id)
                    .action(IsolateAction::Cleanup)
                    .cgroup(cgroup);
                let args = cmd.build();

                if let Some(program) = args.first() {
                    match std::process::Command::new(program)
                        .args(&args[1..])
                        .output()
                    {
                        Ok(output) if output.status.success() => {
                            debug!(box_id = id, "best-effort cleanup succeeded");
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            warn!(
                                box_id = id,
                                stderr = %stderr,
                                "best-effort cleanup failed"
                            );
                        }
                        Err(e) => {
                            warn!(box_id = id, error = %e, "best-effort cleanup spawn failed");
                        }
                    }
                }
            });
        }
    }
}

/// Pool of isolate boxes for concurrent execution
#[derive(Debug)]
pub struct BoxPool {
    /// Starting box ID
    start_id: u32,

    /// Number of boxes in the pool
    count: u32,

    /// Path to the isolate binary
    isolate_path: PathBuf,

    /// Whether cgroup support is enabled
    cgroup: bool,

    /// Semaphore to limit concurrent boxes
    semaphore: std::sync::Arc<Semaphore>,

    /// Next box ID to use (wraps around)
    next_id: std::sync::atomic::AtomicU32,
}

impl BoxPool {
    /// Create a new box pool
    pub fn new(start_id: u32, count: u32, isolate_path: impl Into<PathBuf>, cgroup: bool) -> Self {
        Self {
            start_id,
            count,
            isolate_path: isolate_path.into(),
            cgroup,
            semaphore: std::sync::Arc::new(Semaphore::new(count as usize)),
            next_id: std::sync::atomic::AtomicU32::new(start_id),
        }
    }

    /// Acquire a box from the pool
    #[instrument(skip(self))]
    pub async fn acquire(&self) -> Result<IsolateBox, IsolateError> {
        // Wait for a permit
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| IsolateError::PoolExhausted)?;

        // Get next box ID
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let id = self.start_id + (id - self.start_id) % self.count;

        debug!(id, "acquired box from pool");

        // Initialize the box
        let sandbox = IsolateBox::init(id, &self.isolate_path, self.cgroup).await?;

        Ok(sandbox.with_permit(permit))
    }

    /// Get the number of available boxes
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get the total number of boxes in the pool
    pub fn capacity(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require the isolate binary and root privileges.
    // Run with: cargo test --features integration-tests -- --include-ignored

    #[tokio::test]
    #[cfg(feature = "integration-tests")]
    #[ignore = "requires root"]
    async fn test_box_init_cleanup() {
        let mut sandbox = IsolateBox::init(99, "isolate", false).await.unwrap();
        assert!(sandbox.path().exists());
        sandbox.cleanup().await.unwrap();
    }

    #[tokio::test]
    #[cfg(feature = "integration-tests")]
    #[ignore = "requires root"]
    async fn test_box_file_operations() {
        let mut sandbox = IsolateBox::init(98, "isolate", false).await.unwrap();

        sandbox
            .write_file("test.txt", b"hello world")
            .await
            .unwrap();
        let content = sandbox.read_file("test.txt").await.unwrap();
        assert_eq!(content, b"hello world");

        sandbox.cleanup().await.unwrap();
    }

    #[test]
    fn test_file_path_validation() {
        // Create a mock IsolateBox for path validation testing
        let sandbox = IsolateBox {
            id: 0,
            box_path: std::path::PathBuf::from("/tmp/box0"),
            isolate_path: std::path::PathBuf::from("isolate"),
            initialized: false,
            cgroup: false,
            _permit: None,
        };

        // Valid paths should work
        assert!(sandbox.file_path("main.cpp").is_ok());
        assert!(sandbox.file_path("subdir/file.txt").is_ok());

        // Path traversal should be rejected
        assert!(sandbox.file_path("../escape").is_err());
        assert!(sandbox.file_path("foo/../bar").is_err());
        assert!(sandbox.file_path("/absolute/path").is_err());
    }

    #[test]
    fn test_sandbox_path() {
        let sandbox = IsolateBox {
            id: 0,
            box_path: std::path::PathBuf::from("/var/local/lib/isolate/0"),
            isolate_path: std::path::PathBuf::from("isolate"),
            initialized: false,
            cgroup: false,
            _permit: None,
        };

        assert_eq!(
            sandbox.sandbox_path("stdin.txt").unwrap(),
            PathBuf::from("/box/stdin.txt")
        );
        assert_eq!(
            sandbox.sandbox_path("compile_stderr.txt").unwrap(),
            PathBuf::from("/box/compile_stderr.txt")
        );

        // Path traversal should be rejected
        assert!(sandbox.sandbox_path("../escape").is_err());
        assert!(sandbox.sandbox_path("/absolute/path").is_err());
    }
}
