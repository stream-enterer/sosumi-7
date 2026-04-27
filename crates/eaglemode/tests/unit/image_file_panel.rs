use emcore::emImage::emImage;
use emcore::emImageFileImageFilePanel::emImageFilePanel;

const EPS: f64 = 1e-9;

fn assert_approx(actual: (f64, f64, f64, f64), expected: (f64, f64, f64, f64)) {
    assert!(
        (actual.0 - expected.0).abs() < EPS
            && (actual.1 - expected.1).abs() < EPS
            && (actual.2 - expected.2).abs() < EPS
            && (actual.3 - expected.3).abs() < EPS,
        "expected {:?}, got {:?}",
        expected,
        actual
    );
}

#[test]
fn essence_rect_landscape() {
    let mut panel = emImageFilePanel::new();
    panel.set_current_image(Some(emImage::new(200, 100, 3)));
    let rect = panel
        .GetEssenceRect(400.0, 400.0)
        .expect("should return rect");
    assert_approx(rect, (0.0, 100.0, 400.0, 200.0));
}

#[test]
fn essence_rect_portrait() {
    let mut panel = emImageFilePanel::new();
    panel.set_current_image(Some(emImage::new(100, 200, 3)));
    let rect = panel
        .GetEssenceRect(400.0, 400.0)
        .expect("should return rect");
    assert_approx(rect, (100.0, 0.0, 200.0, 400.0));
}

#[test]
fn essence_rect_no_image() {
    let panel = emImageFilePanel::new();
    assert!(panel.GetEssenceRect(400.0, 400.0).is_none());
}

#[test]
fn file_panel_delegation() {
    use emcore::emFileModel::{emFileModel, FileModelState};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    let model: Rc<RefCell<emFileModel<String>>> = Rc::new(RefCell::new(emFileModel::new(
        PathBuf::from("/tmp/test"),
        Default::default(),
    )));
    model.borrow_mut().complete_load("data".to_string());

    let mut panel = emImageFilePanel::new();
    panel
        .file_panel_mut()
        .SetFileModel(Some(model as Rc<RefCell<dyn FileModelState>>));
    panel.file_panel_mut().refresh_vir_file_state();
    assert!(panel.file_panel().GetVirFileState().is_good());
}
