//! Command builder for the Isolate CLI
//!
//! Builds command-line arguments for the Isolate sandbox tool.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::{MountConfig, ResourceLimits};

/// Builder for Isolate command-line arguments
#[derive(Debug)]
pub struct IsolateCommand {
    /// Path to Isolate binary
    isolate_path: PathBuf,
    /// One of --init, --run, --cleanup
    action: IsolateAction,
    /// -b, --box-id
    box_id: u32,
    /// Resource limits
    limits: ResourceLimits,
    mounts: Vec<MountConfig>,
    /// -E, --env
    env: HashMap<String, String>,
    env_inherit: Vec<String>,
    /// -e, --full-env
    full_env: bool,
    /// -M, --meta
    meta_file: Option<PathBuf>,
    /// -i, --stdin
    stdin: Option<PathBuf>,
    /// -o, --stdout
    stdout: Option<PathBuf>,
    /// -r, --stderr
    stderr: Option<PathBuf>,
    working_dir: Option<String>,
    command: Vec<String>,
    cgroup: bool,
}

impl IsolateCommand {
    /// Create a new isolate command builder
    pub fn new(isolate_path: impl Into<PathBuf>, box_id: u32) -> Self {
        Self {
            isolate_path: isolate_path.into(),
            action: IsolateAction::Run,
            box_id,
            limits: ResourceLimits::default(),
            mounts: Vec::new(),
            env: HashMap::new(),
            env_inherit: Vec::new(),
            full_env: false,
            meta_file: None,
            stdin: None,
            stdout: None,
            stderr: None,
            working_dir: None,
            command: Vec::new(),
            cgroup: false,
        }
    }

    /// Set the action to perform
    pub fn action(mut self, action: IsolateAction) -> Self {
        self.action = action;
        self
    }

    /// Set resource limits
    pub fn limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Add a directory mount
    pub fn mount(mut self, mount: MountConfig) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Add multiple directory mounts
    pub fn mounts(mut self, mounts: impl IntoIterator<Item = MountConfig>) -> Self {
        self.mounts.extend(mounts);
        self
    }

    /// Set an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Inherit an environment variable from the parent process
    pub fn env_inherit(mut self, key: impl Into<String>) -> Self {
        self.env_inherit.push(key.into());
        self
    }

    /// Set the meta file path for execution results
    pub fn meta_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.meta_file = Some(path.into());
        self
    }

    /// Set stdin file path
    pub fn stdin(mut self, path: impl Into<PathBuf>) -> Self {
        self.stdin = Some(path.into());
        self
    }

    /// Set stdout file path
    pub fn stdout(mut self, path: impl Into<PathBuf>) -> Self {
        self.stdout = Some(path.into());
        self
    }

    /// Set stderr file path
    pub fn stderr(mut self, path: impl Into<PathBuf>) -> Self {
        self.stderr = Some(path.into());
        self
    }

    /// Set the working directory inside the sandbox
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set the command to run
    pub fn command(mut self, cmd: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.command = cmd.into_iter().map(Into::into).collect();
        self
    }

    /// Enable full environment (inherit all env vars)
    pub fn full_env(mut self, enable: bool) -> Self {
        self.full_env = enable;
        self
    }

    /// Enable cgroup support for memory limiting
    pub fn cgroup(mut self, enable: bool) -> Self {
        self.cgroup = enable;
        self
    }

    /// Build the command-line arguments
    ///
    /// Consumes self to avoid cloning the command vector.
    pub fn build(self) -> Vec<String> {
        let mut args = vec![self.isolate_path.to_string_lossy().into_owned()];

        // Box ID
        args.push(format!("--box-id={}", self.box_id));

        // Cgroup support
        if self.cgroup {
            args.push("--cg".to_string());
        }

        match self.action {
            IsolateAction::Init => {
                args.push("--init".to_string());
            }
            IsolateAction::Cleanup => {
                args.push("--cleanup".to_string());
            }
            IsolateAction::Run => {
                args.push("--run".to_string());

                // Resource limits
                if let Some(time) = self.limits.time_limit {
                    args.push(format!("--time={time}"));
                }
                if let Some(wall_time) = self.limits.wall_time_limit {
                    args.push(format!("--wall-time={wall_time}"));
                }
                if let Some(extra_time) = self.limits.extra_time {
                    args.push(format!("--extra-time={extra_time}"));
                }
                if let Some(memory) = self.limits.memory_limit {
                    if self.cgroup {
                        args.push(format!("--cg-mem={memory}"));
                    } else {
                        args.push(format!("--mem={memory}"));
                    }
                }
                if let Some(stack) = self.limits.stack_limit {
                    args.push(format!("--stack={stack}"));
                }
                if let Some(procs) = self.limits.max_processes {
                    args.push(format!("--processes={procs}"));
                }
                if let Some(fsize) = self.limits.max_output {
                    args.push(format!("--fsize={fsize}"));
                }
                if let Some(open_files) = self.limits.max_open_files {
                    args.push(format!("--open-files={open_files}"));
                }

                // Mounts
                for mount in &self.mounts {
                    // Skip optional mounts whose source doesn't exist
                    if mount.optional && !std::path::Path::new(&mount.source).exists() {
                        continue;
                    }
                    let mut opts = String::new();
                    if mount.writable {
                        opts.push_str(":rw");
                    }
                    if mount.optional {
                        opts.push_str(":maybe");
                    }
                    args.push(format!("--dir={}={}{}", mount.target, mount.source, opts));
                }

                // Environment
                if self.full_env {
                    args.push("--full-env".to_string());
                }
                for (key, value) in &self.env {
                    args.push(format!("--env={key}={value}"));
                }
                for key in &self.env_inherit {
                    args.push(format!("--env={key}"));
                }

                // Meta file
                if let Some(ref meta) = self.meta_file {
                    args.push(format!("--meta={}", meta.display()));
                }

                // I/O redirection
                if let Some(ref stdin) = self.stdin {
                    args.push(format!("--stdin={}", stdin.display()));
                }
                if let Some(ref stdout) = self.stdout {
                    args.push(format!("--stdout={}", stdout.display()));
                }
                if let Some(ref stderr) = self.stderr {
                    args.push(format!("--stderr={}", stderr.display()));
                }

                // Working directory
                if let Some(ref dir) = self.working_dir {
                    args.push(format!("--chdir={dir}"));
                }

                // Separator and command
                args.push("--".to_string());
                args.extend(self.command);
            }
        }

        args
    }

    /// Get the isolate binary path
    pub fn isolate_path(&self) -> &Path {
        &self.isolate_path
    }

    /// Get the box ID
    pub fn box_id(&self) -> u32 {
        self.box_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolateAction {
    /// Initialize a new box
    Init,
    /// Run a command in the box
    Run,
    /// Clean up a box
    Cleanup,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_command() {
        let cmd = IsolateCommand::new("isolate", 0).action(IsolateAction::Init);
        let args = cmd.build();
        assert_eq!(args, vec!["isolate", "--box-id=0", "--init"]);
    }

    #[test]
    fn test_cleanup_command() {
        let cmd = IsolateCommand::new("isolate", 5).action(IsolateAction::Cleanup);
        let args = cmd.build();
        assert_eq!(args, vec!["isolate", "--box-id=5", "--cleanup"]);
    }

    #[test]
    fn test_run_command_with_limits() {
        let limits = ResourceLimits {
            time_limit: Some(2.0),
            memory_limit: Some(262144),
            ..Default::default()
        };
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .limits(limits)
            .command(vec!["./main"]);
        let args = cmd.build();
        assert!(args.contains(&"--time=2".to_string()));
        assert!(args.contains(&"--mem=262144".to_string()));
        assert!(args.contains(&"--".to_string()));
        assert!(args.contains(&"./main".to_string()));
    }

    #[test]
    fn test_run_command_with_cgroup() {
        let limits = ResourceLimits {
            memory_limit: Some(262144),
            ..Default::default()
        };
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .limits(limits)
            .cgroup(true)
            .command(vec!["./main"]);
        let args = cmd.build();
        assert!(args.contains(&"--cg".to_string()));
        assert!(args.contains(&"--cg-mem=262144".to_string()));
    }

    #[test]
    fn test_all_resource_limits() {
        let limits = ResourceLimits {
            time_limit: Some(2.0),
            wall_time_limit: Some(5.0),
            memory_limit: Some(262144),
            stack_limit: Some(131072),
            max_processes: Some(4),
            max_output: Some(65536),
            max_open_files: Some(128),
            extra_time: Some(0.5),
        };
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .limits(limits)
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--time=2".to_string()));
        assert!(args.contains(&"--wall-time=5".to_string()));
        assert!(args.contains(&"--mem=262144".to_string()));
        assert!(args.contains(&"--stack=131072".to_string()));
        assert!(args.contains(&"--processes=4".to_string()));
        assert!(args.contains(&"--fsize=65536".to_string()));
        assert!(args.contains(&"--open-files=128".to_string()));
        assert!(args.contains(&"--extra-time=0.5".to_string()));
    }

    #[test]
    fn test_no_limits_set() {
        let limits = ResourceLimits {
            time_limit: None,
            wall_time_limit: None,
            memory_limit: None,
            stack_limit: None,
            max_processes: None,
            max_output: None,
            max_open_files: None,
            extra_time: None,
        };
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .limits(limits)
            .command(vec!["./main"]);
        let args = cmd.build();

        // Should not contain any limit flags
        assert!(!args.iter().any(|a| a.starts_with("--time=")));
        assert!(!args.iter().any(|a| a.starts_with("--wall-time=")));
        assert!(!args.iter().any(|a| a.starts_with("--mem=")));
        assert!(!args.iter().any(|a| a.starts_with("--stack=")));
        assert!(!args.iter().any(|a| a.starts_with("--processes=")));
        assert!(!args.iter().any(|a| a.starts_with("--fsize=")));
        assert!(!args.iter().any(|a| a.starts_with("--open-files=")));
        assert!(!args.iter().any(|a| a.starts_with("--extra-time=")));
    }

    #[test]
    fn test_mount_read_only() {
        let mount = MountConfig {
            source: "/usr/lib".to_string(),
            target: "/lib".to_string(),
            writable: false,
            optional: false,
        };
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .mount(mount)
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--dir=/lib=/usr/lib".to_string()));
    }

    #[test]
    fn test_mount_read_write() {
        let mount = MountConfig {
            source: "/tmp/work".to_string(),
            target: "/work".to_string(),
            writable: true,
            optional: false,
        };
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .mount(mount)
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--dir=/work=/tmp/work:rw".to_string()));
    }

    #[test]
    fn test_multiple_mounts() {
        let mounts = vec![
            MountConfig {
                source: "/usr/lib".to_string(),
                target: "/lib".to_string(),
                writable: false,
                optional: false,
            },
            MountConfig {
                source: "/tmp/data".to_string(),
                target: "/data".to_string(),
                writable: true,
                optional: false,
            },
        ];
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .mounts(mounts)
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--dir=/lib=/usr/lib".to_string()));
        assert!(args.contains(&"--dir=/data=/tmp/data:rw".to_string()));
    }

    #[test]
    fn test_env_single() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .env("PATH", "/usr/bin")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--env=PATH=/usr/bin".to_string()));
    }

    #[test]
    fn test_env_multiple() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .env("PATH", "/usr/bin")
            .env("HOME", "/home/user")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.iter().any(|a| a == "--env=PATH=/usr/bin"));
        assert!(args.iter().any(|a| a == "--env=HOME=/home/user"));
    }

    #[test]
    fn test_env_inherit() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .env_inherit("LANG")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--env=LANG".to_string()));
    }

    #[test]
    fn test_full_env() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .full_env(true)
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--full-env".to_string()));
    }

    #[test]
    fn test_full_env_disabled() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .full_env(false)
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(!args.contains(&"--full-env".to_string()));
    }

    #[test]
    fn test_stdin_redirect() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .stdin("/tmp/input.txt")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--stdin=/tmp/input.txt".to_string()));
    }

    #[test]
    fn test_stdout_redirect() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .stdout("/tmp/output.txt")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--stdout=/tmp/output.txt".to_string()));
    }

    #[test]
    fn test_stderr_redirect() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .stderr("/tmp/error.txt")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--stderr=/tmp/error.txt".to_string()));
    }

    #[test]
    fn test_all_io_redirects() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .stdin("/tmp/in.txt")
            .stdout("/tmp/out.txt")
            .stderr("/tmp/err.txt")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--stdin=/tmp/in.txt".to_string()));
        assert!(args.contains(&"--stdout=/tmp/out.txt".to_string()));
        assert!(args.contains(&"--stderr=/tmp/err.txt".to_string()));
    }

    #[test]
    fn test_meta_file() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .meta_file("/tmp/meta.txt")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--meta=/tmp/meta.txt".to_string()));
    }

    #[test]
    fn test_working_dir() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .working_dir("/box")
            .command(vec!["./main"]);
        let args = cmd.build();

        assert!(args.contains(&"--chdir=/box".to_string()));
    }

    #[test]
    fn test_command_with_args() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Run)
            .command(vec!["python3", "script.py", "--verbose"]);
        let args = cmd.build();

        // Find the separator position
        let sep_pos = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(args[sep_pos + 1], "python3");
        assert_eq!(args[sep_pos + 2], "script.py");
        assert_eq!(args[sep_pos + 3], "--verbose");
    }

    #[test]
    fn test_isolate_path_accessor() {
        let cmd = IsolateCommand::new("/usr/local/bin/isolate", 0);
        assert_eq!(cmd.isolate_path(), Path::new("/usr/local/bin/isolate"));
    }

    #[test]
    fn test_box_id_accessor() {
        let cmd = IsolateCommand::new("isolate", 42);
        assert_eq!(cmd.box_id(), 42);
    }

    #[test]
    fn test_init_ignores_run_options() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Init)
            .env("PATH", "/usr/bin")
            .stdin("/tmp/in.txt")
            .working_dir("/box")
            .command(vec!["./main"]);
        let args = cmd.build();

        // Init should only have box-id and --init
        assert_eq!(args, vec!["isolate", "--box-id=0", "--init"]);
    }

    #[test]
    fn test_cleanup_ignores_run_options() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Cleanup)
            .env("PATH", "/usr/bin")
            .limits(ResourceLimits::default());
        let args = cmd.build();

        // Cleanup should only have box-id and --cleanup
        assert_eq!(args, vec!["isolate", "--box-id=0", "--cleanup"]);
    }

    #[test]
    fn test_cgroup_with_init() {
        let cmd = IsolateCommand::new("isolate", 0)
            .action(IsolateAction::Init)
            .cgroup(true);
        let args = cmd.build();

        assert!(args.contains(&"--cg".to_string()));
        assert!(args.contains(&"--init".to_string()));
    }
}
