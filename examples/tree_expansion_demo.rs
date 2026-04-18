//! Tree-expansion demo derived from C++ `TreeExpansionExample.cpp`.
//!
//! Each panel fills itself with its background color and, on auto-expand,
//! creates four children whose color is the inverse of the GetParentContext's.
//! Zooming into any panel reveals the next level.

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emPanel::{PanelBehavior, PanelState};
use eaglemode_rs::emCore::emPanelCtx::PanelCtx;
use eaglemode_rs::emCore::emView::ViewFlags;
use eaglemode_rs::emCore::emPainter::emPainter;
use eaglemode_rs::emCore::emGUIFramework::App;
use eaglemode_rs::emCore::emWindow::WindowFlags;

struct MyPanel {
    bg: emColor,
}

impl PanelBehavior for MyPanel {
    fn IsOpaque(&self) -> bool {
        self.bg.IsOpaque()
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        painter.PaintRect(0.0, 0.0, w, h, self.bg, emColor::TRANSPARENT);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let rect = ctx.layout_rect();
        let h = rect.h / rect.w;

        if !children.is_empty() {
            // Reposition existing children.
            for (idx, child) in children.iter().enumerate() {
                let i = idx;
                let cx = 0.1 + (i & 1) as f64 * 0.5;
                let cy = (0.1 + ((i >> 1) & 1) as f64 * 0.5) * h;
                ctx.layout_child(*child, cx, cy, 0.3, 0.3 * h);
            }
            return;
        }

        // Create four children with inverted color.
        let inv = emColor::rgba(
            255 - self.bg.GetRed(),
            255 - self.bg.GetGreen(),
            255 - self.bg.GetBlue(),
            self.bg.GetAlpha(),
        );
        for i in 0..4u32 {
            let name = format!("{i}");
            let cx = 0.1 + (i & 1) as f64 * 0.5;
            let cy = (0.1 + ((i >> 1) & 1) as f64 * 0.5) * h;
            let child = ctx.create_child_with(&name, Box::new(MyPanel { bg: inv }));
            ctx.layout_child(child, cx, cy, 0.3, 0.3 * h);
        }
    }
}

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let root = app.tree.create_root("root");
        app.tree
            .set_behavior(root, Box::new(MyPanel { bg: emColor::WHITE }));
        app.tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        let close_sig = app.scheduler.borrow_mut().create_signal();
        let flags_sig = app.scheduler.borrow_mut().create_signal();
        let focus_sig = app.scheduler.borrow_mut().create_signal();
        let geometry_sig = app.scheduler.borrow_mut().create_signal();
        let win = eaglemode_rs::emCore::emWindow::emWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
            flags_sig,
            focus_sig,
            geometry_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        app.windows.get_mut(&wid).unwrap().view_mut().flags |= ViewFlags::ROOT_SAME_TALLNESS;
    }));
    app.run();
}
