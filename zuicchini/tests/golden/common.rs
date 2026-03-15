use std::fmt;
use std::io::Write;
use std::path::PathBuf;

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

// ────────────────────── Painter golden files ──────────────────────

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

// ────────────────────── Image comparison ──────────────────────

/// Compare two RGBA images pixel-by-pixel on RGB channels only.
///
/// The alpha channel is **excluded** because C++ emPainter uses channel 3 to
/// track "remaining canvas visibility" (not standard compositing alpha), while
/// the Rust painter stores standard alpha.  The visual output (RGB) is what
/// matters for parity.
///
/// `name`: stable test identifier emitted in JSONL output (see `MEASURE_DIVERGENCE`).
/// `channel_tolerance`: max per-channel absolute diff allowed per pixel.
/// `max_failure_pct`: max percentage of pixels that may exceed tolerance.
///
/// # Measurement mode
///
/// Two independent env vars control output, usable separately or together:
///
/// - `MEASURE_DIVERGENCE=1` — emit one JSONL record per call to **stderr**.
/// - `DIVERGENCE_LOG=<path>` — **append** one JSONL record per call to `<path>`.
///   Safe to use with parallel test threads: each write is a single `write(2)`
///   syscall in append mode, which is atomic on Linux for records this small.
///
/// Each record:
/// ```text
/// {"test":"<name>","tol":<u8>,"fail":<n>,"total":<n>,"pct":<f>,"max_diff":<u8>,"pass":<bool>}
/// ```
/// Run with `--test-threads=1` for deterministic ordering.
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

    let mut fail_count = 0usize;
    let mut max_diff: u8 = 0;
    let mut first_failures: Vec<(usize, usize, usize)> = Vec::new();

    for i in 0..total {
        let off = i * 4;
        let mut pixel_fail = false;
        // Compare RGB only (channels 0-2), skip alpha (channel 3)
        for ch in 0..3 {
            let diff = actual[off + ch].abs_diff(expected[off + ch]);
            if diff > channel_tolerance {
                pixel_fail = true;
                max_diff = max_diff.max(diff);
            }
        }
        if pixel_fail {
            fail_count += 1;
            if first_failures.len() < 10 {
                first_failures.push((i % width as usize, i / width as usize, off));
            }
        }
    }

    let fail_pct = fail_count as f64 / total as f64 * 100.0;
    let measure = std::env::var("MEASURE_DIVERGENCE").map_or(false, |v| v == "1");
    let log_path = std::env::var("DIVERGENCE_LOG").ok();
    if measure || log_path.is_some() {
        let pass = fail_pct <= max_failure_pct;
        let line = format!(
            r#"{{"test":{name:?},"tol":{channel_tolerance},"fail":{fail_count},"total":{total},"pct":{fail_pct:.4},"max_diff":{max_diff},"pass":{pass}}}"#
        );
        if measure {
            eprintln!("{line}");
        }
        if let Some(ref path) = log_path {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
    }
    if fail_pct > max_failure_pct {
        let mut msg = format!(
            "Image mismatch: {fail_count}/{total} pixels ({fail_pct:.2}%) exceed tolerance \
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

// ────────────────────── Image dump helpers ────────────────────────

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
    std::env::var("DUMP_GOLDEN").map_or(false, |v| v == "1")
}

/// Dump actual, expected, and diff images for a test.
pub fn dump_test_images(name: &str, actual: &[u8], expected: &[u8], w: u32, h: u32) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("debug");
    std::fs::create_dir_all(&dir).expect("Cannot create tests/golden/debug/");
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
/// In emCore, parent width = 1.0 and all four (x,y,w,h) are in that unit space.
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
    actual: &[(bool, bool)],
    expected: &[GoldenPanelState],
    panel_names: &[&str],
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        return Err(CompareError {
            message: format!(
                "Panel count mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    for (i, ((a_active, a_path), e)) in actual.iter().zip(expected.iter()).enumerate() {
        let name = panel_names.get(i).copied().unwrap_or("?");
        if *a_active != e.is_active || *a_path != e.in_active_path {
            return Err(CompareError {
                message: format!(
                    "Panel {i} ({name}) mismatch:\n  \
                     actual =(active={a_active}, in_path={a_path})\n  \
                     expected=(active={}, in_path={})",
                    e.is_active, e.in_active_path
                ),
            });
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
/// C++ and Rust use different bit positions:
///   C++ NF_CHILD_LIST_CHANGED    = 1<<0  → Rust CHILDREN_CHANGED     = 0x08
///   C++ NF_LAYOUT_CHANGED        = 1<<1  → Rust LAYOUT_CHANGED       = 0x01
///   C++ NF_VIEWING_CHANGED       = 1<<2  → Rust VISIBILITY           = 0x04
///   C++ NF_ENABLE_CHANGED        = 1<<3  → Rust ENABLE_CHANGED       = 0x40
///   C++ NF_ACTIVE_CHANGED        = 1<<4  → Rust ACTIVE_CHANGED       = 0x100
///   C++ NF_FOCUS_CHANGED         = 1<<5  → Rust FOCUS_CHANGED        = 0x02
///   C++ NF_VIEW_FOCUS_CHANGED    = 1<<6  → Rust VIEW_FOCUS_CHANGED   = 0x200
///   C++ NF_UPDATE_PRIORITY_CHANGED = 1<<7 → Rust UPDATE_PRIORITY_CHANGED = 0x400
///   C++ NF_MEMORY_LIMIT_CHANGED  = 1<<8  → Rust MEMORY_LIMIT_CHANGED = 0x800
///   C++ NF_SOUGHT_NAME_CHANGED   = 1<<9  → Rust SOUGHT_NAME_CHANGED  = 0x80
pub fn translate_cpp_notice_flags(cpp: u32) -> u32 {
    let mut rust: u32 = 0;
    if cpp & (1 << 0) != 0 {
        rust |= 0x08;
    } // CHILDREN_CHANGED
    if cpp & (1 << 1) != 0 {
        rust |= 0x01;
    } // LAYOUT_CHANGED
    if cpp & (1 << 2) != 0 {
        rust |= 0x04;
    } // VISIBILITY
    if cpp & (1 << 3) != 0 {
        rust |= 0x40;
    } // ENABLE_CHANGED
    if cpp & (1 << 4) != 0 {
        rust |= 0x100;
    } // ACTIVE_CHANGED
    if cpp & (1 << 5) != 0 {
        rust |= 0x02;
    } // FOCUS_CHANGED
    if cpp & (1 << 6) != 0 {
        rust |= 0x200;
    } // VIEW_FOCUS_CHANGED
    if cpp & (1 << 7) != 0 {
        rust |= 0x400;
    } // UPDATE_PRIORITY_CHANGED
    if cpp & (1 << 8) != 0 {
        rust |= 0x800;
    } // MEMORY_LIMIT_CHANGED
    if cpp & (1 << 9) != 0 {
        rust |= 0x80;
    } // SOUGHT_NAME_CHANGED
    rust
}

/// Full mask covering all 12 notice flag bits.
pub const NOTICE_FULL_MASK: u32 = 0x0FFF;

/// Compare actual Rust NoticeFlags against C++ golden notice data.
/// `mask` filters which bits are compared (use NOTICE_ACTION_MASK or NOTICE_FULL_MASK).
pub fn compare_notices(
    actual: &[u32],
    expected: &[GoldenNoticeState],
    panel_names: &[&str],
    mask: u32,
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        return Err(CompareError {
            message: format!(
                "Panel count mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        let name = panel_names.get(i).copied().unwrap_or("?");
        let translated = translate_cpp_notice_flags(e.cpp_flags) & mask;
        let masked_actual = *a & mask;
        if masked_actual != translated {
            return Err(CompareError {
                message: format!(
                    "Panel {i} ({name}) notice mismatch (mask=0x{mask:04x}):\n  \
                     actual  =0x{masked_actual:04x} (rust bits, masked)\n  \
                     expected=0x{translated:04x} (translated from C++ 0x{:04x}, masked)",
                    e.cpp_flags
                ),
            });
        }
    }
    Ok(())
}

// ────────────────────── Input golden files ──────────────────────

#[derive(Debug, Clone)]
pub struct GoldenInputState {
    pub received_input: bool,
    pub is_active: bool,
    pub in_active_path: bool,
}

/// Load an input golden file. Returns per-panel input/activation state.
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

/// Compare input/activation state against golden.
/// `check_received`: if true, also compare whether the panel received input.
pub fn compare_input(
    actual: &[(bool, bool, bool)],
    expected: &[GoldenInputState],
    panel_names: &[&str],
    check_received: bool,
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        return Err(CompareError {
            message: format!(
                "Panel count mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    for (i, ((a_recv, a_active, a_path), e)) in actual.iter().zip(expected.iter()).enumerate() {
        let name = panel_names.get(i).copied().unwrap_or("?");
        let recv_mismatch = check_received && *a_recv != e.received_input;
        if recv_mismatch || *a_active != e.is_active || *a_path != e.in_active_path {
            return Err(CompareError {
                message: format!(
                    "Panel {i} ({name}) input mismatch:\n  \
                     actual  =(recv={a_recv}, active={a_active}, path={a_path})\n  \
                     expected=(recv={}, active={}, path={})",
                    e.received_input, e.is_active, e.in_active_path
                ),
            });
        }
    }
    Ok(())
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

/// Compare trajectory against golden data. Returns error with details on first mismatch.
pub fn compare_trajectory(
    actual: &[TrajectoryStep],
    expected: &[TrajectoryStep],
    tolerance: f64,
) -> Result<(), CompareError> {
    if actual.len() != expected.len() {
        return Err(CompareError {
            message: format!(
                "Trajectory length mismatch: actual={} expected={}",
                actual.len(),
                expected.len()
            ),
        });
    }
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        let dx = (a.vel_x - e.vel_x).abs();
        let dy = (a.vel_y - e.vel_y).abs();
        let dz = (a.vel_z - e.vel_z).abs();
        if dx > tolerance || dy > tolerance || dz > tolerance {
            return Err(CompareError {
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
    Ok(())
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
