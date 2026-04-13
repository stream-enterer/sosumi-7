#!/usr/bin/env python3
"""Compare C++ and Rust DrawOp JSONL files parameter-by-parameter.

Usage:
    python3 scripts/diff_draw_ops.py <test_name> [--depth N] [--all-depths]
    python3 scripts/diff_draw_ops.py tktest_1x
    python3 scripts/diff_draw_ops.py tktest_1x --depth 0
    python3 scripts/diff_draw_ops.py tktest_1x --all-depths --verbose
    python3 scripts/diff_draw_ops.py tktest_1x --json
    python3 scripts/diff_draw_ops.py tktest_1x --summary-json
    python3 scripts/diff_draw_ops.py tktest_1x --regions
"""

import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path

FLOAT_TOL = 1e-10

# Always skip: metadata fields
SKIP_KEYS = {"seq", "_unserialized"}

# Noise keys: excluded by default, included with --verbose.
# These are recording format differences, not real divergences.
NOISE_KEYS = {
    "state_alpha",       # Rust-only field
    "box_h_align",       # PaintTextBoxed: Rust records, C++ doesn't
    "box_v_align",
    "formatted",
    "min_width_scale",
    "rel_line_space",
    "text_alignment",
    "n",                 # PaintPolygon: C++ records vertex count, Rust records vertices directly
}

# Hex keys: redundant with float values, excluded by default
HEX_SUFFIX = "_hex"

# State ops that may appear in one side but not the other
STATE_OPS = {
    "SetCanvasColor", "SetAlpha", "PushState", "PopState",
    "SetOffset", "SetScaling", "ClipRect", "SetTransformation",
}

# State fields embedded inline in paint ops
STATE_INLINE_KEYS = {
    "state_sx", "state_sy", "state_ox", "state_oy",
    "state_clip_x1", "state_clip_y1", "state_clip_x2", "state_clip_y2",
}

# Coordinate keys (for categorization)
COORD_KEYS = {"x", "y", "w", "h", "cx", "cy", "rx", "ry", "l", "t", "r", "b",
              "src_x", "src_y", "src_w", "src_h", "src_l", "src_t", "src_r", "src_b",
              "thickness", "stroke_width", "char_height", "width_scale",
              "max_char_height"}

COLOR_KEYS = {"color", "canvas_color", "color1", "color2", "color_a", "color_b",
              "color_inner", "color_outer", "stroke_color"}


def load_ops(path):
    ops = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or not line.startswith("{"):
                continue
            try:
                ops.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return ops


def fmt(v):
    if isinstance(v, float):
        return f"{v:.15g}"
    if isinstance(v, str) and len(v) > 40:
        return v[:37] + "..."
    return str(v)


def should_skip_key(key, verbose=False):
    """Determine if a key should be skipped in comparison."""
    if key in SKIP_KEYS:
        return True
    if key in STATE_INLINE_KEYS:
        return True
    if not verbose:
        if key in NOISE_KEYS:
            return True
        if key.endswith(HEX_SUFFIX):
            return True
    return False


def categorize_key(key):
    """Categorize a divergence key."""
    if key == "op":
        return "structural"
    if key.startswith("STATE:"):
        state_key = key[6:]
        if "clip" in state_key:
            return "clip"
        return "state_transform"
    if key in COORD_KEYS:
        return "coordinate"
    if key in COLOR_KEYS:
        return "color"
    if key in {"depth", "n", "vertices", "img_w", "img_h", "img_ch",
               "which_sub_rects", "alpha", "extension", "horizontal",
               "closed", "text", "box_h_align", "box_v_align",
               "text_alignment", "min_width_scale", "formatted",
               "rel_line_space"}:
        return "parameter"
    return "other"


def lcs_alignment(a_types, b_types):
    """LCS-based alignment of two sequences.
    Returns list of (a_idx|None, b_idx|None) pairs."""
    m, n = len(a_types), len(b_types)
    dp = [[0] * (n + 1) for _ in range(m + 1)]
    for i in range(m):
        for j in range(n):
            if a_types[i] == b_types[j]:
                dp[i + 1][j + 1] = dp[i][j] + 1
            else:
                dp[i + 1][j + 1] = max(dp[i][j + 1], dp[i + 1][j])

    i, j = m, n
    matched = []
    while i > 0 and j > 0:
        if a_types[i - 1] == b_types[j - 1]:
            matched.append((i - 1, j - 1))
            i -= 1
            j -= 1
        elif dp[i - 1][j] >= dp[i][j - 1]:
            i -= 1
        else:
            j -= 1
    matched.reverse()

    pairs = []
    ai, bi = 0, 0
    for ma, mb in matched:
        while ai < ma:
            pairs.append((ai, None))
            ai += 1
        while bi < mb:
            pairs.append((None, bi))
            bi += 1
        pairs.append((ma, mb))
        ai = ma + 1
        bi = mb + 1
    while ai < m:
        pairs.append((ai, None))
        ai += 1
    while bi < n:
        pairs.append((None, bi))
        bi += 1
    return pairs


def track_state(ops):
    """Walk ops and extract painter state at each paint op.
    Uses inline state_* fields when present, falls back to accumulated state ops."""
    state = {
        "offset_x": 0.0, "offset_y": 0.0,
        "scale_x": 1.0, "scale_y": 1.0,
        "clip_x1": None, "clip_y1": None,
        "clip_x2": None, "clip_y2": None,
    }
    stack = []
    paint_ops = []
    for op in ops:
        kind = op.get("op", "?")
        if kind == "PushState":
            stack.append(dict(state))
        elif kind == "PopState":
            if stack:
                state = stack.pop()
        elif kind == "SetOffset":
            state["offset_x"] = op.get("dx", 0.0)
            state["offset_y"] = op.get("dy", 0.0)
        elif kind == "SetScaling":
            state["scale_x"] = op.get("sx", 1.0)
            state["scale_y"] = op.get("sy", 1.0)
        elif kind == "SetTransformation":
            state["offset_x"] = op.get("ox", 0.0)
            state["offset_y"] = op.get("oy", 0.0)
            state["scale_x"] = op.get("sx", 1.0)
            state["scale_y"] = op.get("sy", 1.0)
        elif kind == "ClipRect":
            sx, sy = state["scale_x"], state["scale_y"]
            ox, oy = state["offset_x"], state["offset_y"]
            ux, uy = op.get("x", 0), op.get("y", 0)
            uw, uh = op.get("w", 0), op.get("h", 0)
            state["clip_x1"] = ux * sx + ox
            state["clip_y1"] = uy * sy + oy
            state["clip_x2"] = (ux + uw) * sx + ox
            state["clip_y2"] = (uy + uh) * sy + oy
        elif kind == "SetCanvasColor":
            pass
        elif kind not in STATE_OPS:
            if "state_sx" in op:
                snap = {
                    "offset_x": op.get("state_ox", 0.0),
                    "offset_y": op.get("state_oy", 0.0),
                    "scale_x": op.get("state_sx", 1.0),
                    "scale_y": op.get("state_sy", 1.0),
                    "clip_x1": op.get("state_clip_x1"),
                    "clip_y1": op.get("state_clip_y1"),
                    "clip_x2": op.get("state_clip_x2"),
                    "clip_y2": op.get("state_clip_y2"),
                }
            else:
                snap = dict(state)
            paint_ops.append((op, snap))
    return paint_ops


def pixel_bbox(op, state):
    """Compute approximate pixel-space bounding box for an op."""
    sx = state.get("scale_x", 1.0) or 1.0
    sy = state.get("scale_y", 1.0) or 1.0
    ox = state.get("offset_x", 0.0) or 0.0
    oy = state.get("offset_y", 0.0) or 0.0

    if "cx" in op:
        x = op["cx"] - op.get("rx", 0)
        y = op["cy"] - op.get("ry", 0)
        w = op.get("rx", 0) * 2
        h = op.get("ry", 0) * 2
    else:
        x = op.get("x", 0)
        y = op.get("y", 0)
        w = op.get("w", 0)
        h = op.get("h", 0)

    if not isinstance(x, (int, float)):
        return None
    px = x * sx + ox
    py = y * sy + oy
    pw = w * sx
    ph = h * sy
    return (px, py, px + pw, py + ph)


def assign_region(bbox, width=800, height=600, grid=4):
    """Assign a pixel bbox to a grid region label."""
    if bbox is None:
        return "unknown"
    cx = (bbox[0] + bbox[2]) / 2
    cy = (bbox[1] + bbox[3]) / 2
    col = min(int(cx / width * grid), grid - 1)
    row = min(int(cy / height * grid), grid - 1)
    return f"({col},{row})"


class Divergence:
    """A single divergence record."""
    __slots__ = ("seq", "op", "key", "cpp_val", "rust_val", "delta", "category", "region")

    def __init__(self, seq, op, key, cpp_val, rust_val, delta, category=None, region=None):
        self.seq = seq
        self.op = op
        self.key = key
        self.cpp_val = cpp_val
        self.rust_val = rust_val
        self.delta = delta
        self.category = category or categorize_key(key)
        self.region = region

    def to_dict(self):
        return {
            "seq": self.seq, "op": self.op, "key": self.key,
            "cpp": self.cpp_val, "rust": self.rust_val,
            "delta": self.delta, "category": self.category,
            "region": self.region,
        }


def compare_ops(cpp_ops, rust_ops, verbose=False, compute_regions=False):
    """Compare two op sequences. Returns (matched, structural, divergences)."""
    # Use (op_type, depth) for alignment
    cpp_keys = [(o.get("op", "?"), o.get("depth", 0)) for o in cpp_ops]
    rust_keys = [(o.get("op", "?"), o.get("depth", 0)) for o in rust_ops]
    alignment = lcs_alignment(cpp_keys, rust_keys)

    # Pre-compute inline state for state comparison and optional region analysis
    cpp_states = {}
    for i, op in enumerate(cpp_ops):
        if op.get("op") not in STATE_OPS and "state_sx" in op:
            cpp_states[i] = {
                "offset_x": op.get("state_ox", 0.0),
                "offset_y": op.get("state_oy", 0.0),
                "scale_x": op.get("state_sx", 1.0),
                "scale_y": op.get("state_sy", 1.0),
                "clip_x1": op.get("state_clip_x1"),
                "clip_y1": op.get("state_clip_y1"),
                "clip_x2": op.get("state_clip_x2"),
                "clip_y2": op.get("state_clip_y2"),
            }
    rust_states = {}
    for i, op in enumerate(rust_ops):
        if op.get("op") not in STATE_OPS and "state_sx" in op:
            rust_states[i] = {
                "offset_x": op.get("state_ox", 0.0),
                "offset_y": op.get("state_oy", 0.0),
                "scale_x": op.get("state_sx", 1.0),
                "scale_y": op.get("state_sy", 1.0),
                "clip_x1": op.get("state_clip_x1"),
                "clip_y1": op.get("state_clip_y1"),
                "clip_x2": op.get("state_clip_x2"),
                "clip_y2": op.get("state_clip_y2"),
            }

    divergences = []
    matched = 0
    identical = 0
    structural = 0

    for ci, ri in alignment:
        if ci is None:
            rust = rust_ops[ri]
            region = None
            if compute_regions and ri in rust_states:
                region = assign_region(pixel_bbox(rust, rust_states[ri]))
            divergences.append(Divergence(
                f"-/{ri}", rust.get("op", "?"), "op",
                "(absent)", rust.get("op", "?"), "RUST ONLY",
                "structural", region,
            ))
            structural += 1
            continue
        if ri is None:
            cpp = cpp_ops[ci]
            region = None
            if compute_regions and ci in cpp_states:
                region = assign_region(pixel_bbox(cpp, cpp_states[ci]))
            divergences.append(Divergence(
                f"{ci}/-", cpp.get("op", "?"), "op",
                cpp.get("op", "?"), "(absent)", "C++ ONLY",
                "structural", region,
            ))
            structural += 1
            continue

        cpp = cpp_ops[ci]
        rust = rust_ops[ri]
        matched += 1

        region = None
        if compute_regions and ci in cpp_states:
            region = assign_region(pixel_bbox(cpp, cpp_states[ci]))

        all_keys = (set(cpp.keys()) | set(rust.keys()))
        op_has_diffs = False
        for key in sorted(all_keys):
            if should_skip_key(key, verbose):
                continue
            cv = cpp.get(key)
            rv = rust.get(key)
            if cv is None:
                divergences.append(Divergence(
                    f"{ci}/{ri}", cpp.get("op", "?"), key,
                    "(missing)", fmt(rv), "RUST EXTRA",
                    region=region,
                ))
                op_has_diffs = True
                continue
            if rv is None:
                divergences.append(Divergence(
                    f"{ci}/{ri}", cpp.get("op", "?"), key,
                    fmt(cv), "(missing)", "C++ EXTRA",
                    region=region,
                ))
                op_has_diffs = True
                continue
            if isinstance(cv, float) and isinstance(rv, float):
                d = abs(cv - rv)
                if d > FLOAT_TOL:
                    divergences.append(Divergence(
                        f"{ci}/{ri}", cpp.get("op", "?"), key,
                        fmt(cv), fmt(rv), f"{d:.6e}",
                        region=region,
                    ))
                    op_has_diffs = True
            elif cv != rv:
                divergences.append(Divergence(
                    f"{ci}/{ri}", cpp.get("op", "?"), key,
                    fmt(cv), fmt(rv), "MISMATCH",
                    region=region,
                ))
                op_has_diffs = True

        # Also compare accumulated state at matched ops
        cs = cpp_states.get(ci, {})
        rs = rust_states.get(ri, {})

        for sk in sorted(set(cs.keys()) | set(rs.keys())):
            csv = cs.get(sk)
            rsv = rs.get(sk)
            if csv is None and rsv is None:
                continue
            if isinstance(csv, (int, float)) and isinstance(rsv, (int, float)):
                d = abs(float(csv) - float(rsv))
                if d > FLOAT_TOL:
                    divergences.append(Divergence(
                        f"{ci}/{ri}", cpp.get("op", "?"), f"STATE:{sk}",
                        fmt(csv), fmt(rsv), f"{d:.6e}",
                        region=region,
                    ))
                    op_has_diffs = True
            elif csv != rsv:
                divergences.append(Divergence(
                    f"{ci}/{ri}", cpp.get("op", "?"), f"STATE:{sk}",
                    fmt(csv), fmt(rsv), "MISMATCH",
                    region=region,
                ))
                op_has_diffs = True

        if not op_has_diffs:
            identical += 1

    return matched, identical, structural, divergences


def print_table(divergences, limit=None):
    """Print divergences as a formatted table."""
    print(f"{'seq':>7}  {'op':<28} {'param':<24} {'C++':<24} {'Rust':<24} {'delta'}")
    print(f"{'---':>7}  {'---':<28} {'---':<24} {'---':<24} {'---':<24} {'---'}")
    shown = 0
    for d in divergences:
        print(f"{d.seq:>7}  {d.op:<28} {d.key:<24} {str(d.cpp_val):<24} {str(d.rust_val):<24} {d.delta}")
        shown += 1
        if limit and shown >= limit:
            remaining = len(divergences) - shown
            if remaining > 0:
                print(f"  ... and {remaining} more divergences (use --limit 0 to show all)")
            break


def print_summary(name, cpp_count, rust_count, matched, identical, structural, divergences):
    """Print structured summary."""
    print(f"\n{'=' * 70}")
    print(f"  {name}")
    print(f"{'=' * 70}")
    print(f"  Total ops:     C++ {cpp_count}, Rust {rust_count}")
    print(f"  Matched:       {matched} ({identical} identical, {matched - identical} with diffs)")
    print(f"  Structural:    {structural} ({sum(1 for d in divergences if d.delta == 'C++ ONLY')} C++ only, "
          f"{sum(1 for d in divergences if d.delta == 'RUST ONLY')} Rust only)")
    print(f"  Divergences:   {len(divergences)}")

    if not divergences:
        print(f"  IDENTICAL")
        return

    # Category breakdown
    cats = Counter(d.category for d in divergences)
    print(f"\n  By category:")
    for cat in ["structural", "coordinate", "color", "clip", "state_transform", "parameter", "other"]:
        if cats[cat]:
            print(f"    {cat:<20} {cats[cat]:>6}")

    # Op type breakdown for structural
    struct_divs = [d for d in divergences if d.category == "structural"]
    if struct_divs:
        cpp_only = Counter(d.op for d in struct_divs if d.delta == "C++ ONLY")
        rust_only = Counter(d.op for d in struct_divs if d.delta == "RUST ONLY")
        if cpp_only:
            print(f"\n  C++ only ops:")
            for op, n in cpp_only.most_common():
                print(f"    {op:<28} {n:>4}")
        if rust_only:
            print(f"\n  Rust only ops:")
            for op, n in rust_only.most_common():
                print(f"    {op:<28} {n:>4}")

    # Coordinate diff magnitude summary
    coord_divs = [d for d in divergences if d.category == "coordinate"]
    if coord_divs:
        magnitudes = []
        for d in coord_divs:
            try:
                magnitudes.append(float(d.delta))
            except ValueError:
                pass
        if magnitudes:
            print(f"\n  Coordinate diffs (n={len(magnitudes)}):")
            print(f"    min={min(magnitudes):.2e}  median={sorted(magnitudes)[len(magnitudes)//2]:.2e}  max={max(magnitudes):.2e}")
            # Bucket by magnitude
            buckets = {"<1e-6": 0, "1e-6..1e-3": 0, "1e-3..1": 0, ">1": 0}
            for m in magnitudes:
                if m < 1e-6:
                    buckets["<1e-6"] += 1
                elif m < 1e-3:
                    buckets["1e-6..1e-3"] += 1
                elif m < 1:
                    buckets["1e-3..1"] += 1
                else:
                    buckets[">1"] += 1
            for label, count in buckets.items():
                if count:
                    print(f"    {label:<16} {count:>6}")

    print()


def print_regions(divergences):
    """Print per-region divergence summary."""
    regions = defaultdict(lambda: {"count": 0, "categories": Counter(), "ops": Counter()})
    for d in divergences:
        r = d.region or "unknown"
        regions[r]["count"] += 1
        regions[r]["categories"][d.category] += 1
        regions[r]["ops"][d.op] += 1

    print(f"\n  By region (4x4 grid):")
    print(f"  {'region':<12} {'total':>6}  {'struct':>6}  {'coord':>6}  {'color':>6}  {'clip':>6}  {'other':>6}")
    print(f"  {'------':<12} {'-----':>6}  {'------':>6}  {'-----':>6}  {'-----':>6}  {'----':>6}  {'-----':>6}")
    for region in sorted(regions.keys()):
        r = regions[region]
        cats = r["categories"]
        print(f"  {region:<12} {r['count']:>6}  {cats['structural']:>6}  {cats['coordinate']:>6}  "
              f"{cats['color']:>6}  {cats['clip']:>6}  "
              f"{cats['state_transform'] + cats['parameter'] + cats['other']:>6}")


def make_summary_json(name, cpp_count, rust_count, matched, identical, structural, divergences):
    """Return summary as a dict."""
    cats = Counter(d.category for d in divergences)
    struct_divs = [d for d in divergences if d.category == "structural"]
    return {
        "test": name,
        "cpp_ops": cpp_count,
        "rust_ops": rust_count,
        "matched": matched,
        "identical": identical,
        "with_diffs": matched - identical,
        "structural": structural,
        "cpp_only": sum(1 for d in struct_divs if d.delta == "C++ ONLY"),
        "rust_only": sum(1 for d in struct_divs if d.delta == "RUST ONLY"),
        "total_divergences": len(divergences),
        "by_category": dict(cats),
        "cpp_only_ops": dict(Counter(d.op for d in struct_divs if d.delta == "C++ ONLY")),
        "rust_only_ops": dict(Counter(d.op for d in struct_divs if d.delta == "RUST ONLY")),
    }


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Compare C++ and Rust DrawOp JSONL files")
    parser.add_argument("test_name", help="Test name (e.g., tktest_1x)")
    parser.add_argument("divergence_dir", nargs="?",
                        default="crates/eaglemode/target/golden-divergence",
                        help="Directory containing JSONL files")
    parser.add_argument("--depth", type=int, default=None,
                        help="Filter to ops at this depth level only")
    parser.add_argument("--all-depths", action="store_true",
                        help="Include all depths (default: depth 0 only)")
    parser.add_argument("--verbose", action="store_true",
                        help="Include noise keys (_hex, state_alpha) in comparison")
    parser.add_argument("--json", action="store_true",
                        help="Output divergences as JSONL")
    parser.add_argument("--summary-json", action="store_true",
                        help="Output summary as single JSON object")
    parser.add_argument("--regions", action="store_true",
                        help="Show per-region divergence breakdown")
    parser.add_argument("--limit", type=int, default=200,
                        help="Max divergences to print in table (0=unlimited)")
    parser.add_argument("--no-table", action="store_true",
                        help="Skip the per-divergence table, show summary only")
    args = parser.parse_args()

    name = args.test_name
    div_dir = Path(args.divergence_dir)

    cpp_path = div_dir / f"{name}.cpp_ops.jsonl"
    rust_path = div_dir / f"{name}.rust_ops.jsonl"

    missing = []
    if not cpp_path.exists():
        missing.append(f"  C++:  {cpp_path}  (run: make -C crates/eaglemode/tests/golden/gen && make -C crates/eaglemode/tests/golden/gen run)")
    if not rust_path.exists():
        missing.append(f"  Rust: {rust_path}  (run: DUMP_DRAW_OPS=1 cargo test --test golden {name} -- --test-threads=1)")
    if missing:
        print(f"Missing files for '{name}':", file=sys.stderr)
        for m in missing:
            print(m, file=sys.stderr)
        sys.exit(1)

    cpp_ops = load_ops(cpp_path)
    rust_ops = load_ops(rust_path)

    # Depth filtering: default depth 0 unless --all-depths or --depth specified
    if args.depth is not None:
        cpp_ops = [o for o in cpp_ops if o.get("depth", 0) == args.depth]
        rust_ops = [o for o in rust_ops if o.get("depth", 0) == args.depth]
    elif not args.all_depths:
        cpp_ops = [o for o in cpp_ops if o.get("depth", 0) == 0]
        rust_ops = [o for o in rust_ops if o.get("depth", 0) == 0]

    # Filter state ops from both sides for alignment
    cpp_paint = [o for o in cpp_ops if o.get("op") not in STATE_OPS]
    rust_paint = [o for o in rust_ops if o.get("op") not in STATE_OPS]

    matched, identical, structural, divergences = compare_ops(
        cpp_paint, rust_paint,
        verbose=args.verbose,
        compute_regions=args.regions,
    )

    if args.summary_json:
        summary = make_summary_json(name, len(cpp_paint), len(rust_paint),
                                     matched, identical, structural, divergences)
        print(json.dumps(summary))
        sys.exit(1 if divergences else 0)

    if args.json:
        for d in divergences:
            print(json.dumps(d.to_dict()))
        sys.exit(1 if divergences else 0)

    # Human-readable output
    print_summary(name, len(cpp_paint), len(rust_paint),
                  matched, identical, structural, divergences)

    if args.regions and divergences:
        print_regions(divergences)

    if divergences and not args.no_table:
        print()
        limit = args.limit if args.limit > 0 else None
        print_table(divergences, limit=limit)

    sys.exit(1 if divergences else 0)


if __name__ == "__main__":
    main()
