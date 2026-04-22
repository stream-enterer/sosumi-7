# Phase 4d — emRec Persistence IO — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Port the emRec persistence stack: `emRecReader`, `emRecWriter`, `emRecFileReader`, `emRecFileWriter`, `emRecMemReader`, `emRecMemWriter`. Wire `emConfigModel::LoadAndSave` through the IO classes.

**Architecture:** Each `emRec` concrete type ported in Phases 4a through 4c gains `TryRead(&mut dyn emRecReader, ctx) -> Result<()>` and `TryWrite(&mut dyn emRecWriter) -> Result<()>` methods. The reader/writer pair serializes to a textual format byte-for-byte compatible with C++ `emRec`-file format.

**Companion:** spec §7 D7.1 (Phase 4d scope). C++ reference: `emRec.h` / `emRec.cpp` for the IO classes.

**JSON entries closed:** none (E026 at Phase 4e).

**Phase-specific invariants (C4):**
- **I4d-1.** Files `emRecReader.rs`, `emRecWriter.rs`, `emRecFileReader.rs`, `emRecFileWriter.rs`, `emRecMemReader.rs`, `emRecMemWriter.rs` exist.
- **I4d-2.** Round-trip test: serialize → parse → re-serialize produces identical bytes for every concrete type from Phases 4a through 4c (primitives, listener tree, Color/Alignment migration, structural compounds).
- **I4d-3.** A compatibility test reads a known C++-produced `emRec` file (committed fixture from `/home/a0/git/eaglemode-0.96.4/` test data) and asserts the parsed values match an expected set.
- **I4d-4.** `emConfigModel::LoadAndSave` method exists and wires through the IO classes.

**Entry-precondition.** Phase 4c Closeout COMPLETE.

> **Drift note (2026-04-20, post-phase-1.76):** Significant pre-existing Rust persistence code exists that this plan was written without knowledge of:
> - `crates/emcore/src/emRec.rs` already implements C++-text-format parsing (`parse_rec`, `write_rec`).
> - `crates/emcore/src/emRecFileModel.rs` already implements file-backed load/save.
> - `crates/emcore/src/emConfigModel.rs` already implements `TryLoad`, `Save`, `TryLoadOrInstall`, `Set`/`modify`.
>
> Before Tasks 2–6 execute, each task must begin with a **pre-audit beat**: compare the proposed work against what's already implemented and identify the delta. The C++-fidelity direction is correct; the implementation gap is likely smaller than the plan assumes. Decide per-task whether to (a) port-new types alongside existing code (e.g. add `emRecFileReader`/`emRecFileWriter` as new types, keeping `emRecFileModel` separate), or (b) rewrite existing types to match C++ shape. Document the per-task decision in the phase ledger at task-start.

---

## Bootstrap

Run B1–B12 with `<N>` = `4d`. At B3 read C++ `emRec.cpp` sections for reader/writer.

---

## File Structure

**New files:**
- `crates/emcore/src/emRecReader.rs` — trait.
- `crates/emcore/src/emRecWriter.rs` — trait.
- `crates/emcore/src/emRecFileReader.rs`, `emRecFileWriter.rs` — file-backed.
- `crates/emcore/src/emRecMemReader.rs`, `emRecMemWriter.rs` — memory-backed.

**Modified:** every Phase-4a/4b rec file gains `TryRead`/`TryWrite` impls. `crates/emcore/src/emConfigModel.rs` (locate or create) wires `LoadAndSave`.

---

## Task 1: `emRecReader` / `emRecWriter` traits

- [ ] **Step 1: Write failing test.** In a new `emRecReader.rs` inline `#[cfg(test)]` module, construct a minimal `emRecMemReader` (stub) and verify it implements `emRecReader` trait.

- [ ] **Step 2: FAIL.**
- [ ] **Step 3: Implement** the two traits:
```rust
pub trait emRecReader {
    fn read_bool(&mut self) -> Result<bool, RecIoError>;
    fn read_int(&mut self) -> Result<i64, RecIoError>;
    fn read_double(&mut self) -> Result<f64, RecIoError>;
    fn read_string(&mut self) -> Result<String, RecIoError>;
    fn read_identifier(&mut self) -> Result<String, RecIoError>;
    // additional primitives mirroring C++ emRecReader
}
pub trait emRecWriter { /* symmetric */ }
```
- [ ] **Step 4: PASS.** Commit.

---

## Task 2: `emRecMemWriter` + `emRecMemReader` (byte-format compatible)

**Pre-audit (per Drift note):** read the existing implementation before writing the failing test. The delta may be narrower than the task prose suggests.

Mirror the C++ text format exactly. Start with a round-trip test for `emBoolRec` → bytes → `emBoolRec` check.

- [ ] **Step 1: Write failing round-trip test.**
```rust
#[test]
fn bool_rec_roundtrip() {
    let mut fixture = TestFixture::new();
    let rec = emBoolRec::new(&mut fixture.init_ctx(), true);
    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    let mut r = emRecMemReader::new(&bytes);
    let mut rec2 = emBoolRec::new(&mut fixture.init_ctx(), false);
    rec2.TryRead(&mut r, &mut fixture.sched_ctx()).unwrap();
    assert_eq!(rec2.GetValue(), rec.GetValue());
}
```
- [ ] **Step 2: FAIL.** Implement MemReader/MemWriter + `TryRead`/`TryWrite` for `emBoolRec`. Follow C++ format exactly.
- [ ] **Step 3: PASS.** Commit.

---

## Task 3: Extend round-trip to every concrete type

For each of: `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`, `emFlagsRec`, `emAlignmentRec`, `emColorRec`, `emStructRec`, `emUnionRec`, `emTArrayRec`.

- [ ] **Step 1–3** per type: failing round-trip test, implement TryRead/TryWrite, pass.

- [ ] **Step 4: Commit one large commit.**
```bash
git commit -m "phase-4c: TryRead/TryWrite for every concrete emRec type"
```

---

## Task 4: `emRecFileReader` / `emRecFileWriter` (file-backed)

Wraps the Mem variants with a file handle. Buffers to memory, reads from disk on demand.

- [ ] **Step 1: Failing test:** write `emBoolRec` to a temp file, read back, assert.
- [ ] **Step 2–4.**

---

## Task 5: Compatibility test with C++-produced fixture

- [ ] **Step 1:** Generate a fixture by running the C++ Eagle Mode tool that saves an emRec (or commit a pre-existing config file from `/home/a0/git/eaglemode-0.96.4/etc/`).
- [ ] **Step 2:** Parse with Rust `emRecFileReader`; assert expected values.
- [ ] **Step 3:** If byte-format mismatches, fix the reader until compat.
- [ ] **Step 4:** Commit with the fixture file added to `tests/data/`.

---

## Task 6: `emConfigModel::LoadAndSave`

**Pre-audit (per Drift note):** read the existing implementation before writing the failing test. The delta may be narrower than the task prose suggests.

**Files:**
- Create or modify: `crates/emcore/src/emConfigModel.rs`.

- [ ] **Step 1: Failing test.** `LoadAndSave` round-trip on a config-shaped emStructRec.
- [ ] **Step 2: Implement.** Wire through `emRecFileReader`/`emRecFileWriter`.
- [ ] **Step 3: PASS.**
- [ ] **Step 4: Commit.**

---

## Task 7: Full gate + invariants.

---

## Closeout

Run C1–C11 with `<N>` = `4d`. No JSON entries close yet.
