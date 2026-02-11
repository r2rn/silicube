//! Integration tests for silicube
//!
//! These tests require the isolate binary to be installed and accessible.
//! Run with: cargo test -p silicube --features integration-tests
//!
//! Tests that require root are marked `#[ignore]`. To include them:
//!   cargo test -p silicube --features integration-tests -- --include-ignored

#![cfg(feature = "integration-tests")]

use silicube::config::Config;
use silicube::isolate::{BoxPool, IsolateBox, MetaFile};
use silicube::runner::{CompileError, Runner};
use silicube::types::{ExecutionStatus, LimitExceeded, ResourceLimits};

/// Path to test fixtures
const FIXTURES_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// Helper to get fixture file content
fn fixture_source(name: &str) -> Vec<u8> {
    let path = format!("{}/sources/{}", FIXTURES_PATH, name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path, e))
}

/// Create a test config with cgroup support if available, falling back to non-cgroup mode.
fn test_config() -> Config {
    let mut config = Config::default();
    if config.cgroup {
        match silicube::prepare_cgroup(&config.cg_root) {
            Ok(true) => {}              // cgroups ready
            _ => config.cgroup = false, // not available, fall back
        }
    }
    config
}

mod sandbox_lifecycle {
    use super::*;

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_box_init_and_cleanup() {
        let config = test_config();
        let mut sandbox = IsolateBox::init(0, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        // Verify box directory exists
        assert!(sandbox.path().exists());

        // Cleanup
        sandbox.cleanup().await.expect("Failed to cleanup sandbox");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_box_write_and_read_file() {
        let config = test_config();
        let mut sandbox = IsolateBox::init(1, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        // Write a file
        let content = b"Hello, World!";
        sandbox
            .write_file("test.txt", content)
            .await
            .expect("Failed to write file");

        // Read it back
        let read_content = sandbox
            .read_file("test.txt")
            .await
            .expect("Failed to read file");

        assert_eq!(read_content, content);

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_box_file_exists() {
        let config = test_config();
        let mut sandbox = IsolateBox::init(2, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        // File should not exist initially
        assert!(!sandbox.file_exists("nonexistent.txt").await.unwrap());

        // Write a file
        sandbox
            .write_file("exists.txt", b"content")
            .await
            .expect("Failed to write file");

        // Now it should exist
        assert!(sandbox.file_exists("exists.txt").await.unwrap());

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_box_pool() {
        let config = test_config();
        let pool = BoxPool::new(10, 5, config.isolate_binary(), config.cgroup);

        // Acquire a box
        let mut sandbox1 = pool.acquire().await.expect("Failed to acquire box");
        let id1 = sandbox1.id();

        // Acquire another box
        let mut sandbox2 = pool.acquire().await.expect("Failed to acquire second box");
        let id2 = sandbox2.id();

        // IDs should be different
        assert_ne!(id1, id2);

        // Cleanup boxes
        sandbox1
            .cleanup()
            .await
            .expect("Failed to cleanup sandbox1");
        sandbox2
            .cleanup()
            .await
            .expect("Failed to cleanup sandbox2");
    }
}

mod compilation {
    use super::*;

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_compile_cpp_success() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(20, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("hello.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        let result = runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation failed");

        assert!(result.is_success());
        assert!(sandbox.file_exists("main").await.unwrap());

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_compile_cpp_error() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(21, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("compile_error.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        let result = runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation call failed");

        assert!(!result.is_success());
        assert!(!result.output.is_empty()); // Should have error message

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_compile_interpreted_language_fails() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(22, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("hello.py");
        let language = config.get_language("python3").expect("python3 not found");

        let result = runner.compile(&sandbox, &source, language, None).await;

        assert!(matches!(result, Err(CompileError::NotCompiled(_))));

        sandbox.cleanup().await.expect("Failed to cleanup");
    }
}

mod execution {
    use super::*;

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_run_hello_world() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(30, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("hello.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        // Compile first
        let compile_result = runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation failed");
        assert!(compile_result.is_success());

        // Run
        let result = runner
            .run(&sandbox, None, language, None)
            .await
            .expect("Execution failed");

        assert!(result.is_success());
        assert_eq!(result.status, ExecutionStatus::Ok);
        assert_eq!(result.exit_code, Some(0));

        if let Some(stdout) = &result.stdout {
            let output = String::from_utf8_lossy(stdout);
            assert!(output.contains("Hello, World!"));
        }

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_run_with_stdin() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(31, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("echo.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        // Compile
        let compile_result = runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation failed");
        assert!(compile_result.is_success());

        // Run with input
        let input = b"test input\n";
        let result = runner
            .run(&sandbox, Some(input), language, None)
            .await
            .expect("Execution failed");

        assert!(result.is_success());

        if let Some(stdout) = &result.stdout {
            let output = String::from_utf8_lossy(stdout);
            assert!(output.contains("test input"));
        }

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_run_time_limit_exceeded() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(32, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("infinite_loop.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        // Compile
        let compile_result = runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation failed");
        assert!(compile_result.is_success());

        // Run with strict time limit
        let limits = ResourceLimits::new()
            .with_time_limit(0.5)
            .with_wall_time_limit(1.0);

        let result = runner
            .run(&sandbox, None, language, Some(&limits))
            .await
            .expect("Execution call failed");

        assert_eq!(result.status, ExecutionStatus::TimeLimitExceeded);
        assert!(
            result.limit_exceeded == LimitExceeded::Time
                || result.limit_exceeded == LimitExceeded::WallTime
        );

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_run_runtime_error() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(33, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("runtime_error.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        // Compile
        let compile_result = runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation failed");
        assert!(compile_result.is_success());

        // Run
        let result = runner
            .run(&sandbox, None, language, None)
            .await
            .expect("Execution call failed");

        assert!(!result.is_success());
        assert_eq!(result.exit_code, Some(1));

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_run_segfault() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(34, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("segfault.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        // Compile
        let compile_result = runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation failed");
        assert!(compile_result.is_success());

        // Run
        let result = runner
            .run(&sandbox, None, language, None)
            .await
            .expect("Execution call failed");

        assert_eq!(result.status, ExecutionStatus::Signaled);
        assert_eq!(result.signal, Some(11)); // SIGSEGV

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_run_interpreted_python() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(35, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("hello.py");
        let language = config.get_language("python3").expect("python3 not found");

        // Run interpreted
        let result = runner
            .run_interpreted(&sandbox, &source, None, language, None)
            .await
            .expect("Execution failed");

        assert!(result.is_success());

        if let Some(stdout) = &result.stdout {
            let output = String::from_utf8_lossy(stdout);
            assert!(output.contains("Hello, World!"));
        }

        sandbox.cleanup().await.expect("Failed to cleanup");
    }
}

mod compile_and_run {
    use silicube::runner::CompileAndRunRequest;

    use super::*;

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_compile_and_run_success() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(40, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("hello.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        let request = CompileAndRunRequest {
            sandbox: &sandbox,
            source: &source,
            input: None,
            language,
            compile_limits: None,
            run_limits: None,
        };

        let (compile_result, run_result) = runner
            .compile_and_run(request)
            .await
            .expect("Compile and run failed");

        assert!(compile_result.is_success());
        assert!(run_result.is_some());

        let run_result = run_result.unwrap();
        assert!(run_result.is_success());

        sandbox.cleanup().await.expect("Failed to cleanup");
    }

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_compile_and_run_compile_failure() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(41, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("compile_error.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        let request = CompileAndRunRequest {
            sandbox: &sandbox,
            source: &source,
            input: None,
            language,
            compile_limits: None,
            run_limits: None,
        };

        let (compile_result, run_result) = runner
            .compile_and_run(request)
            .await
            .expect("Compile and run call failed");

        assert!(!compile_result.is_success());
        assert!(run_result.is_none()); // Should not run if compile fails

        sandbox.cleanup().await.expect("Failed to cleanup");
    }
}

mod resource_limits {
    use super::*;

    #[tokio::test]
    #[ignore = "requires root"]
    async fn test_custom_time_limit() {
        let config = test_config();
        let runner = Runner::new(config.clone());
        let mut sandbox = IsolateBox::init(50, config.isolate_binary(), config.cgroup)
            .await
            .expect("Failed to create sandbox");

        let source = fixture_source("hello.cpp");
        let language = config.get_language("cpp17").expect("cpp17 not found");

        // Compile
        runner
            .compile(&sandbox, &source, language, None)
            .await
            .expect("Compilation failed");

        // Run with custom limits
        let limits = ResourceLimits::new()
            .with_time_limit(10.0)
            .with_memory_limit(128 * 1024); // 128 MB

        let result = runner
            .run(&sandbox, None, language, Some(&limits))
            .await
            .expect("Execution failed");

        assert!(result.is_success());
        // Time should be under our generous limit
        assert!(result.time < 10.0);

        sandbox.cleanup().await.expect("Failed to cleanup");
    }
}

mod config_loading {
    use super::*;

    #[test]
    fn test_load_valid_config() {
        let path = format!("{}/configs/valid_full.toml", FIXTURES_PATH);
        let config = Config::from_file(&path).expect("Failed to load config");

        assert!(config.languages.contains_key("cpp17"));
        assert!(config.languages.contains_key("python3"));
        assert_eq!(config.default_limits.time_limit, Some(2.0));
    }

    #[test]
    fn test_load_minimal_config() {
        let path = format!("{}/configs/valid_minimal.toml", FIXTURES_PATH);
        let config = Config::from_file(&path).expect("Failed to load config");

        assert!(config.languages.contains_key("test"));
    }

    #[test]
    fn test_load_invalid_empty_name() {
        let path = format!("{}/configs/invalid_empty_name.toml", FIXTURES_PATH);
        let result = Config::from_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_empty_extension() {
        let path = format!("{}/configs/invalid_empty_extension.toml", FIXTURES_PATH);
        let result = Config::from_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_empty_run_command() {
        let path = format!("{}/configs/invalid_empty_run_command.toml", FIXTURES_PATH);
        let result = Config::from_file(&path);
        assert!(result.is_err());
    }
}

mod meta_file_fixtures {
    use super::*;

    fn load_meta_fixture(name: &str) -> MetaFile {
        let path = format!("{}/meta/{}", FIXTURES_PATH, name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read meta fixture {}: {}", path, e));
        MetaFile::parse(&content)
    }

    #[test]
    fn test_meta_success() {
        let meta = load_meta_fixture("success.meta");
        assert_eq!(meta.status(), ExecutionStatus::Ok);
        assert_eq!(meta.exit_code(), Some(0));
        assert!((meta.time() - 0.042).abs() < 0.001);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::NotExceeded);
    }

    #[test]
    fn test_meta_tle() {
        let meta = load_meta_fixture("tle.meta");
        assert_eq!(meta.status(), ExecutionStatus::TimeLimitExceeded);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::Time);
    }

    #[test]
    fn test_meta_wall_tle() {
        let meta = load_meta_fixture("wall_tle.meta");
        assert_eq!(meta.status(), ExecutionStatus::TimeLimitExceeded);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::WallTime);
    }

    #[test]
    fn test_meta_mle() {
        let meta = load_meta_fixture("mle.meta");
        assert_eq!(meta.status(), ExecutionStatus::Signaled);
        assert_eq!(meta.limit_exceeded(), LimitExceeded::Memory);
        assert_eq!(meta.signal(), Some(9));
    }

    #[test]
    fn test_meta_ole() {
        let meta = load_meta_fixture("ole.meta");
        assert_eq!(meta.limit_exceeded(), LimitExceeded::Output);
    }

    #[test]
    fn test_meta_signal() {
        let meta = load_meta_fixture("signal.meta");
        assert_eq!(meta.status(), ExecutionStatus::Signaled);
        assert_eq!(meta.signal(), Some(11)); // SIGSEGV
    }

    #[test]
    fn test_meta_runtime_error() {
        let meta = load_meta_fixture("runtime_error.meta");
        assert_eq!(meta.status(), ExecutionStatus::RuntimeError);
        assert_eq!(meta.exit_code(), Some(1));
    }

    #[test]
    fn test_meta_cgroup_mem_priority() {
        let meta = load_meta_fixture("cgroup_mem.meta");
        // cg-mem should be preferred over max-rss
        assert_eq!(meta.memory(), 524288);
    }
}
