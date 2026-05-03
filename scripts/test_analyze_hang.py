"""Unit tests for analyze_hang.py extensions."""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyze_hang import (
    parse_register, parse_stayawake, parse_wake,
    parse_notice, parse_blink_cycle, parse_inval_req, parse_inval_drain,
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
