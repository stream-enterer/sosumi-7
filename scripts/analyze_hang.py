#!/usr/bin/env python3
"""Phase 0 v3 analyzer for hang-instrumentation log.

Streams /tmp/em_instr.phase0.log (or path passed on argv). Parses
SLICE, CB, AW, RENDER, MARKER lines. Slices the timeline between the
first two MARKER lines (or the whole log if fewer than 2 markers) and
emits a ranked breakdown of where wall-clock went per chokepoint type.

No magic thresholds. Verdict is the dominant chokepoint.
"""
import sys
from collections import defaultdict


def parse_kv(line):
    parts = line.rstrip("\n").split("|")
    head = parts[0]
    kv = {}
    for part in parts[1:]:
        k, _, v = part.partition("=")
        try:
            kv[k] = int(v)
        except ValueError:
            kv[k] = v
    return head, kv


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/em_instr.phase0.log"
    rows = []
    with open(path) as f:
        for line in f:
            if not line or "|" not in line:
                continue
            rows.append(parse_kv(line))

    if not rows:
        print(f"FAIL: empty log {path}", file=sys.stderr)
        return 1

    markers = [r for r in rows if r[0] == "MARKER"]
    if len(markers) >= 2:
        t_start = markers[0][1]["wall_us"]
        t_end = markers[1][1]["wall_us"]
        window_label = f"between markers {t_start}us and {t_end}us"
    elif len(markers) == 1:
        t_start = markers[0][1]["wall_us"]
        t_end = max(
            (r[1].get("wall_exit_us") or r[1].get("exit_us") or r[1].get("present_done_us") or 0)
            for r in rows
        )
        window_label = f"from marker {t_start}us to end {t_end}us"
    else:
        t_start = 0
        all_t = []
        for r in rows:
            for k in ("wall_enter_us", "enter_us", "wall_us"):
                if k in r[1]:
                    all_t.append(r[1][k])
                    break
        t_end = max(all_t) if all_t else 0
        window_label = f"full log 0us to {t_end}us (no markers)"

    duration_us = t_end - t_start
    if duration_us <= 0:
        print(f"FAIL: marker window has zero duration ({window_label})",
              file=sys.stderr)
        return 1
    if duration_us < 5_000_000 and len(markers) >= 2:
        print(f"WARN: marker window only {duration_us}us "
              f"(<5s); hang may not have built up enough.",
              file=sys.stderr)

    print(f"Window: {window_label}")
    print(f"Duration: {duration_us} us = {duration_us/1e6:.2f} s\n")

    bucket_us = defaultdict(int)
    bucket_n = defaultdict(int)

    for head, kv in rows:
        # Filter to events whose enter timestamp is in window.
        if head == "CB":
            t = kv.get("enter_us", 0)
            if not (t_start <= t <= t_end):
                continue
            name = kv.get("name", "?")
            ev = kv.get("event", "")
            key = f"CB:{name}" + (f":{ev}" if ev else "")
            bucket_us[key] += kv.get("dur_us", 0)
            bucket_n[key] += 1
        elif head == "RENDER":
            t = kv.get("enter_us", 0)
            if not (t_start <= t <= t_end):
                continue
            bucket_us["RENDER:paint"] += kv.get("paint_dur_us", 0)
            bucket_us["RENDER:present"] += kv.get("present_dur_us", 0)
            bucket_n["RENDER:paint"] += 1
            bucket_n["RENDER:present"] += 1
        elif head == "SLICE":
            t = kv.get("wall_enter_us", 0)
            if not (t_start <= t <= t_end):
                continue
            bucket_us["SLICE:scheduler"] += kv.get("t_us", 0)
            bucket_n["SLICE:scheduler"] += 1

    # RENDER is nested inside CB:window_event:redraw. Subtract so the
    # ranking attributes time to the innermost chokepoint that we
    # measured, not the wrapper.
    nested = bucket_us.get("RENDER:paint", 0) + bucket_us.get("RENDER:present", 0)
    if nested and "CB:window_event:redraw" in bucket_us:
        bucket_us["CB:window_event:redraw"] = max(
            0, bucket_us["CB:window_event:redraw"] - nested
        )

    aw_in_window = [
        kv for h, kv in rows
        if h == "AW" and t_start <= kv.get("wall_us", 0) <= t_end
    ]
    aw_total = len(aw_in_window)
    aw_awake = sum(1 for kv in aw_in_window if kv.get("has_awake") == 1)

    print("Chokepoint breakdown (sorted by total wall-clock):")
    print(f"{'bucket':40s} {'count':>8s} {'total_us':>14s} {'pct':>6s} {'avg_us':>8s}")
    print("-" * 80)
    items = sorted(bucket_us.items(), key=lambda kv: -kv[1])
    for k, total in items:
        n = bucket_n[k]
        pct = (total / duration_us) * 100
        avg = total // n if n else 0
        print(f"{k:40s} {n:>8d} {total:>14d} {pct:>5.1f}% {avg:>8d}")

    print()
    print(f"AW lines: {aw_total}, has_awake=1 in {aw_awake} ({100*aw_awake/aw_total if aw_total else 0:.1f}%)")
    print()

    if not items:
        print("FAIL: no events in marker window", file=sys.stderr)
        return 1

    top_bucket, top_us = items[0]
    top_pct = (top_us / duration_us) * 100

    print(f"Verdict: dominant chokepoint is {top_bucket} ({top_pct:.1f}% of window)")

    if top_bucket == "RENDER:paint":
        print("→ Phase A row: 7-RENDER. Paint dominates. Instrument inside")
        print("  emWindow::render to break down: dirty-tile detection,")
        print("  view.Paint, tile_cache uploads. Identify what marks tiles")
        print("  dirty every frame.")
    elif top_bucket == "RENDER:present":
        print("→ Phase A row: 7-PRESENT. Present (wgpu submit/vsync) dominates.")
        print("  Investigate present mode and surface configuration.")
    elif top_bucket.startswith("CB:window_event:redraw") and (
        bucket_us.get("RENDER:paint", 0) + bucket_us.get("RENDER:present", 0)
        < top_us * 0.5
    ):
        print("→ CB:redraw is hot but RENDER is small — render bracket may be")
        print("  missing some work. Add inner-render instrumentation.")
    elif top_bucket.startswith("CB:window_event"):
        print(f"→ Phase A row: 7-INPUT. {top_bucket} dominates. Log per-handler")
        print("  inner work for that variant.")
    elif top_bucket == "SLICE:scheduler":
        if aw_awake == aw_total and aw_total > 0:
            print("→ Phase A row: 7-LOOP-CHAIN. Scheduler dominates AND")
            print("  has_awake_engines() stays true throughout. Self-perpetuating")
            print("  redraw chain at emGUIFramework.rs:1307 likely at fault.")
        else:
            print("→ Phase A row: scheduler-internal (FIREHOSE/REARM/HOTCYC/HOLE).")
            print("  Drop into the v2 verdict matrix for further breakdown.")
    else:
        print(f"→ Phase A row: investigate {top_bucket} internals.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
