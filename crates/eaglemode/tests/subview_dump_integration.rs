//! Integration test for the F010-unblocking sub-view crossing work.
//! Gated `#[ignore]` because it requires a live display (X11 or Xvfb)
//! and a compiled `eaglemode` binary.
//!
//! What this test asserts:
//! 1. The outer view paints + the home window settles (via `wait_idle`).
//! 2. Visiting the `content view` emSubViewPanel makes it `Viewed` (proves
//!    `Visit` dispatches the outer view, the wrapper engine cycles, and the
//!    SVP propagates `viewed` correctly — F010's structural fix).
//! 3. The `content view`'s sub-view tree contains an `emVirtualCosmosPanel`
//!    record after the visit (proves the inner sub-view's wrapper engine
//!    cycled and `update_children` ran inside the sub-view tree).
//!
//! What this test does NOT assert:
//! - That cosmos auto-expanded its child items (`home`, `fs`, ...) or that
//!   any `emDirPanel` exists. Reaching those requires cascading view-
//!   condition-driven auto-expansion (zoom into cosmos → cosmos expands →
//!   zoom into `home` item → its emFileLinkPanel expands → its emDirPanel
//!   appears + loads). The control channel today exposes `Visit` /
//!   `VisitFullsized` but Phase 2 calibration showed those alone do not
//!   drive a deep enough zoom to cross all three thresholds before timeout
//!   in this harness. F015 reverses the prior unreachable assertion; a
//!   follow-up should add cascade primitives (synthetic scroll-zoom input,
//!   or an explicit "zoom-and-wait" command) and a separate test that
//!   asserts emDirPanel.

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
        .env("EMCORE_DLOG", "1")
        .spawn()
        .expect("spawn eaglemode");
    let pid = child.id();
    let sock_path = PathBuf::from(format!("/tmp/eaglemode-rs.{}.sock", pid));

    match wait_for_socket(&sock_path) {
        Some(s) => (child, s),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("socket did not appear within 60s at {:?}", sock_path);
        }
    }
}

fn wait_for_socket(sock_path: &PathBuf) -> Option<UnixStream> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if sock_path.exists() {
            if let Ok(s) = UnixStream::connect(sock_path) {
                return Some(s);
            }
        }
        if Instant::now() > deadline {
            return None;
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

    // Settle the StartupEngine's initial animation.
    let reply = send(&mut s, r#"{"cmd":"wait_idle","timeout_ms":30000}"#);
    assert!(
        reply.contains("\"ok\":true"),
        "startup wait_idle: {}",
        reply
    );

    // Baseline dump — confirms the dump endpoint works and the home window
    // is fully constructed (root context + outer view + 2 sub-views =
    // 3 view contexts).
    let reply = send(&mut s, r#"{"cmd":"dump"}"#);
    assert!(reply.contains("\"ok\":true"), "baseline dump: {}", reply);
    let baseline = std::fs::read_to_string("/tmp/debug.emTreeDump").expect("dump file");
    assert!(
        baseline.contains("Root Context:"),
        "baseline must contain root context"
    );
    assert!(
        baseline.matches("View (Context):").count() >= 3,
        "baseline must contain outer view + control-view + content-view sub-views, got {}",
        baseline.matches("View (Context):").count()
    );

    // Visit the content view SVP, then wait until it reports `viewed=true`.
    // This is the F010 structural assertion: visit propagates to the outer
    // view, the SVP becomes part of the viewed path, and its inner wrapper
    // engine wakes up to cycle the sub-tree.
    let reply = send(&mut s, r#"{"cmd":"visit","identity":"root:content view"}"#);
    assert!(
        reply.contains("\"ok\":true"),
        "visit content view: {}",
        reply
    );
    let reply = send(
        &mut s,
        r#"{"cmd":"wait_for","condition":{"kind":"panel_viewed","identity":"root:content view"},"timeout_ms":60000}"#,
    );
    assert!(
        reply.contains("\"ok\":true"),
        "wait_for panel_viewed content view: {}",
        reply
    );

    // Let the sub-view's scheduler quiesce so AutoExpand inside the sub-view
    // has a chance to run (creates emVirtualCosmosPanel under the unnamed
    // sub-view root).
    let reply = send(&mut s, r#"{"cmd":"wait_idle","timeout_ms":10000}"#);
    assert!(
        reply.contains("\"ok\":true"),
        "post-visit wait_idle: {}",
        reply
    );

    // Post-visit dump.
    let reply = send(&mut s, r#"{"cmd":"dump"}"#);
    assert!(reply.contains("\"ok\":true"), "post-visit dump: {}", reply);
    let dump = std::fs::read_to_string("/tmp/debug.emTreeDump").expect("dump file");

    // The content view's sub-view tree must now contain the cosmos panel
    // that emMainWindow installs as the sub-tree root behavior. Its
    // appearance proves the inner wrapper engine cycled at least once
    // after the visit (F010's fix).
    assert!(
        dump.contains("emVirtualCosmos::emVirtualCosmosPanel"),
        "post-visit dump must contain emVirtualCosmosPanel inside the content sub-view tree"
    );

    // Sanity: same view-context count as baseline (we did not lose any).
    assert!(
        dump.matches("View (Context):").count() >= 3,
        "post-visit dump must still contain ≥3 view contexts, got {}",
        dump.matches("View (Context):").count()
    );

    // get_state round-trip — proves focused-state queries work after the
    // visit settles. focused_view/focused_identity may be absent when no
    // panel is focused (the post-visit state has focus on the outer view's
    // root, which is reported via view_rect rather than focused_*); only
    // assert ok+view_rect.
    let gs = send(&mut s, r#"{"cmd":"get_state"}"#);
    assert!(gs.contains("\"ok\":true"), "get_state reply: {}", gs);
    assert!(gs.contains("view_rect"), "get_state reply: {}", gs);

    // Clean shutdown.
    send(&mut s, r#"{"cmd":"quit"}"#);
    let _ = child.wait();
}
