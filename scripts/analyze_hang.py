#!/usr/bin/env python3
"""Hang-instrumentation log analyzer.

Subcommands:
  phase0   — Phase 0 v3 chokepoint breakdown (SLICE/CB/AW/RENDER)
  idle     — A1 has_awake findings from idle capture
  blink    — A2 path-trace findings from blink capture

Default (no subcommand): runs phase0 for backwards compatibility.
"""
import sys
import argparse
from collections import defaultdict


# ---------------------------------------------------------------------------
# Low-level KV parsers (new line types added in Phase A source instrumentation)
# ---------------------------------------------------------------------------

def _parse_kv_line(line, expected_prefix):
    """Parse a |-separated key=value line. Returns dict of fields."""
    line = line.rstrip("\n")
    parts = line.split("|")
    if not parts or parts[0] != expected_prefix:
        raise ValueError(f"expected {expected_prefix} prefix, got: {line[:80]}")
    out = {}
    for kv in parts[1:]:
        if "=" not in kv:
            continue
        k, _, v = kv.partition("=")
        out[k] = v
    return out

def _to_int(s):
    return int(s, 0)  # handles 0x prefix and decimal

def _to_bool_tf(s):
    return s == "t"

def parse_register(line):
    f = _parse_kv_line(line, "REGISTER")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "engine_type": f["engine_type"],
        "scope": f["scope"],
    }

def parse_stayawake(line):
    f = _parse_kv_line(line, "STAYAWAKE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "slice": _to_int(f["slice"]),
        "engine_id": f["engine_id"],
        "engine_type": f["engine_type"],
        "stay_awake": _to_bool_tf(f["stay_awake"]),
    }

def parse_wake(line):
    f = _parse_kv_line(line, "WAKE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "engine_type": f["engine_type"],
        "caller": f["caller"],
    }

def parse_notice(line):
    f = _parse_kv_line(line, "NOTICE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "recipient_panel_id": f["recipient_panel_id"],
        "recipient_type": f["recipient_type"],
        "flags": _to_int(f["flags"]),
    }

def parse_blink_cycle(line):
    f = _parse_kv_line(line, "BLINK_CYCLE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "panel_id": f["panel_id"],
        "focused": _to_bool_tf(f["focused"]),
        "flipped": _to_bool_tf(f["flipped"]),
        "busy": _to_bool_tf(f["busy"]),
    }

def parse_inval_req(line):
    f = _parse_kv_line(line, "INVAL_REQ")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "panel_id": f["panel_id"],
        "source": f["source"],
    }

def parse_inval_drain(line):
    f = _parse_kv_line(line, "INVAL_DRAIN")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "panel_id": f["panel_id"],
        "drained": _to_bool_tf(f["drained"]),
    }


def parse_notice_fc_decode(line):
    """B2 Phase 0: parse NOTICE_FC_DECODE."""
    try:
        f = _parse_kv_line(line, "NOTICE_FC_DECODE")
    except ValueError:
        return None
    return {
        "kind": "NOTICE_FC_DECODE",
        "wall_us": int(f["wall_us"]),
        "panel_id": f["panel_id"],
        "behavior_type": f.get("behavior_type", ""),
        "in_active_path": f["in_active_path"] == "t",
        "window_focused": f["window_focused"] == "t",
        "flags": int(f["flags"], 16),
    }


def parse_set_active_result(line):
    """B2 Phase 0: parse SET_ACTIVE_RESULT."""
    try:
        f = _parse_kv_line(line, "SET_ACTIVE_RESULT")
    except ValueError:
        return None
    return {
        "kind": "SET_ACTIVE_RESULT",
        "wall_us": int(f["wall_us"]),
        "target_panel_id": f["target_panel_id"],
        "window_focused": f["window_focused"] == "t",
        "notice_count": int(f["notice_count"]),
        "sched_some": f["sched_some"] == "t",
    }


def parse_set_focused_result(line):
    """B2 Phase 0: parse SET_FOCUSED_RESULT."""
    try:
        f = _parse_kv_line(line, "SET_FOCUSED_RESULT")
    except ValueError:
        return None
    return {
        "kind": "SET_FOCUSED_RESULT",
        "wall_us": int(f["wall_us"]),
        "update_engine_id": f["update_engine_id"],
        "focused": f["focused"] == "t",
        "panels_notified": int(f["panels_notified"]),
    }


# ---------------------------------------------------------------------------
# Validation pre-pass
# ---------------------------------------------------------------------------

def validate_capture(log_content, kind, focus_changed_bit=0x20):
    """Returns (ok, reason). For kind in {"idle", "blink"}."""
    markers = []
    has_register = False
    notices = []
    for ln in log_content.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        if ln.startswith("MARKER|"):
            markers.append(ln)
        elif ln.startswith("REGISTER|"):
            has_register = True
        elif ln.startswith("NOTICE|") and kind == "blink":
            try:
                n = parse_notice(ln)
                notices.append(n)
            except (KeyError, ValueError):
                pass

    if len(markers) != 2:
        return False, f"expected 2 MARKER lines, got {len(markers)}"
    if not has_register:
        return False, "no REGISTER lines (instrumentation did not initialize)"
    if kind == "blink":
        focus_changes = [
            n for n in notices
            if (n["flags"] & focus_changed_bit) and "TextFieldPanel" in n["recipient_type"]
        ]
        if not focus_changes:
            return False, "no NOTICE FOCUS_CHANGED to TextFieldPanel — click did not land on TextField"
    return True, ""


# ---------------------------------------------------------------------------
# idle command
# ---------------------------------------------------------------------------

def idle_command_text(log_content, threshold=0.8):
    """Produce Markdown idle aggregation report from raw log content."""
    ok, reason = validate_capture(log_content, kind="idle")
    if not ok:
        return f"capture invalid: {reason}\n"

    # Parse markers; extract bracketed window
    markers = []
    register_records = []
    stayawake_records = []
    wake_records = []
    for ln in log_content.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        if ln.startswith("MARKER|"):
            f = _parse_kv_line(ln, "MARKER")
            markers.append(_to_int(f["wall_us"]))
        elif ln.startswith("REGISTER|"):
            register_records.append(parse_register(ln))
        elif ln.startswith("STAYAWAKE|"):
            stayawake_records.append(parse_stayawake(ln))
        elif ln.startswith("WAKE|"):
            wake_records.append(parse_wake(ln))

    t_open, t_close = sorted(markers)
    in_window = lambda r: t_open <= r["wall_us"] <= t_close

    # Filter to in-window
    sa = [r for r in stayawake_records if in_window(r)]
    wk = [r for r in wake_records if in_window(r)]

    # engine_id -> type_name (from register; falls back to type seen in stayawake)
    eid_to_type = {r["engine_id"]: r["engine_type"] for r in register_records}
    for r in sa:
        eid_to_type.setdefault(r["engine_id"], r["engine_type"])

    # Per-engine-type aggregation
    by_type = defaultdict(lambda: {"cycles": 0, "stay_awake_t": 0, "ext_wakes": 0, "callers": defaultdict(int)})
    # cycles + stay_awake_t
    for r in sa:
        b = by_type[r["engine_type"]]
        b["cycles"] += 1
        if r["stay_awake"]:
            b["stay_awake_t"] += 1
    # ext_wakes: for each STAYAWAKE(stay_awake=f), check if there is any WAKE
    # for the same engine that arrived before this cycle (anywhere in the window).
    # This identifies cycles that were externally triggered — the engine was woken
    # by an external caller rather than self-perpetuating.  Count how many such
    # stay_awake=f cycles have a preceding WAKE as evidence of external driving.
    wk_by_eid = defaultdict(list)
    for r in wk:
        wk_by_eid[r["engine_id"]].append(r)
    sa_by_eid = defaultdict(list)
    for r in sa:
        sa_by_eid[r["engine_id"]].append(r)

    for eid, cycles_for_eid in sa_by_eid.items():
        wakes_for_eid = wk_by_eid.get(eid, [])
        if not wakes_for_eid:
            continue
        t = eid_to_type.get(eid, "<unknown>")
        for cycle in cycles_for_eid:
            if cycle["stay_awake"]:
                continue
            # Is there any WAKE for this engine before this cycle?
            preceding_wakes = [w for w in wakes_for_eid if w["wall_us"] <= cycle["wall_us"]]
            if preceding_wakes:
                by_type[t]["ext_wakes"] += 1
                # Record the most recent WAKE's caller for attribution
                most_recent = max(preceding_wakes, key=lambda w: w["wall_us"])
                by_type[t]["callers"][most_recent["caller"]] += 1

    # Slice count: number of distinct slice values in the window
    slice_count = len({r["slice"] for r in sa})

    # Build classification per type
    def classify(b):
        if b["cycles"] == 0:
            return "never-awake"
        sa_pct = b["stay_awake_t"] / b["cycles"] if b["cycles"] else 0.0
        if sa_pct >= threshold:
            return "self-perpetuating"
        if b["cycles"] and b["ext_wakes"] >= threshold * b["cycles"]:
            return "externally-rewoken"
        return "episodic"

    rows = []
    for t, b in sorted(by_type.items(), key=lambda kv: -kv[1]["stay_awake_t"]):
        sa_pct = (b["stay_awake_t"] / b["cycles"]) if b["cycles"] else 0.0
        rows.append({
            "type": t, "cycles": b["cycles"], "sa_pct": sa_pct,
            "ext_wakes": b["ext_wakes"], "classification": classify(b),
            "callers": dict(b["callers"]),
        })

    # Format report
    out = []
    out.append(f"## Window")
    out.append(f"{slice_count} slices, {(t_close - t_open) / 1_000_000:.2f}s\n")
    out.append("## Per-engine-type aggregation\n")
    out.append("| engine_type | cycles | stay_awake_pct | ext_wakes | classification |")
    out.append("|---|---:|---:|---:|---|")
    for r in rows:
        out.append(f"| `{r['type']}` | {r['cycles']} | {r['sa_pct']*100:.1f}% | {r['ext_wakes']} | {r['classification']} |")
    out.append("")

    offenders = [r for r in rows if r["classification"] in ("self-perpetuating", "externally-rewoken")]
    out.append("## Offenders")
    if not offenders:
        out.append(f"_None at threshold={threshold*100:.0f}%._")
    else:
        for r in offenders:
            out.append(f"- `{r['type']}` — {r['classification']} (cycles={r['cycles']}, stay_awake={r['sa_pct']*100:.1f}%, ext_wakes={r['ext_wakes']})")
    out.append("")

    out.append("## External-wake caller breakdown")
    any_ext = False
    for r in offenders:
        if r["callers"]:
            any_ext = True
            out.append(f"### `{r['type']}`")
            for caller, n in sorted(r["callers"].items(), key=lambda kv: -kv[1]):
                out.append(f"- `{caller}` — count={n}")
    if not any_ext:
        out.append("_None._")
    out.append("")

    out.append(f"_Next step: spec B1 — compare {{offenders}} to C++ ground truth._\n")
    return "\n".join(out)


# ---------------------------------------------------------------------------
# blink command
# ---------------------------------------------------------------------------

def blink_command_text(log_content, focus_changed_bit=0x20):
    """Produce Markdown blink path-trace report."""
    ok, reason = validate_capture(log_content, kind="blink", focus_changed_bit=focus_changed_bit)
    if not ok:
        return f"capture invalid: {reason}\n"

    markers = []
    notices = []
    wakes = []
    stays = []
    blinks = []
    invreqs = []
    drains = []
    registers = []
    for ln in log_content.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        try:
            if ln.startswith("MARKER|"):
                markers.append(_to_int(_parse_kv_line(ln, "MARKER")["wall_us"]))
            elif ln.startswith("NOTICE|"):
                notices.append(parse_notice(ln))
            elif ln.startswith("WAKE|"):
                wakes.append(parse_wake(ln))
            elif ln.startswith("STAYAWAKE|"):
                stays.append(parse_stayawake(ln))
            elif ln.startswith("BLINK_CYCLE|"):
                blinks.append(parse_blink_cycle(ln))
            elif ln.startswith("INVAL_REQ|"):
                invreqs.append(parse_inval_req(ln))
            elif ln.startswith("INVAL_DRAIN|"):
                drains.append(parse_inval_drain(ln))
            elif ln.startswith("REGISTER|"):
                registers.append(parse_register(ln))
        except (ValueError, KeyError):
            pass  # malformed line, skip

    t_open, t_close = sorted(markers)

    # Locate focus-change
    focus_notices = [
        n for n in notices
        if t_open <= n["wall_us"] <= t_close
        and (n["flags"] & focus_changed_bit)
        and "TextFieldPanel" in n["recipient_type"]
    ]
    if not focus_notices:
        return "capture invalid: no NOTICE FOCUS_CHANGED to TextFieldPanel within window\n"
    focus = focus_notices[0]
    t_focus = focus["wall_us"]
    target_panel_id = focus["recipient_panel_id"]

    # Find the engine for this panel: latest REGISTER for a PanelCycleEngine whose scope mentions target_panel_id
    target_engine_id = None
    for r in registers:
        if "PanelCycleEngine" in r["engine_type"] and target_panel_id in r["scope"]:
            target_engine_id = r["engine_id"]
            break
    # Fallback: pick by latest BLINK_CYCLE for the panel
    if not target_engine_id:
        post_focus_blinks = [b for b in blinks if b["wall_us"] >= t_focus and b["panel_id"] == target_panel_id]
        if post_focus_blinks:
            target_engine_id = post_focus_blinks[0]["engine_id"]

    # Path-trace verdict
    out = []
    out.append("## Path-trace verdict (transition)\n")
    out.append(f"Focus-change identified at +{(t_focus - t_open)/1000:.1f}ms (`{target_panel_id}`, `{focus['recipient_type']}`).\n")

    chain = []  # list of (label, ok, evidence)

    chain.append(("NOTICE FOCUS_CHANGED → TextFieldPanel", True,
                  f"`NOTICE|wall_us={focus['wall_us']}|recipient_panel_id={focus['recipient_panel_id']}|flags={focus['flags']:#x}`"))

    if target_engine_id is None:
        chain.append(("Engine REGISTER for PanelCycleEngine", False, "no REGISTER record matches target panel"))
    else:
        post_wake = [w for w in wakes if w["wall_us"] >= t_focus and w["engine_id"] == target_engine_id]
        chain.append(("WAKE → PanelCycleEngine", bool(post_wake),
                      f"`{post_wake[0]['caller']}` at +{(post_wake[0]['wall_us']-t_focus)/1000:.1f}ms" if post_wake else "no WAKE within window"))
        if post_wake:
            t_wake = post_wake[0]["wall_us"]
            post_stay = [s for s in stays if s["wall_us"] >= t_wake and s["engine_id"] == target_engine_id]
            chain.append(("STAYAWAKE within 1 slice of WAKE", bool(post_stay),
                          f"slice={post_stay[0]['slice']}, stay_awake={post_stay[0]['stay_awake']}" if post_stay else "no STAYAWAKE for engine after WAKE"))

        post_blinks = [b for b in blinks if b["wall_us"] >= t_focus and b["engine_id"] == target_engine_id]
        focused_blinks = [b for b in post_blinks if b["focused"]]
        chain.append(("BLINK_CYCLE focused=true", bool(focused_blinks),
                      f"first focused=t at +{(focused_blinks[0]['wall_us']-t_focus)/1000:.1f}ms" if focused_blinks else "no BLINK_CYCLE focused=t"))
        flipped_blinks = [b for b in post_blinks if b["flipped"]]
        chain.append(("BLINK_CYCLE flipped=true at ~500ms cadence", bool(flipped_blinks),
                      f"{len(flipped_blinks)} flips in window" if flipped_blinks else "no BLINK_CYCLE flipped=t"))

        post_invreq = [i for i in invreqs if i["wall_us"] >= t_focus and i["engine_id"] == target_engine_id]
        chain.append(("INVAL_REQ from cycle_blink", bool(post_invreq),
                      f"first source={post_invreq[0]['source']}" if post_invreq else "no INVAL_REQ"))

        post_drain = [d for d in drains if d["wall_us"] >= t_focus and d["engine_id"] == target_engine_id and d["drained"]]
        chain.append(("INVAL_DRAIN drained=true", bool(post_drain),
                      f"first drain at +{(post_drain[0]['wall_us']-t_focus)/1000:.1f}ms" if post_drain else "no INVAL_DRAIN drained=t"))

    for label, ok, evidence in chain:
        marker = "✓" if ok else "✗"
        out.append(f"- {marker} **{label}** — {evidence}")
    out.append("")

    first_break = next((label for label, ok, _ in chain if not ok), None)
    out.append("## Identified break\n")
    if first_break:
        out.append(f"First ✗: **{first_break}**.\n")
        out.append(f"_Next step: spec B2 — investigate {first_break}._\n")
    else:
        out.append("No break in path-trace. If blink still not visually working, run A2-prod contingency capture.\n")
        out.append("_Next step: A2-prod follow-up capture._\n")

    return "\n".join(out)


# ---------------------------------------------------------------------------
# Phase 0 v3 legacy helpers
# ---------------------------------------------------------------------------

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


def _phase0_main(path):
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


def main():
    # Detect legacy invocation: first arg is a file path (not a subcommand).
    # Preserve backward compat: `analyze_hang.py /path/to/log` still works.
    subcommands = {"phase0", "idle", "blink"}
    if len(sys.argv) >= 2 and sys.argv[1] not in subcommands and not sys.argv[1].startswith("-"):
        # Legacy: treat first arg as log path for phase0
        return _phase0_main(sys.argv[1])

    parser = argparse.ArgumentParser(
        description="Hang-instrumentation log analyzer."
    )
    subparsers = parser.add_subparsers(dest="cmd")

    sp_phase0 = subparsers.add_parser("phase0", help="Phase 0 v3 chokepoint breakdown")
    sp_phase0.add_argument("log", nargs="?", default="/tmp/em_instr.phase0.log",
                           help="path to log (default /tmp/em_instr.phase0.log)")

    sp_idle = subparsers.add_parser("idle", help="A1 has_awake findings from idle capture")
    sp_idle.add_argument("log", help="path to /tmp/em_instr.idle.log")
    sp_idle.add_argument("--threshold", type=float, default=0.8)

    sp_blink = subparsers.add_parser("blink", help="A2 path-trace findings from blink capture")
    sp_blink.add_argument("log", help="path to /tmp/em_instr.blink.log")
    sp_blink.add_argument("--focus-changed-bit", type=lambda s: int(s, 0), default=0x20)

    args = parser.parse_args()

    if args.cmd == "phase0" or args.cmd is None:
        log_path = getattr(args, "log", "/tmp/em_instr.phase0.log")
        return _phase0_main(log_path)
    elif args.cmd == "idle":
        with open(args.log) as f:
            content = f.read()
        print(idle_command_text(content, threshold=args.threshold))
        return 0
    elif args.cmd == "blink":
        with open(args.log) as f:
            content = f.read()
        print(blink_command_text(content, focus_changed_bit=args.focus_changed_bit))
        return 0
    else:
        parser.print_help()
        return 1


if __name__ == "__main__":
    sys.exit(main())
