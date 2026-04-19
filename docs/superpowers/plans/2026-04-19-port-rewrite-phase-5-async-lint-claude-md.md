# Phase 5 — Async Plugins + Annotations Lint + CLAUDE.md Deltas — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Three deliverables. (1) Port `emFileModel` async loading; replace the `load_image_from_file` synchronous stub with a real scheduler-engine-driven `emImageFileModel` matching C++. (2) Add `cargo xtask annotations` binary that validates every `DIVERGED:` block cites a P4 forced category and every `RUST_ONLY:` block cites a P5 charter category; wire into pre-commit hook and CI. (3) Apply the CLAUDE.md amendments specified in §9.1, §9.2, §9.3. Re-audit every in-tree DIVERGED block and rewrite rationales to conform to the new vocabulary.

**Architecture:** (1) `emImageFileModel` registers a scheduler engine that loads images off the hot cycle; file IO happens inside the engine's `Cycle` via `std::fs` (synchronous but scheduler-yielded, matching C++'s cooperative model). The engine fires a `load_complete_signal` when done. (2) `cargo xtask annotations` is a stable-Rust binary in a new `crates/xtask` crate that ripgrep-scans every `DIVERGED:` / `RUST_ONLY:` match and parses out the category tag. Missing tag → non-zero exit. (3) CLAUDE.md edits are textual per spec §9 and wholly mechanical.

**Companion:** spec §7 D7.5, §9, §10.5, P5/P6 enforcement.

**JSON entries closed:** E028, E035, E037.

**Phase-specific invariants (C4):**
- **I5a.** `load_image_from_file` sync stub removed from `crates/emcore/src/emImageFile.rs`; `emImageFileModel` ported with a real scheduler engine.
- **I5b.** `cargo xtask annotations` exists, runs, and passes on the tree.
- **I5c.** Every `DIVERGED:` block in-tree carries one of `(language-forced)`, `(dependency-forced)`, `(upstream-gap-forced)`, `(performance-forced)` category tags. `cargo xtask annotations --check-diverged-categories` passes.
- **I5d.** Every `RUST_ONLY:` block in-tree carries one of `(language-forced-utility)`, `(performance-forced-alternative)`, `(dependency-forced)` category tags.
- **I5e.** CLAUDE.md §"Code Rules" ownership line replaced (per spec §9.1). New §"Annotation Vocabulary" section added (per §9.2). §"Port Ideology" forced-divergence test replaced (per §9.3).
- **I5f.** Pre-commit hook runs `cargo xtask annotations`.
- **I3.** (Deferred from Phase 3) every DIVERGED block carries a forced-category tag — completes the coverage started in Phase 3.
- **I4.** (Deferred) every RUST_ONLY block carries a charter tag.
- **I6.** Goldens preserved.

**Entry-precondition.** Phase 4d Closeout COMPLETE.

---

## Bootstrap

Run B1–B12 with `<N>` = `5`.

---

## File Structure

**New files:**
- `crates/xtask/Cargo.toml` and `crates/xtask/src/main.rs` — xtask binary.
- `crates/xtask/src/annotations.rs` — annotation-lint logic.
- Workspace: add `xtask` member to root `Cargo.toml`'s `[workspace].members`.

**Heavy modifications:**
- `crates/emcore/src/emImageFile.rs` — delete `load_image_from_file` sync path; replace with `emImageFileModel` registration-and-cycle.
- `crates/emcore/src/emImageFileModel.rs` (create or heavily modify) — the real model.
- `CLAUDE.md` (root) — three edits per spec §9.
- `.git/hooks/pre-commit` or equivalent — add `cargo xtask annotations`.

**Tree-wide edits:**
- Every `DIVERGED:` block (enumerate first via `rg -n 'DIVERGED:' crates/`) gets a category tag.
- Every `RUST_ONLY:` block gets a charter tag.

---

## Task 1: Create `crates/xtask` workspace crate

**Files:**
- Create: `crates/xtask/Cargo.toml`, `crates/xtask/src/main.rs`.
- Modify: root `Cargo.toml` to add to workspace.

- [ ] **Step 1: Scaffold.**
```toml
# crates/xtask/Cargo.toml
[package]
name = "xtask"
version = "0.0.1"
edition = "2021"
publish = false

[[bin]]
name = "xtask"
path = "src/main.rs"

[dependencies]
```

```rust
// crates/xtask/src/main.rs
use std::process::ExitCode;

mod annotations;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("annotations") => annotations::run(args),
        Some(cmd) => {
            eprintln!("xtask: unknown subcommand '{cmd}'");
            ExitCode::from(2)
        }
        None => {
            eprintln!("usage: cargo xtask <annotations>");
            ExitCode::from(2)
        }
    }
}
```

- [ ] **Step 2: Register in root `Cargo.toml`** workspace members.

- [ ] **Step 3: Write failing test.** `crates/xtask/src/annotations.rs` contains a placeholder `pub fn run` that returns `ExitCode::from(1)`; a unit test asserts the real implementation passes.

- [ ] **Step 4: Verify `cargo xtask annotations` runs** (returns non-zero because placeholder): `cargo xtask annotations; echo $?` — expect `1`.

- [ ] **Step 5: Commit.**
```bash
git add crates/xtask Cargo.toml
git commit -m "phase-5: scaffold cargo xtask binary"
```

---

## Task 2: Implement annotation lint

**Files:** `crates/xtask/src/annotations.rs`.

- [ ] **Step 1: Define valid categories.**
```rust
const DIVERGED_CATEGORIES: &[&str] = &[
    "language-forced", "dependency-forced", "upstream-gap-forced", "performance-forced",
];
const RUST_ONLY_CATEGORIES: &[&str] = &[
    "language-forced-utility", "performance-forced-alternative", "dependency-forced",
];
```

- [ ] **Step 2: Implement the scan.**
```rust
pub fn run(_args: std::env::Args) -> std::process::ExitCode {
    let mut failures = Vec::new();
    for hit in scan("DIVERGED:", DIVERGED_CATEGORIES) {
        failures.push(hit);
    }
    for hit in scan("RUST_ONLY:", RUST_ONLY_CATEGORIES) {
        failures.push(hit);
    }
    if failures.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        for f in &failures { eprintln!("{}", f); }
        std::process::ExitCode::from(1)
    }
}

fn scan(tag: &str, valid_categories: &[&str]) -> Vec<String> {
    // Invoke ripgrep-style scan over crates/**/*.rs files.
    // For each line matching `<tag>`, look at the same line or the
    // next 3 lines for a `(<category>)` literal. If none found, record
    // the hit as a failure.
    let mut failures = Vec::new();
    let walker = walkdir::WalkDir::new("crates")
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().map_or(false, |x| x == "rs"));
    for entry in walker {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let lines: Vec<_> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if let Some(idx) = line.find(tag) {
                let window = lines[i..(i+4).min(lines.len())].join("\n");
                if !valid_categories.iter().any(|c| window.contains(&format!("({c})"))) {
                    failures.push(format!(
                        "{}:{} {} missing category tag",
                        entry.path().display(), i+1, tag
                    ));
                }
                let _ = idx;
            }
        }
    }
    failures
}
```

(Add `walkdir` to xtask's `Cargo.toml`.)

- [ ] **Step 3: Write failing lint test.** The lint initially will fail because the tree has DIVERGED blocks without category tags. Capture the full list as Task 3 input.

- [ ] **Step 4:**
```bash
cargo run -p xtask -- annotations 2>&1 | tee /tmp/anno-failures.txt
```

Expected: many failures. The failure list becomes the work-list for Task 3.

- [ ] **Step 5: Commit the lint implementation.**
```bash
git add crates/xtask/src/annotations.rs crates/xtask/Cargo.toml
git commit -m "phase-5: implement cargo xtask annotations lint"
```

---

## Task 3: Retrofit category tags onto every DIVERGED / RUST_ONLY block

- [ ] **Step 1: Enumerate.** `cargo run -p xtask -- annotations 2>&1 > /tmp/anno-failures.txt`. Count: `wc -l /tmp/anno-failures.txt`.

- [ ] **Step 2: For each failing site, classify and annotate.**

For each DIVERGED block, read its rationale and decide which P4 category applies:
- "Rust has no virtual inheritance" → `(language-forced)`
- "wgpu/winit API can't admit this shape" → `(dependency-forced)`
- "C++ ships as a no-op on this platform" → `(upstream-gap-forced)`
- "benchmark shows X% regression" → `(performance-forced)` (only if a real benchmark exists; otherwise reclassify)

Edit the comment to include the tag:
```rust
// DIVERGED: (language-forced) Rust has no virtual inheritance; ...
```

For each RUST_ONLY block similarly with P5 charters.

This is a bulk edit — expect ~30–60 sites across the tree. Do not remove any comment unless the rationale is now wrong (in which case investigate; the rewrite might belong to one of the earlier phases as a fidelity-bug).

- [ ] **Step 3: Re-run lint until green.**
```bash
cargo run -p xtask -- annotations
echo $?
```
Must print `0`.

- [ ] **Step 4: Commit in grouped commits by crate.**
```bash
git add crates/emcore/
git commit -m "phase-5: DIVERGED/RUST_ONLY category tags in emcore"
git add crates/eaglemode/
git commit -m "phase-5: DIVERGED/RUST_ONLY category tags in eaglemode"
# etc per crate
```

---

## Task 4: Wire `cargo xtask annotations` into pre-commit hook

**Files:** `.git/hooks/pre-commit` or workspace-level hook manager (per project's existing setup).

- [ ] **Step 1:** Read current hook (CLAUDE.md states: runs `cargo fmt`, `clippy -D warnings`, `cargo-nextest ntr`).
- [ ] **Step 2:** Add `cargo xtask annotations` after the cargo-nextest step.
- [ ] **Step 3:** Run `git commit --allow-empty -m "phase-5 hook test"` on a branch to confirm the hook fires, then reset.
- [ ] **Step 4:** If CI config exists (`.github/workflows/*.yml` or similar), add `cargo xtask annotations` there.
- [ ] **Step 5: Commit.**

---

## Task 5: Port `emImageFileModel` async loading

**Files:** `crates/emcore/src/emImageFile.rs`, `crates/emcore/src/emImageFileModel.rs`.

- [ ] **Step 1: Write failing test.** In `crates/eaglemode/tests/behavioral/image_file_model.rs`:
```rust
#[test]
fn image_model_loads_asynchronously_via_engine() {
    let mut fixture = TestFixture::new();
    let path = fixture.fixture_image_path("test.tga");
    let model = emImageFileModel::register(&fixture.root_context, &path);
    // On register, model is Loading state; has not loaded yet.
    assert_eq!(model.state(), ModelState::Loading);
    // Run a tick.
    fixture.framework.scheduler.DoTimeSlice(&mut fixture.framework.windows, &fixture.framework.root_context);
    // After the engine's Cycle, the image is loaded.
    assert_eq!(model.state(), ModelState::Loaded);
    assert!(model.image().is_some());
}
```

- [ ] **Step 2: FAIL** (current stub loads synchronously on register).

- [ ] **Step 3: Implement the model.**

Define:
```rust
pub struct emImageFileModel {
    path: PathBuf,
    state: State,                 // Loading / Loaded / Failed
    image: Option<emImage>,
    load_complete_signal: SignalId,
    engine_id: EngineId,
}

struct LoaderEngine {
    model_weak: Weak<RefCell<emImageFileModel>>,   // chartered §3.6(a) cross-engine handle
}
impl emEngine for LoaderEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) {
        if let Some(model_rc) = self.model_weak.upgrade() {
            let mut m = model_rc.borrow_mut();
            if m.state == State::Loading {
                match load_image_sync(&m.path) {
                    Ok(img) => { m.image = Some(img); m.state = State::Loaded; }
                    Err(_)  => { m.state = State::Failed; }
                }
                ctx.fire(m.load_complete_signal);
                ctx.remove_engine(m.engine_id);  // one-shot
            }
        }
    }
}
```

Registration:
```rust
impl emImageFileModel {
    pub fn register<C: ConstructCtx>(ctx: &mut C, path: &Path) -> Rc<RefCell<Self>> {
        let signal = ctx.create_signal();
        let model = Rc::new(RefCell::new(Self {
            path: path.to_owned(),
            state: State::Loading,
            image: None,
            load_complete_signal: signal,
            engine_id: EngineId::INVALID,
        }));
        let loader = LoaderEngine { model_weak: Rc::downgrade(&model) };
        let eid = ctx.register_engine(Box::new(loader), Priority::Low);
        ctx.wake_up(eid);
        model.borrow_mut().engine_id = eid;
        model
    }
}
```

- [ ] **Step 4: Delete `load_image_from_file` stub + its callers.** Callers now go through `emImageFileModel::register`.

- [ ] **Step 5: Test passes.** Commit.

---

## Task 6: CLAUDE.md edits

**Files:** `/home/a0/git/eaglemode-rs/CLAUDE.md`.

- [ ] **Step 1: §9.1 — Replace ownership line.**

Find:
```
- **Ownership**: `Rc`/`RefCell` shared state, `Weak` parent refs.
```
Replace with spec §9.1's new wording (the full paragraph).

- [ ] **Step 2: §9.2 — Insert `## Annotation Vocabulary` section.** Place after `## File and Name Correspondence`. Copy the body from spec §9.2 verbatim.

- [ ] **Step 3: §9.3 — Replace Forced divergence bullet** in `## Port Ideology` with the four-category test from spec §9.3.

- [ ] **Step 4: Commit.**
```bash
git add CLAUDE.md
git commit -m "phase-5: CLAUDE.md ownership, annotation vocabulary, forced-divergence test"
```

---

## Task 7: Re-audit every in-tree DIVERGED block for P4 conformance

- [ ] **Step 1:** `rg -n 'DIVERGED:' crates/ | wc -l` — capture count.

- [ ] **Step 2:** For each block, verify that its rationale actually matches the category tag applied in Task 3. If a block claims `(language-forced)` but the rationale reads "would be awkward in Rust", that's a fidelity-bug — file an issue (or fix inline if trivial).

- [ ] **Step 3:** `cargo run -p xtask -- annotations` must pass.

- [ ] **Step 4:** Commit any fixes.

---

## Task 8: Full gate + invariants

- [ ] **Step 1:**
```bash
cargo fmt --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo-nextest ntr && \
cargo test --test golden -- --test-threads=1 && \
cargo run -p xtask -- annotations
```

- [ ] **Step 2: Invariants.**
```bash
rg -q 'fn load_image_from_file' crates/emcore/src/ && echo "I5a FAIL" || echo "I5a PASS"
[ -x "$(cargo run -p xtask -- annotations --help 2>/dev/null; echo yes)" ] || cargo run -p xtask -- annotations >/dev/null 2>&1 && echo "I5b PASS" || echo "I5b FAIL"
grep -q 'Plain owned values are the default' CLAUDE.md && echo "I5e-9.1 PASS" || echo "I5e-9.1 FAIL"
grep -q '## Annotation Vocabulary' CLAUDE.md && echo "I5e-9.2 PASS" || echo "I5e-9.2 FAIL"
grep -q 'language-forced' CLAUDE.md && echo "I5e-9.3 PASS" || echo "I5e-9.3 FAIL"
```

- [ ] **Step 3: Proceed to Closeout.**

---

## Closeout

Run C1–C11 with `<N>` = `5`. At C5 close **E028** (Task 5), **E035** and **E037** (Tasks 2–4 + 7 — governance entries are now effective because the lint is enforcing).

At C11, additionally announce: **"Port Ownership Rewrite series COMPLETE."** Tag `port-rewrite-series-complete`.
