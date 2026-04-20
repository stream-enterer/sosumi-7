use std::any::Any;

use bitflags::bitflags;

use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emInput::emInputEvent;
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;

use super::emPanelTree::{PanelId, PlaybackState};
use crate::emEngineCtx::EngineCtx;
use crate::emEngineCtx::PanelCtx;

// RUST_ONLY: rect.rs -- Consolidates C++ pattern of passing 4 separate
// doubles (GetLayoutX/Y/Width/Height in emPanel.h) into a typed struct.
// C++ has no dedicated layout rect type.

/// Logical rectangle (f64) — layout coordinates.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);
        if x2 > x && y2 > y {
            Some(Rect {
                x,
                y,
                w: x2 - x,
                h: y2 - y,
            })
        } else {
            None
        }
    }

    pub fn contains_point(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    pub fn area(&self) -> f64 {
        self.w * self.h
    }
}

/// Invalidation signals that a panel behavior wants to propagate to the parent
/// view. Used by [`emSubViewPanel`](super::emSubViewPanel::emSubViewPanel) to forward its sub-view's
/// dirty rects, title changes, and cursor changes to the enclosing view.
///
/// Corresponds to the C++ invalidation chain:
/// - `SubViewClass::InvalidateTitle()` → `SuperPanel.InvalidateTitle()`
/// - `SubViewPortClass::InvalidateCursor()` → `SuperPanel.InvalidateCursor()`
/// - `SubViewPortClass::InvalidatePainting(x,y,w,h)` → `SuperPanel.InvalidatePaintingOnView(x,y,w,h)`
#[derive(Clone, Debug, Default)]
pub struct ParentInvalidation {
    /// Dirty rectangles in absolute view (pixel) coordinates to push to the
    /// parent view.
    pub dirty_rects: Vec<Rect>,
    /// Whether the parent view's title should be marked invalid.
    pub title_invalid: bool,
    /// Whether the parent view's cursor should be marked invalid.
    pub cursor_invalid: bool,
}

/// Provides downcasting for trait objects. Automatically implemented
/// for all `'static` types via blanket impl — no per-widget boilerplate.
pub trait AsAny: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Read-only snapshot of panel state, passed to behavior callbacks.
///
/// Built by the framework before each `paint()`, `notice()`, and `input()`
/// call. Fields reflect the panel's state at the moment of the call.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct PanelState {
    /// This panel's ID.
    pub id: PanelId,
    /// Whether this panel is the active (focused-path leaf) panel.
    pub is_active: bool,
    /// Whether this panel is in the active path (ancestor of active, or active itself).
    pub in_active_path: bool,
    /// Whether the owning view/window is focused.
    pub window_focused: bool,
    /// Whether the panel is enabled (enable_switch AND all ancestors enabled).
    pub enabled: bool,
    /// Whether the panel is currently viewed (visible in the viewport).
    pub viewed: bool,
    /// The panel's clip rectangle in absolute view coordinates.
    pub clip_rect: Rect,
    /// The panel's full viewed rectangle in absolute view coordinates.
    pub viewed_rect: Rect,
    /// Update priority (0.0–1.0), based on centrality and focus.
    pub priority: f64,
    /// Memory limit in bytes for this panel's subtree.
    pub memory_limit: u64,
    /// Pixel tallness of the view (height/width ratio of a single pixel).
    ///
    /// Corresponds to `emPanel::GetViewedPixelTallness` /
    /// `emView::CurrentPixelTallness`.
    pub pixel_tallness: f64,
    /// Panel height in its own coordinate system: `layout_h / layout_w`.
    ///
    /// Corresponds to C++ `emPanel::GetHeight()`.
    pub height: f64,
}

impl PanelState {
    /// True if active AND window is focused. Matches C++ `emPanel::IsFocused()`.
    pub fn is_focused(&self) -> bool {
        self.is_active && self.window_focused
    }

    /// True if in active path AND window is focused. Matches C++ `emPanel::IsInFocusedPath()`.
    pub fn in_focused_path(&self) -> bool {
        self.in_active_path && self.window_focused
    }

    /// Create a test-only PanelState with sensible defaults.
    ///
    /// Useful for unit tests that call widget `input()` methods directly
    /// without the full panel framework.
    pub fn default_for_test() -> Self {
        use crate::emPanelTree::PanelId;
        use slotmap::Key as _;
        Self {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        }
    }
}

bitflags! {
    /// Port of C++ `emPanel::NoticeFlags` (emPanel.h:542-553). Names and
    /// bit values match C++ one-for-one.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct NoticeFlags: u32 {
        const CHILD_LIST_CHANGED      = 1 << 0;
        const LAYOUT_CHANGED          = 1 << 1;
        const VIEWING_CHANGED         = 1 << 2;
        const ENABLE_CHANGED          = 1 << 3;
        const ACTIVE_CHANGED          = 1 << 4;
        const FOCUS_CHANGED           = 1 << 5;
        const VIEW_FOCUS_CHANGED      = 1 << 6;
        const UPDATE_PRIORITY_CHANGED = 1 << 7;
        const MEMORY_LIMIT_CHANGED    = 1 << 8;
        const SOUGHT_NAME_CHANGED     = 1 << 9;
    }
}

/// Trait for panel behavior — the logic attached to a panel node.
///
/// All methods have default no-op implementations. Implementors override
/// only the methods they need.
pub trait PanelBehavior: AsAny {
    /// Paint the panel's content.
    fn Paint(&mut self, _painter: &mut emPainter, _w: f64, _h: f64, _state: &PanelState) {}

    /// Handle an input event. Returns true if the event was consumed.
    fn Input(
        &mut self,
        _event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        false
    }

    /// Get the cursor to display when the mouse is over this panel.
    fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }

    /// Whether this panel is fully opaque (no need to paint panels behind it).
    fn IsOpaque(&self) -> bool {
        false
    }

    /// Layout child panels. Called when the panel's layout rect changes.
    fn LayoutChildren(&mut self, _ctx: &mut PanelCtx) {}

    /// Create child panels by auto-expansion.
    ///
    /// Port of C++ `emPanel::AutoExpand`. Called when the view condition
    /// reaches a threshold value OR the panel is the seek target. Panels
    /// that dynamically create children based on view condition should
    /// override this. The default implementation does nothing.
    ///
    /// Children created inside this call are marked `CreatedByAE` and
    /// will be deleted by the default `AutoShrink` when the view
    /// condition falls below threshold.
    fn AutoExpand(&mut self, _ctx: &mut PanelCtx) {}

    /// Delete child panels created by auto-expansion.
    ///
    /// Port of C++ `emPanel::AutoShrink`. The default behavior deletes
    /// all children with `created_by_ae=true` (handled by the panel
    /// tree, not here). Panels only need to override this to reset
    /// panel pointer variables after children are deleted.
    fn AutoShrink(&mut self, _ctx: &mut PanelCtx) {}

    /// Receive a notice about state changes.
    ///
    /// Port of C++ `emPanel::Notice(NoticeFlags flags)`. In C++ the method
    /// runs on `*this` with implicit tree access (can create/delete
    /// children, queue more notices, navigate the view). In Rust the same
    /// access is exposed via `ctx: &mut PanelCtx`. Implementations that
    /// don't need tree access can accept `_ctx` and ignore it.
    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}

    /// Whether the panel wants to auto-expand to fill available space.
    fn auto_expand(&self) -> bool {
        false
    }

    /// Whether the panel wants to auto-shrink to fit its content.
    fn auto_shrink(&self) -> bool {
        false
    }

    /// Get the minimum size this panel needs.
    fn min_size(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// Get the preferred/natural size of this panel.
    fn preferred_size(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// Get the canvas color for this panel (used for canvas blending).
    fn GetCanvasColor(&self) -> emColor {
        emColor::TRANSPARENT
    }

    /// Return a title for this panel, or `None` to delegate to the parent.
    ///
    /// The tree walks up the parent chain; the root returns `"untitled"` if
    /// no behavior along the chain provides a title.
    fn get_title(&self) -> Option<String> {
        None
    }

    /// Return an icon filename for this panel, or `None` to delegate to the
    /// parent.
    ///
    /// The tree walks up the parent chain; the root returns `""` if no
    /// behavior along the chain provides an icon filename.
    fn GetIconFileName(&self) -> Option<String> {
        None
    }

    /// Return the current playback state.
    ///
    /// The default returns `PlaybackState { playing: false, pos: 0.0,
    /// supported: false }`.
    fn GetPlaybackState(&self) -> PlaybackState {
        PlaybackState::default()
    }

    /// Attempt to set the playback state. Returns `true` if the panel
    /// supports playback and accepted the new state.
    fn SetPlaybackState(&mut self, _playing: bool, _pos: f64) -> bool {
        false
    }

    /// Whether this panel has hope that a seeking operation can succeed.
    ///
    /// The default returns `false`.
    fn IsHopeForSeeking(&self) -> bool {
        false
    }

    /// TF-003: Return a panel-pixel rect that the view should scroll to make
    /// visible. Called by the framework after `input()`. The rect is in the
    /// same coordinate space as `paint(w, h)`.
    ///
    /// Returns `Some((x, y, w, h))` if a scroll is needed, `None` otherwise.
    fn take_scroll_to_visible(&mut self) -> Option<(f64, f64, f64, f64)> {
        None
    }

    /// Create a control panel as a child of `parent_ctx` with `name`.
    ///
    /// Return the new panel's id, or `None` to delegate to the parent
    /// (the tree walks up the parent chain; the root returns `None`).
    fn CreateControlPanel(&mut self, _parent_ctx: &mut PanelCtx, _name: &str) -> Option<PanelId> {
        None
    }

    /// Called each scheduler cycle.
    ///
    /// Corresponds to the C++ `emPanel::Cycle` protected virtual (inherited
    /// from `emEngine`). The default implementation does nothing and returns
    /// `false`.
    ///
    /// Per spec §3.3, `Cycle` receives `EngineCtx` (scheduler/windows/framework
    /// access) alongside `PanelCtx` (tree/panel access); the ctx split is
    /// field-disjoint so both may be held simultaneously. Impls that don't
    /// need scheduler access silence the unused binding with `let _ = ectx;`.
    fn Cycle(&mut self, _ectx: &mut EngineCtx<'_>, _pctx: &mut PanelCtx) -> bool {
        false
    }

    /// Drain any invalidation signals that this behavior wants to propagate to
    /// the parent view. Called by the framework after notice delivery and
    /// viewing updates.
    ///
    /// The default returns `None` (no propagation). Override in behaviors that
    /// manage a sub-view (e.g. [`emSubViewPanel`](super::emSubViewPanel::emSubViewPanel)) to
    /// forward dirty rects, title, and cursor invalidation from the embedded
    /// view to the enclosing view.
    fn drain_parent_invalidation(&mut self) -> Option<ParentInvalidation> {
        None
    }

    /// Return the type name for this behavior (used by tree dump).
    /// Defaults to `std::any::type_name_of_val(self)`.
    fn type_name(&self) -> &str {
        std::any::type_name_of_val(self)
    }

    /// Downcast to `emSubViewPanel` without `Any`. Phase 1.75 uses this in
    /// the scheduler dispatch walk to reach a sub-view's `sub_tree` when
    /// resolving a `TreeLocation::SubView` owner.
    ///
    /// The default returns `None`; only [`emSubViewPanel`](super::emSubViewPanel::emSubViewPanel)
    /// overrides this to return `Some(self)`.
    fn as_sub_view_panel_mut(&mut self) -> Option<&mut crate::emSubViewPanel::emSubViewPanel> {
        None
    }
}
