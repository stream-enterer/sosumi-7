# Phase 5: Wire cargo xtask annotations into Pre-Commit Hook

## Change

Added `cargo xtask annotations` to `.git/hooks/pre-commit` after the nextest step.

### Rationale

The `cargo xtask annotations` command ensures every commit with Rust source changes validates that all DIVERGED: and RUST_ONLY: annotation blocks have category tags. This is a quality gate that runs after clippy and tests in the pre-commit hook's `NEEDS_GATE` block.

### Implementation

Modified `.git/hooks/pre-commit`:
- Placed `cargo xtask annotations` in the `if [ "$NEEDS_GATE" -eq 1 ]` block after the nextest step
- Only runs on commits that touch non-documentation files (same gate as clippy and tests)
- Docs-only commits skip this step (as they skip clippy/tests)

### Verification

- Hook syntax validated: `bash -n .git/hooks/pre-commit` ✓
- Annotations command tested: `cargo xtask annotations` exit code 0 ✓
- Hook will fire on next commit with Rust changes ✓

### Note

The pre-commit hook itself lives in `.git/hooks/pre-commit` which is not version-controlled. This documentation file serves as the permanent record of the hook change.
