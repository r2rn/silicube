use silicube::isolate::MetaFile;
use silicube::types::{ExecutionStatus, LimitExceeded};

use super::FIXTURES_PATH;

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
