use std::cell::RefCell;
use std::rc::Rc;

use crate::emConfigModel::emConfigModel;
use crate::emContext::emContext;
use crate::emInstallInfo::{emGetInstallPath, InstallDirType};
use crate::emRec::{RecError, RecStruct};
use crate::emRecRecord::Record;
use slotmap::Key as _;

use crate::emSignal::SignalId;

/// Toolkit-wide configuration settings.
///
/// Port of C++ `emCoreConfig`. Holds navigation speeds, rendering quality,
/// and resource limits. Backed by a `emConfigModel` for file persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct emCoreConfig {
    pub stick_mouse_when_navigating: bool,
    pub emulate_middle_button: bool,
    pub pan_function: bool,
    pub mouse_zoom_speed: f64,
    pub mouse_scroll_speed: f64,
    pub mouse_wheel_zoom_speed: f64,
    pub mouse_wheel_zoom_acceleration: f64,
    pub keyboard_zoom_speed: f64,
    pub keyboard_scroll_speed: f64,
    pub kinetic_zooming_and_scrolling: f64,
    pub magnetism_radius: f64,
    pub magnetism_speed: f64,
    pub visit_speed: f64,
    pub max_megabytes_per_view: i32,
    pub max_render_threads: i32,
    pub allow_simd: bool,
    pub downscale_quality: i32,
    pub upscale_quality: i32,
}

impl Default for emCoreConfig {
    fn default() -> Self {
        Self {
            stick_mouse_when_navigating: false,
            emulate_middle_button: false,
            pan_function: false,
            mouse_zoom_speed: 1.0,
            mouse_scroll_speed: 1.0,
            mouse_wheel_zoom_speed: 1.0,
            mouse_wheel_zoom_acceleration: 1.0,
            keyboard_zoom_speed: 1.0,
            keyboard_scroll_speed: 1.0,
            kinetic_zooming_and_scrolling: 1.0,
            magnetism_radius: 1.0,
            magnetism_speed: 1.0,
            visit_speed: 1.0,
            max_megabytes_per_view: 2048,
            max_render_threads: 8,
            allow_simd: true,
            // DQ_3X3 = 3
            downscale_quality: 3,
            // UQ_BILINEAR = 2
            upscale_quality: 2,
        }
    }
}

/// Clamp a value to [min, max].
fn clamp_f64(val: f64, min: f64, max: f64) -> f64 {
    val.clamp(min, max)
}

fn clamp_i32(val: i32, min: i32, max: i32) -> i32 {
    val.clamp(min, max)
}

impl Record for emCoreConfig {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let d = Self::default();
        Ok(Self {
            stick_mouse_when_navigating: rec
                .get_bool("StickMouseWhenNavigating")
                .unwrap_or(d.stick_mouse_when_navigating),
            emulate_middle_button: rec
                .get_bool("EmulateMiddleButton")
                .unwrap_or(d.emulate_middle_button),
            pan_function: rec.get_bool("PanFunction").unwrap_or(d.pan_function),
            mouse_zoom_speed: clamp_f64(
                rec.get_double("MouseZoomSpeed")
                    .unwrap_or(d.mouse_zoom_speed),
                0.25,
                4.0,
            ),
            mouse_scroll_speed: clamp_f64(
                rec.get_double("MouseScrollSpeed")
                    .unwrap_or(d.mouse_scroll_speed),
                0.25,
                4.0,
            ),
            mouse_wheel_zoom_speed: clamp_f64(
                rec.get_double("MouseWheelZoomSpeed")
                    .unwrap_or(d.mouse_wheel_zoom_speed),
                0.25,
                4.0,
            ),
            mouse_wheel_zoom_acceleration: clamp_f64(
                rec.get_double("MouseWheelZoomAcceleration")
                    .unwrap_or(d.mouse_wheel_zoom_acceleration),
                0.25,
                2.0,
            ),
            keyboard_zoom_speed: clamp_f64(
                rec.get_double("KeyboardZoomSpeed")
                    .unwrap_or(d.keyboard_zoom_speed),
                0.25,
                4.0,
            ),
            keyboard_scroll_speed: clamp_f64(
                rec.get_double("KeyboardScrollSpeed")
                    .unwrap_or(d.keyboard_scroll_speed),
                0.25,
                4.0,
            ),
            kinetic_zooming_and_scrolling: clamp_f64(
                rec.get_double("KineticZoomingAndScrolling")
                    .unwrap_or(d.kinetic_zooming_and_scrolling),
                0.25,
                2.0,
            ),
            magnetism_radius: clamp_f64(
                rec.get_double("MagnetismRadius")
                    .unwrap_or(d.magnetism_radius),
                0.25,
                4.0,
            ),
            magnetism_speed: clamp_f64(
                rec.get_double("MagnetismSpeed")
                    .unwrap_or(d.magnetism_speed),
                0.25,
                4.0,
            ),
            visit_speed: clamp_f64(
                rec.get_double("VisitSpeed").unwrap_or(d.visit_speed),
                0.1,
                10.0,
            ),
            max_megabytes_per_view: clamp_i32(
                rec.get_int("MaxMegabytesPerView")
                    .unwrap_or(d.max_megabytes_per_view),
                8,
                16384,
            ),
            max_render_threads: clamp_i32(
                rec.get_int("MaxRenderThreads")
                    .unwrap_or(d.max_render_threads),
                1,
                32,
            ),
            allow_simd: rec.get_bool("AllowSIMD").unwrap_or(d.allow_simd),
            downscale_quality: clamp_i32(
                rec.get_int("DownscaleQuality")
                    .unwrap_or(d.downscale_quality),
                2, // DQ_2X2
                6, // DQ_6X6
            ),
            upscale_quality: clamp_i32(
                rec.get_int("UpscaleQuality").unwrap_or(d.upscale_quality),
                1, // UQ_AREA_SAMPLING
                5, // UQ_ADAPTIVE
            ),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_bool("StickMouseWhenNavigating", self.stick_mouse_when_navigating);
        s.set_bool("EmulateMiddleButton", self.emulate_middle_button);
        s.set_bool("PanFunction", self.pan_function);
        s.set_double("MouseZoomSpeed", self.mouse_zoom_speed);
        s.set_double("MouseScrollSpeed", self.mouse_scroll_speed);
        s.set_double("MouseWheelZoomSpeed", self.mouse_wheel_zoom_speed);
        s.set_double(
            "MouseWheelZoomAcceleration",
            self.mouse_wheel_zoom_acceleration,
        );
        s.set_double("KeyboardZoomSpeed", self.keyboard_zoom_speed);
        s.set_double("KeyboardScrollSpeed", self.keyboard_scroll_speed);
        s.set_double(
            "KineticZoomingAndScrolling",
            self.kinetic_zooming_and_scrolling,
        );
        s.set_double("MagnetismRadius", self.magnetism_radius);
        s.set_double("MagnetismSpeed", self.magnetism_speed);
        s.set_double("VisitSpeed", self.visit_speed);
        s.set_int("MaxMegabytesPerView", self.max_megabytes_per_view);
        s.set_int("MaxRenderThreads", self.max_render_threads);
        s.set_bool("AllowSIMD", self.allow_simd);
        s.set_int("DownscaleQuality", self.downscale_quality);
        s.set_int("UpscaleQuality", self.upscale_quality);
        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

impl emCoreConfig {
    /// Acquire the singleton `emConfigModel<emCoreConfig>` from the context registry.
    ///
    /// Port of C++ `emCoreConfig::Acquire`. On first call, creates the model,
    /// registers it, and loads from disk (or installs defaults).
    pub fn Acquire(ctx: &emContext) -> Rc<RefCell<emConfigModel<Self>>> {
        ctx.acquire::<emConfigModel<Self>>("", || {
            let path = emGetInstallPath(InstallDirType::UserConfig, "emCore", Some("config.rec"))
                .unwrap_or_else(|_| {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                    std::path::PathBuf::from(home)
                        .join(".eaglemode-rs")
                        .join("emCore")
                        .join("config.rec")
                });

            let mut model = emConfigModel::new(Self::default(), path, SignalId::null());

            if let Err(e) = model.TryLoadOrInstall() {
                log::warn!("CoreConfig: failed to load or install config: {e}");
            }

            model
        })
    }

    /// Upper bound for `VisitSpeed`.
    ///
    /// DIVERGED: C++ exposes this via `VisitSpeed.GetMaxValue()` on an
    /// `emRec`-typed field (`emDoubleRec VisitSpeed;` in emCoreConfig.h:51;
    /// bound declared by `VisitSpeed(this,"VisitSpeed",1.0,0.1,10.0)` in
    /// emCoreConfig.cpp:53). Rust flattens to a plain `f64` field + const
    /// because the `emRec`-backed scalar-field infrastructure is not ported.
    pub const VISIT_SPEED_MAX: f64 = 10.0;

    /// C++ `emCoreConfig::VisitSpeed.GetMaxValue()`.
    ///
    /// DIVERGED: flattened from member-of-emRec-field to method-on-config,
    /// see `VISIT_SPEED_MAX` docs.
    pub fn VisitSpeed_GetMaxValue(&self) -> f64 {
        Self::VISIT_SPEED_MAX
    }
}

#[cfg(test)]
mod sp3_tests {
    use super::*;

    #[test]
    fn visit_speed_max_matches_cpp_schema() {
        assert_eq!(emCoreConfig::VISIT_SPEED_MAX, 10.0);
    }

    #[test]
    fn visit_speed_getmaxvalue_returns_const() {
        let cfg = emCoreConfig::default();
        assert_eq!(cfg.VisitSpeed_GetMaxValue(), emCoreConfig::VISIT_SPEED_MAX);
    }
}

#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_emCoreConfig_IsSetToDefault() {
        let mut self_val = emCoreConfig {
            stick_mouse_when_navigating: kani::any::<bool>(),
            emulate_middle_button: kani::any::<bool>(),
            pan_function: kani::any::<bool>(),
            mouse_zoom_speed: kani::any::<f64>(),
            mouse_scroll_speed: kani::any::<f64>(),
            mouse_wheel_zoom_speed: kani::any::<f64>(),
            mouse_wheel_zoom_acceleration: kani::any::<f64>(),
            keyboard_zoom_speed: kani::any::<f64>(),
            keyboard_scroll_speed: kani::any::<f64>(),
            kinetic_zooming_and_scrolling: kani::any::<f64>(),
            magnetism_radius: kani::any::<f64>(),
            magnetism_speed: kani::any::<f64>(),
            visit_speed: kani::any::<f64>(),
            max_megabytes_per_view: kani::any::<i32>(),
            max_render_threads: kani::any::<i32>(),
            allow_simd: kani::any::<bool>(),
            downscale_quality: kani::any::<i32>(),
            upscale_quality: kani::any::<i32>(),
        };
        let _r = self_val.IsSetToDefault();
    }

    #[kani::proof]
    fn kani_private_emCoreConfig_SetToDefault() {
        let mut self_val = emCoreConfig {
            stick_mouse_when_navigating: kani::any::<bool>(),
            emulate_middle_button: kani::any::<bool>(),
            pan_function: kani::any::<bool>(),
            mouse_zoom_speed: kani::any::<f64>(),
            mouse_scroll_speed: kani::any::<f64>(),
            mouse_wheel_zoom_speed: kani::any::<f64>(),
            mouse_wheel_zoom_acceleration: kani::any::<f64>(),
            keyboard_zoom_speed: kani::any::<f64>(),
            keyboard_scroll_speed: kani::any::<f64>(),
            kinetic_zooming_and_scrolling: kani::any::<f64>(),
            magnetism_radius: kani::any::<f64>(),
            magnetism_speed: kani::any::<f64>(),
            visit_speed: kani::any::<f64>(),
            max_megabytes_per_view: kani::any::<i32>(),
            max_render_threads: kani::any::<i32>(),
            allow_simd: kani::any::<bool>(),
            downscale_quality: kani::any::<i32>(),
            upscale_quality: kani::any::<i32>(),
        };
        let _r = self_val.SetToDefault();
    }

    #[kani::proof]
    fn kani_private_clamp_f64() {
        let mut p_val: f64 = kani::any::<f64>();
        kani::assume(p_val.is_finite());
        let mut p_min: f64 = kani::any::<f64>();
        kani::assume(p_min.is_finite());
        let mut p_max: f64 = kani::any::<f64>();
        kani::assume(p_max.is_finite());
        let _r = clamp_f64(p_val, p_min, p_max);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_clamp_i32() {
        let mut p_val: i32 = kani::any::<i32>();
        let mut p_min: i32 = kani::any::<i32>();
        let mut p_max: i32 = kani::any::<i32>();
        let _r = clamp_i32(p_val, p_min, p_max);
    }
}
