use std::cell::RefCell;
use std::rc::Rc;

use crate::emBoolRec::emBoolRec;
use crate::emContext::emContext;
use crate::emDoubleRec::emDoubleRec;
use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emInstallInfo::{emGetInstallPath, InstallDirType};
use crate::emIntRec::emIntRec;
use crate::emRecNode::emRecNode;
use crate::emRecNodeConfigModel::emRecNodeConfigModel;
use crate::emRecReader::{emRecReader, RecIoError};
use crate::emRecWriter::emRecWriter;
use crate::emSignal::SignalId;
use crate::emStructRec::emStructRec;

/// Toolkit-wide configuration settings.
///
/// Port of C++ `emCoreConfig` (include/emCore/emCoreConfig.h,
/// src/emCore/emCoreConfig.cpp). Holds navigation speeds, rendering quality,
/// and resource limits. Backed by an `emRecNodeConfigModel` for file persistence.
pub struct emCoreConfig {
    /// DIVERGED: C++ `emCoreConfig` inherits `emStructRec` via multiple
    /// inheritance (`class emCoreConfig : public emConfigModel, public
    /// emStructRec`). Rust has no MI; `emStructRec` is composed as a field.
    pub inner: emStructRec,
    pub StickMouseWhenNavigating: emBoolRec,
    pub EmulateMiddleButton: emBoolRec,
    pub PanFunction: emBoolRec,
    pub MouseZoomSpeed: emDoubleRec,
    pub MouseScrollSpeed: emDoubleRec,
    pub MouseWheelZoomSpeed: emDoubleRec,
    pub MouseWheelZoomAcceleration: emDoubleRec,
    pub KeyboardZoomSpeed: emDoubleRec,
    pub KeyboardScrollSpeed: emDoubleRec,
    pub KineticZoomingAndScrolling: emDoubleRec,
    pub MagnetismRadius: emDoubleRec,
    pub MagnetismSpeed: emDoubleRec,
    pub VisitSpeed: emDoubleRec,
    pub MaxMegabytesPerView: emIntRec,
    pub MaxRenderThreads: emIntRec,
    pub AllowSIMD: emBoolRec,
    pub DownscaleQuality: emIntRec,
    pub UpscaleQuality: emIntRec,
}

impl emCoreConfig {
    /// Construct a new `emCoreConfig` with all 18 fields at their C++ defaults.
    ///
    /// Mirrors C++ `emCoreConfig::emCoreConfig(emContext&, const emString&)`
    /// (emCoreConfig.cpp:38-81).
    pub fn new<C: ConstructCtx>(ctx: &mut C) -> Self {
        let mut inner = emStructRec::new(ctx);
        let mut stick_mouse = emBoolRec::new(ctx, false);
        let mut emulate_middle = emBoolRec::new(ctx, false);
        let mut pan_function = emBoolRec::new(ctx, false);
        let mut mouse_zoom = emDoubleRec::new(ctx, 1.0, 0.25, 4.0);
        let mut mouse_scroll = emDoubleRec::new(ctx, 1.0, 0.25, 4.0);
        let mut mouse_wheel_zoom = emDoubleRec::new(ctx, 1.0, 0.25, 4.0);
        let mut mouse_wheel_accel = emDoubleRec::new(ctx, 1.0, 0.25, 2.0);
        let mut keyboard_zoom = emDoubleRec::new(ctx, 1.0, 0.25, 4.0);
        let mut keyboard_scroll = emDoubleRec::new(ctx, 1.0, 0.25, 4.0);
        let mut kinetic = emDoubleRec::new(ctx, 1.0, 0.25, 2.0);
        let mut magnetism_radius = emDoubleRec::new(ctx, 1.0, 0.25, 4.0);
        let mut magnetism_speed = emDoubleRec::new(ctx, 1.0, 0.25, 4.0);
        let mut visit_speed = emDoubleRec::new(ctx, 1.0, 0.1, 10.0);
        let mut max_mb = emIntRec::new(ctx, 2048, 8, 16384);
        let mut max_threads = emIntRec::new(ctx, 8, 1, 32);
        let mut allow_simd = emBoolRec::new(ctx, true);
        let mut downscale_quality = emIntRec::new(ctx, 3, 2, 6); // DQ_3X3=3, DQ_2X2=2, DQ_6X6=6
        let mut upscale_quality = emIntRec::new(ctx, 2, 1, 5); // UQ_BILINEAR=2, UQ_AREA_SAMPLING=1, UQ_ADAPTIVE=5

        inner.AddMember(&mut stick_mouse, "StickMouseWhenNavigating");
        inner.AddMember(&mut emulate_middle, "EmulateMiddleButton");
        inner.AddMember(&mut pan_function, "PanFunction");
        inner.AddMember(&mut mouse_zoom, "MouseZoomSpeed");
        inner.AddMember(&mut mouse_scroll, "MouseScrollSpeed");
        inner.AddMember(&mut mouse_wheel_zoom, "MouseWheelZoomSpeed");
        inner.AddMember(&mut mouse_wheel_accel, "MouseWheelZoomAcceleration");
        inner.AddMember(&mut keyboard_zoom, "KeyboardZoomSpeed");
        inner.AddMember(&mut keyboard_scroll, "KeyboardScrollSpeed");
        inner.AddMember(&mut kinetic, "KineticZoomingAndScrolling");
        inner.AddMember(&mut magnetism_radius, "MagnetismRadius");
        inner.AddMember(&mut magnetism_speed, "MagnetismSpeed");
        inner.AddMember(&mut visit_speed, "VisitSpeed");
        inner.AddMember(&mut max_mb, "MaxMegabytesPerView");
        inner.AddMember(&mut max_threads, "MaxRenderThreads");
        inner.AddMember(&mut allow_simd, "AllowSIMD");
        inner.AddMember(&mut downscale_quality, "DownscaleQuality");
        inner.AddMember(&mut upscale_quality, "UpscaleQuality");

        Self {
            inner,
            StickMouseWhenNavigating: stick_mouse,
            EmulateMiddleButton: emulate_middle,
            PanFunction: pan_function,
            MouseZoomSpeed: mouse_zoom,
            MouseScrollSpeed: mouse_scroll,
            MouseWheelZoomSpeed: mouse_wheel_zoom,
            MouseWheelZoomAcceleration: mouse_wheel_accel,
            KeyboardZoomSpeed: keyboard_zoom,
            KeyboardScrollSpeed: keyboard_scroll,
            KineticZoomingAndScrolling: kinetic,
            MagnetismRadius: magnetism_radius,
            MagnetismSpeed: magnetism_speed,
            VisitSpeed: visit_speed,
            MaxMegabytesPerView: max_mb,
            MaxRenderThreads: max_threads,
            AllowSIMD: allow_simd,
            DownscaleQuality: downscale_quality,
            UpscaleQuality: upscale_quality,
        }
    }

    /// Acquire the singleton `emRecNodeConfigModel<emCoreConfig>` from the
    /// context registry.
    ///
    /// Port of C++ `emCoreConfig::Acquire` (emCoreConfig.cpp:83-90). On first
    /// call, creates the model, registers it, and loads from disk (or installs
    /// defaults).
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<emRecNodeConfigModel<Self>>> {
        use crate::emClipboard::emClipboard;
        use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction};
        use crate::emScheduler::EngineScheduler;

        let root = ctx.GetRootContext();
        root.acquire::<emRecNodeConfigModel<Self>>("", || {
            // DIVERGED: `emRecNodeConfigModel::new` requires `SchedCtx` for
            // signal allocation and listener registration. `emContext::acquire`
            // provides no context parameter to this closure.
            //
            // A private `EngineScheduler` allocates the emCoreConfig signal
            // IDs. Those IDs are orphaned (not reachable via the main framework
            // scheduler), so the auto-dirty listener registered by
            // `emRecNodeConfigModel` never fires from a scheduler tick.
            // `modify()` sets the dirty flag synchronously, so the observable
            // save/load contract is preserved. No consumer in the current port
            // observes emCoreConfig field signals directly.
            let mut priv_sched = EngineScheduler::new();
            let root_ctx = ctx.GetRootContext();
            let mut actions: Vec<DeferredAction> = Vec::new();
            let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
            let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            let mut sc = SchedCtx {
                scheduler: &mut priv_sched,
                framework_actions: &mut actions,
                root_context: &root_ctx,
                framework_clipboard: &cb,
                current_engine: None,
                pending_actions: &pa,
            };

            let path =
                emGetInstallPath(InstallDirType::UserConfig, "emCore", Some("config.rec"))
                    .unwrap_or_else(|_| {
                        let home =
                            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                        std::path::PathBuf::from(home)
                            .join(".eaglemode-rs")
                            .join("emCore")
                            .join("config.rec")
                    });

            let mut model =
                emRecNodeConfigModel::new(Self::new(&mut sc), path, &mut sc)
                    .with_format_name("emCoreConfig");
            if let Err(e) = model.TryLoadOrInstall(&mut sc) {
                log::warn!("CoreConfig: failed to load or install config: {e}");
            }
            // Detach the listener engine before the private scheduler is
            // dropped: the auto-dirty-on-mutation behavior requires the
            // scheduler to tick the listener engine, which cannot happen with
            // an orphaned private scheduler. `modify()` sets the dirty flag
            // synchronously, so the observable save/load contract is preserved.
            //
            // Abort pending signals to satisfy the scheduler's drop invariant
            // (signals fired by SetValue during TryRead must not remain pending
            // when the private scheduler is dropped).
            model.detach(&mut sc);
            sc.scheduler.abort_all_pending();
            model
        })
    }
}

impl emRecNode for emCoreConfig {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    /// Forward `register_aggregate` to the inner struct AND every sibling
    /// field record. Matches the C++ `UpperNode` chain semantics via the
    /// reified aggregate Vec (ADR 2026-04-21-phase-4b-listener-tree-adr.md).
    fn register_aggregate(&mut self, sig: SignalId) {
        self.inner.register_aggregate(sig);
        self.StickMouseWhenNavigating.register_aggregate(sig);
        self.EmulateMiddleButton.register_aggregate(sig);
        self.PanFunction.register_aggregate(sig);
        self.MouseZoomSpeed.register_aggregate(sig);
        self.MouseScrollSpeed.register_aggregate(sig);
        self.MouseWheelZoomSpeed.register_aggregate(sig);
        self.MouseWheelZoomAcceleration.register_aggregate(sig);
        self.KeyboardZoomSpeed.register_aggregate(sig);
        self.KeyboardScrollSpeed.register_aggregate(sig);
        self.KineticZoomingAndScrolling.register_aggregate(sig);
        self.MagnetismRadius.register_aggregate(sig);
        self.MagnetismSpeed.register_aggregate(sig);
        self.VisitSpeed.register_aggregate(sig);
        self.MaxMegabytesPerView.register_aggregate(sig);
        self.MaxRenderThreads.register_aggregate(sig);
        self.AllowSIMD.register_aggregate(sig);
        self.DownscaleQuality.register_aggregate(sig);
        self.UpscaleQuality.register_aggregate(sig);
    }

    fn listened_signal(&self) -> SignalId {
        self.inner.listened_signal()
    }

    fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        let members = self.inner.member_identifiers();
        emStructRec::try_read_body(&members, reader, |idx, r| match idx {
            0 => self.StickMouseWhenNavigating.TryRead(r, ctx),
            1 => self.EmulateMiddleButton.TryRead(r, ctx),
            2 => self.PanFunction.TryRead(r, ctx),
            3 => self.MouseZoomSpeed.TryRead(r, ctx),
            4 => self.MouseScrollSpeed.TryRead(r, ctx),
            5 => self.MouseWheelZoomSpeed.TryRead(r, ctx),
            6 => self.MouseWheelZoomAcceleration.TryRead(r, ctx),
            7 => self.KeyboardZoomSpeed.TryRead(r, ctx),
            8 => self.KeyboardScrollSpeed.TryRead(r, ctx),
            9 => self.KineticZoomingAndScrolling.TryRead(r, ctx),
            10 => self.MagnetismRadius.TryRead(r, ctx),
            11 => self.MagnetismSpeed.TryRead(r, ctx),
            12 => self.VisitSpeed.TryRead(r, ctx),
            13 => self.MaxMegabytesPerView.TryRead(r, ctx),
            14 => self.MaxRenderThreads.TryRead(r, ctx),
            15 => self.AllowSIMD.TryRead(r, ctx),
            16 => self.DownscaleQuality.TryRead(r, ctx),
            17 => self.UpscaleQuality.TryRead(r, ctx),
            _ => unreachable!(),
        })
    }

    fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
        let members = self.inner.member_identifiers();
        emStructRec::try_write_body(&members, writer, |_| true, |idx, w| match idx {
            0 => self.StickMouseWhenNavigating.TryWrite(w),
            1 => self.EmulateMiddleButton.TryWrite(w),
            2 => self.PanFunction.TryWrite(w),
            3 => self.MouseZoomSpeed.TryWrite(w),
            4 => self.MouseScrollSpeed.TryWrite(w),
            5 => self.MouseWheelZoomSpeed.TryWrite(w),
            6 => self.MouseWheelZoomAcceleration.TryWrite(w),
            7 => self.KeyboardZoomSpeed.TryWrite(w),
            8 => self.KeyboardScrollSpeed.TryWrite(w),
            9 => self.KineticZoomingAndScrolling.TryWrite(w),
            10 => self.MagnetismRadius.TryWrite(w),
            11 => self.MagnetismSpeed.TryWrite(w),
            12 => self.VisitSpeed.TryWrite(w),
            13 => self.MaxMegabytesPerView.TryWrite(w),
            14 => self.MaxRenderThreads.TryWrite(w),
            15 => self.AllowSIMD.TryWrite(w),
            16 => self.DownscaleQuality.TryWrite(w),
            17 => self.UpscaleQuality.TryWrite(w),
            _ => unreachable!(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emClipboard::emClipboard;
    use crate::emContext::emContext;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction};
    use crate::emRec::emRec;
    use crate::emScheduler::EngineScheduler;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_sched_ctx<'a>(
        sched: &'a mut EngineScheduler,
        actions: &'a mut Vec<DeferredAction>,
        ctx_root: &'a Rc<emContext>,
        cb: &'a RefCell<Option<Box<dyn emClipboard>>>,
        pa: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
    ) -> SchedCtx<'a> {
        SchedCtx {
            scheduler: sched,
            framework_actions: actions,
            root_context: ctx_root,
            framework_clipboard: cb,
            current_engine: None,
            pending_actions: pa,
        }
    }

    #[test]
    fn core_config_has_emrec_fields() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let config = emCoreConfig::new(&mut sc);
        // type-check: confirm field types at compile time
        let _: &emDoubleRec = &config.VisitSpeed;
        let _: &emBoolRec = &config.StickMouseWhenNavigating;
        let _: &emIntRec = &config.MaxMegabytesPerView;
    }

    #[test]
    fn defaults_match_cpp() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let config = emCoreConfig::new(&mut sc);
        assert!(!*config.StickMouseWhenNavigating.GetValue());
        assert!(!*config.EmulateMiddleButton.GetValue());
        assert!(!*config.PanFunction.GetValue());
        assert_eq!(*config.MouseZoomSpeed.GetValue(), 1.0);
        assert_eq!(*config.MouseScrollSpeed.GetValue(), 1.0);
        assert_eq!(*config.MouseWheelZoomSpeed.GetValue(), 1.0);
        assert_eq!(*config.MouseWheelZoomAcceleration.GetValue(), 1.0);
        assert_eq!(*config.KeyboardZoomSpeed.GetValue(), 1.0);
        assert_eq!(*config.KeyboardScrollSpeed.GetValue(), 1.0);
        assert_eq!(*config.KineticZoomingAndScrolling.GetValue(), 1.0);
        assert_eq!(*config.MagnetismRadius.GetValue(), 1.0);
        assert_eq!(*config.MagnetismSpeed.GetValue(), 1.0);
        assert_eq!(*config.VisitSpeed.GetValue(), 1.0);
        assert_eq!(*config.MaxMegabytesPerView.GetValue(), 2048);
        assert_eq!(*config.MaxRenderThreads.GetValue(), 8);
        assert!(*config.AllowSIMD.GetValue());
        assert_eq!(*config.DownscaleQuality.GetValue(), 3); // DQ_3X3
        assert_eq!(*config.UpscaleQuality.GetValue(), 2); // UQ_BILINEAR
    }

    #[test]
    fn visit_speed_has_correct_bounds() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let config = emCoreConfig::new(&mut sc);
        assert_eq!(*config.VisitSpeed.GetMaxValue().unwrap(), 10.0);
        assert_eq!(*config.VisitSpeed.GetMinValue().unwrap(), 0.1);
    }

    #[test]
    fn member_count_matches_cpp() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let config = emCoreConfig::new(&mut sc);
        assert_eq!(config.inner.GetCount(), 18);
        assert_eq!(config.inner.GetIdentifierOf(0), Some("StickMouseWhenNavigating"));
        assert_eq!(config.inner.GetIdentifierOf(17), Some("UpscaleQuality"));
    }
}
