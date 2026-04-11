#!/usr/bin/env python3
"""Compare C++ and Rust DrawOp JSONL files parameter-by-parameter.

Usage:
    python3 scripts/diff_draw_ops.py <test_name> [divergence_dir]
    python3 scripts/diff_draw_ops.py cosmos_item_border
    python3 scripts/diff_draw_ops.py testpanel_root crates/eaglemode/target/golden-divergence
"""

import json
import sys
from pathlib import Path

FLOAT_TOL = 1e-10
SKIP_KEYS = {"seq", "_unserialized"}
# State ops that may appear in one side but not the other.
# C++ passes canvas_color per-call; Rust has explicit SetCanvasColor ops.
STATE_OPS = {"SetCanvasColor", "SetAlpha", "PushState", "PopState", "SetOffset", "ClipRect", "SetTransformation"}
# Keys embedded in C++ paint ops for state — exclude from parameter comparison.
STATE_INLINE_KEYS = {"state_sx", "state_sy", "state_ox", "state_oy",
                     "state_clip_x1", "state_clip_y1", "state_clip_x2", "state_clip_y2"}


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
                pass  # skip unparseable lines
    return ops


def fmt(v):
    if isinstance(v, float):
        return f"{v:.15g}"
    if isinstance(v, str) and len(v) > 40:
        return v[:37] + "..."
    return str(v)


def lcs_alignment(a_types, b_types):
    """LCS-based alignment of two op type sequences.
    Returns list of (a_idx|None, b_idx|None) pairs."""
    m, n = len(a_types), len(b_types)
    # Build LCS table
    dp = [[0] * (n + 1) for _ in range(m + 1)]
    for i in range(m):
        for j in range(n):
            if a_types[i] == b_types[j]:
                dp[i + 1][j + 1] = dp[i][j] + 1
            else:
                dp[i + 1][j + 1] = max(dp[i][j + 1], dp[i + 1][j])

    # Backtrack to find alignment
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

    # Build full alignment with unmatched entries
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


def diff_ops(cpp_ops, rust_ops, name):
    divergences = []

    cpp_types = [o.get("op", "?") for o in cpp_ops]
    rust_types = [o.get("op", "?") for o in rust_ops]
    alignment = lcs_alignment(cpp_types, rust_types)

    matched = 0
    structural = 0
    for ci, ri in alignment:
        if ci is None:
            rust = rust_ops[ri]
            divergences.append(
                (f"-/{ri}", rust.get("op", "?"), "op", "(absent)", rust.get("op", "?"), "RUST ONLY")
            )
            structural += 1
            continue
        if ri is None:
            cpp = cpp_ops[ci]
            divergences.append(
                (f"{ci}/-", cpp.get("op", "?"), "op", cpp.get("op", "?"), "(absent)", "C++ ONLY")
            )
            structural += 1
            continue

        cpp = cpp_ops[ci]
        rust = rust_ops[ri]
        matched += 1

        all_keys = (set(cpp.keys()) | set(rust.keys())) - SKIP_KEYS - STATE_INLINE_KEYS
        for key in sorted(all_keys):
            cv = cpp.get(key)
            rv = rust.get(key)
            if cv is None:
                divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, "(missing)", fmt(rv), "RUST EXTRA"))
                continue
            if rv is None:
                divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, fmt(cv), "(missing)", "C++ EXTRA"))
                continue
            if isinstance(cv, float) and isinstance(rv, float):
                d = abs(cv - rv)
                if d > FLOAT_TOL:
                    divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, fmt(cv), fmt(rv), f"{d:.6e}"))
            elif cv != rv:
                divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, fmt(cv), fmt(rv), "MISMATCH"))

    print(f"\n=== {name}: {matched} matched, {structural} structural, {len(divergences)} divergence(s) ===")
    if not divergences:
        print("  IDENTICAL")
        return 0

    print(f"{'seq':>7}  {'op':<28} {'param':<20} {'C++':<24} {'Rust':<24} {'delta'}")
    print(f"{'---':>7}  {'---':<28} {'---':<20} {'---':<24} {'---':<24} {'---'}")
    for seq, op, param, cv, rv, delta in divergences:
        print(f"{seq:>7}  {op:<28} {param:<20} {str(cv):<24} {str(rv):<24} {delta}")

    return len(divergences)


def track_state(ops):
    """Walk ops in order, tracking painter state. Return list of (paint_op, state_snapshot)
    for each paint op, where state_snapshot captures the accumulated painter state."""
    state = {"offset_x": 0.0, "offset_y": 0.0, "scale_x": 1.0, "scale_y": 1.0,
             "clip_x": None, "clip_y": None, "clip_w": None, "clip_h": None,
             "canvas_color": "00000000"}
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
        elif kind == "SetTransformation":
            state["offset_x"] = op.get("ox", 0.0)
            state["offset_y"] = op.get("oy", 0.0)
            state["scale_x"] = op.get("sx", 1.0)
            state["scale_y"] = op.get("sy", 1.0)
        elif kind == "ClipRect":
            # ClipRect values are in user-space; transform to pixel-space
            # using current offset/scale so they're comparable with C++ inline
            # state (which is already in pixel-space).
            ux, uy = op.get("x", 0), op.get("y", 0)
            uw, uh = op.get("w", 0), op.get("h", 0)
            sx, sy = state["scale_x"], state["scale_y"]
            ox, oy = state["offset_x"], state["offset_y"]
            state["clip_x"] = ux * sx + ox
            state["clip_y"] = uy * sy + oy
            state["clip_w"] = uw * sx
            state["clip_h"] = uh * sy
        elif kind == "SetCanvasColor":
            state["canvas_color"] = op.get("color", "00000000")
        elif kind not in STATE_OPS:
            # Paint op — use inline state if present (C++), else accumulated state (Rust)
            if "state_sx" in op:
                snap = {
                    "offset_x": op.get("state_ox", 0.0),
                    "offset_y": op.get("state_oy", 0.0),
                    "scale_x": op.get("state_sx", 1.0),
                    "scale_y": op.get("state_sy", 1.0),
                    "clip_x": op.get("state_clip_x1"),
                    "clip_y": op.get("state_clip_y1"),
                    "clip_w": (op["state_clip_x2"] - op["state_clip_x1"]) if "state_clip_x1" in op else None,
                    "clip_h": (op["state_clip_y2"] - op["state_clip_y1"]) if "state_clip_y1" in op else None,
                    "canvas_color": op.get("canvas_color", "00000000"),
                }
                paint_ops.append((op, snap))
            else:
                snap = dict(state)
                # Use per-call canvas_color from paint op (both C++ and Rust
                # have it) instead of accumulated SetCanvasColor state, since
                # widget code can pass explicit canvas_color to individual calls.
                if "canvas_color" in op:
                    snap["canvas_color"] = op["canvas_color"]
                paint_ops.append((op, snap))
    return paint_ops


def diff_with_state(cpp_ops, rust_ops, name):
    """Compare paint ops AND the painter state active at each paint op."""
    cpp_ps = track_state(cpp_ops)
    rust_ps = track_state(rust_ops)

    cpp_types = [o.get("op", "?") for o, _ in cpp_ps]
    rust_types = [o.get("op", "?") for o, _ in rust_ps]
    alignment = lcs_alignment(cpp_types, rust_types)

    divergences = []
    matched = 0
    for ci, ri in alignment:
        if ci is None:
            rust_op, _ = rust_ps[ri]
            divergences.append((f"-/{ri}", rust_op.get("op", "?"), "op", "(absent)", rust_op.get("op", "?"), "RUST ONLY"))
            continue
        if ri is None:
            cpp_op, _ = cpp_ps[ci]
            divergences.append((f"{ci}/-", cpp_op.get("op", "?"), "op", cpp_op.get("op", "?"), "(absent)", "C++ ONLY"))
            continue

        cpp_op, cpp_st = cpp_ps[ci]
        rust_op, rust_st = rust_ps[ri]
        matched += 1

        # Compare paint op parameters (same as before)
        all_keys = (set(cpp_op.keys()) | set(rust_op.keys())) - SKIP_KEYS - STATE_INLINE_KEYS
        for key in sorted(all_keys):
            cv = cpp_op.get(key)
            rv = rust_op.get(key)
            if cv is None:
                divergences.append((f"{ci}/{ri}", cpp_op.get("op", "?"), key, "(missing)", fmt(rv), "RUST EXTRA"))
                continue
            if rv is None:
                divergences.append((f"{ci}/{ri}", cpp_op.get("op", "?"), key, fmt(cv), "(missing)", "C++ EXTRA"))
                continue
            if isinstance(cv, float) and isinstance(rv, float):
                d = abs(cv - rv)
                if d > FLOAT_TOL:
                    divergences.append((f"{ci}/{ri}", cpp_op.get("op", "?"), key, fmt(cv), fmt(rv), f"{d:.6e}"))
            elif cv != rv:
                divergences.append((f"{ci}/{ri}", cpp_op.get("op", "?"), key, fmt(cv), fmt(rv), "MISMATCH"))

        # Compare painter state at this op
        for sk in sorted(cpp_st.keys()):
            csv = cpp_st.get(sk)
            rsv = rust_st.get(sk)
            if csv is None and rsv is None:
                continue
            if isinstance(csv, float) and isinstance(rsv, float):
                d = abs(csv - rsv)
                if d > FLOAT_TOL:
                    divergences.append((f"{ci}/{ri}", cpp_op.get("op", "?"), f"STATE:{sk}", fmt(csv), fmt(rsv), f"{d:.6e}"))
            elif csv != rsv:
                divergences.append((f"{ci}/{ri}", cpp_op.get("op", "?"), f"STATE:{sk}", fmt(csv), fmt(rsv), "MISMATCH"))

    print(f"\n=== {name} (paint ops + state): {matched} matched, {len(divergences)} divergence(s) ===")
    if not divergences:
        print("  IDENTICAL")
        return 0

    # Only show STATE divergences (param divergences already shown in other sections)
    state_divs = [d for d in divergences if "STATE:" in d[2]]
    if not state_divs:
        print("  (no state divergences)")
        return 0

    print(f"{'seq':>7}  {'op':<28} {'param':<20} {'C++':<24} {'Rust':<24} {'delta'}")
    print(f"{'---':>7}  {'---':<28} {'---':<20} {'---':<24} {'---':<24} {'---'}")
    for seq, op, param, cv, rv, delta in state_divs:
        print(f"{seq:>7}  {op:<28} {param:<20} {str(cv):<24} {str(rv):<24} {delta}")

    return len(state_divs)


def main():
    if len(sys.argv) < 2:
        print("Usage: diff_draw_ops.py <test_name> [divergence_dir]")
        sys.exit(1)

    name = sys.argv[1]
    div_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(
        "crates/eaglemode/target/golden-divergence"
    )

    cpp_path = div_dir / f"{name}.cpp_ops.jsonl"
    rust_path = div_dir / f"{name}.rust_ops.jsonl"

    missing = []
    if not cpp_path.exists():
        missing.append(f"  C++:  {cpp_path}  (run: make -C crates/eaglemode/tests/golden/gen run)")
    if not rust_path.exists():
        missing.append(f"  Rust: {rust_path}  (run: DUMP_DRAW_OPS=1 cargo test --test golden {name})")
    if missing:
        print(f"Missing files for '{name}':")
        for m in missing:
            print(m)
        sys.exit(1)

    cpp_ops = load_ops(cpp_path)
    rust_ops = load_ops(rust_path)

    # Full comparison (including state ops)
    n = diff_ops(cpp_ops, rust_ops, name)

    # Paint-only comparison (filter state ops for alignment)
    cpp_paint = [o for o in cpp_ops if o.get("op") not in STATE_OPS]
    rust_paint = [o for o in rust_ops if o.get("op") not in STATE_OPS]
    n2 = diff_ops(cpp_paint, rust_paint, f"{name} (paint ops only)")

    # Paint ops with accumulated state comparison
    n3 = diff_with_state(cpp_ops, rust_ops, name)

    sys.exit(1 if (n > 0 or n2 > 0 or n3 > 0) else 0)


if __name__ == "__main__":
    main()
