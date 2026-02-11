use silicube::isolate::IsolateBox;
use silicube::runner::{CompileError, Runner};

use super::{fixture_source, test_config};

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
