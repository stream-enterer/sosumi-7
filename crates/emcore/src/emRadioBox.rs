use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::{emPainter, BORDER_EDGES_ONLY};
use crate::emPanel::PanelState;
use crate::emPanel::Rect;

use super::emBorder::{emBorder, OuterBorderType};
use crate::emBorder::with_toolkit_images;
use crate::emEngineCtx::PanelCtx;
use crate::emLook::emLook;
use crate::emRadioButton::RadioGroup;

/// Small radio box widget — box indicator with label text.
///
/// C++ `emRadioBox` inherits `emRadioButton : emCheckButton : emButton : emBorder`.
/// Constructor sets: `OBT_MARGIN`, `LabelAlignment=LEFT`, `ShownBoxed=true`,
/// `ShownRadioed=true`.
///
/// Paint uses the C++ DoButton boxed+radioed path (emButton.cpp:233-341):
/// content_rect → box geometry → circular face → radio dot → emRadioBox image overlay.
pub struct emRadioBox {
    border: emBorder,
    look: Rc<emLook>,
    group: Rc<RefCell<RadioGroup>>,
    index_cell: Rc<Cell<usize>>,
    pressed: bool,
    box_pressed: bool,
    /// Cached enabled state from the last paint call. Gates input handling.
    enabled: bool,
    last_w: f64,
    last_h: f64,
}

impl emRadioBox {
    pub fn new(
        label: &str,
        look: Rc<emLook>,
        group: Rc<RefCell<RadioGroup>>,
        _index: usize,
    ) -> Self {
        let index_cell = group.borrow_mut().register();
        Self {
            border: emBorder::new(OuterBorderType::Margin)
                .with_caption(label)
                .with_label_in_border(false)
                .with_label_alignment(crate::emPainter::TextAlignment::Left)
                .with_how_to(true),
            look,
            group,
            index_cell,
            pressed: false,
            box_pressed: false,
            enabled: true,
            last_w: 0.0,
            last_h: 0.0,
        }
    }

    pub fn index(&self) -> usize {
        self.index_cell.get()
    }

    pub fn set_index(&mut self, index: usize) {
        self.index_cell.set(index);
    }

    pub fn IsSelected(&self) -> bool {
        self.group.borrow().GetChecked() == Some(self.index_cell.get())
    }

    pub fn set_checked(&mut self, checked: bool, ctx: &mut PanelCtx<'_>) {
        if checked {
            self.group
                .borrow_mut()
                .SetChecked(self.index_cell.get(), ctx);
        } else if self.IsSelected() {
            self.group.borrow_mut().SetCheckIndex(None, ctx);
        }
    }

    /// Compute the box + label geometry from the content rect (C++ lines 235-260).
    /// Returns (bx0, by0, bw0, lx, ly, lw, lh).
    fn box_label_geometry(&self, cr: &Rect) -> (f64, f64, f64, f64, f64, f64, f64) {
        let has_label = self.border.HasLabel();
        if has_label {
            let label_tallness = self.border.GetBestLabelTallness().max(0.2);
            let mut box_w = label_tallness;
            let mut d = box_w * 0.1;
            let f = (cr.w / (box_w + d + 1.0)).min(cr.h / label_tallness);
            box_w *= f;
            d *= f;
            let lw = cr.w - box_w - d;
            let lh = box_w;
            let lx = cr.x + cr.w - lw;
            let ly = cr.y + (cr.h - lh) * 0.5;
            let bw0 = box_w;
            let bx0 = cr.x;
            let by0 = cr.y + (cr.h - bw0) * 0.5;
            (bx0, by0, bw0, lx, ly, lw, lh)
        } else {
            let bw0 = cr.w.min(cr.h);
            let bx0 = cr.x;
            let by0 = cr.y + (cr.h - bw0) * 0.5;
            (bx0, by0, bw0, cr.x, cr.y, 0.0, 0.0)
        }
    }

    /// Paint using the C++ DoButton ShownBoxed=true, ShownRadioed=true path.
    pub fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        enabled: bool,
        pixel_scale: f64,
    ) {
        self.last_w = w;
        self.last_h = h;
        self.enabled = enabled;
        self.border.paint_border(
            painter,
            canvas_color,
            w,
            h,
            &self.look,
            false,
            true,
            pixel_scale,
        );
        let canvas_color = self
            .border
            .content_canvas_color(canvas_color, &self.look, enabled);

        let cr = self.border.GetContentRect(w, h, &self.look);
        let (bx0, by0, bw0, mut lx, mut ly, mut lw, mut lh) = self.box_label_geometry(&cr);

        // Inset for image area: d = bw * 0.13 (C++ line 262).
        let d = bw0 * 0.13;
        let mut bx = bx0 + d;
        let by = by0 + d;
        let bw = bw0 - 2.0 * d;
        let bh = bw;

        // Face inset: d = bw * 30/380 (C++ line 268).
        let d2 = bw * 30.0 / 380.0;
        let mut fx = bx + d2;
        let fy = by + d2;
        let fw = bw - 2.0 * d2;
        let fh = bh - 2.0 * d2;
        // C++ line 273: ShownRadioed → fr = fw * 0.5 (fully circular).
        let fr = fw * 0.5;

        // C++ lines 294-300: Pressed && !BoxPressed nudges box/label.
        if self.pressed && !self.box_pressed {
            bx += lw * 0.003;
            fx += lw * 0.003;
            lx += lw * 0.003;
            ly += lh * 0.007;
            lw *= 0.986;
            lh *= 0.986;
        }

        // Paint label to the right of the box.
        if self.border.HasLabel() {
            self.border.paint_label(
                painter,
                canvas_color,
                Rect::new(lx, ly, lw, lh),
                &self.look,
                enabled,
            );
        }

        // Paint face (InputBgColor) — circular for radio.
        let face_color = self.look.input_bg_color;
        painter.PaintRoundRect(fx, fy, fw, fh, fr, fr, face_color, canvas_color);
        painter.SetCanvasColor(face_color);

        // Paint radio dot if selected (C++ PaintBoxSymbol, lines 161-167).
        // C++ PaintEllipse takes bounding rect (x, y, w, h).
        // Rust paint_ellipse takes center + radii (cx, cy, rx, ry).
        if self.IsSelected() {
            let dot_d = fw * 0.25;
            let dot_w = fw - 2.0 * dot_d;
            let dot_h = fh - 2.0 * dot_d;
            painter.PaintEllipse(
                fx + dot_d,
                fy + dot_d,
                dot_w,
                dot_h,
                self.look.input_fg_color,
                face_color,
            );
        }

        // Paint radio box image overlay (C++ lines 318-331).
        // BoxPressed → RadioBoxPressed image, else → emRadioBox image.
        with_toolkit_images(|img| {
            let box_img = if self.box_pressed {
                &img.radio_box_pressed
            } else {
                &img.radio_box
            };
            painter.paint_image_full(bx, by, bw, bh, box_img, 255, emColor::TRANSPARENT);
        });

        // C++ lines 333-340: Pressed && !BoxPressed → GroupInnerBorder overlay.
        if self.pressed && !self.box_pressed {
            let r = cr.h * 0.2;
            with_toolkit_images(|img| {
                painter.PaintBorderImage(
                    cr.x,
                    cr.y,
                    cr.w,
                    cr.h,
                    r,
                    r,
                    r,
                    r,
                    &img.group_inner_border,
                    225,
                    225,
                    225,
                    225,
                    255,
                    emColor::TRANSPARENT,
                    BORDER_EDGES_ONLY,
                );
            });
        }

        // C++ DoButton: disabled gray overlay for boxed+radioed path.
        // PaintRoundRect(fx, fy, fw, fh, fr, fr, 0x888888E0).
        if !enabled {
            painter.PaintRoundRect(
                fx,
                fy,
                fw,
                fh,
                fr,
                fr,
                emColor::rgba(0x88, 0x88, 0x88, 0xE0),
                emColor::TRANSPARENT,
            );
        }
    }

    /// Rounded-rect hit test matching C++ `emButton::CheckMouse` boxed path.
    /// Uses content_rect with r = h * 0.2 (C++ emButton.cpp:276).
    fn hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let rect = self.border.GetContentRect(1.0, tallness, &self.look);
        let r = rect.h * 0.2;
        // RUST_ONLY: (language-forced-utility) widget_utils.rs -- C++ inlines this formula per widget
        let dx = ((rect.x - mx).max(mx - rect.x - rect.w) + r).max(0.0);
        let dy = ((rect.y - my).max(my - rect.y - rect.h) + r).max(0.0);
        dx * dx + dy * dy <= r * r
    }

    /// Box-specific hit test matching C++ `emButton::CheckMouse` inBox check.
    /// Tests whether (mx, my) is within the radio circle's face area.
    fn box_hit_test(&self, mx: f64, my: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w;
        let cr = self.border.GetContentRect(1.0, tallness, &self.look);
        let (_bx0, _by0, bw0, _lx, _ly, _lw, _lh) = self.box_label_geometry(&cr);

        let d = bw0 * 0.13;
        let bx = cr.x + d;
        let by = cr.y + (cr.h - bw0) * 0.5 + d;
        let bw = bw0 - 2.0 * d;
        let bh = bw;

        let d2 = bw * 30.0 / 380.0;
        let fx = bx + d2;
        let fy = by + d2;
        let fw = bw - 2.0 * d2;
        let fh = bh - 2.0 * d2;
        // ShownRadioed: fr = fw * 0.5 (fully circular).
        let fr = fw * 0.5;

        let dx = ((fx - mx).max(mx - fx - fw) + fr).max(0.0);
        let dy = ((fy - my).max(my - fy - fh) + fr).max(0.0);
        dx * dx + dy * dy <= fr * fr
    }

    pub fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        _input_state: &emInputState,
        ctx: &mut crate::emEngineCtx::PanelCtx,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        // RUST_ONLY: (language-forced-utility) widget_utils.rs -- debug trace aid, no C++ equivalent
        let trace = {
            use std::sync::OnceLock;
            static ENABLED: OnceLock<bool> = OnceLock::new();
            *ENABLED.get_or_init(|| std::env::var("TRACE_INPUT").is_ok())
        };
        match event.key {
            InputKey::MouseLeft => match event.variant {
                InputVariant::Press => {
                    // C++ emButton.cpp:82: (state.IsNoMod() || state.IsShiftMod())
                    if event.ctrl || event.alt || event.meta {
                        return false;
                    }
                    // C++ emButton.cpp:84: GetViewCondition(VCT_MIN_EXT) >= 8.0
                    let min_ext = state.viewed_rect.w.min(state.viewed_rect.h);
                    if min_ext < 8.0 {
                        return false;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [RadioBox {:?}] Press mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed
                        );
                    }
                    if !hit {
                        return false;
                    }
                    self.pressed = true;
                    self.box_pressed = self.box_hit_test(event.mouse_x, event.mouse_y);
                    true
                }
                InputVariant::Release => {
                    if !self.pressed {
                        return false;
                    }
                    // C++ emButton.cpp:101: IsViewed check on release.
                    if !state.viewed {
                        self.pressed = false;
                        self.box_pressed = false;
                        return true;
                    }
                    let hit = self.hit_test(event.mouse_x, event.mouse_y);
                    if trace {
                        eprintln!(
                            "    [RadioBox {:?}] Release mouse=({:.4},{:.4}) last=({:.4},{:.4}) hit={} pressed={} box_pressed={} selected_before={}",
                            self.border.caption, event.mouse_x, event.mouse_y, self.last_w, self.last_h, hit, self.pressed, self.box_pressed, self.IsSelected()
                        );
                    }
                    self.pressed = false;
                    self.box_pressed = false;
                    if hit {
                        self.group
                            .borrow_mut()
                            .SetChecked(self.index_cell.get(), ctx);
                    }
                    true
                }
                _ => false,
            },
            // C++ emButton.cpp:113-119: Enter only, instant Click().
            // Gated on (IsNoMod || IsShiftMod).
            InputKey::Enter
                if event.variant == InputVariant::Press
                    && !event.alt
                    && !event.meta
                    && !event.ctrl
                    && state.viewed_rect.w.min(state.viewed_rect.h) >= 8.0 =>
            {
                self.group
                    .borrow_mut()
                    .SetChecked(self.index_cell.get(), ctx);
                true
            }
            _ => false,
        }
    }

    pub fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let th = 13.0;
        let tw = emPainter::measure_text_width(&self.border.caption, th);
        self.border.preferred_size_for_content(tw + 8.0, th + 4.0)
    }
}

impl Drop for emRadioBox {
    fn drop(&mut self) {
        self.group.borrow_mut().deregister(&self.index_cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx, PanelCtx};
    use crate::emScheduler::EngineScheduler;
    use std::rc::Rc;

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<crate::emContext::emContext>,
        pa: Rc<RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl Drop for TestInit {
        fn drop(&mut self) {
            // B3.4c: clear pending signals accumulated during Input-path tests
            self.sched.clear_pending_for_tests();
        }
    }

    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: crate::emContext::emContext::NewRoot(),
                pa: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
                pending_actions: &self.pa,
            }
        }
    }
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use slotmap::Key as _;

    fn test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

    fn default_panel_state() -> PanelState {
        PanelState {
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

    fn default_input_state() -> emInputState {
        emInputState::new()
    }

    #[test]
    fn radio_box_selection() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());

        let mut rb0 = emRadioBox::new("X", look.clone(), group.clone(), 0);
        let mut rb1 = emRadioBox::new("Y", look, group.clone(), 1);
        let ps = default_panel_state();
        let is = default_input_state();

        assert!(!rb0.IsSelected());
        assert!(!rb1.IsSelected());

        // Enter is instant: selects on press, no release needed.
        rb0.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(rb0.IsSelected()); // Selected immediately on press
        assert!(!rb1.IsSelected());

        rb1.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!rb0.IsSelected());
        assert!(rb1.IsSelected());
    }

    #[test]
    fn pressed_state_tracks_press_release() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        // Enter is instant — no visual press state. Verify pressed stays false.
        let look = emLook::new();
        let group = RadioGroup::new(&mut __init.ctx());
        let mut rb = emRadioBox::new("X", look, group.clone(), 0);
        let ps = default_panel_state();
        let is = default_input_state();
        assert!(!rb.pressed);
        rb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        assert!(!rb.pressed); // Enter selects instantly, no press state
        assert!(rb.IsSelected()); // But the selection did happen
    }
}
