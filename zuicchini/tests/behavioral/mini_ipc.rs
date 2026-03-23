use zuicchini::emCore::emMiniIpc::{decode_message, encode_message};

#[test]
fn encode_decode_round_trip() {
    let args = &["hello", "world", "test"];
    let encoded = encode_message(args);
    let (decoded, consumed) = decode_message(&encoded).expect("should decode");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded.len(), 3);
    assert_eq!(decoded[0], "hello");
    assert_eq!(decoded[1], "world");
    assert_eq!(decoded[2], "test");
}

#[test]
fn encode_decode_empty_args() {
    let args: &[&str] = &[];
    let encoded = encode_message(args);
    let (decoded, consumed) = decode_message(&encoded).expect("should decode");
    assert_eq!(consumed, encoded.len());
    assert!(decoded.is_empty());
}

#[test]
fn encode_decode_single_arg() {
    let args = &["single"];
    let encoded = encode_message(args);
    let (decoded, consumed) = decode_message(&encoded).expect("should decode");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded, vec!["single"]);
}

#[test]
fn encode_decode_empty_string_arg() {
    let args = &["", "nonempty", ""];
    let encoded = encode_message(args);
    let (decoded, consumed) = decode_message(&encoded).expect("should decode");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded, vec!["", "nonempty", ""]);
}

#[test]
fn decode_incomplete_returns_none() {
    let args = &["hello", "world"];
    let encoded = encode_message(args);
    // Truncate the message
    let truncated = &encoded[..encoded.len() - 2];
    assert!(decode_message(truncated).is_none());
}

#[test]
fn decode_multiple_messages_in_buffer() {
    let msg1 = encode_message(&["a", "b"]);
    let msg2 = encode_message(&["c"]);
    let mut buf = msg1.clone();
    buf.extend_from_slice(&msg2);

    let (decoded1, consumed1) = decode_message(&buf).expect("first message");
    assert_eq!(decoded1, vec!["a", "b"]);

    let (decoded2, consumed2) = decode_message(&buf[consumed1..]).expect("second message");
    assert_eq!(decoded2, vec!["c"]);
    assert_eq!(consumed1 + consumed2, buf.len());
}

#[test]
fn wire_format_matches_cpp() {
    // C++ format: ASCII argc + null + null-terminated argv
    let encoded = encode_message(&["open", "file.txt"]);
    // "2\0open\0file.txt\0"
    assert_eq!(encoded, b"2\0open\0file.txt\0");
}

#[test]
fn decode_garbage_returns_none() {
    assert!(decode_message(b"not_a_number\0data").is_none());
}

#[test]
fn decode_empty_buffer_returns_none() {
    assert!(decode_message(b"").is_none());
}

// ── Integration tests (Linux only, FIFO-based) ─────────────────────

#[cfg(target_os = "linux")]
mod linux {
    use std::cell::RefCell;
    use std::rc::Rc;

    use zuicchini::emCore::emMiniIpc::{emMiniIpcClient, emMiniIpcServer};
    use zuicchini::emCore::emScheduler::EngineScheduler;

    #[test]
    fn server_not_found() {
        let result = emMiniIpcClient::TrySend("nonexistent_test_server_12345", &["hello"]);
        assert!(result.is_err());
    }

    #[test]
    fn client_server_round_trip() {
        let mut sched = EngineScheduler::new();
        let received: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let received_clone = Rc::clone(&received);

        let mut server = emMiniIpcServer::new(
            &mut sched,
            Box::new(move |args: &[String]| {
                received_clone.borrow_mut().push(args.to_vec());
            }),
        );

        let name = format!("test-mini-ipc-{}", std::process::id());
        server
            .StartServing(&mut sched, Some(&name))
            .expect("start serving");
        assert!(server.IsServing());
        assert_eq!(server.GetServerName(), name);

        // Send a message
        emMiniIpcClient::TrySend(&name, &["hello", "world"]).expect("send message");

        // Poll to receive
        // Directly invoke poll by running scheduler time slices
        // First fire the timer signal manually to trigger the engine
        let dummy_sig = sched.create_signal();
        sched.fire(dummy_sig);
        sched.do_time_slice();
        sched.remove_signal(dummy_sig);

        // The timer fires after 200ms, but we can trigger it by running time slices.
        // For the test, directly poll via the scheduler. We need to wait for
        // the timer to fire. Let's just do time slices until we get the message.
        let start = std::time::Instant::now();
        while received.borrow().is_empty() {
            sched.do_time_slice();
            if start.elapsed() > std::time::Duration::from_secs(2) {
                panic!("timed out waiting for message");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let msgs = received.borrow();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], vec!["hello", "world"]);
        drop(msgs);

        server.cleanup(&mut sched);
    }

    #[test]
    fn server_cleanup_removes_fifo() {
        let mut sched = EngineScheduler::new();
        let mut server = emMiniIpcServer::new(&mut sched, Box::new(|_: &[String]| {}));

        let name = format!("test-cleanup-{}", std::process::id());
        server
            .StartServing(&mut sched, Some(&name))
            .expect("start serving");
        assert!(server.IsServing());

        server.StopServing(&mut sched);
        assert!(!server.IsServing());

        // Verify FIFO is removed — sending should fail
        let result = emMiniIpcClient::TrySend(&name, &["test"]);
        assert!(result.is_err());

        server.cleanup(&mut sched);
    }

    #[test]
    fn multiple_messages() {
        let mut sched = EngineScheduler::new();
        let received: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let received_clone = Rc::clone(&received);

        let mut server = emMiniIpcServer::new(
            &mut sched,
            Box::new(move |args: &[String]| {
                received_clone.borrow_mut().push(args.to_vec());
            }),
        );

        let name = format!("test-multi-{}", std::process::id());
        server
            .StartServing(&mut sched, Some(&name))
            .expect("start serving");

        emMiniIpcClient::TrySend(&name, &["msg1"]).expect("send 1");
        emMiniIpcClient::TrySend(&name, &["msg2", "arg2"]).expect("send 2");
        emMiniIpcClient::TrySend(&name, &["msg3"]).expect("send 3");

        let start = std::time::Instant::now();
        while received.borrow().len() < 3 {
            sched.do_time_slice();
            if start.elapsed() > std::time::Duration::from_secs(2) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let msgs = received.borrow();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0], vec!["msg1"]);
        assert_eq!(msgs[1], vec!["msg2", "arg2"]);
        assert_eq!(msgs[2], vec!["msg3"]);
        drop(msgs);

        server.cleanup(&mut sched);
    }
}
