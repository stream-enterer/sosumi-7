use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emFileModel::FileState;
use emcore::emImage::emImage;
use emcore::emImageFile::{emImageFileModel, ImageFileData};
use emcore::emImageFileImageFilePanel::emImageFilePanel;
use emcore::emScheduler::EngineScheduler;
use winit::window::WindowId;

fn make_model() -> emImageFileModel {
    let mut sched = EngineScheduler::new();
    let data_change = sched.create_signal();
    emImageFileModel::new(PathBuf::from("test.png"), data_change)
}

#[test]
fn initial_state_is_waiting() {
    let m = make_model();
    assert!(matches!(m.state(), &FileState::Waiting));
}

#[test]
fn no_data_initially() {
    let m = make_model();
    assert!(m.GetImage().is_none());
    assert!(m.GetComment().is_none());
    assert!(m.GetFileFormatInfo().is_none());
}

#[test]
fn saving_quality_default_100() {
    let m = make_model();
    assert_eq!(m.GetSavingQuality(), 100);
}

#[test]
fn set_saving_quality() {
    let mut m = make_model();
    m.set_saving_quality(75);
    assert_eq!(m.GetSavingQuality(), 75);
}

#[test]
fn set_saving_quality_clamped() {
    let mut m = make_model();
    m.set_saving_quality(200);
    assert_eq!(m.GetSavingQuality(), 100);
}

#[test]
fn set_image_changes_data() {
    let mut m = make_model();
    let data = ImageFileData::default();
    m.file_model_mut().complete_load(data);
    assert!(matches!(m.state(), &FileState::Loaded));

    let img = emImage::new(10, 10, 4);
    let changed = m.set_image(img);
    assert!(changed);
    assert!(matches!(m.state(), &FileState::Unsaved));
}

#[test]
fn set_image_same_value_no_change() {
    let mut m = make_model();
    let data = ImageFileData {
        image: emImage::new(10, 10, 4),
        comment: String::new(),
        format_info: String::new(),
    };
    m.file_model_mut().complete_load(data);

    let same_img = emImage::new(10, 10, 4);
    let changed = m.set_image(same_img);
    assert!(!changed);
    assert!(matches!(m.state(), &FileState::Loaded));
}

#[test]
fn set_comment_changes_data() {
    let mut m = make_model();
    m.file_model_mut().complete_load(ImageFileData::default());

    let changed = m.set_comment("hello".to_string());
    assert!(changed);
    assert_eq!(m.GetComment(), Some("hello"));
    assert!(matches!(m.state(), &FileState::Unsaved));
}

#[test]
fn set_comment_same_value_no_change() {
    let mut m = make_model();
    let data = ImageFileData {
        image: emImage::new(0, 0, 4),
        comment: "hello".to_string(),
        format_info: String::new(),
    };
    m.file_model_mut().complete_load(data);

    let changed = m.set_comment("hello".to_string());
    assert!(!changed);
    assert!(matches!(m.state(), &FileState::Loaded));
}

#[test]
fn set_format_info_changes_data() {
    let mut m = make_model();
    m.file_model_mut().complete_load(ImageFileData::default());

    let changed = m.SetFileFormatInfo("PNG 8-bit".to_string());
    assert!(changed);
    assert_eq!(m.GetFileFormatInfo(), Some("PNG 8-bit"));
}

#[test]
fn set_format_info_same_value_no_change() {
    let mut m = make_model();
    let data = ImageFileData {
        image: emImage::new(0, 0, 4),
        comment: String::new(),
        format_info: "PNG".to_string(),
    };
    m.file_model_mut().complete_load(data);

    let changed = m.SetFileFormatInfo("PNG".to_string());
    assert!(!changed);
}

#[test]
fn reset_data_clears() {
    let mut m = make_model();
    m.file_model_mut().complete_load(ImageFileData::default());
    assert!(matches!(m.state(), &FileState::Loaded));

    m.reset_data();
    assert!(matches!(m.state(), &FileState::Waiting));
    assert!(m.GetImage().is_none());
}

#[test]
fn set_on_no_data_returns_false() {
    let mut m = make_model();
    assert!(!m.set_image(emImage::new(5, 5, 4)));
    assert!(!m.set_comment("test".to_string()));
    assert!(!m.SetFileFormatInfo("test".to_string()));
}

// ── emImageFilePanel tests ─────────────────────────────────────────

#[test]
fn essence_rect_no_image_returns_none() {
    let panel = emImageFilePanel::new();
    assert!(panel.GetEssenceRect(100.0, 100.0).is_none());
}

#[test]
fn essence_rect_square_image_in_square_panel() {
    let mut panel = emImageFilePanel::new();
    panel.set_current_image(Some(emImage::new(100, 100, 4)));

    let (x, y, w, h) = panel.GetEssenceRect(200.0, 200.0).unwrap();
    assert!((x - 0.0).abs() < 1e-10);
    assert!((y - 0.0).abs() < 1e-10);
    assert!((w - 200.0).abs() < 1e-10);
    assert!((h - 200.0).abs() < 1e-10);
}

#[test]
fn essence_rect_landscape_image_in_square_panel() {
    let mut panel = emImageFilePanel::new();
    panel.set_current_image(Some(emImage::new(200, 100, 4)));

    let (x, y, w, h) = panel.GetEssenceRect(200.0, 200.0).unwrap();
    // Landscape GetImage fits width, centered vertically
    assert!((w - 200.0).abs() < 1e-10);
    assert!((h - 100.0).abs() < 1e-10);
    assert!((x - 0.0).abs() < 1e-10);
    assert!((y - 50.0).abs() < 1e-10);
}

#[test]
fn essence_rect_portrait_image_in_square_panel() {
    let mut panel = emImageFilePanel::new();
    panel.set_current_image(Some(emImage::new(100, 200, 4)));

    let (x, y, w, h) = panel.GetEssenceRect(200.0, 200.0).unwrap();
    // Portrait GetImage fits height, centered horizontally
    assert!((h - 200.0).abs() < 1e-10);
    assert!((w - 100.0).abs() < 1e-10);
    assert!((x - 50.0).abs() < 1e-10);
    assert!((y - 0.0).abs() < 1e-10);
}

#[test]
fn essence_rect_wide_panel() {
    let mut panel = emImageFilePanel::new();
    panel.set_current_image(Some(emImage::new(100, 100, 4)));

    let (x, y, w, h) = panel.GetEssenceRect(400.0, 200.0).unwrap();
    // Square GetImage in wide panel: fits height
    assert!((h - 200.0).abs() < 1e-10);
    assert!((w - 200.0).abs() < 1e-10);
    assert!((x - 100.0).abs() < 1e-10);
    assert!((y - 0.0).abs() < 1e-10);
}

#[test]
fn essence_rect_zero_dim_image() {
    let mut panel = emImageFilePanel::new();
    panel.set_current_image(Some(emImage::new(0, 0, 4)));
    assert!(panel.GetEssenceRect(100.0, 100.0).is_none());
}

// ── async loading test ─────────────────────────────────────────────

/// Build a minimal 1×1 Type-10 (RLE true-colour 32 bpp) TGA.
fn make_test_tga() -> Vec<u8> {
    let mut data = vec![0u8; 18];
    data[2] = 10; // image type: RLE true-color
    data[12] = 1; // width low byte
    data[13] = 0; // width high byte
    data[14] = 1; // height low byte
    data[15] = 0; // height high byte
    data[16] = 32; // bits per pixel
                   // RLE packet: 1 pixel (header 0x80 = RLE, count=1)
    data.push(0x80);
    // BGRA pixel
    data.extend_from_slice(&[0x10, 0x20, 0x30, 0xFF]);
    data
}

/// Run one scheduler time-slice (mirrors the `slice()` helper in
/// pri_sched_agent.rs).
fn do_slice(sched: &mut EngineScheduler) {
    let mut windows: HashMap<WindowId, emcore::emWindow::emWindow> = HashMap::new();
    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let mut pending_inputs: Vec<(WindowId, emcore::emInput::emInputEvent)> = Vec::new();
    let mut input_state = emcore::emInputState::emInputState::new();
    let cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));
    sched.DoTimeSlice(
        &mut windows,
        &root_ctx,
        &mut fw,
        &mut pending_inputs,
        &mut input_state,
        &cb,
        &pa,
    );
}

#[test]
fn image_model_loads_asynchronously_via_engine() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("async_test.tga");
    std::fs::write(&path, make_test_tga()).expect("write tga");

    let mut sched = EngineScheduler::new();
    let file_update_signal = sched.create_signal();
    sched.file_update_signal = file_update_signal;
    let model = {
        // InitCtx borrows &mut sched, so it must be dropped before do_slice.
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut ctx = emcore::emEngineCtx::InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            view_context: None,
            pending_actions: &pa,
        };
        emImageFileModel::register(&mut ctx, path)
    };

    assert!(
        matches!(model.borrow().state(), FileState::Loading { .. }),
        "expected Loading after register(), got {:?}",
        model.borrow().state()
    );

    do_slice(&mut sched);

    assert!(
        matches!(model.borrow().state(), &FileState::Loaded),
        "expected Loaded after one slice, got {:?}",
        model.borrow().state()
    );
    assert!(
        model.borrow().GetImage().is_some(),
        "GetImage() should be Some after load"
    );

    // Cleanup: drop model and wake engine so it detects dead model_weak and removes itself.
    drop(model);
    sched.fire(file_update_signal);
    do_slice(&mut sched);
}

#[test]
fn image_model_fails_for_nonexistent_path() {
    let mut sched = EngineScheduler::new();
    let file_update_signal = sched.create_signal();
    sched.file_update_signal = file_update_signal;
    let model = {
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut ctx = emcore::emEngineCtx::InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw_actions,
            root_context: &root_ctx,
            view_context: None,
            pending_actions: &pa,
        };
        emImageFileModel::register(&mut ctx, std::path::PathBuf::from("/nonexistent/no.tga"))
    };

    assert!(
        matches!(model.borrow().state(), FileState::Loading { .. }),
        "expected Loading before slice"
    );

    do_slice(&mut sched);

    assert!(
        matches!(model.borrow().state(), FileState::LoadError(_)),
        "expected LoadError after slice for nonexistent path, got {:?}",
        model.borrow().state()
    );

    // Cleanup: drop model and wake engine so it detects dead model_weak and removes itself.
    drop(model);
    sched.fire(file_update_signal);
    do_slice(&mut sched);
}
