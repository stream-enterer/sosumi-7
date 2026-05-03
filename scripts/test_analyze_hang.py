"""Unit tests for analyze_hang.py extensions."""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyze_hang import (
    parse_register, parse_stayawake, parse_wake,
    parse_notice, parse_blink_cycle, parse_inval_req, parse_inval_drain,
    parse_notice_fc_decode, parse_set_active_result, parse_set_focused_result,
    _phase0_verdict,
)

def test_parse_register():
    line = "REGISTER|wall_us=12345|engine_id=EngineId(7v3)|engine_type=emcore::FooEngine|scope=Toplevel(WindowId(1))"
    r = parse_register(line)
    assert r["wall_us"] == 12345
    assert r["engine_id"] == "EngineId(7v3)"
    assert r["engine_type"] == "emcore::FooEngine"
    assert r["scope"] == "Toplevel(WindowId(1))"

def test_parse_stayawake():
    line = "STAYAWAKE|wall_us=200|slice=42|engine_id=EngineId(7v3)|engine_type=emcore::FooEngine|stay_awake=t"
    r = parse_stayawake(line)
    assert r["wall_us"] == 200
    assert r["slice"] == 42
    assert r["stay_awake"] is True

def test_parse_wake():
    line = "WAKE|wall_us=300|engine_id=EngineId(7v3)|engine_type=emcore::FooEngine|caller=src/foo.rs:42"
    r = parse_wake(line)
    assert r["caller"] == "src/foo.rs:42"

def test_parse_notice():
    line = "NOTICE|wall_us=400|recipient_panel_id=PanelId(2v1)|recipient_type=emcore::TextFieldPanel|flags=0x20"
    r = parse_notice(line)
    assert r["flags"] == 0x20
    assert r["recipient_type"] == "emcore::TextFieldPanel"

def test_parse_blink_cycle():
    line = "BLINK_CYCLE|wall_us=500|engine_id=EngineId(9v2)|panel_id=PanelId(2v1)|focused=t|flipped=t|busy=t"
    r = parse_blink_cycle(line)
    assert r["focused"] is True
    assert r["flipped"] is True
    assert r["busy"] is True

def test_parse_inval_req():
    line = "INVAL_REQ|wall_us=600|engine_id=EngineId(9v2)|panel_id=PanelId(2v1)|source=src/textfield.rs:100"
    r = parse_inval_req(line)
    assert r["source"] == "src/textfield.rs:100"

def test_parse_inval_drain():
    line = "INVAL_DRAIN|wall_us=700|engine_id=EngineId(9v2)|panel_id=PanelId(2v1)|drained=f"
    r = parse_inval_drain(line)
    assert r["drained"] is False


# T13 — idle command tests

def test_idle_aggregation_self_perpetuating():
    # Synthetic log: engine X cycles 10 times, all stay_awake=t.
    log_lines = [
        "MARKER|wall_us=100|sig=USR1\n",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework\n",
    ]
    for i in range(10):
        log_lines.append(
            f"STAYAWAKE|wall_us={200+i*10}|slice={i}|engine_id=EngineId(1v1)|engine_type=test::Foo|stay_awake=t\n"
        )
    log_lines.append("MARKER|wall_us=400|sig=USR1\n")
    log = "".join(log_lines)
    from analyze_hang import idle_command_text
    out = idle_command_text(log, threshold=0.8)
    assert "test::Foo" in out
    assert "self-perpetuating" in out.lower()

def test_idle_aggregation_externally_rewoken():
    # Engine X: 10 cycles, all stay_awake=f, but a WAKE precedes each.
    log_lines = [
        "MARKER|wall_us=100|sig=USR1\n",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Bar|scope=Framework\n",
    ]
    for i in range(10):
        log_lines.append(
            f"WAKE|wall_us={150+i*10}|engine_id=EngineId(1v1)|engine_type=test::Bar|caller=src/x.rs:1\n"
        )
        log_lines.append(
            f"STAYAWAKE|wall_us={200+i*10}|slice={i}|engine_id=EngineId(1v1)|engine_type=test::Bar|stay_awake=f\n"
        )
    log_lines.append("MARKER|wall_us=400|sig=USR1\n")
    log = "".join(log_lines)
    from analyze_hang import idle_command_text
    out = idle_command_text(log, threshold=0.8)
    assert "externally-rewoken" in out.lower()
    assert "src/x.rs:1" in out  # caller breakdown present

def test_idle_aggregation_episodic():
    # Engine: 50% stay_awake=t. Below 80% threshold = episodic.
    log_lines = [
        "MARKER|wall_us=100|sig=USR1\n",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Mid|scope=Framework\n",
    ]
    for i in range(10):
        sa = "t" if i % 2 == 0 else "f"
        log_lines.append(
            f"STAYAWAKE|wall_us={200+i*10}|slice={i}|engine_id=EngineId(1v1)|engine_type=test::Mid|stay_awake={sa}\n"
        )
    log_lines.append("MARKER|wall_us=400|sig=USR1\n")
    log = "".join(log_lines)
    from analyze_hang import idle_command_text
    out = idle_command_text(log, threshold=0.8)
    assert "episodic" in out.lower()


# T14 — blink command tests

def test_blink_path_trace_breaks_at_wake():
    # Synthetic: NOTICE FOCUS_CHANGED fires, but no WAKE follows.
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|scope=Toplevel(WindowId(1))",
        "NOTICE|wall_us=200|recipient_panel_id=PanelId(2v1)|recipient_type=emcore::TextFieldPanel|flags=0x20",
        # No WAKE follows the FOCUS_CHANGED.
        "MARKER|wall_us=10000|sig=USR1",
        "",
    ])
    from analyze_hang import blink_command_text
    out = blink_command_text(log, focus_changed_bit=0x20)
    assert "FOCUS_CHANGED" in out
    assert "✓" in out  # NOTICE ✓
    assert "✗" in out  # first break

def test_blink_path_trace_complete_chain():
    # Synthetic: full chain fires.
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|scope=Toplevel(WindowId(1))",
        "NOTICE|wall_us=200|recipient_panel_id=PanelId(2v1)|recipient_type=emcore::TextFieldPanel|flags=0x20",
        "WAKE|wall_us=210|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|caller=src/textfield.rs:1",
        "STAYAWAKE|wall_us=300|slice=1|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|stay_awake=t",
        "BLINK_CYCLE|wall_us=310|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|focused=t|flipped=f|busy=t",
        "BLINK_CYCLE|wall_us=810|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|focused=t|flipped=t|busy=t",
        "INVAL_REQ|wall_us=811|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|source=src/textfield.rs:50",
        "INVAL_DRAIN|wall_us=812|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|drained=t",
        "MARKER|wall_us=10000|sig=USR1",
        "",
    ])
    from analyze_hang import blink_command_text
    out = blink_command_text(log, focus_changed_bit=0x20)
    # All ✓ in path-trace section before "Identified break"
    # Test does not assert exact format; asserts presence of key markers
    assert "BLINK_CYCLE" in out
    assert "INVAL_DRAIN" in out
    # Should NOT find a break before INVAL_DRAIN
    # (We cross-check by ensuring "first ✗" or "no break" wording present)
    assert ("no break" in out.lower()) or ("contingency" in out.lower())


# T15 — validation tests

def test_validate_capture_rejects_zero_markers():
    log = "REGISTER|wall_us=10|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework\n"
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="idle")
    assert not ok
    assert "MARKER" in reason

def test_validate_capture_rejects_missing_register():
    log = "MARKER|wall_us=100|sig=USR1\nMARKER|wall_us=200|sig=USR1\n"
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="idle")
    assert not ok
    assert "REGISTER" in reason

def test_validate_capture_blink_requires_focus_changed():
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework",
        "MARKER|wall_us=200|sig=USR1",
        "",
    ])
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="blink")
    assert not ok
    assert "FOCUS_CHANGED" in reason

def test_validate_capture_passes_valid_idle():
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework",
        "MARKER|wall_us=200|sig=USR1",
        "",
    ])
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="idle")
    assert ok


# T16 — B2 parser tests

def test_parse_notice_fc_decode_full():
    line = ("NOTICE_FC_DECODE|wall_us=77680728|panel_id=PanelId(497v1)|"
            "behavior_type=TextFieldPanel|in_active_path=t|window_focused=f|flags=0xf0")
    ev = parse_notice_fc_decode(line)
    assert ev["wall_us"] == 77680728
    assert ev["panel_id"] == "PanelId(497v1)"
    assert ev["behavior_type"] == "TextFieldPanel"
    assert ev["in_active_path"] is True
    assert ev["window_focused"] is False
    assert ev["flags"] == 0xf0


def test_phase0_verdict_o1_iap_false():
    notice = [{
        "wall_us": 1000, "panel_id": "P", "behavior_type": "T",
        "in_active_path": False, "window_focused": True, "flags": 0xf0,
    }]
    v = _phase0_verdict(notice, [], "P", [("emTestPanel.rs", 240, 245)])
    assert v["outcome"] == "O1"


def test_phase0_verdict_o2_wf_false():
    notice = [{
        "wall_us": 1000, "panel_id": "P", "behavior_type": "T",
        "in_active_path": True, "window_focused": False, "flags": 0xf0,
    }]
    v = _phase0_verdict(notice, [], "P", [("emTestPanel.rs", 240, 245)])
    assert v["outcome"] == "O2"


def test_phase0_verdict_o3_branch_fires():
    notice = [{
        "wall_us": 1000, "panel_id": "P", "behavior_type": "T",
        "in_active_path": True, "window_focused": True, "flags": 0xf0,
    }]
    wake = [{
        "wall_us": 1050, "caller": "crates/emtest/src/emTestPanel.rs:242",
    }]
    v = _phase0_verdict(notice, wake, "P", [("emTestPanel.rs", 240, 245)])
    assert v["outcome"] == "O3"


def test_phase0_verdict_o3_ambig_branch_does_not_fire():
    notice = [{
        "wall_us": 1000, "panel_id": "P", "behavior_type": "T",
        "in_active_path": True, "window_focused": True, "flags": 0xf0,
    }]
    v = _phase0_verdict(notice, [], "P", [("emTestPanel.rs", 240, 245)])
    assert v["outcome"] == "O3-AMBIG"


def test_phase0_verdict_o4_no_notice():
    v = _phase0_verdict([], [], "P", [("emTestPanel.rs", 240, 245)])
    assert v["outcome"] == "O4"


# B2.1 Phase 0 tests

from analyze_hang import (
    parse_handler_entry, parse_wup_result, parse_cycle_entry,
    _pick_click_target, _b21_verdict,
)


def test_parse_handler_entry_full():
    line = ("HANDLER_ENTRY|wall_us=100|panel_id=PanelId(125v1)|"
            "impl=emTestPanel::TextFieldPanel|flags=0xf0|"
            "is_focused_path=t|branch_taken=t")
    ev = parse_handler_entry(line)
    assert ev["wall_us"] == 100
    assert ev["panel_id"] == "PanelId(125v1)"
    assert ev["impl"] == "emTestPanel::TextFieldPanel"
    assert ev["flags"] == 0xf0
    assert ev["is_focused_path"] is True
    assert ev["branch_taken"] is True


def test_parse_wup_result_engine_id_none():
    line = ("WUP_RESULT|wall_us=200|panel_id=PanelId(125v1)|"
            "caller=crates/emtest/src/emTestPanel.rs:255|panel_found=t|"
            "engine_id=None|scheduler_some=t|wake_dispatched=f")
    ev = parse_wup_result(line)
    assert ev["panel_found"] is True
    assert ev["engine_id"] == "None"
    assert ev["scheduler_some"] is True
    assert ev["wake_dispatched"] is False


def test_parse_cycle_entry():
    line = ("CYCLE_ENTRY|wall_us=300|engine_id=EngineId(7v1)|"
            "panel_id=PanelId(125v1)|behavior_type=emTestPanel::TextFieldPanel")
    ev = parse_cycle_entry(line)
    assert ev["engine_id"] == "EngineId(7v1)"
    assert ev["behavior_type"] == "emTestPanel::TextFieldPanel"


def test_pick_click_target_picks_latest_post_marker():
    events = [
        {"target_panel_id": "P_A", "window_focused": True, "wall_us": 1000},
        {"target_panel_id": "P_B", "window_focused": True, "wall_us": 2000},
        {"target_panel_id": "P_C", "window_focused": False, "wall_us": 2500},
    ]
    ev = _pick_click_target(events, 500, 3000)
    assert ev["target_panel_id"] == "P_B"


def test_pick_click_target_none_when_outside_markers():
    events = [
        {"target_panel_id": "P_A", "window_focused": True, "wall_us": 100},
    ]
    assert _pick_click_target(events, 500, 3000) is None


def test_b21_verdict_oa1_no_handler_entry():
    v = _b21_verdict("P", 1000, {})
    assert v["bin"] == "OA1"


def test_b21_verdict_ob1_panel_not_found():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": False, "engine_id": "None", "scheduler_some": True, "wake_dispatched": False}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OB1"


def test_b21_verdict_ob2_engine_id_none():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": True, "engine_id": "None", "scheduler_some": True, "wake_dispatched": False}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OB2"


def test_b21_verdict_ob3_scheduler_none():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": True, "engine_id": "Some(EngineId(7v1))", "scheduler_some": False, "wake_dispatched": False}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OB3"


def test_b21_verdict_oc_nopickup_stale():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": True, "engine_id": "Some(EngineId(7v1))", "scheduler_some": True, "wake_dispatched": True}],
        "WAKE": [{"engine_type": "<unregistered>"}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OC-NOPICKUP-STALE"


def test_b21_verdict_oc_dispatch():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": True, "engine_id": "Some(EngineId(7v1))", "scheduler_some": True, "wake_dispatched": True}],
        "WAKE": [{"engine_type": "emcore::emPanelCycleEngine::PanelCycleEngine"}],
        "CYCLE_ENTRY": [{}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OC-DISPATCH"


def test_b21_verdict_od2_no_flip():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": True, "engine_id": "Some(EngineId(7v1))", "scheduler_some": True, "wake_dispatched": True}],
        "WAKE": [{"engine_type": "PanelCycleEngine"}],
        "CYCLE_ENTRY": [{}],
        "BLINK_CYCLE": [{"flipped": False}, {"flipped": False}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OD2"


def test_b21_verdict_od3_drain_false():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": True, "engine_id": "Some(EngineId(7v1))", "scheduler_some": True, "wake_dispatched": True}],
        "WAKE": [{"engine_type": "PanelCycleEngine"}],
        "CYCLE_ENTRY": [{}],
        "BLINK_CYCLE": [{"flipped": True}],
        "INVAL_DRAIN": [{"panel_id": "P", "drained": False}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OD3"


def test_b21_verdict_od_ok():
    eb = {
        "HANDLER_ENTRY": [{"branch_taken": True, "is_focused_path": True}],
        "WUP_RESULT": [{"panel_found": True, "engine_id": "Some(EngineId(7v1))", "scheduler_some": True, "wake_dispatched": True}],
        "WAKE": [{"engine_type": "PanelCycleEngine"}],
        "CYCLE_ENTRY": [{}],
        "BLINK_CYCLE": [{"flipped": True}],
        "INVAL_DRAIN": [{"panel_id": "P", "drained": True}],
    }
    v = _b21_verdict("P", 1000, eb)
    assert v["bin"] == "OD-OK"


if __name__ == "__main__":
    failed = 0
    for name, fn in list(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"PASS {name}")
            except AssertionError as e:
                print(f"FAIL {name}: {e}")
                failed += 1
    sys.exit(1 if failed else 0)
