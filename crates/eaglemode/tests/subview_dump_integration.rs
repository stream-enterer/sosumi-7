//! Integration test for the F010-unblocking sub-view crossing work.
//! Gated `#[ignore]` because it requires a live display (X11 or Xvfb)
//! and a compiled `eaglemode` binary.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

fn spawn_and_connect() -> (Child, UnixStream) {
    let mut child = Command::new("cargo")
        .args(["run", "--bin", "eaglemode", "--quiet"])
        .env("EMCORE_DEBUG_CONTROL", "1")
        .spawn()
        .expect("spawn eaglemode");
    let pid = child.id();
    let sock_path = PathBuf::from(format!("/tmp/eaglemode-rs.{}.sock", pid));

    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if sock_path.exists() {
            if let Ok(s) = UnixStream::connect(&sock_path) {
                return (child, s);
            }
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("socket did not appear within 60s at {:?}", sock_path);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn send(s: &mut UnixStream, line: &str) -> String {
    writeln!(s, "{}", line).unwrap();
    let mut reader = BufReader::new(s.try_clone().unwrap());
    let mut buf = String::new();
    reader.read_line(&mut buf).unwrap();
    buf
}

#[test]
#[ignore = "requires display + binary build"]
fn f010_subview_dump_nests_under_home_view_context() {
    let (mut child, mut s) = spawn_and_connect();

    // Baseline dump.
    let reply = send(&mut s, r#"{"cmd":"dump"}"#);
    assert!(reply.contains("\"ok\":true"), "baseline dump reply: {}", reply);

    // Zoom outer → content SVP.
    send(&mut s, r#"{"cmd":"visit","identity":"root:content view"}"#);
    send(&mut s, r#"{"cmd":"wait_idle","timeout_ms":60000}"#);

    // Zoom content sub-view → home.
    send(&mut s, r#"{"cmd":"visit","view":"root:content view","identity":"::home"}"#);
    send(&mut s, r#"{"cmd":"wait_idle","timeout_ms":60000}"#);

    let reply = send(&mut s, r#"{"cmd":"dump"}"#);
    assert!(reply.contains("\"ok\":true"), "post-visit dump reply: {}", reply);
    let dump = std::fs::read_to_string("/tmp/debug.emTreeDump").expect("dump file");

    // Structural assertions.
    assert!(dump.contains("Root Context:"), "must contain root context rec");
    assert!(
        dump.contains("View (Context):"),
        "must contain at least one view rec"
    );
    // After Phase 0's port fix + Phase 3's cascade, sub-views appear
    // as child contexts of the home view. The cosmos sub-view's content
    // tree must include emDirPanel rec(s) for the home directory.
    assert!(
        dump.contains("emDirPanel"),
        "after visiting home, dump must contain emDirPanel rec"
    );
    assert!(
        dump.matches("View (Context):").count() >= 2,
        "must contain outer view + at least one sub-view, got {} matches",
        dump.matches("View (Context):").count()
    );

    // get_state round-trip.
    let gs = send(&mut s, r#"{"cmd":"get_state"}"#);
    assert!(gs.contains("focused_view"), "get_state reply: {}", gs);
    assert!(gs.contains("focused_identity"), "get_state reply: {}", gs);

    // Clean shutdown.
    send(&mut s, r#"{"cmd":"quit"}"#);
    let _ = child.wait();
}
