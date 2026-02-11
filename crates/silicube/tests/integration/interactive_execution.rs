use std::time::Duration;

use silicube::isolate::IsolateBox;
use silicube::runner::{InteractiveEvent, InteractiveEventStream, Runner};
use silicube::types::ResourceLimits;

use super::{fixture_source, test_config};

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_echo() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(60, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("echo.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    // Compile first
    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    // Start interactive session
    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Write a line and read it back
    session
        .write_line("hello interactive")
        .await
        .expect("Failed to write");
    let line = session
        .read_line()
        .await
        .expect("Failed to read line")
        .expect("Expected a line");
    assert_eq!(line, "hello interactive");

    // Write another line
    session
        .write_line("second line")
        .await
        .expect("Failed to write");
    let line = session
        .read_line()
        .await
        .expect("Failed to read line")
        .expect("Expected a line");
    assert_eq!(line, "second line");

    // Close stdin and wait for exit
    session.close_stdin();
    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");

    assert!(result.is_success());
    assert_eq!(result.exit_code, Some(0));

    sandbox.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_multi_turn() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(61, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("interactive_adder.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    // Compile
    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    // Start interactive session
    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Multiple rounds of interaction
    let test_cases = [(1, 2, 3), (10, 20, 30), (0, 0, 0), (-5, 15, 10)];

    for (a, b, expected) in &test_cases {
        session
            .write_line(&format!("{} {}", a, b))
            .await
            .expect("Failed to write");
        let line = session
            .read_line()
            .await
            .expect("Failed to read line")
            .expect("Expected a line");
        assert_eq!(
            line,
            expected.to_string(),
            "Expected {} + {} = {}, got {}",
            a,
            b,
            expected,
            line
        );
    }

    // Close stdin and wait
    session.close_stdin();
    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");

    assert!(result.is_success());

    sandbox.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_close_stdin_exits() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(62, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("echo.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Close stdin immediately - program should exit
    session.close_stdin();

    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");

    assert!(result.is_success());
    assert_eq!(result.exit_code, Some(0));

    sandbox.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_kill() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(63, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("infinite_loop.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Process should be running
    assert!(!session.is_terminated());

    // Kill it
    session.kill().await.expect("Failed to kill session");

    // Should now be terminated
    assert!(session.is_terminated());

    sandbox.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_wait_timeout_expires() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(64, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("infinite_loop.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    // Short wall time so isolate kills the process soon after our timeout,
    // but long enough that the 100ms timeout fires first.
    let limits = ResourceLimits::new()
        .with_time_limit(1.0)
        .with_wall_time_limit(1.0);

    let session = runner
        .run_interactive(&sandbox, language, Some(&limits))
        .await
        .expect("Failed to start interactive session");

    // Wait with a very short timeout - should fail because the infinite
    // loop won't exit within 100ms (isolate limit is 1s).
    let result = session.wait_timeout(Duration::from_millis(100)).await;
    assert!(result.is_err());

    // Session is consumed by wait_timeout; the isolate process is still
    // running. Wait for isolate's wall time limit to kill it before cleanup.
    tokio::time::sleep(Duration::from_secs(3)).await;

    sandbox.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_write_after_terminated() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(65, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("hello.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    // hello.cpp exits immediately (no stdin read)
    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Wait for process to exit
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Writing after termination should fail
    let result = session.write_line("should fail").await;
    assert!(result.is_err());

    sandbox.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_event_stream() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(66, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("echo.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    // Short wall time so isolate kills the process for cleanup
    let limits = ResourceLimits::new()
        .with_time_limit(2.0)
        .with_wall_time_limit(2.0);

    let session = runner
        .run_interactive(&sandbox, language, Some(&limits))
        .await
        .expect("Failed to start interactive session");

    // Create event stream
    let (mut stream, handle) = InteractiveEventStream::new(session);

    // Write via handle and verify Stdout event is received
    handle
        .write_line("event stream test")
        .await
        .expect("Failed to write");

    let event = tokio::time::timeout(Duration::from_secs(5), stream.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Stream closed unexpectedly");

    match event {
        InteractiveEvent::Stdout(data) => {
            let output = String::from_utf8_lossy(&data);
            assert!(
                output.contains("event stream test"),
                "Expected 'event stream test' in output, got: {}",
                output
            );
        }
        other => panic!("Expected Stdout event, got: {:?}", other),
    }

    // Write a second message and verify
    handle
        .write_line("second message")
        .await
        .expect("Failed to write second message");

    let event = tokio::time::timeout(Duration::from_secs(5), stream.recv())
        .await
        .expect("Timeout waiting for second event")
        .expect("Stream closed unexpectedly");

    match event {
        InteractiveEvent::Stdout(data) => {
            let output = String::from_utf8_lossy(&data);
            assert!(
                output.contains("second message"),
                "Expected 'second message' in output, got: {}",
                output
            );
        }
        other => panic!("Expected Stdout event, got: {:?}", other),
    }

    // Drop resources; wait for isolate wall time limit to kill process
    drop(handle);
    drop(stream);
    tokio::time::sleep(Duration::from_secs(4)).await;

    sandbox.cleanup().await.expect("Failed to cleanup");
}

/// Test staggered I/O with a C++ program that outputs prompts before reading input.
/// Verifies that we can read output, then write input, then read the response,
/// alternating back and forth.
#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_staggered_prompt_response_cpp() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(68, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("staggered_prompt.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Round 1: program outputs prompt, we read it, then provide input
    let prompt = session
        .read_line()
        .await
        .expect("Failed to read prompt")
        .expect("Expected prompt line");
    assert_eq!(prompt, "What is your name?");

    session
        .write_line("Alice")
        .await
        .expect("Failed to write name");
    let response = session
        .read_line()
        .await
        .expect("Failed to read response")
        .expect("Expected response line");
    assert_eq!(response, "Hello, Alice!");

    // Round 2: program outputs another prompt, we read, write, read two response lines
    let prompt = session
        .read_line()
        .await
        .expect("Failed to read prompt")
        .expect("Expected prompt line");
    assert_eq!(prompt, "Enter a number:");

    session
        .write_line("7")
        .await
        .expect("Failed to write number");
    let line1 = session
        .read_line()
        .await
        .expect("Failed to read response")
        .expect("Expected response line");
    assert_eq!(line1, "Double: 14");
    let line2 = session
        .read_line()
        .await
        .expect("Failed to read response")
        .expect("Expected response line");
    assert_eq!(line2, "Triple: 21");

    // Round 3: another prompt/response cycle
    let prompt = session
        .read_line()
        .await
        .expect("Failed to read prompt")
        .expect("Expected prompt line");
    assert_eq!(prompt, "Enter a word:");

    session
        .write_line("Rust")
        .await
        .expect("Failed to write word");
    let response = session
        .read_line()
        .await
        .expect("Failed to read response")
        .expect("Expected response line");
    assert_eq!(response, "You said: Rust");
    let done = session
        .read_line()
        .await
        .expect("Failed to read final line")
        .expect("Expected final line");
    assert_eq!(done, "Done!");

    // Program should exit on its own
    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");
    assert!(result.is_success());
    assert_eq!(result.exit_code, Some(0));

    sandbox.cleanup().await.expect("Failed to cleanup");
}

/// Test staggered I/O with a program that outputs multi-line banners and
/// multiple output lines between input reads.
#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_multi_line_output_between_inputs() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(69, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("multi_step_quiz.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Read the multi-line banner before any input is needed
    let line1 = session.read_line().await.unwrap().unwrap();
    assert_eq!(line1, "Welcome to the quiz!");
    let line2 = session.read_line().await.unwrap().unwrap();
    assert_eq!(line2, "You will answer 3 questions.");
    let blank = session.read_line().await.unwrap().unwrap();
    assert_eq!(blank, "");

    // Q1: read prompt, answer correctly
    let q1 = session.read_line().await.unwrap().unwrap();
    assert_eq!(q1, "Q1: What is 2+2?");
    session.write_line("4").await.unwrap();
    let r1 = session.read_line().await.unwrap().unwrap();
    assert_eq!(r1, "Correct!");

    // Blank line + Q2 prompt
    let blank = session.read_line().await.unwrap().unwrap();
    assert_eq!(blank, "");
    let q2 = session.read_line().await.unwrap().unwrap();
    assert_eq!(q2, "Q2: What is 3*5?");
    session.write_line("10").await.unwrap(); // wrong answer
    let r2 = session.read_line().await.unwrap().unwrap();
    assert_eq!(r2, "Wrong! The answer is 15.");

    // Blank line + Q3 prompt
    let blank = session.read_line().await.unwrap().unwrap();
    assert_eq!(blank, "");
    let q3 = session.read_line().await.unwrap().unwrap();
    assert_eq!(q3, "Q3: What is 10-7?");
    session.write_line("3").await.unwrap();
    let r3 = session.read_line().await.unwrap().unwrap();
    assert_eq!(r3, "Correct!");

    // Final score
    let blank = session.read_line().await.unwrap().unwrap();
    assert_eq!(blank, "");
    let score = session.read_line().await.unwrap().unwrap();
    assert_eq!(score, "Final score: 2/3");

    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");
    assert!(result.is_success());
    assert_eq!(result.exit_code, Some(0));

    sandbox.cleanup().await.expect("Failed to cleanup");
}

/// Test staggered I/O through the event stream API, verifying that
/// Stdout events arrive in the correct order relative to writes.
#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_staggered_event_stream() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(70, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("staggered_prompt.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    let limits = ResourceLimits::new()
        .with_time_limit(5.0)
        .with_wall_time_limit(5.0);

    let session = runner
        .run_interactive(&sandbox, language, Some(&limits))
        .await
        .expect("Failed to start interactive session");

    let (mut stream, handle) = InteractiveEventStream::new(session);

    // Helper to collect stdout until we see a target string
    let mut accumulated = String::new();

    // Read initial prompt via event stream
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), stream.recv())
            .await
            .expect("Timeout waiting for prompt")
            .expect("Stream closed");
        match event {
            InteractiveEvent::Stdout(data) => {
                accumulated.push_str(&String::from_utf8_lossy(&data));
                if accumulated.contains("What is your name?\n") {
                    break;
                }
            }
            InteractiveEvent::Exited(_) => panic!("Process exited before prompt"),
            _ => {}
        }
    }
    assert!(accumulated.contains("What is your name?"));
    accumulated.clear();

    // Write name and read greeting
    handle.write_line("Bob").await.expect("Failed to write");
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), stream.recv())
            .await
            .expect("Timeout waiting for greeting")
            .expect("Stream closed");
        match event {
            InteractiveEvent::Stdout(data) => {
                accumulated.push_str(&String::from_utf8_lossy(&data));
                if accumulated.contains("Enter a number:\n") {
                    break;
                }
            }
            InteractiveEvent::Exited(_) => panic!("Process exited before greeting"),
            _ => {}
        }
    }
    assert!(
        accumulated.contains("Hello, Bob!"),
        "Expected greeting in: {}",
        accumulated
    );
    accumulated.clear();

    // Write number and read double+triple+next prompt
    handle.write_line("5").await.expect("Failed to write");
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), stream.recv())
            .await
            .expect("Timeout waiting for results")
            .expect("Stream closed");
        match event {
            InteractiveEvent::Stdout(data) => {
                accumulated.push_str(&String::from_utf8_lossy(&data));
                if accumulated.contains("Enter a word:\n") {
                    break;
                }
            }
            InteractiveEvent::Exited(_) => panic!("Process exited before results"),
            _ => {}
        }
    }
    assert!(
        accumulated.contains("Double: 10"),
        "Expected double in: {}",
        accumulated
    );
    assert!(
        accumulated.contains("Triple: 15"),
        "Expected triple in: {}",
        accumulated
    );
    accumulated.clear();

    // Write word and read final output
    handle.write_line("hello").await.expect("Failed to write");
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), stream.recv())
            .await
            .expect("Timeout waiting for final output")
            .expect("Stream closed");
        match event {
            InteractiveEvent::Stdout(data) => {
                accumulated.push_str(&String::from_utf8_lossy(&data));
                if accumulated.contains("Done!\n") {
                    break;
                }
            }
            InteractiveEvent::Exited(_) => panic!("Process exited before Done"),
            _ => {}
        }
    }
    assert!(
        accumulated.contains("You said: hello"),
        "Expected echo in: {}",
        accumulated
    );
    assert!(
        accumulated.contains("Done!"),
        "Expected Done in: {}",
        accumulated
    );

    // Process should exit — we may get an Exited event or the stream
    // may simply close (the background task can race between detecting
    // stdout EOF and the process actually terminating).
    drop(handle);
    let event = tokio::time::timeout(Duration::from_secs(5), stream.recv()).await;
    match event {
        Ok(Some(InteractiveEvent::Exited(result))) => {
            assert!(result.is_success());
            assert_eq!(result.exit_code, Some(0));
        }
        Ok(Some(other)) => {
            // Possible trailing stdout chunk — just drain
            debug_assert!(
                matches!(other, InteractiveEvent::Stdout(_)),
                "Unexpected event: {:?}",
                other
            );
        }
        Ok(None) => {
            // Stream closed — process exited and background task finished
        }
        Err(_) => {
            panic!("Timed out waiting for process to exit");
        }
    }

    drop(stream);

    sandbox.cleanup().await.expect("Failed to cleanup");
}

/// Test staggered I/O with an interpreted Python program that outputs
/// prompts before reading input.
#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_staggered_prompt_response_python() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(71, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("staggered_prompt.py");
    let language = config.get_language("python3").expect("python3 not found");

    // Write source file to sandbox (interpreted language)
    let source_name = language.source_name();
    sandbox
        .write_file(&source_name, &source)
        .await
        .expect("Failed to write source");

    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Round 1: read prompt, write name, read greeting
    let prompt = session.read_line().await.unwrap().unwrap();
    assert_eq!(prompt, "What is your name?");

    session.write_line("Alice").await.unwrap();
    let greeting = session.read_line().await.unwrap().unwrap();
    assert_eq!(greeting, "Hello, Alice!");

    // Round 2: read prompt, write number, read double and triple
    let prompt = session.read_line().await.unwrap().unwrap();
    assert_eq!(prompt, "Enter a number:");

    session.write_line("7").await.unwrap();
    let double = session.read_line().await.unwrap().unwrap();
    assert_eq!(double, "Double: 14");
    let triple = session.read_line().await.unwrap().unwrap();
    assert_eq!(triple, "Triple: 21");

    // Round 3: read prompt, write word, read echo and done
    let prompt = session.read_line().await.unwrap().unwrap();
    assert_eq!(prompt, "Enter a word:");

    session.write_line("Rust").await.unwrap();
    let echo = session.read_line().await.unwrap().unwrap();
    assert_eq!(echo, "You said: Rust");
    let done = session.read_line().await.unwrap().unwrap();
    assert_eq!(done, "Done!");

    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");
    assert!(result.is_success());

    sandbox.cleanup().await.expect("Failed to cleanup");
}

/// Test that rapid alternating writes and reads work correctly —
/// the adder program processes each input immediately, so we can
/// verify output arrives for each input before sending the next.
#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_strict_alternation() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(72, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("interactive_adder.cpp");
    let language = config.get_language("cpp17").expect("cpp17 not found");

    let compile_result = runner
        .compile(&sandbox, &source, language, None)
        .await
        .expect("Compilation failed");
    assert!(compile_result.is_success());

    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Strictly alternate: write one pair, read one result, repeat.
    // This ensures the process flushes and we receive output between
    // each write, rather than batching.
    for i in 0..20 {
        let a = i;
        let b = i * 2;
        session
            .write_line(&format!("{} {}", a, b))
            .await
            .expect("Failed to write");

        let line = session
            .read_line()
            .await
            .expect("Failed to read")
            .expect("Expected output line");
        assert_eq!(
            line,
            (a + b).to_string(),
            "Mismatch on iteration {}: {} + {} should be {}",
            i,
            a,
            b,
            a + b
        );
    }

    session.close_stdin();
    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");
    assert!(result.is_success());

    sandbox.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore = "requires root"]
async fn test_interactive_interpreted_python() {
    let config = test_config();
    let runner = Runner::new(config.clone());
    let mut sandbox = IsolateBox::init(67, config.isolate_binary(), config.cgroup)
        .await
        .expect("Failed to create sandbox");

    let source = fixture_source("interactive_echo.py");
    let language = config.get_language("python3").expect("python3 not found");

    // Write source file to sandbox (interpreted language)
    let source_name = language.source_name();
    sandbox
        .write_file(&source_name, &source)
        .await
        .expect("Failed to write source");

    // Start interactive session
    let mut session = runner
        .run_interactive(&sandbox, language, None)
        .await
        .expect("Failed to start interactive session");

    // Write and read
    session
        .write_line("python interactive")
        .await
        .expect("Failed to write");
    let line = session
        .read_line()
        .await
        .expect("Failed to read line")
        .expect("Expected a line");
    assert_eq!(line, "python interactive");

    // Another round
    session
        .write_line("second round")
        .await
        .expect("Failed to write");
    let line = session
        .read_line()
        .await
        .expect("Failed to read line")
        .expect("Expected a line");
    assert_eq!(line, "second round");

    // Close and wait
    session.close_stdin();
    let result = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("Wait failed");

    assert!(result.is_success());

    sandbox.cleanup().await.expect("Failed to cleanup");
}
