#!/usr/bin/env bash
# harness_correspondence.sh — Verify C++ header ↔ Rust file mapping.
#
# Pattern: spec-as-test-feedback-loop, deterministic-security-scanning-build-loop
# Requirements: V7 (identify uncovered code), M3 (coverage measurement)
#
# Enforces the File and Name Correspondence rules from CLAUDE.md:
#   - Every .h in C++ include/emCore/ has exactly one of: .rs or .no_rust_equivalent
#   - Every .rs in src/emCore/ (except mod.rs) has a matching .h, .rust_only, or SPLIT: comment
#
# Usage: .harness/harness_correspondence.sh [OUTDIR]

set -euo pipefail

CPP_DIR="${CPP_DIR:-$HOME/git/eaglemode-0.96.4/include/emCore}"
RS_DIR="src/emCore"
OUTDIR="${1:-}"
ERRORS=0
WARNINGS=0

fail()    { echo "FAIL: $1" >&2; ERRORS=$((ERRORS + 1)); }
warn()    { echo "WARN: $1" >&2; WARNINGS=$((WARNINGS + 1)); }

# ── Check 1: Every .h → .rs or .no_rust_equivalent ───────────────────────────

h_count=0
for h in "$CPP_DIR"/*.h; do
  [ -f "$h" ] || continue
  h_count=$((h_count + 1))
  base=$(basename "$h" .h)
  if [ ! -f "$RS_DIR/$base.rs" ] && [ ! -f "$RS_DIR/$base.no_rust_equivalent" ]; then
    fail "$base.h has no .rs or .no_rust_equivalent"
  fi
done

# ── Check 2: Every .rs → .h or .rust_only or SPLIT: ──────────────────────────

rs_count=0
for rs in "$RS_DIR"/*.rs; do
  [ -f "$rs" ] || continue
  base=$(basename "$rs" .rs)
  [ "$base" = "mod" ] && continue
  # Kani proofs moved to tests/kani/
  rs_count=$((rs_count + 1))
  if [ ! -f "$CPP_DIR/$base.h" ] && [ ! -f "$RS_DIR/$base.rust_only" ]; then
    if ! head -5 "$rs" | grep -q 'SPLIT:'; then
      fail "$base.rs has no C++ header, no .rust_only, and no SPLIT: comment"
    fi
  fi
done

# ── Statistics ────────────────────────────────────────────────────────────────

no_equiv=$(ls "$RS_DIR"/*.no_rust_equivalent 2>/dev/null | wc -l | tr -d ' ')
rust_only=$(ls "$RS_DIR"/*.rust_only 2>/dev/null | wc -l | tr -d ' ')

report=$(cat << EOF
{
  "cpp_headers": $h_count,
  "rust_files": $rs_count,
  "no_rust_equivalent": $no_equiv,
  "rust_only": $rust_only,
  "errors": $ERRORS,
  "warnings": $WARNINGS
}
EOF
)

echo "$report" | jq .

if [ -n "$OUTDIR" ]; then
  echo "$report" > "$OUTDIR/correspondence.json"
fi

if [ $ERRORS -gt 0 ]; then
  echo "Correspondence audit failed with $ERRORS error(s)." >&2
  exit 1
fi

echo "Correspondence audit passed: $h_count C++ headers, $rs_count Rust files, $no_equiv exempt, $rust_only Rust-only."
