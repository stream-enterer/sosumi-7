//! RUST_ONLY: (language-forced-utility)
//! No C++ analogue; agent-driven debugging requires a programmatic channel
//! that C++'s GUI-only cheat codes (emViewInputFilter::DoCheat) do not
//! provide. Gated behind EMCORE_DEBUG_CONTROL=1 — zero runtime cost when
//! unset.
//!
//! Unix-domain socket at $TMPDIR/eaglemode-rs.<pid>.sock. JSON-lines
//! protocol. Acceptor thread + per-connection worker threads dispatch
//! commands through winit::EventLoopProxy onto the main thread, which
//! mutates view state and sends replies via std::sync::mpsc. This module
//! defines the wire types only — acceptor / worker / dispatch land in
//! Tasks 3.3-3.6.

#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::sync::mpsc::SyncSender;

/// Top-level command tag — wire format `{"cmd":"<name>", ...}`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum CtrlCmd {
    Dump,
    GetState,
    Quit,
    Visit {
        panel_path: String,
        #[serde(default)]
        adherent: bool,
    },
    VisitFullsized {
        panel_path: String,
    },
    SetFocus {
        panel_path: String,
    },
    SeekTo {
        panel_path: String,
    },
    WaitIdle {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    Input {
        event: InputPayload,
    },
    InputBatch {
        events: Vec<InputPayload>,
    },
}

/// Synthetic input payload — wire format `{"kind":"<name>", ...}`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputPayload {
    Key {
        key: String,
        press: bool,
        #[serde(default)]
        mods: Modifiers,
    },
    MouseMove {
        x: f64,
        y: f64,
    },
    MouseButton {
        button: MouseButtonName,
        press: bool,
    },
    Scroll {
        dx: f64,
        dy: f64,
    },
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Modifiers {
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub logo: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButtonName {
    Left,
    Middle,
    Right,
}

/// Reply envelope. Optional fields are omitted from the JSON output when
/// `None`/empty so simple commands round-trip as `{"ok":true}`.
#[derive(Debug, Serialize, Default)]
pub struct CtrlReply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_frame: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_rect: Option<[f64; 4]>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub loading: Vec<LoadingEntry>,
}

impl CtrlReply {
    /// Successful reply with no payload — serializes as `{"ok":true}`.
    pub fn ok() -> Self {
        Self {
            ok: true,
            ..Self::default()
        }
    }

    /// Error reply — serializes as `{"ok":false,"error":"..."}`.
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LoadingEntry {
    pub panel_path: String,
    pub pct: u32,
}

/// Message from acceptor-worker threads to the main thread via
/// winit::EventLoopProxy. The reply_tx is a oneshot
/// (`std::sync::mpsc::sync_channel(1)`); the main thread handler sends
/// the reply back, the worker reads it, serializes to JSON, writes to
/// the socket. Wired in Task 3.3.
#[derive(Debug)]
pub struct CtrlMsg {
    pub cmd: CtrlCmd,
    pub reply_tx: SyncSender<CtrlReply>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_cmd_roundtrip() {
        let json = r#"{"cmd":"dump"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, CtrlCmd::Dump));
    }

    #[test]
    fn get_state_cmd_roundtrip() {
        let json = r#"{"cmd":"get_state"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, CtrlCmd::GetState));
    }

    #[test]
    fn quit_cmd_roundtrip() {
        let json = r#"{"cmd":"quit"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, CtrlCmd::Quit));
    }

    #[test]
    fn visit_cmd_roundtrip_with_default_adherent() {
        let json = r#"{"cmd":"visit","panel_path":"/cosmos/home"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Visit { panel_path, adherent } => {
                assert_eq!(panel_path, "/cosmos/home");
                assert!(!adherent);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn visit_cmd_roundtrip_with_explicit_adherent() {
        let json = r#"{"cmd":"visit","panel_path":"/foo","adherent":true}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Visit { adherent, .. } => assert!(adherent),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn visit_fullsized_cmd_roundtrip() {
        let json = r#"{"cmd":"visit_fullsized","panel_path":"/foo"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::VisitFullsized { panel_path } => assert_eq!(panel_path, "/foo"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn set_focus_cmd_roundtrip() {
        let json = r#"{"cmd":"set_focus","panel_path":"/foo"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::SetFocus { panel_path } => assert_eq!(panel_path, "/foo"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn seek_to_cmd_roundtrip() {
        let json = r#"{"cmd":"seek_to","panel_path":"/foo"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::SeekTo { panel_path } => assert_eq!(panel_path, "/foo"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn wait_idle_cmd_roundtrip_no_timeout() {
        let json = r#"{"cmd":"wait_idle"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::WaitIdle { timeout_ms } => assert_eq!(timeout_ms, None),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn wait_idle_cmd_roundtrip_with_timeout() {
        let json = r#"{"cmd":"wait_idle","timeout_ms":5000}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::WaitIdle { timeout_ms } => assert_eq!(timeout_ms, Some(5000)),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_key_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"key","key":"Return","press":true}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::Key { key, press, mods } } => {
                assert_eq!(key, "Return");
                assert!(press);
                assert!(!mods.shift && !mods.ctrl && !mods.alt && !mods.logo);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_key_with_mods_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"key","key":"a","press":true,"mods":{"shift":true,"ctrl":true}}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::Key { mods, .. } } => {
                assert!(mods.shift && mods.ctrl && !mods.alt && !mods.logo);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_mouse_move_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"mouse_move","x":1.5,"y":2.5}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::MouseMove { x, y } } => {
                assert_eq!(x, 1.5);
                assert_eq!(y, 2.5);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_mouse_button_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"mouse_button","button":"left","press":true}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::MouseButton { button, press } } => {
                assert!(matches!(button, MouseButtonName::Left));
                assert!(press);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_mouse_button_middle_right_roundtrip() {
        let json_m = r#"{"kind":"mouse_button","button":"middle","press":false}"#;
        let json_r = r#"{"kind":"mouse_button","button":"right","press":true}"#;
        let m: InputPayload = serde_json::from_str(json_m).unwrap();
        let r: InputPayload = serde_json::from_str(json_r).unwrap();
        assert!(matches!(m, InputPayload::MouseButton { button: MouseButtonName::Middle, press: false }));
        assert!(matches!(r, InputPayload::MouseButton { button: MouseButtonName::Right, press: true }));
    }

    #[test]
    fn input_scroll_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"scroll","dx":0.0,"dy":-3.0}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::Scroll { dx, dy } } => {
                assert_eq!(dx, 0.0);
                assert_eq!(dy, -3.0);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_batch_roundtrip() {
        let json = r#"{"cmd":"input_batch","events":[{"kind":"key","key":"a","press":true},{"kind":"key","key":"a","press":false}]}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::InputBatch { events } => assert_eq!(events.len(), 2),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn reply_ok_omits_none_fields() {
        let r = CtrlReply::ok();
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, r#"{"ok":true}"#);
    }

    #[test]
    fn reply_err_includes_error() {
        let r = CtrlReply::err("bad path");
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, r#"{"ok":false,"error":"bad path"}"#);
    }

    #[test]
    fn reply_with_path_serializes() {
        let r = CtrlReply {
            ok: true,
            path: Some("/tmp/dump".to_string()),
            ..CtrlReply::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, r#"{"ok":true,"path":"/tmp/dump"}"#);
    }

    #[test]
    fn reply_full_payload_serializes() {
        let r = CtrlReply {
            ok: true,
            error: None,
            path: None,
            idle_frame: Some(42),
            focused_path: Some("/foo".to_string()),
            view_rect: Some([0.0, 0.0, 100.0, 100.0]),
            loading: vec![LoadingEntry {
                panel_path: "/bar".to_string(),
                pct: 50,
            }],
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains(r#""idle_frame":42"#));
        assert!(j.contains(r#""focused_path":"/foo""#));
        assert!(j.contains(r#""view_rect":[0.0,0.0,100.0,100.0]"#));
        assert!(j.contains(r#""loading":[{"panel_path":"/bar","pct":50}]"#));
    }

    #[test]
    fn ctrl_msg_constructs() {
        let (tx, _rx) = std::sync::mpsc::sync_channel::<CtrlReply>(1);
        let msg = CtrlMsg {
            cmd: CtrlCmd::Dump,
            reply_tx: tx,
        };
        assert!(matches!(msg.cmd, CtrlCmd::Dump));
    }
}
