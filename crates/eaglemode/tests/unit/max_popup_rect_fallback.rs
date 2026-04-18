//! Phase 9 acceptance test: when no monitor info has been seeded via
//! `set_max_popup_rect`, `GetMaxPopupViewRect` falls back to the view's
//! home rect (HomeX, HomeY, HomeWidth, HomeHeight). This is the
//! headless/Wayland-without-position-queries path.

use emcore::emPanelTree::PanelTree;
use emcore::emView::emView;

#[test]
fn max_popup_rect_falls_back_to_home() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");

    // Construct emView without calling set_max_popup_rect.
    let view = emView::new(root, 800.0, 600.0);

    // Sanity: the popup rect is unset.
    assert!(view.max_popup_rect().is_none());

    // GetMaxPopupViewRect should return the home rect.
    let mut out = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    view.GetMaxPopupViewRect(&mut out);

    assert_eq!(
        out,
        (view.HomeX, view.HomeY, view.HomeWidth, view.HomeHeight),
        "fallback should equal the view's home rect"
    );
}
