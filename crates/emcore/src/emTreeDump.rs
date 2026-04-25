//! Port of C++ `emTreeDump` package (`src/emTreeDump/emTreeDumpUtil.cpp`).
//!
//! Produces an `emTreeDumpRec`-faithful emRec serialization of the running
//! object graph. Used by the `td!` cheat and by the future `emCtrlSocket`
//! `dump` command.
//!
//! Schema matches C++ `emTreeDumpRec` field names and per-type visual style
//! constants (Frame / BgColor / FgColor) so a future port of
//! `emTreeDumpFilePanel` can consume the same file.

#![allow(non_snake_case)]

use crate::emContext::emContext;
use crate::emPanelTree::{PanelId, PanelTree};
use crate::emRecParser::{RecStruct, RecValue};
use crate::emView::{emView, ViewFlags};
use crate::emWindow::{emWindow, WindowFlags};

/// C++ `emTreeDumpRec::FrameType` (include/emTreeDump/emTreeDumpRec.h).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Frame {
    None = 0,
    Rectangle = 1,
    RoundRect = 2,
    Ellipse = 3,
    Hexagon = 4,
}

impl Frame {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Frame::None => "FRAME_NONE",
            Frame::Rectangle => "FRAME_RECTANGLE",
            Frame::RoundRect => "FRAME_ROUND_RECT",
            Frame::Ellipse => "FRAME_ELLIPSE",
            Frame::Hexagon => "FRAME_HEXAGON",
        }
    }
}

/// Per-object visual style (Frame + BgColor + FgColor). Color constants
/// come from the C++ `emTreeDumpFromObject` cascade in
/// `src/emTreeDump/emTreeDumpUtil.cpp`. Colors are packed 0xRRGGBB (alpha
/// dropped — emTreeDumpRec uses 24-bit color).
pub struct VisualStyle {
    pub frame: Frame,
    pub bg: i32,
    pub fg: i32,
}

impl VisualStyle {
    pub fn engine() -> Self {
        Self { frame: Frame::Rectangle, bg: 0x000000, fg: 0xEEEEEE }
    }
    pub(crate) fn context(_is_root: bool) -> Self {
        // C++ uses the same color for root and child context; is_root
        // affects only the Title string (handled at call site).
        Self { frame: Frame::Ellipse, bg: 0x777777, fg: 0xEEEEEE }
    }
    pub(crate) fn view(focused: bool) -> Self {
        let fg = if focused { 0xEEEE44 } else { 0xEEEEEE };
        Self { frame: Frame::RoundRect, bg: 0x448888, fg }
    }
    pub fn window() -> Self {
        // Window branch overlays the view branch in C++; frame stays
        // ROUND_RECT (from view), only Bg is overridden.
        Self { frame: Frame::RoundRect, bg: 0x222288, fg: 0xEEEEEE }
    }
    pub(crate) fn panel(
        viewed: bool,
        in_viewed_path: bool,
        in_focused_path: bool,
        in_active_path: bool,
    ) -> Self {
        let bg = if viewed {
            0x338833
        } else if in_viewed_path {
            0x225522
        } else {
            0x445544
        };
        let fg = if in_focused_path {
            0xEEEE44
        } else if in_active_path {
            0xEEEE88
        } else {
            0xEEEEEE
        };
        Self { frame: Frame::Rectangle, bg, fg }
    }
    pub fn model() -> Self {
        Self { frame: Frame::Hexagon, bg: 0x440000, fg: 0xBBBBBB }
    }
    pub fn file_model() -> Self {
        Self { frame: Frame::Hexagon, bg: 0x440033, fg: 0xBBBBBB }
    }
}

/// Construct an `emTreeDumpRec`-shaped `RecStruct` with Frame / BgColor /
/// FgColor / Title / Text populated, plus empty `Commands` and `Files`
/// arrays. `Children` is NOT inserted here — callers must call
/// `set_children` explicitly (even with an empty Vec) to populate it.
///
/// Commands and Files are always empty in this port (see spec §(A)
/// Schema — keep in mind for future emTreeDumpFilePanel port); Children
/// must be added by the caller via `set_children`.
pub(crate) fn empty_rec(title: String, text: String, style: VisualStyle) -> RecStruct {
    let mut rec = RecStruct::new();
    rec.set_ident("Frame", style.frame.as_str());
    rec.set_int("BgColor", style.bg);
    rec.set_int("FgColor", style.fg);
    rec.set_str("Title", &title);
    rec.set_str("Text", &text);
    // Empty Commands and Files — Children is deferred to `set_children`.
    rec.SetValue("Commands", RecValue::Array(Vec::new()));
    rec.SetValue("Files", RecValue::Array(Vec::new()));
    rec
}

/// Sets the Children field of `rec`. Must be called exactly once per rec;
/// `empty_rec` does not pre-populate Children. Callers that have no
/// children should still call with an empty Vec for schema completeness.
pub(crate) fn set_children(rec: &mut RecStruct, children: Vec<RecValue>) {
    // Replace any existing "Children" entry rather than appending.
    // RecStruct::SetValue is push-based; without a remove-prior step,
    // calling set_children twice on the same rec leaves stale entries
    // ahead of the new one and `get_array` (first-match) returns the
    // stale array. The cross-view cascade (Phase 3) calls `dump_context`
    // (which sets empty children) and then overlays its own children
    // list, so this replace-semantic matters.
    rec.remove_field("Children");
    rec.SetValue("Children", RecValue::Array(children));
}

/// Full emPanel branch. Mirrors C++ `emTreeDumpFromObject`'s emPanel cascade
/// at `src/emTreeDump/emTreeDumpUtil.cpp:246-315`. Appends engine + context
/// fields first (C++ order), then emPanel fields, then recurses into
/// children.
///
/// Arguments:
///   `tree`:            mutable — needed for `take_behavior`/`put_behavior`.
///   `id`:              panel to dump.
///   `current_frame`:   view's current frame counter (for `LastPaintFrame` output).
///   `focused_id`:      id of the currently focused panel, or `None`.
///   `view_home_w` / `view_home_h`: view's home dimensions (for
///                      `GetUpdatePriority` / `GetMemoryLimit`).
///   `window_focused`:  the view's parent window's focused flag.
///
/// Visibility is `pub` rather than `pub(crate)` pending wiring to a real
/// consumer (the `td!` cheat / `emCtrlSocket` `dump`). Downgrading now
/// trips `dead_code` under `-D warnings` because all current callers are
/// `#[cfg(test)]`.
///
/// Note: C++ emTreeDumpFromObject emits `Engine Priority` in the
/// emEngine branch which applies to every emPanel via inheritance.
/// The Rust port does not unify panels with engines, so this field is
/// omitted rather than rendered as a misleading placeholder. If a
/// future task wires panel-as-engine, add the line back here.
/// RUST_ONLY: (language-forced-utility)
///
/// Map of `Rc::as_ptr(emContext) → (&emView, &PanelTree)` built by a
/// single panel-tree walk. C++ uses `dynamic_cast<emView*>(ctx)` inside
/// the context cascade to discover which contexts belong to views; Rust
/// composition has no equivalent cast, and the Rust port's `emView` is
/// not `Rc`'d (it's owned directly by `Window` / `emSubViewPanel`), so
/// a back-pointer on `emContext` is not feasible. Panel-side discovery
/// substitutes: walk each view's panel tree, and wherever an
/// `emSubViewPanel` appears, recurse into its inner view/tree.
///
/// Keys are raw pointer addresses (`Rc::as_ptr`) — used only for
/// equality comparison, never dereferenced.
// TEMP: `pub` (not `pub(crate)`) — no non-test callers land until Phase
// 3's `dump_context_with_cascade`. The plan tightens visibility back to
// `pub(crate)` at that point. `pub(crate)` trips `dead_code` because
// `#[cfg(test)]`-only uses don't count for the lib build.
pub type ViewMap<'a> = std::collections::HashMap<
    *const crate::emContext::emContext,
    (&'a crate::emView::emView, &'a crate::emPanelTree::PanelTree),
>;

/// Recursively walk `view`'s panel tree; for every `emSubViewPanel`
/// found, descend into its sub-view/sub-tree. Keys the resulting map by
/// each view's context pointer.
// TEMP: `pub` for the same reason as `ViewMap`. Tightened in Phase 3.
pub fn collect_views<'a>(
    view: &'a crate::emView::emView,
    tree: &'a crate::emPanelTree::PanelTree,
) -> ViewMap<'a> {
    let mut map = ViewMap::new();
    collect_into(view, tree, &mut map);
    map
}

fn collect_into<'a>(
    view: &'a crate::emView::emView,
    tree: &'a crate::emPanelTree::PanelTree,
    out: &mut ViewMap<'a>,
) {
    out.insert(std::rc::Rc::as_ptr(view.GetContext()), (view, tree));
    for pid in tree.panel_ids() {
        if let Some(svp) = tree.behavior(pid).and_then(|b| b.as_sub_view_panel()) {
            collect_into(&svp.sub_view, svp.sub_tree(), out);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dump_panel(
    tree: &PanelTree,
    id: PanelId,
    current_frame: u64,
    focused_id: Option<PanelId>,
    view_home_w: f64,
    view_home_h: f64,
    window_focused: bool,
) -> RecStruct {
    // --- Derive all read-only state first (avoids borrow conflicts) ---

    // in_focused_path: walk parent chain from focused_id, check membership.
    let in_focused_path = match focused_id {
        Some(fid) => {
            let mut cur = Some(fid);
            let mut found = false;
            while let Some(c) = cur {
                if c == id {
                    found = true;
                    break;
                }
                cur = tree.panels[c].parent;
            }
            found
        }
        None => false,
    };

    let height = tree.get_height(id);
    let (ex, ey, ew, eh) = tree.GetEssenceRect(id);
    let update_priority = tree.GetUpdatePriority(id, view_home_w, view_home_h, window_focused);
    let memory_limit = tree.GetMemoryLimit(id, view_home_w, view_home_h, 2_048_000_000, None);

    // Subtype fields + type_name via shared-ref accessor (Phase 2's
    // PanelTree::behavior + PanelBehavior::dump_state(&self)).
    let (type_name, subtype_pairs) = match tree.behavior(id) {
        Some(behavior) => (behavior.type_name().to_string(), behavior.dump_state()),
        None => ("(no behavior)".to_string(), Vec::new()),
    };

    let panel_title = tree.get_title(id);
    // Snapshot PanelData fields we need after this point.
    let data = &tree.panels[id];
    let name = data.name.clone();
    let layout = data.layout_rect;
    let is_viewed = data.viewed;
    let is_in_viewed_path = data.in_viewed_path;
    let in_active_path = data.in_active_path;
    let is_active = data.is_active;
    let focusable = data.focusable;
    let enable_switch = data.enable_switch;
    let enabled = data.enabled;
    let paint_count = data.paint_count;
    let last_paint_frame = data.last_paint_frame;
    let viewed_xywh = if is_viewed {
        Some((data.viewed_x, data.viewed_y, data.viewed_width, data.viewed_height))
    } else {
        None
    };
    let clip_x1y1x2y2 = if is_viewed {
        Some((data.clip_x, data.clip_y, data.clip_x + data.clip_w, data.clip_y + data.clip_h))
    } else {
        None
    };
    let is_focused = focused_id == Some(id);

    // --- Build the Text body (C++ emTreeDumpUtil.cpp:256-307 order) ---

    let mut text = String::new();

    // Name, Title, Layout/Height/Essence, Viewed flags, Clip, Enable, etc.
    text.push_str(&format!("\nName: {}", name));
    text.push_str(&format!("\nTitle: {}", panel_title));
    text.push_str(&format!(
        "\nLayout XYWH: {}",
        fmt_xywh(layout.x, layout.y, layout.w, layout.h)
    ));
    text.push_str(&format!("\nHeight: {}", fmt_g(height)));
    text.push_str(&format!("\nEssence XYWH: {}", fmt_xywh(ex, ey, ew, eh)));
    text.push_str(&format!("\nViewed: {}", yes_no(is_viewed)));
    text.push_str(&format!("\nInViewedPath: {}", yes_no(is_in_viewed_path)));
    text.push_str("\nViewed XYWH: ");
    text.push_str(&match viewed_xywh {
        Some((x, y, w, h)) => fmt_xywh(x, y, w, h),
        None => "-".to_string(),
    });
    text.push_str("\nClip X1Y1X2Y2: ");
    text.push_str(&match clip_x1y1x2y2 {
        Some((x1, y1, x2, y2)) => fmt_xywh(x1, y1, x2, y2),
        None => "-".to_string(),
    });
    text.push_str(&format!("\nEnableSwitch: {}", yes_no(enable_switch)));
    text.push_str(&format!("\nEnabled: {}", yes_no(enabled)));
    text.push_str(&format!("\nFocusable: {}", yes_no(focusable)));
    text.push_str(&format!("\nActive: {}", yes_no(is_active)));
    text.push_str(&format!("\nInActivePath: {}", yes_no(in_active_path)));
    text.push_str(&format!("\nFocused: {}", yes_no(is_focused)));
    text.push_str(&format!("\nInFocusedPath: {}", yes_no(in_focused_path)));
    text.push_str(&format!("\nUpdate Priority: {}", fmt_g(update_priority)));
    text.push_str(&format!("\nMemory Limit: {}", memory_limit));

    // RUST_ONLY: (language-forced-utility) paint-counter fields — not
    // present in C++ dump. C++ uses gdb for per-panel paint inspection;
    // the Rust port lacks an equivalent live-inspection path, so paint
    // attribution is baked into the data model and surfaced here.
    text.push_str(&format!("\nPaintCount: {}", paint_count));
    text.push_str(&format!(
        "\nLastPaintFrame: {} (current: {})",
        last_paint_frame, current_frame
    ));

    // Subtype fields (PanelBehavior::dump_state).
    for (label, value) in &subtype_pairs {
        text.push_str(&format!("\n{}: {}", label, value));
    }

    // --- Compose rec ---

    let title = format!("Panel:\n{}\n\"{}\"", type_name, name);
    let style = VisualStyle::panel(is_viewed, is_in_viewed_path, in_focused_path, in_active_path);
    let mut rec = empty_rec(title, text, style);

    // --- Recurse into children ---

    let child_ids: Vec<PanelId> = tree.children(id).collect();
    let mut children: Vec<RecValue> = Vec::with_capacity(child_ids.len());
    for child_id in child_ids {
        let child_rec = dump_panel(
            tree,
            child_id,
            current_frame,
            focused_id,
            view_home_w,
            view_home_h,
            window_focused,
        );
        children.push(RecValue::Struct(child_rec));
    }
    set_children(&mut rec, children);

    rec
}

/// Full emView branch. Mirrors C++ `emTreeDumpFromObject`'s emView cascade
/// at `src/emTreeDump/emTreeDumpUtil.cpp:144-215`. Caller-owned `tree` is
/// required so the root-panel recursion can re-enter `dump_panel`.
///
/// `window_focused` is threaded through to `dump_panel` for its
/// update-priority/memory-limit calculations; callers without a window
/// should pass `view.IsFocused()` (best-effort proxy — matches what
/// `emView` itself uses when there's no containing window).
///
/// Visibility is `pub` pending wiring to real consumers (the `td!`
/// cheat / `emCtrlSocket` `dump`) — see `dump_panel` for rationale.
pub(crate) fn dump_view(view: &emView, tree: &PanelTree, window_focused: bool) -> RecStruct {
    // --- Context-branch fields (C++ emView IS-A emContext). ---
    // The C++ cascade for a View first runs the Context branch (lines
    // 109-142) which emits Common/Private Models, *then* the View branch.
    // In Rust, emView holds an Rc<emContext> — access it via GetContext.
    let ctx = view.GetContext();
    let common = ctx.common_model_count();

    // --- Build Text body in C++ order. ---
    let mut text = String::new();

    // Context fields first (C++ cascade order). Private-model count is
    // UPSTREAM-GAP: Rust's emContext does not track per-context
    // `emModel`-like private instances the same way C++ does (no unified
    // model base type; see emContext.rs). Emit `0 (not listed)` to keep
    // the dump format stable for cross-implementation diff.
    text.push_str(&format!("\nCommon Models: {}", common));
    text.push_str("\nPrivate Models: 0 (not listed)");

    // View flags.
    text.push_str("\nView Flags: ");
    text.push_str(&fmt_view_flags(view.flags));
    text.push_str(&format!("\nTitle: {}", view.title));
    text.push_str(&format!("\nFocused: {}", yes_no(view.IsFocused())));
    text.push_str(&format!(
        "\nActivation Adherent: {}",
        yes_no(view.IsActivationAdherent())
    ));
    text.push_str(&format!("\nPopped Up: {}", yes_no(view.IsPoppedUp())));
    let bg = view.GetBackgroundColor();
    // C++ formats Background Color as "0x%08X" of the packed emColor
    // value; emColor in C++ packs as 0xAABBGGRR in memory but the cast
    // `(unsigned int)asView->GetBackgroundColor()` yields the raw
    // packed word. Rust emColor accessors yield channels; pack as
    // 0xRRGGBBAA for a stable human-readable form (matches the C++
    // visual intent even if the exact word differs — noted for the
    // cross-impl diff).
    let packed = ((bg.GetRed() as u32) << 24)
        | ((bg.GetGreen() as u32) << 16)
        | ((bg.GetBlue() as u32) << 8)
        | (bg.GetAlpha() as u32);
    text.push_str(&format!("\nBackground Color: 0x{:08X}", packed));
    text.push_str(&format!(
        "\nHome XYWH: {}",
        fmt_xywh(view.HomeX, view.HomeY, view.HomeWidth, view.HomeHeight)
    ));
    text.push_str(&format!(
        "\nCurrent XYWH: {}",
        fmt_xywh(
            view.CurrentX,
            view.CurrentY,
            view.CurrentWidth,
            view.CurrentHeight
        )
    ));

    // --- Compose rec. ---
    let style = VisualStyle::view(view.IsFocused());
    let title = "View (Context):\nemView".to_string();
    let mut rec = empty_rec(title, text, style);

    // --- Recurse into root panel. ---
    let root_id = view.GetRootPanel();
    let focused_id = view.GetFocusedPanel();
    let current_frame = view.current_frame.get();
    let view_home_w = view.HomeWidth;
    let view_home_h = view.HomeHeight;
    let panel_rec = dump_panel(
        tree,
        root_id,
        current_frame,
        focused_id,
        view_home_w,
        view_home_h,
        window_focused,
    );
    set_children(&mut rec, vec![RecValue::Struct(panel_rec)]);
    rec
}

fn fmt_view_flags(flags: ViewFlags) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if flags.contains(ViewFlags::POPUP_ZOOM) {
        parts.push("VF_POPUP_ZOOM");
    }
    if flags.contains(ViewFlags::ROOT_SAME_TALLNESS) {
        parts.push("VF_ROOT_SAME_TALLNESS");
    }
    if flags.contains(ViewFlags::NO_ZOOM) {
        parts.push("VF_NO_ZOOM");
    }
    if flags.contains(ViewFlags::NO_USER_NAVIGATION) {
        parts.push("VF_NO_USER_NAVIGATION");
    }
    if flags.contains(ViewFlags::NO_FOCUS_HIGHLIGHT) {
        parts.push("VF_NO_FOCUS_HIGHLIGHT");
    }
    if flags.contains(ViewFlags::NO_ACTIVE_HIGHLIGHT) {
        parts.push("VF_NO_ACTIVE_HIGHLIGHT");
    }
    if flags.contains(ViewFlags::EGO_MODE) {
        parts.push("VF_EGO_MODE");
    }
    if flags.contains(ViewFlags::STRESS_TEST) {
        parts.push("VF_STRESS_TEST");
    }
    // NO_SCROLL, NO_NAVIGATE, FULLSCREEN are Rust-only — omit from the
    // dump so the output matches the C++ VF set exactly. Emitting them
    // would confuse cross-implementation diff.
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(", ")
    }
}

/// emWindow branch. Mirrors C++ `emTreeDumpFromObject`'s emWindow cascade
/// at `src/emTreeDump/emTreeDumpUtil.cpp:217-244`. In C++ this branch
/// overlays the View branch (emWindow IS-A emView); here the caller is
/// expected to have already produced the view rec via `dump_view` and
/// then apply the window overlay — but in practice the dump emits one
/// rec per object. This function returns a standalone window rec whose
/// children should be populated with a single view rec by the caller
/// (Task 1.7/1.8 wiring).
///
/// Visibility is `pub` pending real consumers.
pub fn dump_window(window: &emWindow) -> RecStruct {
    let mut text = String::new();
    text.push_str("\nWindow Flags: ");
    text.push_str(&fmt_window_flags(window.flags));
    text.push_str(&format!("\nWMResName: {}", window.GetWMResName()));

    let style = VisualStyle::window();
    let title = "Window (View, Context):\nemWindow".to_string();
    let mut rec = empty_rec(title, text, style);
    set_children(&mut rec, Vec::new());
    rec
}

fn fmt_window_flags(flags: WindowFlags) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if flags.contains(WindowFlags::MODAL) {
        parts.push("WF_MODAL");
    }
    if flags.contains(WindowFlags::UNDECORATED) {
        parts.push("WF_UNDECORATED");
    }
    if flags.contains(WindowFlags::POPUP) {
        parts.push("WF_POPUP");
    }
    if flags.contains(WindowFlags::FULLSCREEN) {
        parts.push("WF_FULLSCREEN");
    }
    // MAXIMIZED and AUTO_DELETE are Rust-only window flags — omit from
    // the dump so the emitted set matches the C++ WF_* enumeration.
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(", ")
    }
}

/// emContext branch. Mirrors C++ `emTreeDumpFromObject`'s emContext
/// cascade at `src/emTreeDump/emTreeDumpUtil.cpp:109-142`. `is_root`
/// selects between "Root Context:" and "Context:" titles (C++ uses
/// `GetParentContext() ? "Context:" : "Root Context:"`).
///
/// Child contexts and common models are NOT enumerated here:
/// - DIVERGED: (upstream-gap-forced) Rust's emContext does not expose a
///   child-context iterator equivalent to C++ `GetFirstChildContext` /
///   `GetNextContext`, and its named-model registry yields
///   `(name, type_info)` pairs (`GetListing`) rather than the
///   emModel-like trait objects required to recurse into model
///   sub-records via `emTreeDumpFromObject`. Emit counts only; a
///   future task can extend this with a listing once the missing
///   accessors land.
///
/// Visibility is `pub` pending real consumers.
pub(crate) fn dump_context(ctx: &emContext, is_root: bool) -> RecStruct {
    let mut text = String::new();
    text.push_str(&format!("\nCommon Models: {}", ctx.common_model_count()));
    text.push_str("\nPrivate Models: 0 (not listed)");

    let style = VisualStyle::context(is_root);
    let title = if is_root {
        "Root Context:\nemRootContext".to_string()
    } else {
        "Context:\nemContext".to_string()
    };
    let mut rec = empty_rec(title, text, style);
    set_children(&mut rec, Vec::new());
    rec
}

/// Like `dump_context`, but recursively emits child contexts. Each
/// upgraded child is dispatched to `dump_view` (if present in the
/// pre-pass `view_map`) or to itself recursively for plain contexts.
///
/// Mirrors C++ `emTreeDumpFromObject`'s emContext cascade with view
/// dispatch (the `dynamic_cast<emView*>(ctx)` branch in C++).
// TEMP: `pub` — non-test caller (`dump_from_root_context_with_home`)
// lands in the next commit. `pub(crate)` here would trip `dead_code`
// because tests don't count for the lib build. Tightened once the
// home-context entry point is wired.
pub fn dump_context_with_cascade(
    ctx: &crate::emContext::emContext,
    is_root: bool,
    view_map: &ViewMap<'_>,
) -> RecStruct {
    let mut rec = dump_context(ctx, is_root);

    let mut children: Vec<RecValue> = Vec::new();
    for weak in ctx.children().iter() {
        let Some(child_ctx) = weak.upgrade() else {
            continue; // dead weak — skip
        };
        let ptr = std::rc::Rc::as_ptr(&child_ctx);
        if let Some(&(view, tree)) = view_map.get(&ptr) {
            // Known view: emit the view branch. Use the view's own
            // IsFocused() for window_focused (matches the existing
            // dump_tree shim's `self.window_focused` access pattern).
            let view_rec = dump_view(view, tree, view.IsFocused());
            children.push(RecValue::Struct(view_rec));
        } else {
            // Plain context: recurse with the same view_map.
            let child_rec = dump_context_with_cascade(&child_ctx, false, view_map);
            children.push(RecValue::Struct(child_rec));
        }
    }

    set_children(&mut rec, children);
    rec
}

/// File-state enum mirroring C++ `emFileModel::FileState` constants exactly.
///
/// The Rust port's `emFileModel::FileState` carries `Loading { progress: f64 }`
/// and `LoadError(String)` / `SaveError(String)`, but the dump output uses
/// only the discriminant names — extract here as a label-only enum so the
/// walker doesn't depend on the concrete carrier types.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileStateLabel {
    Waiting,
    Loading,
    Loaded,
    Unsaved,
    Saving,
    TooCostly,
    LoadError,
    SaveError,
}

impl FileStateLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "FS_WAITING",
            Self::Loading => "FS_LOADING",
            Self::Loaded => "FS_LOADED",
            Self::Unsaved => "FS_UNSAVED",
            Self::Saving => "FS_SAVING",
            Self::TooCostly => "FS_TOO_COSTLY",
            Self::LoadError => "FS_LOAD_ERROR",
            Self::SaveError => "FS_SAVE_ERROR",
        }
    }
}

/// Convert from the Rust port's `crate::emFileModel::FileState`.
impl From<&crate::emFileModel::FileState> for FileStateLabel {
    fn from(s: &crate::emFileModel::FileState) -> Self {
        use crate::emFileModel::FileState;
        match s {
            FileState::Waiting => Self::Waiting,
            FileState::Loading { .. } => Self::Loading,
            FileState::Loaded => Self::Loaded,
            FileState::Unsaved => Self::Unsaved,
            FileState::Saving => Self::Saving,
            FileState::TooCostly => Self::TooCostly,
            FileState::LoadError(_) => Self::LoadError,
            FileState::SaveError(_) => Self::SaveError,
        }
    }
}

/// emModel branch — mirrors C++ `emTreeDumpUtil.cpp:317-329`.
///
/// DIVERGED: (language-forced) takes primitive arguments because the Rust
/// port's emModel is not a unified trait with virtual dispatch — models are
/// stored as `Rc<RefCell<T>>` keyed by `TypeId`, with no `dyn emModel`
/// equivalent to C++'s `emModel*`. The caller extracts
/// `name` / `type_name` / `min_common_lifetime` from the concrete type and
/// passes them in. Validated by the round-trip tests
/// `dump_model_emits_name_and_lifetime` below.
///
/// Visibility is `pub` pending real consumers (Task 1.9 downgrade).
pub fn dump_model(name: &str, type_name: &str, min_common_lifetime: u32) -> RecStruct {
    let mut text = String::new();
    text.push_str(&format!("\nName: {}", name));
    text.push_str(&format!("\nMin Common Lifetime: {}", min_common_lifetime));
    let title = format!("Common Model:\n{}\n\"{}\"", type_name, name);
    empty_rec(title, text, VisualStyle::model())
}

/// emFileModel branch — mirrors C++ `emTreeDumpUtil.cpp:331-353`.
///
/// Same primitive-argument shape as [`dump_model`] (see DIVERGED note there
/// — language-forced because emFileModel in the Rust port is a generic
/// `emFileModel<T>`, not a virtual-dispatch class hierarchy).
///
/// Visibility is `pub` pending real consumers (Task 1.9 downgrade).
pub fn dump_file_model(
    name: &str,
    type_name: &str,
    file_path: &str,
    file_state: FileStateLabel,
    memory_need: u64,
) -> RecStruct {
    let mut text = String::new();
    text.push_str(&format!("\nFile Path: {}", file_path));
    text.push_str(&format!("\nFile State: {}", file_state.as_str()));
    text.push_str(&format!("\nMemory Need: {}", memory_need));
    let title = format!("Common File Model:\n{}\n\"{}\"", type_name, name);
    empty_rec(title, text, VisualStyle::file_model())
}

/// Top-level entry point — port of C++ `emTreeDumpFromRootContext` at
/// src/emTreeDump/emTreeDumpUtil.cpp:360-414. Builds the General Info
/// rec, attaches the root context as Children[0]. The view + panel tree
/// are walked when the caller iterates the root context's child views.
///
/// Note: this entry point does NOT walk views directly. The Rust port's
/// emContext doesn't enumerate child views or contexts in a unified way
/// (Task 1.6 documented this gap). Callers that need a full
/// view+panel-tree dump should also call `dump_view` and append it to
/// the rec's Children. The shim in `emView::dump_tree` (Task 1.9) does
/// exactly that.
pub(crate) fn dump_from_root_context(root_ctx: &emContext) -> RecStruct {
    let title =
        "Tree Dump\nof the top-level objects\nof a running emCore-based program".to_string();
    let text = general_info_text();
    let style = VisualStyle {
        frame: Frame::Rectangle,
        bg: 0x444466,
        fg: 0xBBBBEE,
    };
    let mut rec = empty_rec(title, text, style);

    // Children[0] = the root context's own dump rec.
    let ctx_rec = dump_context(root_ctx, /* is_root */ true);
    set_children(&mut rec, vec![RecValue::Struct(ctx_rec)]);
    rec
}

fn general_info_text() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let time_str = format_unix_time(now_secs);
    let host = hostname_best_effort();
    let user = std::env::var("USER").unwrap_or_else(|_| "-".to_string());
    let pid = std::process::id();
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.into_os_string().into_string().ok())
        .unwrap_or_else(|| "-".to_string());
    let utf8 = "yes"; // Rust strings are always UTF-8.
    let byte_order = if cfg!(target_endian = "little") {
        "1234"
    } else {
        "4321"
    };
    let ptr_size = std::mem::size_of::<*const ()>();
    let long_size = std::mem::size_of::<i64>();
    // C++ `char` signedness — Rust's `i8` is always signed; `c_char`
    // is platform-dependent. Report based on c_char.
    let char_signed = if (std::os::raw::c_char::MIN as i32) < 0 {
        "signed"
    } else {
        "unsigned"
    };

    let mut s = String::new();
    s.push_str("General Info");
    s.push_str("\n~~~~~~~~~~~~");
    s.push_str(&format!("\n\nTime       : {}", time_str));
    s.push_str(&format!("\nHost Name  : {}", host));
    s.push_str(&format!("\nUser Name  : {}", user));
    s.push_str(&format!("\nProcess Id : {}", pid));
    s.push_str(&format!("\nCurrent Dir: {}", cwd));
    s.push_str(&format!("\nUTF8       : {}", utf8));
    s.push_str(&format!("\nByte Order : {}", byte_order));
    s.push_str(&format!("\nsizeof(ptr): {}", ptr_size));
    s.push_str(&format!("\nsizeof(lng): {}", long_size));
    s.push_str(&format!("\nchar       : {}", char_signed));
    // DIVERGED: (upstream-gap-forced) Rust port does not expose CPU TSC;
    // no portable Rust RDTSC equivalent. Emit `-` to keep field stable.
    s.push_str("\nCPU-TSC    : -");
    s.push_str("\n\nPaths of emCore:");
    s.push_str(&install_paths_block());
    s
}

fn format_unix_time(secs: u64) -> String {
    let days = secs / 86400;
    let tod = secs % 86400;
    let hour = tod / 3600;
    let minute = (tod % 3600) / 60;
    let second = tod % 60;
    let (y, m, d) = days_to_ymd(days as i64);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        y, m, d, hour, minute, second
    )
}

fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Howard Hinnant date algorithm.
    let z = days + 719468;
    let era = if z >= 0 {
        z / 146097
    } else {
        (z - 146096) / 146097
    };
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = (y + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

fn hostname_best_effort() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "-".to_string())
}

fn install_paths_block() -> String {
    use crate::emInstallInfo::{emGetInstallPath, InstallDirType};
    let kinds: &[(&str, InstallDirType)] = &[
        ("Bin        ", InstallDirType::Bin),
        ("Include    ", InstallDirType::Include),
        ("Lib        ", InstallDirType::Lib),
        ("Html Doc   ", InstallDirType::HtmlDoc),
        ("Pdf Doc    ", InstallDirType::PdfDoc),
        ("Ps Doc     ", InstallDirType::PsDoc),
        ("User Config", InstallDirType::UserConfig),
        ("Host Config", InstallDirType::HostConfig),
        ("Tmp        ", InstallDirType::Tmp),
        ("Res        ", InstallDirType::Res),
        ("Home       ", InstallDirType::Home),
    ];
    let mut s = String::new();
    for (label, kind) in kinds {
        let path = match emGetInstallPath(*kind, "emCore", None) {
            Ok(p) => p.display().to_string(),
            Err(_) => "-".to_string(),
        };
        s.push_str(&format!("\n{}: {}", label, path));
    }
    s
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Format a single f64 in C++ `%.9G` style: 9 significant digits, with
/// scientific notation when the exponent is `< -4` or `>= 9`, trailing
/// zeros stripped, and a trailing `.` stripped when the mantissa is
/// integer. Matches `printf("%.9G", v)` for the contracts required by
/// the tree dump; exponent field uses `E[+-]dd` (2-digit minimum) as
/// printf does on glibc.
fn fmt_g(v: f64) -> String {
    const PRECISION: i32 = 9;

    if v == 0.0 {
        // Handles +0.0 and -0.0 uniformly; C's %G prints "0" here.
        return "0".to_string();
    }
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf".to_string() } else { "inf".to_string() };
    }

    let exp = v.abs().log10().floor() as i32;

    if !(-4..PRECISION).contains(&exp) {
        // Scientific. Rust `{:E}` precision = digits after decimal; %G
        // precision = total significant digits, so subtract 1.
        let raw = format!("{:.*E}", (PRECISION - 1) as usize, v);
        // raw looks like "1.230000000E2" or "-1.23E-5".
        let (mantissa, exp_str) = raw.split_once('E').expect("format!E always contains E");
        // Strip trailing zeros from the mantissa; then strip a trailing
        // '.' if the mantissa collapses to an integer.
        let mut m = mantissa.trim_end_matches('0').trim_end_matches('.').to_string();
        if m.is_empty() || m == "-" {
            m.push('0');
        }
        // Normalize exponent: C printf emits E[+-]dd with at least 2
        // digits. Rust's {:E} emits e.g. "E2" or "E-5" (no sign for
        // non-negative, no zero-pad).
        let (sign, digits) = if let Some(rest) = exp_str.strip_prefix('-') {
            ('-', rest)
        } else if let Some(rest) = exp_str.strip_prefix('+') {
            ('+', rest)
        } else {
            ('+', exp_str)
        };
        let padded = if digits.len() < 2 {
            format!("0{}", digits)
        } else {
            digits.to_string()
        };
        format!("{}E{}{}", m, sign, padded)
    } else {
        // Fixed. digits-after-decimal = precision - 1 - exp (clamped ≥ 0).
        let digits_after = (PRECISION - 1 - exp).max(0) as usize;
        let raw = format!("{:.*}", digits_after, v);
        if raw.contains('.') {
            let trimmed = raw.trim_end_matches('0').trim_end_matches('.');
            if trimmed.is_empty() || trimmed == "-" {
                "0".to_string()
            } else {
                trimmed.to_string()
            }
        } else {
            raw
        }
    }
}

fn fmt_xywh(x: f64, y: f64, w: f64, h: f64) -> String {
    format!("{}, {}, {}, {}", fmt_g(x), fmt_g(y), fmt_g(w), fmt_g(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_as_str_matches_cpp_names() {
        assert_eq!(Frame::None.as_str(), "FRAME_NONE");
        assert_eq!(Frame::Rectangle.as_str(), "FRAME_RECTANGLE");
        assert_eq!(Frame::RoundRect.as_str(), "FRAME_ROUND_RECT");
        assert_eq!(Frame::Ellipse.as_str(), "FRAME_ELLIPSE");
        assert_eq!(Frame::Hexagon.as_str(), "FRAME_HEXAGON");
    }

    #[test]
    fn empty_rec_has_all_top_level_fields() {
        let rec = empty_rec("t".into(), "txt".into(), VisualStyle::engine());
        assert!(rec.get_ident("Frame").is_some());
        assert!(rec.get_int("BgColor").is_some());
        assert!(rec.get_int("FgColor").is_some());
        assert_eq!(rec.get_str("Title"), Some("t"));
        assert_eq!(rec.get_str("Text"), Some("txt"));
        assert!(rec.get_array("Commands").is_some());
        assert!(rec.get_array("Files").is_some());
        // Children is NOT populated by empty_rec — callers must call
        // set_children explicitly.
        assert!(rec.get_array("Children").is_none());
    }

    #[test]
    fn set_children_populates_children_field() {
        let mut rec = empty_rec("t".into(), "".into(), VisualStyle::engine());
        assert!(rec.get_array("Children").is_none());
        set_children(&mut rec, Vec::new());
        let arr = rec.get_array("Children").expect("Children exists");
        assert!(arr.is_empty());
    }

    #[test]
    fn visual_style_panel_viewed_bg_matches_cpp() {
        // C++ emTreeDumpUtil.cpp:249: IsViewed() ⇒ emColor(51,136,51) = 0x338833
        let s = VisualStyle::panel(true, false, false, false);
        assert_eq!(s.bg, 0x338833);
    }

    #[test]
    fn visual_style_panel_focused_fg_matches_cpp() {
        // C++ emTreeDumpUtil.cpp:252: IsInFocusedPath() ⇒ emColor(238,238,68) = 0xEEEE44
        let s = VisualStyle::panel(false, false, true, false);
        assert_eq!(s.fg, 0xEEEE44);
    }

    #[test]
    fn visual_style_context_matches_cpp() {
        let s = VisualStyle::context(true);
        assert_eq!(s.frame, Frame::Ellipse);
        assert_eq!(s.bg, 0x777777);
        assert_eq!(s.fg, 0xEEEEEE);
        let s2 = VisualStyle::context(false);
        assert_eq!(s2.bg, 0x777777);
    }

    #[test]
    fn visual_style_view_focused_matches_cpp() {
        let s = VisualStyle::view(true);
        assert_eq!(s.frame, Frame::RoundRect);
        assert_eq!(s.bg, 0x448888);
        assert_eq!(s.fg, 0xEEEE44);
        let s2 = VisualStyle::view(false);
        assert_eq!(s2.fg, 0xEEEEEE);
    }

    #[test]
    fn visual_style_window_matches_cpp() {
        let s = VisualStyle::window();
        assert_eq!(s.frame, Frame::RoundRect);
        assert_eq!(s.bg, 0x222288);
        assert_eq!(s.fg, 0xEEEEEE);
    }

    #[test]
    fn visual_style_model_matches_cpp() {
        let s = VisualStyle::model();
        assert_eq!(s.frame, Frame::Hexagon);
        assert_eq!(s.bg, 0x440000);
        assert_eq!(s.fg, 0xBBBBBB);
    }

    #[test]
    fn visual_style_file_model_matches_cpp() {
        let s = VisualStyle::file_model();
        assert_eq!(s.frame, Frame::Hexagon);
        assert_eq!(s.bg, 0x440033);
        assert_eq!(s.fg, 0xBBBBBB);
    }

    #[test]
    fn visual_style_panel_in_viewed_path_bg_matches_cpp() {
        // C++ emTreeDumpUtil.cpp: IsInViewedPath() ⇒ emColor(34,85,34) = 0x225522
        let s = VisualStyle::panel(false, true, false, false);
        assert_eq!(s.bg, 0x225522);
        // Default branch
        let s2 = VisualStyle::panel(false, false, false, false);
        assert_eq!(s2.bg, 0x445544);
        // Active-path fg
        let s3 = VisualStyle::panel(false, false, false, true);
        assert_eq!(s3.fg, 0xEEEE88);
    }

    #[test]
    fn set_children_replaces_children_array() {
        let mut rec = empty_rec("t".into(), "".into(), VisualStyle::engine());
        set_children(&mut rec, vec![RecValue::Int(7)]);
        let arr = rec.get_array("Children").expect("Children exists");
        assert_eq!(arr.len(), 1);
        assert!(matches!(arr[0], RecValue::Int(7)));
    }

    #[test]
    fn fmt_g_matches_c_percent_g() {
        // Expected values verified against `printf("%.9G", x)` on glibc.
        assert_eq!(fmt_g(0.0), "0");
        assert_eq!(fmt_g(1.0), "1");
        assert_eq!(fmt_g(1.5), "1.5");
        assert_eq!(fmt_g(123456789.0), "123456789");
        assert_eq!(fmt_g(0.5), "0.5");
        assert_eq!(fmt_g(-1.5), "-1.5");
        assert_eq!(fmt_g(0.0001), "0.0001");
        // < 1e-4 → scientific
        assert_eq!(fmt_g(0.00001), "1E-05");
        // >= 1e9 → scientific
        assert_eq!(fmt_g(1e9), "1E+09");
        // 9 significant digits (C printf rounds half-to-even here).
        assert_eq!(fmt_g(1.2345678901), "1.23456789");
    }

    // --- dump_panel tests ---

    use crate::emPanel::PanelBehavior;

    /// Minimal no-op behavior for exercising the `take_behavior` path in
    /// `dump_panel`. All trait methods use defaults.
    struct NoopBehavior;
    impl PanelBehavior for NoopBehavior {}

    #[test]
    fn dump_panel_leaf_has_all_labels() {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.put_behavior(root, Box::new(NoopBehavior));

        let rec = dump_panel(&tree, root, 0, None, 1.0, 1.0, false);

        let title = rec.get_str("Title").expect("Title exists").to_string();
        let text = rec.get_str("Text").expect("Text exists").to_string();

        assert!(title.starts_with("Panel:\n"), "Title: {}", title);
        assert!(title.contains("\"root\""), "Title: {}", title);

        assert!(!text.contains("Engine Priority"));

        for label in [
            "Name:",
            "Title:",
            "Layout XYWH",
            "Height:",
            "Essence XYWH",
            "Viewed: no",
            "InViewedPath: no",
            "Viewed XYWH: -",
            "Clip X1Y1X2Y2: -",
            "EnableSwitch",
            "Enabled",
            "Focusable",
            "Active",
            "InActivePath",
            "Focused: no",
            "InFocusedPath: no",
            "Update Priority",
            "Memory Limit:",
            "PaintCount: 0",
            "LastPaintFrame: 0 (current: 0)",
        ] {
            assert!(text.contains(label), "Text missing label `{}`:\n{}", label, text);
        }

        let children = rec.get_array("Children").expect("Children exists");
        assert!(children.is_empty());
    }

    #[test]
    fn dump_panel_recurses_into_children() {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.put_behavior(root, Box::new(NoopBehavior));
        let c1 = tree.create_child(root, "alpha", None);
        tree.put_behavior(c1, Box::new(NoopBehavior));
        let c2 = tree.create_child(root, "beta", None);
        tree.put_behavior(c2, Box::new(NoopBehavior));

        let rec = dump_panel(&tree, root, 0, None, 1.0, 1.0, false);

        let children = rec.get_array("Children").expect("Children exists");
        assert_eq!(children.len(), 2);

        // Each child rec's Text must contain its own Name: line.
        let mut names: Vec<String> = Vec::new();
        for child in children {
            let s = match child {
                RecValue::Struct(s) => s,
                other => panic!("child is not Struct: {:?}", other),
            };
            let text = s.get_str("Text").expect("child Text exists");
            if text.contains("\nName: alpha") {
                names.push("alpha".into());
            }
            if text.contains("\nName: beta") {
                names.push("beta".into());
            }
        }
        names.sort();
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    }

    // --- dump_view / dump_window / dump_context tests ---

    use crate::emView::emView;

    #[test]
    fn dump_view_emits_required_labels() {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.put_behavior(root, Box::new(NoopBehavior));

        let ctx = crate::emContext::emContext::NewRoot();
        let view = emView::new(ctx, root, 800.0, 600.0);

        let rec = dump_view(&view, &tree, false);

        let title = rec.get_str("Title").expect("Title exists").to_string();
        assert_eq!(title, "View (Context):\nemView");

        let text = rec.get_str("Text").expect("Text exists").to_string();
        for label in [
            "Common Models:",
            "Private Models: 0 (not listed)",
            "View Flags:",
            "Title:",
            "Focused:",
            "Activation Adherent:",
            "Popped Up:",
            "Background Color:",
            "Home XYWH:",
            "Current XYWH:",
        ] {
            assert!(text.contains(label), "Text missing `{}`:\n{}", label, text);
        }

        let children = rec.get_array("Children").expect("Children exists");
        assert_eq!(children.len(), 1, "view rec must have one (root-panel) child");
        assert!(matches!(children[0], RecValue::Struct(_)));
    }

    #[test]
    fn dump_view_no_flags_emits_zero() {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.put_behavior(root, Box::new(NoopBehavior));

        let ctx = crate::emContext::emContext::NewRoot();
        let view = emView::new(ctx, root, 100.0, 100.0);

        let rec = dump_view(&view, &tree, false);
        let text = rec.get_str("Text").expect("Text").to_string();
        assert!(
            text.contains("View Flags: 0"),
            "empty ViewFlags should render as `0`:\n{}",
            text
        );
    }

    #[test]
    fn dump_window_emits_flags_and_wmresname() {
        use crate::emColor::emColor;
        use crate::emScheduler::EngineScheduler;
        use crate::emWindow::WindowFlags;

        let ctx = crate::emContext::emContext::NewRoot();
        let mut scheduler = EngineScheduler::new();
        let close_sig = scheduler.create_signal();
        let flags_sig = scheduler.create_signal();
        let focus_sig = scheduler.create_signal();
        let geom_sig = scheduler.create_signal();

        let win = crate::emWindow::emWindow::new_popup_pending(
            ctx,
            WindowFlags::POPUP | WindowFlags::UNDECORATED,
            "caption".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            emColor::TRANSPARENT,
        );

        let rec = dump_window(&win);

        let title = rec.get_str("Title").expect("Title").to_string();
        assert_eq!(title, "Window (View, Context):\nemWindow");

        let text = rec.get_str("Text").expect("Text").to_string();
        assert!(text.contains("Window Flags:"), "missing `Window Flags:`:\n{}", text);
        assert!(text.contains("WF_POPUP"), "missing WF_POPUP:\n{}", text);
        assert!(text.contains("WF_UNDECORATED"), "missing WF_UNDECORATED:\n{}", text);
        assert!(text.contains("WMResName:"), "missing `WMResName:`:\n{}", text);

        // MAXIMIZED / AUTO_DELETE are Rust-only and must NOT appear.
        assert!(!text.contains("WF_MAXIMIZED"), "Rust-only flag leaked:\n{}", text);
        assert!(!text.contains("WF_AUTO_DELETE"), "Rust-only flag leaked:\n{}", text);
    }

    #[test]
    fn dump_context_with_cascade_iterates_child_contexts() {
        let root = crate::emContext::emContext::NewRoot();
        let child1 = crate::emContext::emContext::NewChild(&root);
        let child2 = crate::emContext::emContext::NewChild(&root);
        // Sanity: parent records both children before we walk.
        assert_eq!(root.child_count(), 2, "parent should hold 2 live children");

        // Empty ViewMap: no child is a known view, so both children emit
        // as plain dump_context recs.
        let view_map = ViewMap::new();
        let rec = dump_context_with_cascade(&root, true, &view_map);
        let children = rec.get_array("Children").expect("Children exists");
        assert_eq!(
            children.len(),
            2,
            "expected 2 child context recs, got {}",
            children.len()
        );

        drop(child1);
        drop(child2);
    }

    #[test]
    fn dump_context_root_vs_child_titles() {
        let root = crate::emContext::emContext::NewRoot();
        let child = crate::emContext::emContext::NewChild(&root);

        let root_rec = dump_context(&root, true);
        let child_rec = dump_context(&child, false);

        assert_eq!(
            root_rec.get_str("Title"),
            Some("Root Context:\nemRootContext")
        );
        assert_eq!(child_rec.get_str("Title"), Some("Context:\nemContext"));

        // Both should carry Common/Private Models lines.
        let rtxt = root_rec.get_str("Text").unwrap();
        assert!(rtxt.contains("Common Models:"));
        assert!(rtxt.contains("Private Models: 0 (not listed)"));
    }

    #[test]
    fn dump_panel_appends_subtype_dump_state_pairs() {
        struct LoadingBehavior;
        impl PanelBehavior for LoadingBehavior {
            fn dump_state(&self) -> Vec<(&'static str, String)> {
                vec![
                    ("loading_pct", "42".to_string()),
                    ("loading_done", "false".to_string()),
                ]
            }
        }

        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.put_behavior(root, Box::new(LoadingBehavior));

        let rec = dump_panel(&tree, root, 0, None, 1.0, 1.0, false);

        let text = rec.get_str("Text").expect("Text exists").to_string();
        assert!(
            text.contains("\nloading_pct: 42"),
            "Text missing loading_pct pair:\n{}",
            text
        );
        assert!(
            text.contains("\nloading_done: false"),
            "Text missing loading_done pair:\n{}",
            text
        );
        // Insertion order: loading_pct must come before loading_done.
        let idx_pct = text.find("\nloading_pct:").expect("loading_pct present");
        let idx_done = text.find("\nloading_done:").expect("loading_done present");
        assert!(
            idx_pct < idx_done,
            "dump_state Vec order not preserved: pct@{} done@{}",
            idx_pct,
            idx_done
        );
    }

    #[test]
    fn dump_model_emits_name_and_lifetime() {
        let rec = dump_model("foo", "MyModel", 42);
        assert_eq!(
            rec.get_str("Title"),
            Some("Common Model:\nMyModel\n\"foo\"")
        );
        let text = rec.get_str("Text").expect("Text present");
        assert!(text.contains("\nName: foo"), "Text missing Name:\n{}", text);
        assert!(
            text.contains("\nMin Common Lifetime: 42"),
            "Text missing Min Common Lifetime:\n{}",
            text
        );
        // Visual style: model.
        assert_eq!(rec.get_ident("Frame"), Some("frame_hexagon"));
        assert_eq!(rec.get_int("BgColor"), Some(0x440000));
        assert_eq!(rec.get_int("FgColor"), Some(0xBBBBBB));
        // Children: empty (no sub-records).
        let children = rec.get_array("Children");
        assert!(
            children.is_none() || children.expect("checked").is_empty(),
            "Children must be empty for model dump"
        );
    }

    #[test]
    fn dump_file_model_emits_file_state() {
        let rec = dump_file_model(
            "doc",
            "TextModel",
            "/tmp/doc.txt",
            FileStateLabel::Loaded,
            1024,
        );
        assert_eq!(
            rec.get_str("Title"),
            Some("Common File Model:\nTextModel\n\"doc\"")
        );
        let text = rec.get_str("Text").expect("Text present");
        assert!(
            text.contains("\nFile Path: /tmp/doc.txt"),
            "Text missing File Path:\n{}",
            text
        );
        assert!(
            text.contains("\nFile State: FS_LOADED"),
            "Text missing File State:\n{}",
            text
        );
        assert!(
            text.contains("\nMemory Need: 1024"),
            "Text missing Memory Need:\n{}",
            text
        );
        // Visual style: file_model.
        assert_eq!(rec.get_ident("Frame"), Some("frame_hexagon"));
        assert_eq!(rec.get_int("BgColor"), Some(0x440033));
        assert_eq!(rec.get_int("FgColor"), Some(0xBBBBBB));
    }

    #[test]
    fn file_state_label_strings_match_cpp() {
        let cases = [
            (FileStateLabel::Waiting, "FS_WAITING"),
            (FileStateLabel::Loading, "FS_LOADING"),
            (FileStateLabel::Loaded, "FS_LOADED"),
            (FileStateLabel::Unsaved, "FS_UNSAVED"),
            (FileStateLabel::Saving, "FS_SAVING"),
            (FileStateLabel::TooCostly, "FS_TOO_COSTLY"),
            (FileStateLabel::LoadError, "FS_LOAD_ERROR"),
            (FileStateLabel::SaveError, "FS_SAVE_ERROR"),
        ];
        for (label, expected) in cases {
            assert_eq!(label.as_str(), expected, "label {:?}", label);
        }
    }

    #[test]
    fn general_info_text_has_all_labels() {
        let s = general_info_text();
        for label in [
            "General Info",
            "Time",
            "Host Name",
            "User Name",
            "Process Id",
            "Current Dir",
            "UTF8",
            "Byte Order",
            "sizeof(ptr)",
            "sizeof(lng)",
            "char",
            "CPU-TSC",
            "Paths of emCore:",
            "Bin",
            "Include",
            "Lib",
            "Html Doc",
            "Pdf Doc",
            "Ps Doc",
            "User Config",
            "Host Config",
            "Tmp",
            "Res",
            "Home",
        ] {
            assert!(s.contains(label), "general info missing `{}`:\n{}", label, s);
        }
    }

    #[test]
    fn days_to_ymd_known_dates() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
        assert_eq!(days_to_ymd(20089), (2025, 1, 1));
        // 2000-01-01 = 10957 days since epoch
        assert_eq!(days_to_ymd(10957), (2000, 1, 1));
        // 1970-12-31
        assert_eq!(days_to_ymd(364), (1970, 12, 31));
    }

    #[test]
    fn dump_from_root_context_has_general_info_and_root_ctx_child() {
        let root = crate::emContext::emContext::NewRoot();
        let rec = dump_from_root_context(&root);

        let title = rec.get_str("Title").expect("Title").to_string();
        assert!(title.starts_with("Tree Dump\n"), "Title: {}", title);

        let text = rec.get_str("Text").expect("Text").to_string();
        assert!(text.contains("General Info"), "Text missing General Info:\n{}", text);

        let children = rec.get_array("Children").expect("Children");
        assert_eq!(children.len(), 1);
        let child = match &children[0] {
            RecValue::Struct(s) => s,
            other => panic!("child[0] not Struct: {:?}", other),
        };
        let child_title = child.get_str("Title").expect("child Title");
        assert!(
            child_title.contains("Root Context"),
            "child title missing `Root Context`: {}",
            child_title
        );
    }

    #[test]
    fn file_state_label_from_runtime_state() {
        use crate::emFileModel::FileState;
        let cases: [(FileState, FileStateLabel); 8] = [
            (FileState::Waiting, FileStateLabel::Waiting),
            (
                FileState::Loading { progress: 0.0 },
                FileStateLabel::Loading,
            ),
            (FileState::Loaded, FileStateLabel::Loaded),
            (FileState::Unsaved, FileStateLabel::Unsaved),
            (FileState::Saving, FileStateLabel::Saving),
            (FileState::TooCostly, FileStateLabel::TooCostly),
            (FileState::LoadError("x".into()), FileStateLabel::LoadError),
            (FileState::SaveError("y".into()), FileStateLabel::SaveError),
        ];
        for (state, expected) in &cases {
            assert_eq!(FileStateLabel::from(state), *expected, "state {:?}", state);
        }
    }
}

#[cfg(test)]
mod collect_views_tests {
    use super::*;
    use crate::emContext::emContext;
    use crate::emPanelTree::PanelTree;
    use crate::emSubViewPanel::emSubViewPanel;
    use crate::emView::emView;
    use std::rc::Rc;

    /// Shared scaffolding: context, scheduler, window id.
    fn fixture_base() -> (
        Rc<emContext>,
        Rc<emContext>,
        crate::emScheduler::EngineScheduler,
        winit::window::WindowId,
    ) {
        let root_ctx = emContext::NewRoot();
        let home_ctx = emContext::NewChild(&root_ctx);
        let sched = crate::emScheduler::EngineScheduler::new();
        let wid = winit::window::WindowId::dummy();
        (root_ctx, home_ctx, sched, wid)
    }

    /// Recursively tear down a tree: for every emSubViewPanel, drain its
    /// sub_view's scheduler-registered engines/signals, recursively tear
    /// down its sub_tree, then remove the slot. Finally remove the root.
    /// Mirrors `SvpTestHarness::teardown` from emSubViewPanel.rs.
    fn teardown_tree(
        mut tree: PanelTree,
        sched: &mut crate::emScheduler::EngineScheduler,
    ) {
        let Some(root) = tree.GetRootPanel() else { return };
        // Collect all SVP slot ids by walking.
        fn collect_svp_ids(tree: &PanelTree, out: &mut Vec<crate::emPanelTree::PanelId>) {
            for pid in tree.panel_ids() {
                if let Some(svp) = tree.behavior(pid).and_then(|b| b.as_sub_view_panel()) {
                    out.push(pid);
                    collect_svp_ids(svp.sub_tree(), out);
                }
            }
        }
        let mut svp_ids = Vec::new();
        collect_svp_ids(&tree, &mut svp_ids);

        // Tear down each SVP (deepest first not required since we extract
        // and re-set behavior individually).
        for pid in svp_ids {
            tree.with_behavior_as::<emSubViewPanel, _>(pid, |svp| {
                // Recursively clean nested sub_tree's SVPs first.
                let sub_root = svp.sub_tree().GetRootPanel();
                if let Some(sr) = sub_root {
                    // Collect nested SVP ids in the sub_tree and tear them
                    // down. We can't recurse into teardown_tree because we
                    // hold &mut svp here — inline minimal cleanup.
                    let mut nested = Vec::new();
                    collect_svp_ids(svp.sub_tree(), &mut nested);
                    for npid in nested {
                        svp.sub_tree_mut()
                            .with_behavior_as::<emSubViewPanel, _>(npid, |nsvp| {
                                if let Some(eid) = nsvp.sub_view.update_engine_id.take() {
                                    sched.remove_engine(eid);
                                }
                                if let Some(eid) = nsvp.sub_view.visiting_va_engine_id.take() {
                                    sched.remove_engine(eid);
                                }
                                if let Some(sig) = nsvp.sub_view.EOISignal.take() {
                                    sched.remove_signal(sig);
                                }
                                let nsr = nsvp.sub_root();
                                nsvp.sub_tree_mut().remove(nsr, Some(sched));
                            });
                    }
                    svp.sub_tree_mut().remove(sr, Some(sched));
                }
                if let Some(eid) = svp.sub_view.update_engine_id.take() {
                    sched.remove_engine(eid);
                }
                if let Some(eid) = svp.sub_view.visiting_va_engine_id.take() {
                    sched.remove_engine(eid);
                }
                if let Some(sig) = svp.sub_view.EOISignal.take() {
                    sched.remove_signal(sig);
                }
            });
        }

        tree.remove(root, Some(sched));
    }

    /// Install a fresh emSubViewPanel as behavior at `slot_id`, parenting
    /// its emContext to `parent_ctx`.
    fn install_svp(
        tree: &mut PanelTree,
        slot_id: crate::emPanelTree::PanelId,
        parent_ctx: &Rc<emContext>,
        root_ctx: &Rc<emContext>,
        sched: &mut crate::emScheduler::EngineScheduler,
        wid: winit::window::WindowId,
    ) {
        use std::cell::RefCell;
        let svp = {
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                RefCell::new(None);
            let pa: Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: sched,
                framework_actions: &mut fw,
                root_context: root_ctx,
                framework_clipboard: &cb,
                current_engine: None,
                pending_actions: &pa,
            };
            emSubViewPanel::new(Rc::clone(parent_ctx), slot_id, wid, &mut sc)
        };
        tree.set_behavior(slot_id, Box::new(svp));
    }

    fn build_outer_with_one_subview()
    -> (emView, PanelTree, crate::emScheduler::EngineScheduler) {
        let (root_ctx, home_ctx, mut sched, wid) = fixture_base();
        let mut tree = PanelTree::new();
        let root = tree.create_root("outer_root", true);
        tree.init_panel_view(root, None);
        let svp_id = tree.create_child(root, "svp_slot", None);
        install_svp(&mut tree, svp_id, &home_ctx, &root_ctx, &mut sched, wid);
        let view = emView::new(home_ctx, root, 1.0, 1.0);
        (view, tree, sched)
    }

    fn build_outer_with_two_subviews()
    -> (emView, PanelTree, crate::emScheduler::EngineScheduler) {
        let (root_ctx, home_ctx, mut sched, wid) = fixture_base();
        let mut tree = PanelTree::new();
        let root = tree.create_root("outer_root", true);
        tree.init_panel_view(root, None);
        let svp1 = tree.create_child(root, "svp1", None);
        let svp2 = tree.create_child(root, "svp2", None);
        install_svp(&mut tree, svp1, &home_ctx, &root_ctx, &mut sched, wid);
        install_svp(&mut tree, svp2, &home_ctx, &root_ctx, &mut sched, wid);
        let view = emView::new(home_ctx, root, 1.0, 1.0);
        (view, tree, sched)
    }

    fn build_nested_subview_fixture()
    -> (emView, PanelTree, crate::emScheduler::EngineScheduler) {
        let (root_ctx, home_ctx, mut sched, wid) = fixture_base();
        let mut tree = PanelTree::new();
        let root = tree.create_root("outer_root", true);
        tree.init_panel_view(root, None);
        let outer_svp_id = tree.create_child(root, "outer_svp", None);
        install_svp(&mut tree, outer_svp_id, &home_ctx, &root_ctx, &mut sched, wid);

        // Drill into outer_svp's sub_tree; install a grandchild SVP there.
        tree.with_behavior_as::<emSubViewPanel, _>(outer_svp_id, |outer_svp| {
            let inner_ctx = Rc::clone(outer_svp.sub_view.GetContext());
            let sub_tree = outer_svp.sub_tree_mut();
            let sub_root = sub_tree
                .GetRootPanel()
                .expect("sub_tree has a root from emSubViewPanel::new");
            let grand_svp_id = sub_tree.create_child(sub_root, "inner_svp", None);
            install_svp(sub_tree, grand_svp_id, &inner_ctx, &root_ctx, &mut sched, wid);
        });

        let view = emView::new(home_ctx, root, 1.0, 1.0);
        (view, tree, sched)
    }

    #[test]
    fn no_subviews_produces_single_entry() {
        let root_ctx = emContext::NewRoot();
        let home_ctx = emContext::NewChild(&root_ctx);
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", true);
        let view = emView::new(Rc::clone(&home_ctx), root, 1.0, 1.0);

        let map = collect_views(&view, &tree);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&Rc::as_ptr(view.GetContext())));
    }

    #[test]
    fn one_subview_produces_two_entries() {
        let (view, tree, mut sched) = build_outer_with_one_subview();
        let map = collect_views(&view, &tree);
        let len = map.len();
        let has_outer = map.contains_key(&Rc::as_ptr(view.GetContext()));
        drop(map);
        drop(view);
        teardown_tree(tree, &mut sched);
        assert_eq!(len, 2, "expected outer view + one inner view");
        assert!(has_outer);
    }

    #[test]
    fn multiple_subviews_all_mapped() {
        let (view, tree, mut sched) = build_outer_with_two_subviews();
        let map = collect_views(&view, &tree);
        let len = map.len();
        drop(map);
        drop(view);
        teardown_tree(tree, &mut sched);
        assert_eq!(len, 3);
    }

    #[test]
    fn nested_subview_recursion() {
        let (view, tree, mut sched) = build_nested_subview_fixture();
        let map = collect_views(&view, &tree);
        let len = map.len();
        drop(map);
        drop(view);
        teardown_tree(tree, &mut sched);
        assert_eq!(len, 3, "outer + two levels of nested subviews");
    }
}
