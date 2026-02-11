use silicube::isolate::IsolateBox;
use silicube::runner::Runner;
use silicube::types::ResourceLimits;

use super::{fixture_source, test_config};

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
