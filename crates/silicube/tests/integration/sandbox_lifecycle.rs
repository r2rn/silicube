use silicube::isolate::{BoxPool, IsolateBox};

use super::test_config;

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
