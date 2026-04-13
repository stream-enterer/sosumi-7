#!/usr/bin/env bash
# Verify a golden test end-to-end: dump Rust ops, diff against C++ ops, produce debug images.
#
# Usage:
#   scripts/verify_golden.sh <test_name>           # diff ops + debug images
#   scripts/verify_golden.sh <test_name> --regions  # with region analysis
#   scripts/verify_golden.sh --all                  # analyze all failing tests
#   scripts/verify_golden.sh --report               # just run tests and print divergence report
#   scripts/verify_golden.sh --regen                # rebuild gen_golden and regenerate ALL data
#
# Steps:
#   1. (if --regen) Rebuild gen_golden and regenerate golden data + C++ ops
#   2. Run DUMP_DRAW_OPS=1 cargo test --test golden <name> -- --test-threads=1
#   3. Run diff_draw_ops.py <name>
#   4. If test failed, run DUMP_GOLDEN=1 to produce diff PPMs
#   5. Print divergence report
#
# NOTE: gen_golden's `make run` regenerates BOTH golden data AND C++ ops files.
#       Only use --regen when you intentionally want to update the golden baseline.
#       Without --regen, the script uses whatever C++ ops files already exist.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GEN_DIR="$PROJECT_ROOT/crates/eaglemode/tests/golden/gen"
DIV_DIR="$PROJECT_ROOT/crates/eaglemode/target/golden-divergence"
DEBUG_DIR="$PROJECT_ROOT/crates/eaglemode/target/golden-debug"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

header() { echo -e "\n${CYAN}${BOLD}=== $1 ===${NC}"; }
ok()     { echo -e "${GREEN}✓${NC} $1"; }
fail()   { echo -e "${RED}✗${NC} $1"; }
warn()   { echo -e "${YELLOW}!${NC} $1"; }

# Parse args
DIFF_ARGS=()
REPORT_ONLY=false
RUN_ALL=false
REGEN=false
TEST_NAME=""

for arg in "$@"; do
    case "$arg" in
        --report)  REPORT_ONLY=true ;;
        --all)     RUN_ALL=true ;;
        --regen)   REGEN=true ;;
        --regions|--verbose|--all-depths|--no-table)
            DIFF_ARGS+=("$arg") ;;
        --limit=*|--depth=*)
            DIFF_ARGS+=("$arg") ;;
        -*)
            echo "Unknown flag: $arg" >&2; exit 1 ;;
        *)
            if [[ -z "$TEST_NAME" ]]; then
                TEST_NAME="$arg"
            else
                echo "Too many positional arguments" >&2; exit 1
            fi
            ;;
    esac
done

if [[ -z "$TEST_NAME" ]] && ! $RUN_ALL && ! $REPORT_ONLY; then
    echo "Usage: $0 <test_name> [--regions] [--verbose] [--all-depths]"
    echo "       $0 --all [--regions]"
    echo "       $0 --report"
    echo "       $0 --regen    (rebuild gen_golden + regenerate golden data)"
    exit 1
fi

cd "$PROJECT_ROOT"

# Step 1: Optionally rebuild gen_golden
if $REGEN; then
    header "Rebuilding gen_golden (--regen)"
    if make -C "$GEN_DIR" clean >/dev/null 2>&1 && make -C "$GEN_DIR" >/dev/null 2>&1; then
        ok "gen_golden compiled"
    else
        fail "gen_golden compilation failed"
        exit 1
    fi
    if make -C "$GEN_DIR" run >/dev/null 2>&1; then
        ok "Golden data + C++ ops regenerated"
        warn "Golden data files have been regenerated — review changes before committing"
    else
        fail "gen_golden run failed"
        exit 1
    fi
fi

# Step 2: Run golden tests
if $REPORT_ONLY; then
    header "Running all golden tests"
    cargo test --test golden -- --test-threads=1 2>&1 | tail -5 || true
    echo ""
    python3 scripts/divergence_report.py
    exit 0
fi

if $RUN_ALL; then
    header "Running all golden tests (with DUMP_DRAW_OPS)"
    TEST_OUTPUT=$(DUMP_DRAW_OPS=1 cargo test --test golden -- --test-threads=1 2>&1 || true)
    echo "$TEST_OUTPUT" | tail -3
    echo ""

    # Extract failing test names
    FAILING=$(echo "$TEST_OUTPUT" | grep -E "^    [a-z]" | sed 's/^ *//' | sort)

    if [[ -z "$FAILING" ]]; then
        ok "All tests passing"
        python3 scripts/divergence_report.py 2>/dev/null || true
        exit 0
    fi

    for test in $FAILING; do
        # Strip module prefix for ops file name
        ops_name=$(echo "$test" | sed 's/^.*:://')
        # Handle common prefix patterns
        ops_name="${ops_name#composition_}"

        header "Analyzing: $test → ops=$ops_name"

        if [[ -f "$DIV_DIR/${ops_name}.cpp_ops.jsonl" ]] && [[ -f "$DIV_DIR/${ops_name}.rust_ops.jsonl" ]]; then
            python3 scripts/diff_draw_ops.py "$ops_name" "${DIFF_ARGS[@]}" --no-table 2>&1 || true
        elif [[ -f "$DIV_DIR/${ops_name}.rust_ops.jsonl" ]]; then
            warn "Rust ops exist but C++ ops missing — run with --regen"
        else
            warn "No ops files for $ops_name"
        fi
    done

    echo ""
    header "Divergence Report"
    python3 scripts/divergence_report.py 2>/dev/null || true
    exit 1
fi

# Single test mode
header "Running: $TEST_NAME (DUMP_DRAW_OPS=1)"
TEST_RESULT=0
DUMP_DRAW_OPS=1 cargo test --test golden "$TEST_NAME" -- --test-threads=1 2>&1 | tail -10 || TEST_RESULT=$?

# Step 3: Diff draw ops
# Try to find the ops file name
OPS_NAME="$TEST_NAME"
# Strip common module prefixes
for prefix in "composition_" "testpanel_"; do
    stripped="${TEST_NAME#$prefix}"
    if [[ -f "$DIV_DIR/${stripped}.cpp_ops.jsonl" ]] || [[ -f "$DIV_DIR/${stripped}.rust_ops.jsonl" ]]; then
        OPS_NAME="$stripped"
        break
    fi
done

if [[ -f "$DIV_DIR/${OPS_NAME}.cpp_ops.jsonl" ]] && [[ -f "$DIV_DIR/${OPS_NAME}.rust_ops.jsonl" ]]; then
    header "DrawOp diff: $OPS_NAME"
    python3 scripts/diff_draw_ops.py "$OPS_NAME" "${DIFF_ARGS[@]}" 2>&1 || true
elif [[ -f "$DIV_DIR/${OPS_NAME}.rust_ops.jsonl" ]]; then
    warn "Rust ops exist but C++ ops missing for $OPS_NAME — run with --regen"
else
    warn "No draw ops files for $OPS_NAME"
fi

# Step 4: Debug images on failure
if [[ $TEST_RESULT -ne 0 ]]; then
    header "Generating debug images (DUMP_GOLDEN=1)"
    DUMP_GOLDEN=1 cargo test --test golden "$TEST_NAME" -- --test-threads=1 2>&1 | grep -E "(DUMP|DIFF|target|ppm|golden-debug)" || true

    if ls "$DEBUG_DIR"/*.ppm >/dev/null 2>&1; then
        ok "Debug images in $DEBUG_DIR/"
        ls -la "$DEBUG_DIR"/*"${OPS_NAME}"* 2>/dev/null || ls -la "$DEBUG_DIR"/*.ppm 2>/dev/null | head -6
    fi
fi

# Step 5: Divergence report
echo ""
header "Divergence Report"
python3 scripts/divergence_report.py 2>/dev/null || true

exit $TEST_RESULT
