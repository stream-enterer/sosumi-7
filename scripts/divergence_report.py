#!/usr/bin/env python3
"""Parse divergence.jsonl and produce a clean status report.

Usage:
    python3 scripts/divergence_report.py                    # table
    python3 scripts/divergence_report.py --json             # machine-readable
    python3 scripts/divergence_report.py --diff             # compare against prev
    python3 scripts/divergence_report.py --failing          # only show failing tests
"""

import json
import sys
from pathlib import Path

DEFAULT_DIR = "crates/eaglemode/target/golden-divergence"


def load_divergence(path):
    """Load divergence.jsonl, return list of records."""
    records = []
    if not path.exists():
        return records
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or not line.startswith("{"):
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return records


def is_pixel_test(rec):
    """Check if this is a pixel comparison test (has fail/total fields)."""
    return "fail" in rec and "total" in rec and "type" not in rec


def is_passing(rec):
    """Determine if a record represents a passing test."""
    if "pass" in rec:
        return rec["pass"]
    if is_pixel_test(rec):
        return rec.get("fail", 0) == 0
    return True


def format_table(records, show_passing=False):
    """Format records as a human-readable table."""
    pixel_tests = [r for r in records if is_pixel_test(r)]
    other_tests = [r for r in records if not is_pixel_test(r)]

    divergent_pixel = [r for r in pixel_tests if r.get("fail", 0) > 0]
    clean_pixel = [r for r in pixel_tests if r.get("fail", 0) == 0]
    failing_other = [r for r in other_tests if not is_passing(r)]
    passing_other = [r for r in other_tests if is_passing(r)]

    lines = []

    # Header
    lines.append("")
    total_tests = len(records)
    total_clean = len(clean_pixel) + len(passing_other)
    total_divergent = len(divergent_pixel) + len(failing_other)
    lines.append(f"  Golden Test Divergence Report (zero tolerance)")
    lines.append(f"  {total_clean} exact-match, {total_divergent} divergent, {total_tests} total")
    lines.append(f"  {'─' * 68}")

    # Divergent pixel tests (sorted by fail count descending)
    if divergent_pixel:
        lines.append("")
        lines.append(f"  {'Test':<36} {'Pixels':>8} {'Max':>5} {'Pct':>8}")
        lines.append(f"  {'─' * 36} {'─' * 8} {'─' * 5} {'─' * 8}")
        for r in sorted(divergent_pixel, key=lambda x: -x["fail"]):
            name = r["test"]
            fail = r["fail"]
            total = r["total"]
            max_diff = r.get("max_diff", 0)
            pct = r.get("pct", fail / total * 100 if total else 0)
            lines.append(f"  {name:<36} {fail:>8} {max_diff:>5} {pct:>7.4f}%")

    # Failing other tests
    if failing_other:
        lines.append("")
        for r in failing_other:
            name = r["test"]
            typ = r.get("type", "unknown")
            lines.append(f"  {name:<36} type={typ:<16} FAIL")

    # Summary
    if clean_pixel or passing_other:
        lines.append("")
        if not show_passing:
            lines.append(f"  ({len(clean_pixel)} pixel-exact + {len(passing_other)} non-pixel passing)")
        else:
            lines.append(f"  Pixel-exact tests ({len(clean_pixel)}):")
            for r in sorted(clean_pixel, key=lambda x: x["test"]):
                lines.append(f"    {r['test']}")
            lines.append(f"  Non-pixel tests ({len(passing_other)}):")
            for r in sorted(passing_other, key=lambda x: x["test"]):
                lines.append(f"    {r['test']} ({r.get('type', 'unknown')})")

    lines.append("")
    return "\n".join(lines)


def format_diff(current, previous):
    """Show diff between current and previous divergence runs."""
    cur_map = {r["test"]: r for r in current if is_pixel_test(r)}
    prev_map = {r["test"]: r for r in previous if is_pixel_test(r)}

    all_tests = sorted(set(cur_map.keys()) | set(prev_map.keys()))
    lines = []
    lines.append("")
    lines.append(f"  {'Test':<36} {'Prev Px':>8} {'Curr Px':>8} {'Delta':>8}  {'Prev Max':>8} {'Curr Max':>8}")
    lines.append(f"  {'─' * 36} {'─' * 8} {'─' * 8} {'─' * 8}  {'─' * 8} {'─' * 8}")

    changes = []
    for name in all_tests:
        cur = cur_map.get(name)
        prev = prev_map.get(name)
        cur_fail = cur["fail"] if cur else 0
        prev_fail = prev["fail"] if prev else 0
        cur_max = cur.get("max_diff", 0) if cur else 0
        prev_max = prev.get("max_diff", 0) if prev else 0

        if cur_fail != prev_fail or cur_max != prev_max:
            delta = cur_fail - prev_fail
            delta_str = f"{delta:+d}" if delta != 0 else "="
            changes.append((name, prev_fail, cur_fail, delta_str, prev_max, cur_max))

    if not changes:
        lines.append("  (no changes)")
    else:
        for name, pf, cf, ds, pm, cm in changes:
            lines.append(f"  {name:<36} {pf:>8} {cf:>8} {ds:>8}  {pm:>8} {cm:>8}")

    # Summary
    cur_total_fail = sum(1 for r in current if is_pixel_test(r) and r.get("fail", 0) > 0)
    prev_total_fail = sum(1 for r in previous if is_pixel_test(r) and r.get("fail", 0) > 0)
    cur_total_px = sum(r.get("fail", 0) for r in current if is_pixel_test(r))
    prev_total_px = sum(r.get("fail", 0) for r in previous if is_pixel_test(r))

    lines.append("")
    lines.append(f"  Tests failing:    {prev_total_fail} → {cur_total_fail}")
    lines.append(f"  Total fail pixels: {prev_total_px} → {cur_total_px} ({cur_total_px - prev_total_px:+d})")
    lines.append("")
    return "\n".join(lines)


def format_json(records):
    """Output records as JSONL."""
    lines = []
    for r in records:
        lines.append(json.dumps(r))
    return "\n".join(lines)


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Golden test divergence report")
    parser.add_argument("divergence_dir", nargs="?", default=DEFAULT_DIR,
                        help="Directory containing divergence.jsonl")
    parser.add_argument("--json", action="store_true",
                        help="Output as JSONL")
    parser.add_argument("--diff", action="store_true",
                        help="Compare against divergence.prev.jsonl")
    parser.add_argument("--failing", action="store_true",
                        help="Only show failing tests")
    parser.add_argument("--all", action="store_true",
                        help="Show all tests including passing")
    args = parser.parse_args()

    div_dir = Path(args.divergence_dir)
    current_path = div_dir / "divergence.jsonl"
    prev_path = div_dir / "divergence.prev.jsonl"

    if not current_path.exists():
        print(f"No divergence data at {current_path}", file=sys.stderr)
        print("Run: cargo test --test golden -- --test-threads=1", file=sys.stderr)
        sys.exit(1)

    records = load_divergence(current_path)

    if args.json:
        if args.failing:
            records = [r for r in records if not is_passing(r)]
        print(format_json(records))
        sys.exit(0)

    if args.diff:
        if not prev_path.exists():
            print(f"No previous divergence data at {prev_path}", file=sys.stderr)
            sys.exit(1)
        prev_records = load_divergence(prev_path)
        print(format_diff(records, prev_records))
        sys.exit(0)

    if args.failing:
        records = [r for r in records if not is_passing(r)]

    print(format_table(records, show_passing=args.all))


if __name__ == "__main__":
    main()
