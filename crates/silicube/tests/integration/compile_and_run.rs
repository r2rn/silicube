use silicube::isolate::IsolateBox;
use silicube::runner::{CompileAndRunRequest, Runner};

use super::{fixture_source, test_config};

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
