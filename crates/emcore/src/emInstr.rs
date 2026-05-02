// RUST_ONLY: (language-forced-utility) Instrumentation primitive for hang
// debugging (2026-05-02 plan). Bypasses Rust static state because
// emfileman/emstocks are dlopen'd cdylibs that get independent copies
// of every `static`/`OnceCell`/`thread_local`. The single shared
// channel is a raw file descriptor inherited from the launcher shell
// via the `EM_INSTR_FD` env var. Each cdylib reads the env var on
// first use and calls `libc::write(2)` directly — atomic across
// handles for writes <= PIPE_BUF (4096).

use std::cell::Cell;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

static INSTR_FD: AtomicI32 = AtomicI32::new(-1);
static INSTR_INIT: std::sync::Once = std::sync::Once::new();

fn fd() -> i32 {
    INSTR_INIT.call_once(|| {
        if let Ok(s) = std::env::var("EM_INSTR_FD") {
            if let Ok(n) = s.parse::<i32>() {
                INSTR_FD.store(n, Ordering::Relaxed);
            }
        }
    });
    INSTR_FD.load(Ordering::Relaxed)
}

/// Append one line to the shared instrumentation fd. No buffering.
/// Caller must ensure `line` ends with `\n` and is <= 4096 bytes.
pub fn write_line(line: &str) {
    let f = fd();
    if f < 0 {
        return;
    }
    let bytes = line.as_bytes();
    debug_assert!(bytes.len() <= 4096, "instrumentation line exceeds PIPE_BUF");
    unsafe {
        libc::write(f, bytes.as_ptr() as *const _, bytes.len());
    }
}

/// Per-slice counters. Reset at slice entry, dumped at slice exit.
/// `Cell` is sufficient (single-threaded scheduler).
pub struct SliceCounters {
    pub cycled: Cell<u32>,
    pub fire_pushes: Cell<u32>,
    pub timer_pushes: Cell<u32>,
    pub direct_wakes: Cell<u32>,
    pub drain_pushes: Cell<u64>,
    pub max_pending_after_drain: Cell<u32>,
    pub stay_awake_rearms: Cell<u32>,
    pub distinct_engines: Cell<u32>,
    pub loud_armed: Cell<bool>,
    pub loud_disarm_at: Cell<u64>,
}

impl SliceCounters {
    pub const fn new() -> Self {
        Self {
            cycled: Cell::new(0),
            fire_pushes: Cell::new(0),
            timer_pushes: Cell::new(0),
            direct_wakes: Cell::new(0),
            drain_pushes: Cell::new(0),
            max_pending_after_drain: Cell::new(0),
            stay_awake_rearms: Cell::new(0),
            distinct_engines: Cell::new(0),
            loud_armed: Cell::new(false),
            loud_disarm_at: Cell::new(0),
        }
    }

    pub fn reset(&self) {
        self.cycled.set(0);
        self.fire_pushes.set(0);
        self.timer_pushes.set(0);
        self.direct_wakes.set(0);
        self.drain_pushes.set(0);
        self.max_pending_after_drain.set(0);
        self.stay_awake_rearms.set(0);
        self.distinct_engines.set(0);
    }
}

impl Default for SliceCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Re-entry guard: distinguishes wake_up_engine called from within
/// process_pending_signals (signal-driven) vs called from anywhere else
/// (direct). Counter (not bool) so nested calls compose. Single-threaded
/// scheduler so AtomicU64 is overkill but harmless.
pub static IN_PROCESS_PENDING: AtomicU64 = AtomicU64::new(0);

pub fn enter_process_pending() {
    IN_PROCESS_PENDING.fetch_add(1, Ordering::Relaxed);
}

pub fn exit_process_pending() {
    IN_PROCESS_PENDING.fetch_sub(1, Ordering::Relaxed);
}

pub fn in_process_pending() -> bool {
    IN_PROCESS_PENDING.load(Ordering::Relaxed) > 0
}
