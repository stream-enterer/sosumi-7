use std::fmt;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Test helper: owns the data needed to construct a `SchedCtx`.
/// Use `.with(|sc| ...)` to call ctx-taking emView / emViewAnimator methods in
/// golden tests that don't have a full TestHarness.
pub struct TestSched {
    sched: emcore::emScheduler::EngineScheduler,
    fw: Vec<emcore::emEngineCtx::DeferredAction>,
    ctx: std::rc::Rc<emcore::emContext::emContext>,
}
impl TestSched {
    pub fn new() -> Self {
        Self {
            sched: emcore::emScheduler::EngineScheduler::new(),
            fw: Vec::new(),
            ctx: emcore::emContext::emContext::NewRoot(),
        }
    }
    pub fn sched_mut(&mut self) -> &mut emcore::emScheduler::EngineScheduler {
        &mut self.sched
    }
    /// Return a construction-context bundle suitable for widget::new.
    /// Phase-3 B3.4b: widget constructors now take `&mut impl ConstructCtx`.
    pub fn cc(&mut self) -> emcore::emEngineCtx::InitCtx<'_> {
        emcore::emEngineCtx::InitCtx {
            scheduler: &mut self.sched,
            framework_actions: &mut self.fw,
            root_context: &self.ctx,
        }
    }

    pub fn with<R>(&mut self, f: impl FnOnce(&mut emcore::emEngineCtx::SchedCtx<'_>) -> R) -> R {
        let __cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut self.sched,
            framework_actions: &mut self.fw,
            root_context: &self.ctx,
            framework_clipboard: &__cb,
            current_engine: None,
        };
        f(&mut sc)
    }
}

/// Path to the current divergence log file.
/// Rotates on first use: `divergence.jsonl` → `divergence.prev.jsonl`.
fn divergence_log_path() -> &'static PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("golden-divergence");
        let _ = std::fs::create_dir_all(&dir);
        let current = dir.join("divergence.jsonl");
        let prev = dir.join("divergence.prev.jsonl");
        if current.exists() {
            let _ = std::fs::rename(&current, &prev);
        }
        current
    })
}

/// Append a JSONL divergence record to `tests/golden/divergence.jsonl`.
fn emit_divergence(line: &str) {
    let path = divergence_log_path();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

#[derive(Debug)]
pub struct CompareError {
    pub message: String,
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("data")
}

/// Returns true if golden directory exists (generator has been run).
pub fn golden_available() -> bool {
    golden_dir().is_dir()
}

// ────────────────────── emPainter golden files ──────────────────────

/// Load a painter golden file. Returns (width, height, rgba_bytes).
pub fn load_painter_golden(name: &str) -> (u32, u32, Vec<u8>) {
    let path = golden_dir()
        .join("painter")
        .join(format!("{name}.painter.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(data.len() >= 8, "Golden file too short: {}", path.display());
    let width = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let height = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let expected_len = 8 + (width as usize * height as usize * 4);
    assert_eq!(
        data.len(),
        expected_len,
        "Golden file size mismatch for {name}: got {} expected {expected_len}",
        data.len()
    );
    (width, height, data[8..].to_vec())
}

// ────────────────────── Compositor golden files ──────────────────────

/// Load a compositor golden file. Returns (width, height, rgba_bytes).
pub fn load_compositor_golden(name: &str) -> (u32, u32, Vec<u8>) {
    let path = golden_dir()
        .join("compositor")
        .join(format!("{name}.compositor.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(data.len() >= 8, "Golden file too short: {}", path.display());
    let width = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let height = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let expected_len = 8 + (width as usize * height as usize * 4);
    assert_eq!(
        data.len(),
        expected_len,
        "Golden file size mismatch for {name}: got {} expected {expected_len}",
        data.len()
    );
    (width, height, data[8..].to_vec())
}

// ────────────────────── emImage comparison ──────────────────────

/// Compare two RGBA images pixel-by-pixel on RGB channels only.
///
/// The alpha channel is **excluded** because C++ emPainter uses channel 3 to
/// track "remaining canvas visibility" (not standard compositing alpha), while
/// the Rust painter stores standard alpha.  The visual output (RGB) is what
/// matters for parity.
///
/// `name`: stable test identifier emitted in the divergence JSONL log.
/// `channel_tolerance`: max per-channel absolute diff allowed per pixel.
/// `max_failure_pct`: max percentage of pixels that may exceed tolerance.
///
/// # Measurement GetMode
///
/// Two independent env vars control output, usable separately or together:
///
/// # Divergence log
///
/// Every call appends a tol=0 JSONL record to `tests/golden/divergence.jsonl`
/// (previous run rotated to `divergence.prev.jsonl`):
///
/// ```text
/// {"test":"<name>","fail":<n>,"total":<n>,"pct":<f>,"max_diff":<u8>}
/// ```
///
/// Pass/fail of the test itself still uses the caller-supplied tolerance.
pub fn compare_images(
    name: &str,
    actual: &[u8],
    expected: &[u8],
    width: u32,
    height: u32,
    channel_tolerance: u8,
    max_failure_pct: f64,
) -> Result<(), CompareError> {
    let total = (width * height) as usize;
    assert_eq!(actual.len(), total * 4);
    assert_eq!(expected.len(), total * 4);

    let mut zero_tol_fail = 0usize;
    let mut max_diff: u8 = 0;
    let mut fail_count = 0usize;
    let mut first_failures: Vec<(usize, usize, usize)> = Vec::new();

    for i in 0..total {
        let off = i * 4;
        let mut pixel_max = 0u8;
        for ch in 0..3 {
            pixel_max = pixel_max.max(actual[off + ch].abs_diff(expected[off + ch]));
        }
        max_diff = max_diff.max(pixel_max);
        if pixel_max > 0 {
            zero_tol_fail += 1;
        }
        if pixel_max > channel_tolerance {
            fail_count += 1;
            if first_failures.len() < 10 {
                first_failures.push((i % width as usize, i / width as usize, off));
            }
        }
    }

    let zero_pct = zero_tol_fail as f64 / total as f64 * 100.0;
    emit_divergence(&format!(
        r#"{{"test":{name:?},"fail":{zero_tol_fail},"total":{total},"pct":{zero_pct:.4},"max_diff":{max_diff}}}"#
    ));

    let fail_pct = fail_count as f64 / total as f64 * 100.0;
    if fail_pct > max_failure_pct {
        let mut msg = format!(
            "emImage mismatch: {fail_count}/{total} pixels ({fail_pct:.2}%) exceed tolerance \
             {channel_tolerance}, max_diff={max_diff}\n"
        );
        for &(x, y, off) in &first_failures {
            msg += &format!(
                "  ({x},{y}): actual=rgb({},{},{}) expected=rgb({},{},{})\n",
                actual[off],
                actual[off + 1],
                actual[off + 2],
                expected[off],
                expected[off + 1],
                expected[off + 2],
            );
        }
        Err(CompareError { message: msg })
    } else {
        Ok(())
    }
}

// ────────────────────── emImage dump helpers ────────────────────────

/// Write RGBA data as PPM (P6 binary) file, dropping the alpha channel.
pub fn dump_ppm(path: &str, data: &[u8], w: u32, h: u32) {
    let mut f = std::fs::File::create(path).expect("Cannot create PPM file");
    write!(f, "P6\n{w} {h}\n255\n").unwrap();
    for i in 0..(w as usize * h as usize) {
        let off = i * 4;
        f.write_all(&data[off..off + 3]).unwrap();
    }
}

/// Write a diff visualization: green = match, red = mismatch, brightness = diff magnitude.
pub fn dump_diff_ppm(path: &str, actual: &[u8], expected: &[u8], w: u32, h: u32) {
    let total = w as usize * h as usize;
    let mut rgb = vec![0u8; total * 3];
    for i in 0..total {
        let off = i * 4;
        let max_ch_diff = (0..3)
            .map(|ch| actual[off + ch].abs_diff(expected[off + ch]))
            .max()
            .unwrap_or(0);
        if max_ch_diff > 1 {
            // Red channel = diff magnitude (amplified for visibility)
            rgb[i * 3] = (max_ch_diff as u16 * 4).min(255) as u8;
            rgb[i * 3 + 1] = 0;
            rgb[i * 3 + 2] = 0;
        } else {
            // Green = matching pixel, dim
            let luma =
                ((actual[off] as u16 + actual[off + 1] as u16 + actual[off + 2] as u16) / 3) as u8;
            rgb[i * 3] = luma / 4;
            rgb[i * 3 + 1] = luma / 2;
            rgb[i * 3 + 2] = luma / 4;
        }
    }
    let mut f = std::fs::File::create(path).expect("Cannot create diff PPM file");
    write!(f, "P6\n{w} {h}\n255\n").unwrap();
    f.write_all(&rgb).unwrap();
}

/// Returns true if DUMP_GOLDEN=1 env var is set.
pub fn dump_golden_enabled() -> bool {
    std::env::var("DUMP_GOLDEN").is_ok_and(|v| v == "1")
}

/// Dump actual, expected, and diff images for a test.
pub fn dump_test_images(name: &str, actual: &[u8], expected: &[u8], w: u32, h: u32) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("golden-debug");
    std::fs::create_dir_all(&dir).expect("Cannot create target/golden-debug/");
    let dir = dir.to_str().unwrap();
    dump_ppm(&format!("{dir}/actual_{name}.ppm"), actual, w, h);
    dump_ppm(&format!("{dir}/expected_{name}.ppm"), expected, w, h);
    dump_diff_ppm(&format!("{dir}/diff_{name}.ppm"), actual, expected, w, h);
    eprintln!("  Dumped: {dir}/{{actual,expected,diff}}_{name}.ppm");
}

/// Print detailed pixel comparison for a specific coordinate.
pub fn _trace_pixel(name: &str, actual: &[u8], expected: &[u8], w: u32, x: u32, y: u32) {
    let off = ((y * w + x) * 4) as usize;
    let ar = actual[off];
    let ag = actual[off + 1];
    let ab = actual[off + 2];
    let aa = actual[off + 3];
    let er = expected[off];
    let eg = expected[off + 1];
    let eb = expected[off + 2];
    let ea = expected[off + 3];
    eprintln!(
        "TRACE {name} ({x},{y}):\n  actual   = rgba({ar},{ag},{ab},{aa})\n  expected = rgba({er},{eg},{eb},{ea})\n  diff     = ({},{},{},{})",
        ar as i16 - er as i16,
        ag as i16 - eg as i16,
        ab as i16 - eb as i16,
        aa as i16 - ea as i16,
    );
}

/// Analyze diff distribution: bucket by max channel diff.
pub fn analyze_diff_distribution(
    actual: &[u8],
    expected: &[u8],
    w: u32,
    h: u32,
    channel_tolerance: u8,
) {
    let total = (w * h) as usize;
    let mut buckets = [0usize; 8]; // 2-3, 4-7, 8-15, 16-31, 32-63, 64-127, 128-191, 192-255
    let mut total_fail = 0usize;
    for i in 0..total {
        let off = i * 4;
        let max_d = (0..3)
            .map(|ch| actual[off + ch].abs_diff(expected[off + ch]))
            .max()
            .unwrap_or(0);
        if max_d > channel_tolerance {
            total_fail += 1;
            let bucket = if max_d <= 3 {
                0
            } else if max_d <= 7 {
                1
            } else if max_d <= 15 {
                2
            } else if max_d <= 31 {
                3
            } else if max_d <= 63 {
                4
            } else if max_d <= 127 {
                5
            } else if max_d <= 191 {
                6
            } else {
                7
            };
            buckets[bucket] += 1;
        }
    }
    eprintln!("DIFF DISTRIBUTION ({total_fail} failing pixels):");
    let labels = [
        "2-3", "4-7", "8-15", "16-31", "32-63", "64-127", "128-191", "192-255",
    ];
    for (label, &count) in labels.iter().zip(buckets.iter()) {
        if count > 0 {
            eprintln!(
                "  diff {label}: {count} pixels ({:.2}%)",
                count as f64 / total as f64 * 100.0
            );
        }
    }
}

// ────────────────────── Layout golden files ──────────────────────

#[derive(Debug, Clone)]
pub struct GoldenRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn load_layout_golden(name: &str) -> Vec<GoldenRect> {
    let path = golden_dir()
        .join("layout")
        .join(format!("{name}.layout.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(data.len() >= 4);
    let child_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let expected_len = 4 + child_count * 32;
    assert_eq!(
        data.len(),
        expected_len,
        "Layout golden size mismatch for {name}"
    );

    let mut rects = Vec::with_capacity(child_count);
    for i in 0..child_count {
        let off = 4 + i * 32;
        let x = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        let y = f64::from_le_bytes(data[off + 8..off + 16].try_into().unwrap());
        let w = f64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap());
        let h = f64::from_le_bytes(data[off + 24..off + 32].try_into().unwrap());
        rects.push(GoldenRect { x, y, w, h });
    }
    rects
}

/// Scale golden rects from emCore normalized coords to absolute coords.
/// In emCore, parent_context width = 1.0 and all four (x,y,w,h) are in that unit space.
pub fn scale_golden_rects(rects: &mut [GoldenRect], parent_width: f64) {
    for r in rects.iter_mut() {
        r.x *= parent_width;
        r.y *= parent_width;
        r.w *= parent_width;
        r.h *= parent_width;
    }
}

// ────────────────────── Behavioral golden files ──────────────────

#[derive(Debug, Clone)]
pub struct GoldenPanelState {
    pub is_active: bool,
    pub in_active_path: bool,
}

/// Load a behavioral golden file. Returns a list of panel states in DFS order.
pub fn load_behavioral_golden(name: &str) -> Vec<GoldenPanelState> {
    let path = golden_dir()
        .join("behavioral")
        .join(format!("{name}.behavioral.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(
        data.len() >= 4,
        "Behavioral golden too short: {}",
        path.display()
    );
    let num_panels = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let expected_len = 4 + num_panels * 2;
    assert_eq!(
        data.len(),
        expected_len,
        "Behavioral golden size mismatch for {name}: got {} expected {expected_len}",
        data.len()
    );
    let mut panels = Vec::with_capacity(num_panels);
    for i in 0..num_panels {
        let off = 4 + i * 2;
        panels.push(GoldenPanelState {
            is_active: data[off] != 0,
            in_active_path: data[off + 1] != 0,
        });
    }
    panels
}

/// Compare behavioral state against golden. Panel order must match DFS traversal.
pub fn compare_behavioral(
    test_name: &str,
    actual: &[(bool, bool)],
    expected: &[GoldenPanelState],
    panel_names: &[&str],
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        let msg = format!(
            "Panel count mismatch: actual={} expected={}",
            actual.len(),
            expected.len()
        );
        emit_divergence(&format!(
            r#"{{"test":{test_name:?},"type":"behavioral","panels":{0},"expected":{1},"pass":false}}"#,
            actual.len(),
            expected.len()
        ));
        return Err(CompareError { message: msg });
    }
    let mut mismatches = 0;
    for ((a_active, a_path), e) in actual.iter().zip(expected.iter()) {
        if *a_active != e.is_active || *a_path != e.in_active_path {
            mismatches += 1;
        }
    }
    let pass = mismatches == 0;
    emit_divergence(&format!(
        r#"{{"test":{test_name:?},"type":"behavioral","panels":{},"mismatches":{mismatches},"pass":{pass}}}"#,
        actual.len()
    ));
    if !pass {
        // Find first mismatch for error message
        for (i, ((a_active, a_path), e)) in actual.iter().zip(expected.iter()).enumerate() {
            let pname = panel_names.get(i).copied().unwrap_or("?");
            if *a_active != e.is_active || *a_path != e.in_active_path {
                return Err(CompareError {
                    message: format!(
                        "Panel {i} ({pname}) mismatch:\n  \
                         actual =(active={a_active}, in_path={a_path})\n  \
                         expected=(active={}, in_path={})",
                        e.is_active, e.in_active_path
                    ),
                });
            }
        }
    }
    Ok(())
}

// ────────────────────── Notice golden files ──────────────────────

#[derive(Debug, Clone)]
pub struct GoldenNoticeState {
    /// Raw C++ NF_* bit flags accumulated for this panel.
    pub cpp_flags: u32,
}

/// Load a notice golden file. Returns per-panel accumulated C++ notice flags.
pub fn load_notice_golden(name: &str) -> Vec<GoldenNoticeState> {
    let path = golden_dir()
        .join("notice")
        .join(format!("{name}.notice.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(
        data.len() >= 4,
        "Notice golden too short: {}",
        path.display()
    );
    let num_panels = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let expected_len = 4 + num_panels * 4;
    assert_eq!(
        data.len(),
        expected_len,
        "Notice golden size mismatch for {name}: got {} expected {expected_len}",
        data.len()
    );
    let mut panels = Vec::with_capacity(num_panels);
    for i in 0..num_panels {
        let off = 4 + i * 4;
        let flags = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
        panels.push(GoldenNoticeState { cpp_flags: flags });
    }
    panels
}

/// Translate C++ NF_* bit flags to Rust NoticeFlags.
///
/// Since Phase 7, Rust NoticeFlags bit values match C++ one-for-one
/// (CHILD_LIST_CHANGED=1<<0 ... SOUGHT_NAME_CHANGED=1<<9), so this is
/// an identity translation.
pub fn translate_cpp_notice_flags(cpp: u32) -> u32 {
    cpp
}

/// Full mask covering all 10 notice flag bits (bits 0-9).
pub const NOTICE_FULL_MASK: u32 = 0x03FF;

/// Compare actual Rust NoticeFlags against C++ golden notice data.
/// `mask` GetFilters which bits are compared (use NOTICE_ACTION_MASK or NOTICE_FULL_MASK).
pub fn compare_notices(
    test_name: &str,
    actual: &[u32],
    expected: &[GoldenNoticeState],
    panel_names: &[&str],
    mask: u32,
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        emit_divergence(&format!(
            r#"{{"test":{test_name:?},"type":"notice","panels":{0},"expected":{1},"pass":false}}"#,
            actual.len(),
            expected.len()
        ));
        return Err(CompareError {
            message: format!(
                "Panel count mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    let mut mismatches = 0;
    let mut first_err: Option<CompareError> = None;
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        let name = panel_names.get(i).copied().unwrap_or("?");
        let translated = translate_cpp_notice_flags(e.cpp_flags) & mask;
        let masked_actual = *a & mask;
        if masked_actual != translated {
            mismatches += 1;
            if first_err.is_none() {
                first_err = Some(CompareError {
                    message: format!(
                        "Panel {i} ({name}) notice mismatch (mask=0x{mask:04x}):\n  \
                         actual  =0x{masked_actual:04x} (rust bits, masked)\n  \
                         expected=0x{translated:04x} (translated from C++ 0x{:04x}, masked)",
                        e.cpp_flags
                    ),
                });
            }
        }
    }
    let pass = mismatches == 0;
    emit_divergence(&format!(
        r#"{{"test":{test_name:?},"type":"notice","panels":{},"mismatches":{mismatches},"pass":{pass}}}"#,
        actual.len()
    ));
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// ────────────────────── Input golden files ──────────────────────

#[derive(Debug, Clone)]
pub struct GoldenInputState {
    pub received_input: bool,
    pub is_active: bool,
    pub in_active_path: bool,
}

/// Load an Input golden file. Returns per-panel Input/activation state.
pub fn load_input_golden(name: &str) -> Vec<GoldenInputState> {
    let path = golden_dir()
        .join("input")
        .join(format!("{name}.input.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(
        data.len() >= 4,
        "Input golden too short: {}",
        path.display()
    );
    let num_panels = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let expected_len = 4 + num_panels * 3;
    assert_eq!(
        data.len(),
        expected_len,
        "Input golden size mismatch for {name}: got {} expected {expected_len}",
        data.len()
    );
    let mut panels = Vec::with_capacity(num_panels);
    for i in 0..num_panels {
        let off = 4 + i * 3;
        panels.push(GoldenInputState {
            received_input: data[off] != 0,
            is_active: data[off + 1] != 0,
            in_active_path: data[off + 2] != 0,
        });
    }
    panels
}

/// Compare Input/activation state against golden.
/// `check_received`: if true, also compare whether the panel received Input.
pub fn compare_input(
    test_name: &str,
    actual: &[(bool, bool, bool)],
    expected: &[GoldenInputState],
    panel_names: &[&str],
    check_received: bool,
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        emit_divergence(&format!(
            r#"{{"test":{test_name:?},"type":"input","panels":{0},"expected":{1},"pass":false}}"#,
            actual.len(),
            expected.len()
        ));
        return Err(CompareError {
            message: format!(
                "Panel count mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    let mut mismatches = 0;
    let mut first_err: Option<CompareError> = None;
    for (i, ((a_recv, a_active, a_path), e)) in actual.iter().zip(expected.iter()).enumerate() {
        let name = panel_names.get(i).copied().unwrap_or("?");
        let recv_mismatch = check_received && *a_recv != e.received_input;
        if recv_mismatch || *a_active != e.is_active || *a_path != e.in_active_path {
            mismatches += 1;
            if first_err.is_none() {
                first_err = Some(CompareError {
                    message: format!(
                        "Panel {i} ({name}) Input mismatch:\n  \
                         actual  =(recv={a_recv}, active={a_active}, path={a_path})\n  \
                         expected=(recv={}, active={}, path={})",
                        e.received_input, e.is_active, e.in_active_path
                    ),
                });
            }
        }
    }
    let pass = mismatches == 0;
    emit_divergence(&format!(
        r#"{{"test":{test_name:?},"type":"input","panels":{},"mismatches":{mismatches},"pass":{pass}}}"#,
        actual.len()
    ));
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// ────────────────────── Trajectory golden files ──────────────────

/// A single trajectory step: (vel_x, vel_y, vel_z) velocity in pixels/second.
#[derive(Debug, Clone)]
pub struct TrajectoryStep {
    pub vel_x: f64,
    pub vel_y: f64,
    pub vel_z: f64,
}

/// Load a trajectory golden file.
/// Format: [u32 step_count][step_count * (f64 vel_x, f64 vel_y, f64 vel_z)]
pub fn load_trajectory_golden(name: &str) -> Vec<TrajectoryStep> {
    let path = golden_dir()
        .join("trajectory")
        .join(format!("{name}.trajectory.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(
        data.len() >= 4,
        "Trajectory golden too short: {}",
        path.display()
    );
    let step_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let expected_len = 4 + step_count * 24; // 3 * f64 = 24 bytes per step
    assert_eq!(
        data.len(),
        expected_len,
        "Trajectory golden size mismatch for {name}: got {} expected {expected_len}",
        data.len()
    );
    let mut steps = Vec::with_capacity(step_count);
    for i in 0..step_count {
        let off = 4 + i * 24;
        let vel_x = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        let vel_y = f64::from_le_bytes(data[off + 8..off + 16].try_into().unwrap());
        let vel_z = f64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap());
        steps.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }
    steps
}

/// Save a trajectory as golden data (same binary format as C++ generator).
pub fn save_trajectory_golden(name: &str, steps: &[TrajectoryStep]) {
    let dir = golden_dir().join("trajectory");
    std::fs::create_dir_all(&dir).expect("Cannot create trajectory golden dir");
    let path = dir.join(format!("{name}.trajectory.golden"));
    let mut data = Vec::with_capacity(4 + steps.len() * 24);
    data.extend_from_slice(&(steps.len() as u32).to_le_bytes());
    for s in steps {
        data.extend_from_slice(&s.vel_x.to_le_bytes());
        data.extend_from_slice(&s.vel_y.to_le_bytes());
        data.extend_from_slice(&s.vel_z.to_le_bytes());
    }
    std::fs::write(&path, &data).unwrap_or_else(|e| panic!("Cannot write {}: {e}", path.display()));
    eprintln!("Saved trajectory golden: {}", path.display());
}

/// Compare trajectory against golden data. Returns error with details on first mismatch.
pub fn compare_trajectory(
    test_name: &str,
    actual: &[TrajectoryStep],
    expected: &[TrajectoryStep],
    tolerance: f64,
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        emit_divergence(&format!(
            r#"{{"test":{test_name:?},"type":"trajectory","steps":{0},"expected":{1},"tol":{tolerance:.2e},"pass":false}}"#,
            actual.len(),
            expected.len()
        ));
        return Err(CompareError {
            message: format!(
                "Trajectory length mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    let mut max_diff: f64 = 0.0;
    let mut fail_steps = 0;
    let mut first_err: Option<CompareError> = None;
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        let dx = (a.vel_x - e.vel_x).abs();
        let dy = (a.vel_y - e.vel_y).abs();
        let dz = (a.vel_z - e.vel_z).abs();
        let step_max = dx.max(dy).max(dz);
        max_diff = max_diff.max(step_max);
        if dx > tolerance || dy > tolerance || dz > tolerance {
            fail_steps += 1;
            if first_err.is_none() {
                first_err = Some(CompareError {
                    message: format!(
                        "Trajectory step {i} mismatch (tol={tolerance:.2e}):\n  \
                         actual  =({:.10e}, {:.10e}, {:.10e})\n  \
                         expected=({:.10e}, {:.10e}, {:.10e})\n  \
                         diff    =({dx:.2e}, {dy:.2e}, {dz:.2e})",
                        a.vel_x, a.vel_y, a.vel_z, e.vel_x, e.vel_y, e.vel_z
                    ),
                });
            }
        }
    }
    let pass = fail_steps == 0;
    emit_divergence(&format!(
        r#"{{"test":{test_name:?},"type":"trajectory","steps":{},"fail":{fail_steps},"tol":{tolerance:.2e},"max_diff":{max_diff:.6e},"pass":{pass}}}"#,
        actual.len()
    ));
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// ────────────────────── Rect comparison ──────────────────────────

pub fn compare_rects(
    actual: &[(f64, f64, f64, f64)],
    expected: &[GoldenRect],
    eps: f64,
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        return Err(CompareError {
            message: format!(
                "Rect count mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        let dx = (a.0 - e.x).abs();
        let dy = (a.1 - e.y).abs();
        let dw = (a.2 - e.w).abs();
        let dh = (a.3 - e.h).abs();
        if dx > eps || dy > eps || dw > eps || dh > eps {
            return Err(CompareError {
                message: format!(
                    "Rect {i} mismatch:\n  actual =({:.6},{:.6},{:.6},{:.6})\n  \
                     expected=({:.6},{:.6},{:.6},{:.6})\n  \
                     diffs   =({dx:.2e},{dy:.2e},{dw:.2e},{dh:.2e})",
                    a.0, a.1, a.2, a.3, e.x, e.y, e.w, e.h
                ),
            });
        }
    }
    Ok(())
}

// ────────────────────── Widget state comparison ──────────────────────

/// Compare widget state fields against golden data and emit JSONL.
///
/// `checks` is a list of `(field_name, passed, detail)` tuples where `detail`
/// describes the mismatch when `passed` is false (e.g. "actual=0 expected=1").
/// Emits one JSONL line per test with type "widget_state".
pub fn compare_widget_state(
    test_name: &str,
    checks: &[(&str, bool, String)],
) -> Result<(), CompareError> {
    let total = checks.len();
    let failures: Vec<_> = checks.iter().filter(|(_, ok, _)| !ok).collect();
    let pass = failures.is_empty();
    emit_divergence(&format!(
        r#"{{"test":{test_name:?},"type":"widget_state","checks":{total},"failures":{},"pass":{pass}}}"#,
        failures.len()
    ));
    if !pass {
        let (field, _, detail) = failures[0];
        Err(CompareError {
            message: format!("{test_name}: {field} mismatch — {detail}"),
        })
    } else {
        Ok(())
    }
}
