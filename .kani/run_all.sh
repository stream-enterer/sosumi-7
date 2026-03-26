#!/usr/bin/env bash
# run_all.sh — Run all Kani harnesses individually, collect results into JSON.
#
# Works around Kani ICE on complex async dependencies by running each
# harness in isolation via --harness. Batches results into a single JSON.
#
# Usage: .kani/run_all.sh
# Output: .kani/results.json

set -uo pipefail

RESULTS=".kani/results.json"
PROOFS_MANUAL="tests/kani/proofs.rs"
PROOFS_GEN="tests/kani/proofs_generated.rs"
TIMEOUT_PER_HARNESS=60

# Collect all harness names from both proof files
harnesses=()
for f in "$PROOFS_MANUAL" "$PROOFS_GEN"; do
  [ -f "$f" ] || continue
  while IFS= read -r line; do
    name=$(echo "$line" | grep -oP 'fn\s+\K\w+')
    [ -n "$name" ] && harnesses+=("$name")
  done < <(grep '^fn \|^    fn ' "$f" | grep -v '//')
done

# Also get harnesses via the #[kani::proof] pattern (more reliable)
harnesses=()
for f in "$PROOFS_MANUAL" "$PROOFS_GEN"; do
  [ -f "$f" ] || continue
  prev_was_proof=false
  while IFS= read -r line; do
    if echo "$line" | grep -q '#\[kani::proof\]'; then
      prev_was_proof=true
      continue
    fi
    if [ "$prev_was_proof" = true ]; then
      name=$(echo "$line" | grep -oP 'fn\s+\K\w+')
      [ -n "$name" ] && harnesses+=("$name")
      prev_was_proof=false
    fi
  done < "$f"
done

total=${#harnesses[@]}
echo "Found $total harnesses to run."
echo "["  > "$RESULTS"

i=0
passed=0
failed=0
errors=0

for h in "${harnesses[@]}"; do
  i=$((i + 1))
  printf "[%3d/%d] %-60s " "$i" "$total" "$h"

  output=$(timeout "$TIMEOUT_PER_HARNESS" cargo kani --harness "$h" 2>&1)
  exit_code=$?

  if [ $exit_code -eq 124 ]; then
    status="timeout"
    errors=$((errors + 1))
    printf "TIMEOUT\n"
  elif echo "$output" | grep -q 'VERIFICATION:- SUCCESSFUL'; then
    status="verified"
    passed=$((passed + 1))
    time_s=$(echo "$output" | grep -oP 'Verification Time: \K[0-9.]+' || echo "0")
    printf "OK (${time_s}s)\n"
  elif echo "$output" | grep -q 'VERIFICATION:- FAILED'; then
    status="failed"
    failed=$((failed + 1))
    # Extract the failing check
    failing=$(echo "$output" | grep 'Failed Checks:' | head -1 | sed 's/.*Failed Checks: //')
    printf "FAILED: %s\n" "$failing"
  elif echo "$output" | grep -q 'error'; then
    status="compile_error"
    errors=$((errors + 1))
    err_msg=$(echo "$output" | grep 'error' | head -1 | cut -c1-80)
    printf "ERROR: %s\n" "$err_msg"
  else
    status="unknown"
    errors=$((errors + 1))
    printf "UNKNOWN\n"
  fi

  # Write JSON entry
  [ $i -gt 1 ] && echo "," >> "$RESULTS"
  time_s=$(echo "$output" | grep -oP 'Verification Time: \K[0-9.]+' || echo "null")
  failing_check=$(echo "$output" | grep 'Failed Checks:' | head -1 | sed 's/.*Failed Checks: //' | sed 's/"/\\"/g')
  printf '  {"harness": "%s", "status": "%s", "time_s": %s, "detail": "%s"}' \
    "$h" "$status" "${time_s:-null}" "${failing_check:-}" >> "$RESULTS"
done

echo "" >> "$RESULTS"
echo "]" >> "$RESULTS"

echo ""
echo "════════════════════════════════════════════"
echo "  KANI RESULTS"
echo "════════════════════════════════════════════"
echo "  Total:    $total"
echo "  Verified: $passed"
echo "  Failed:   $failed"
echo "  Errors:   $errors"
echo "  Output:   $RESULTS"
echo "════════════════════════════════════════════"
