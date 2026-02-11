use silicube::isolate::IsolateBox;
use silicube::runner::Runner;
use silicube::types::{ExecutionStatus, LimitExceeded, ResourceLimits};

use super::{fixture_source, test_config};

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
