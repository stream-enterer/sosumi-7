use std::any::Any;
use std::time::Instant;

use crate::dlog;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;

use super::emView::{emView, ViewFlags};
use crate::emPanelTree::PanelTree;

/// Trait for view input filters that intercept input before it reaches panels.
pub trait emViewInputFilter {
    /// emProcess an input event. Returns true if the event was consumed.
    fn filter(
        &mut self,
        event: &emInputEvent,
        state: &emInputState,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool;

    /// Tick per-frame animations (wheel zoom spring, grip pan spring).
    /// Returns true if animation is still active and needs another frame.
    fn animate(
        &mut self,
        _view: &mut emView,
        _tree: &mut super::emPanelTree::PanelTree,
        _dt: f64,
        _ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        false
    }

    /// Downcast support for concrete VIF access.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Mouse wheel zoom and middle-button pan filter.
pub struct emMouseZoomScrollVIF {
    /// Zoom speed multiplier.
    pub zoom_speed: f64,
    /// Whether middle-button panning is active.
    panning: bool,
    last_x: f64,
    last_y: f64,
    /// Whether Alt-click middle-button emulation is enabled.
    emulate_middle_button: bool,
    /// Timestamp (ms) of the last emulated middle-button press.
    emu_mid_button_time: u64,
    /// Repeat counter for emulated middle-button double/triple click.
    emu_mid_button_repeat: u32,
    /// Current wheel zoom speed (accumulated with acceleration).
    wheel_zoom_speed: f64,
    /// Timestamp (ms) of the last wheel zoom event.
    wheel_zoom_time: u64,
    /// Spring constant for the mouse swiping animator.
    mouse_spring_const: f64,
    /// Friction for the mouse swiping animator.
    mouse_friction: f64,
    /// Whether kinetic mouse behavior is enabled.
    mouse_friction_enabled: bool,
    /// Stored kinetic config for mouse (used by refresh).
    mouse_kinetic_factor: f64,
    mouse_min_kinetic: f64,
    /// Spring constant for the wheel swiping animator.
    wheel_spring_const: f64,
    /// Friction for the wheel swiping animator.
    wheel_friction: f64,
    /// Whether kinetic wheel behavior is enabled.
    wheel_friction_enabled: bool,
    /// Stored kinetic config for wheel (used by refresh).
    wheel_kinetic_factor: f64,
    wheel_min_kinetic: f64,
    /// Output velocity from spring (pixels/second). Drives scroll during grip
    /// and kinetic coasting after release.
    grip_velocity_x: f64,
    grip_velocity_y: f64,
    /// Spring extension accumulator (pixels). Mouse drag adds to this; the
    /// spring physics in `animate_grip` decays it and converts it to velocity.
    grip_spring_x: f64,
    grip_spring_y: f64,
    /// Internal spring velocity used by the critically-damped spring solver
    /// during the gripped phase. Separate from grip_velocity which is the
    /// output velocity that drives scrolling.
    grip_inst_vel_x: f64,
    grip_inst_vel_y: f64,
    /// Z-axis spring extension (zoom). Mouse Ctrl+drag adds to this; the
    /// spring physics in `animate_grip` decays it and converts it to velocity.
    /// Mirrors C++ `MoveGrip(2, ...)` on the swiping animator.
    grip_spring_z: f64,
    /// Output zoom velocity from grip z-axis spring.
    grip_velocity_z: f64,
    /// Internal spring velocity for grip z-axis.
    grip_inst_vel_z: f64,
    /// Whether the grip animation is active (grip or coast phase).
    grip_active: bool,
    /// Zoom fix point for grip-drag operations.
    grip_fix_x: f64,
    grip_fix_y: f64,
    /// Whether wheel zoom spring is active.
    wheel_active: bool,
    /// Wheel spring extension on z-axis (log-zoom units).
    wheel_spring_z: f64,
    /// Internal spring velocity for z-axis.
    wheel_inst_vel_z: f64,
    /// Output zoom velocity from wheel spring.
    wheel_velocity_z: f64,
    /// Zoom fix point for wheel operations.
    wheel_fix_x: f64,
    wheel_fix_y: f64,
    /// Whether wheel animation is in coasting phase (friction-based decay
    /// after VIF stop condition triggers: vel < 10, ext < 0.5).
    /// In C++, this transition activates the magnetic animator which coasts
    /// with the transferred velocity. In Rust, we replicate the coast
    /// directly.
    wheel_coasting: bool,
    /// C++ `emCoreConfig->PanFunction` — when true, scroll direction is reversed
    /// and the 6x speed multiplier is removed. Toggled via emCheatVIF `:pan!`.
    pan_function: bool,
    /// Monotonic clock reference for wheel zoom timestamps.
    clock_start: Instant,
    /// Override clock for deterministic testing (ms). When set, `filter()` uses
    /// this value instead of `clock_start.elapsed()`.
    test_clock_ms: Option<u64>,
    /// Whether the mouse was held still long enough to trigger magnetism on release.
    /// C++ `MagnetismAvoidance`: when true, the magnetic animator is NOT activated.
    magnetism_avoidance: bool,
    /// Accumulated mouse movement since last reset (C++ `MagAvMouseMoveX/Y`).
    mag_av_mouse_move_x: f64,
    mag_av_mouse_move_y: f64,
    /// Timestamp (ms) when cumulative mouse movement was last reset (C++ `MagAvTime`).
    mag_av_time: u64,
    /// C++ `emCoreConfig->StickMouseWhenNavigating` — when true, warp cursor back during drag.
    stick_mouse: bool,
    /// Accumulated mouse warp delta (pixels) to be drained by the window.
    pending_warp: (f64, f64),
}

impl emMouseZoomScrollVIF {
    pub fn new() -> Self {
        Self {
            zoom_speed: 1.1,
            panning: false,
            last_x: 0.0,
            last_y: 0.0,
            emulate_middle_button: false,
            emu_mid_button_time: 0,
            emu_mid_button_repeat: 0,
            wheel_zoom_speed: 0.0,
            wheel_zoom_time: 0,
            mouse_spring_const: 0.0,
            mouse_friction: 0.0,
            mouse_friction_enabled: false,
            mouse_kinetic_factor: 0.0,
            mouse_min_kinetic: 0.0,
            wheel_spring_const: 0.0,
            wheel_friction: 0.0,
            wheel_friction_enabled: false,
            wheel_kinetic_factor: 0.0,
            wheel_min_kinetic: 0.0,
            grip_velocity_x: 0.0,
            grip_velocity_y: 0.0,
            grip_spring_x: 0.0,
            grip_spring_y: 0.0,
            grip_inst_vel_x: 0.0,
            grip_inst_vel_y: 0.0,
            grip_spring_z: 0.0,
            grip_velocity_z: 0.0,
            grip_inst_vel_z: 0.0,
            grip_active: false,
            grip_fix_x: 0.0,
            grip_fix_y: 0.0,
            wheel_active: false,
            wheel_spring_z: 0.0,
            wheel_inst_vel_z: 0.0,
            wheel_velocity_z: 0.0,
            wheel_fix_x: 0.0,
            wheel_fix_y: 0.0,
            wheel_coasting: false,
            pan_function: false,
            clock_start: Instant::now(),
            test_clock_ms: None,
            magnetism_avoidance: false,
            mag_av_mouse_move_x: 0.0,
            mag_av_mouse_move_y: 0.0,
            mag_av_time: 0,
            stick_mouse: false,
            pending_warp: (0.0, 0.0),
        }
    }

    /// Enable or disable Alt-click middle-button emulation.
    pub fn set_emulate_middle_button(&mut self, enabled: bool) {
        self.emulate_middle_button = enabled;
    }

    /// Returns whether middle-button emulation is enabled.
    pub fn emulate_middle_button(&self) -> bool {
        self.emulate_middle_button
    }

    /// C++ `GetMouseScrollSpeed`. Returns the speed factor for mouse-drag
    /// scrolling. When `pan_function` is true the direction is reversed and the
    /// 6x multiplier is removed; when false, the base speed is multiplied by 6.
    /// `fine` (Shift held) scales by 0.1.
    fn get_mouse_scroll_speed(&self, fine: bool) -> f64 {
        // C++: f = emCoreConfig->MouseScrollSpeed (default 1.0)
        let mut f: f64 = 1.0;
        if fine {
            f *= 0.1;
        }
        if self.pan_function {
            -f
        } else {
            6.0 * f
        }
    }

    /// C++ `GetMouseZoomSpeed`. Returns the speed factor for Ctrl+middle-drag
    /// zooming. Base is `MouseZoomSpeed` (default 1.0) * 6.0.
    /// `fine` (Shift held) scales by 0.1.
    fn get_mouse_zoom_speed(&self, fine: bool) -> f64 {
        // C++: f = emCoreConfig->MouseZoomSpeed (default 1.0)
        let mut f: f64 = 1.0;
        if fine {
            f *= 0.1;
        }
        f * 6.0
    }

    /// Set the PanFunction flag. When true, mouse-drag scrolling reverses
    /// direction and uses 1x speed instead of 6x.
    pub(crate) fn set_pan_function(&mut self, enabled: bool) {
        self.pan_function = enabled;
    }

    /// Returns whether PanFunction is enabled.
    pub(crate) fn pan_function(&self) -> bool {
        self.pan_function
    }

    /// Enable or disable stick-mouse-when-navigating.
    pub(crate) fn set_stick_mouse(&mut self, enabled: bool) {
        self.stick_mouse = enabled;
    }

    /// Returns whether stick-mouse is enabled.
    pub(crate) fn stick_mouse(&self) -> bool {
        self.stick_mouse
    }

    /// Drain the pending warp delta and return it, resetting to zero.
    pub fn drain_pending_warp(&mut self) -> (f64, f64) {
        let warp = self.pending_warp;
        self.pending_warp = (0.0, 0.0);
        warp
    }

    /// Translate Alt key presses into emulated middle mouse button events,
    /// and propagate Alt-held state as middle-button-held in the input state.
    ///
    /// Mirrors C++ `emMouseZoomScrollVIF::EmulateMiddleButton`.
    /// When emulation is enabled and the real middle button is not pressed:
    /// - An Alt key press generates a synthetic middle-button event with
    ///   repeat tracking (330ms threshold), and sets middle-button in `state`.
    /// - When Alt is already held (but the event is something else), the
    ///   middle-button state is set in `state` so downstream consumers see
    ///   the button as pressed.
    ///
    /// Returns `Some(synthetic_event)` if an emulated middle-button press
    /// should be generated, or `None` if no emulation occurred. The caller
    /// should process the returned event before normal input handling.
    pub fn emulate_middle_button_event(
        &mut self,
        event: &emInputEvent,
        state: &mut emInputState,
        clock_ms: u64,
    ) -> Option<emInputEvent> {
        if !self.emulate_middle_button {
            return None;
        }
        // Don't emulate if the real middle button is already held
        if state.Get(InputKey::MouseMiddle) {
            return None;
        }

        if event.key == InputKey::Alt && event.variant == InputVariant::Press && !event.is_repeat()
        {
            // Compute repeat from timing
            let d = clock_ms.saturating_sub(self.emu_mid_button_time);
            if d < 330 {
                self.emu_mid_button_repeat += 1;
            } else {
                self.emu_mid_button_repeat = 0;
            }
            self.emu_mid_button_time = clock_ms;

            // Set middle-button state (C++ sets state before synthesizing event)
            state.press(InputKey::MouseMiddle);

            // Synthesize a middle button press event
            let mut synthetic = emInputEvent::press(InputKey::MouseMiddle);
            synthetic.repeat = self.emu_mid_button_repeat as i32;
            synthetic.mouse_x = event.mouse_x;
            synthetic.mouse_y = event.mouse_y;
            return Some(synthetic);
        } else if state.GetAlt() {
            // Alt is already held — propagate middle-button state so downstream
            // consumers (e.g. grip pan) see it as pressed. C++ does:
            //   state.Set(EM_KEY_MIDDLE_BUTTON, true);
            state.press(InputKey::MouseMiddle);
        }

        None
    }

    /// Calculate a new wheel zoom speed with acceleration curve.
    ///
    /// Mirrors C++ `emMouseZoomScrollVIF::UpdateWheelZoomSpeed`.
    /// `down` is true for zoom-out (wheel down), false for zoom-in.
    /// `fine` is true for shift-held fine-mode (0.1x speed).
    /// `clock_ms` is the current timestamp in milliseconds.
    /// `acceleration` is the configured acceleration value (0 = none).
    /// `min_acceleration` is the minimum config value.
    pub fn update_wheel_zoom_speed(
        &mut self,
        down: bool,
        fine: bool,
        clock_ms: u64,
        acceleration: f64,
        min_acceleration: f64,
    ) {
        let mut new_speed = 2.0_f64.sqrt().ln();
        if fine {
            new_speed *= 0.1;
        }
        if down {
            new_speed = -new_speed;
        }

        // Apply acceleration curve if enabled
        if acceleration > min_acceleration * 1.0001 {
            let t1: f64 = 0.03;
            let t2: f64 = 0.35;
            let f1: f64 = 2.2_f64.powf(acceleration);
            let f2: f64 = 0.4_f64.powf(acceleration);

            let mut dt = (clock_ms.saturating_sub(self.wheel_zoom_time)) as f64 * 0.001;

            // Direction reversal resets timing
            if new_speed * self.wheel_zoom_speed < 0.0 {
                dt = t2;
            }
            dt = dt.clamp(t1, t2);

            // Exponential interpolation between f1 (fast) and f2 (slow)
            let t_frac = (dt - t1) / (t2 - t1);
            let factor = f1 * (f2 / f1).powf(t_frac);
            new_speed *= factor;
        }

        self.wheel_zoom_speed = new_speed;
        self.wheel_zoom_time = clock_ms;
    }

    /// Returns the current wheel zoom speed.
    pub fn wheel_zoom_speed(&self) -> f64 {
        self.wheel_zoom_speed
    }

    /// Configure mouse swiping animator parameters from kinetic config.
    ///
    /// Mirrors C++ `emMouseZoomScrollVIF::SetMouseAnimParams`.
    /// `kinetic_factor` is the KineticZoomingAndScrolling config value.
    /// `min_kinetic` is the minimum value of that config range.
    /// `zoom_factor_log_per_pixel` is from `emView::get_zoom_factor_log_per_pixel`.
    pub fn set_mouse_anim_params(
        &mut self,
        kinetic_factor: f64,
        min_kinetic: f64,
        zoom_factor_log_per_pixel: f64,
    ) {
        self.mouse_kinetic_factor = kinetic_factor;
        self.mouse_min_kinetic = min_kinetic;
        let mut k = kinetic_factor;
        if k < min_kinetic * 1.0001 {
            k = 0.001;
        }
        let zflpp = zoom_factor_log_per_pixel.max(1e-10);
        self.mouse_spring_const = 2500.0 / (k * k);
        self.mouse_friction = 2.0 / zflpp / (k * k);
        self.mouse_friction_enabled = k > 0.001;
    }

    /// Re-derive mouse spring/friction constants for the current zflpp.
    pub fn refresh_mouse_anim_params(&mut self, zflpp: f64) {
        self.set_mouse_anim_params(self.mouse_kinetic_factor, self.mouse_min_kinetic, zflpp);
    }

    /// Returns the mouse animator parameters (spring_const, friction, friction_enabled).
    pub fn mouse_anim_params(&self) -> (f64, f64, bool) {
        (
            self.mouse_spring_const,
            self.mouse_friction,
            self.mouse_friction_enabled,
        )
    }

    /// Configure wheel swiping animator parameters from kinetic config.
    ///
    /// Mirrors C++ `emMouseZoomScrollVIF::SetWheelAnimParams`.
    /// Same parameters as `set_mouse_anim_params` but uses a different
    /// spring constant numerator (480 vs 2500).
    pub fn set_wheel_anim_params(
        &mut self,
        kinetic_factor: f64,
        min_kinetic: f64,
        zoom_factor_log_per_pixel: f64,
    ) {
        self.wheel_kinetic_factor = kinetic_factor;
        self.wheel_min_kinetic = min_kinetic;
        let mut k = kinetic_factor;
        if k < min_kinetic * 1.0001 {
            k = 0.001;
        }
        let zflpp = zoom_factor_log_per_pixel.max(1e-10);
        self.wheel_spring_const = 480.0 / (k * k);
        self.wheel_friction = 2.0 / zflpp / (k * k);
        self.wheel_friction_enabled = k > 0.001;
    }

    /// Re-derive wheel spring/friction constants for the current zflpp.
    pub fn refresh_wheel_anim_params(&mut self, zflpp: f64) {
        self.set_wheel_anim_params(self.wheel_kinetic_factor, self.wheel_min_kinetic, zflpp);
    }

    /// Returns the wheel animator parameters (spring_const, friction, friction_enabled).
    pub fn wheel_anim_params(&self) -> (f64, f64, bool) {
        (
            self.wheel_spring_const,
            self.wheel_friction,
            self.wheel_friction_enabled,
        )
    }

    /// Reset magnetism avoidance state at the start of a grip drag.
    ///
    /// Mirrors C++ `emMouseZoomScrollVIF::InitMagnetismAvoidance`.
    /// Called when the middle button is first pressed.
    fn init_magnetism_avoidance(&mut self, clock_ms: u64) {
        self.mag_av_mouse_move_x = 0.0;
        self.mag_av_mouse_move_y = 0.0;
        self.mag_av_time = clock_ms;
        self.magnetism_avoidance = false;
    }

    /// Accumulate mouse movement and determine whether magnetism should be avoided.
    ///
    /// Mirrors C++ `emMouseZoomScrollVIF::UpdateMagnetismAvoidance`.
    /// If the cumulative mouse movement exceeds `MOUSE_HOLD_MAX_MOVE` (2.0 pixels),
    /// the accumulator and timer are reset. If the mouse has been held still for
    /// `MOUSE_HOLD_TIME` (750 ms), `magnetism_avoidance` becomes true — meaning
    /// the user intentionally paused, so the magnetic animator should NOT activate
    /// on release.
    fn update_magnetism_avoidance(&mut self, dmx: f64, dmy: f64, clock_ms: u64) {
        const MOUSE_HOLD_MAX_MOVE: f64 = 2.0;
        const MOUSE_HOLD_TIME: u64 = 750;

        self.mag_av_mouse_move_x += dmx;
        self.mag_av_mouse_move_y += dmy;
        let r = (self.mag_av_mouse_move_x * self.mag_av_mouse_move_x
            + self.mag_av_mouse_move_y * self.mag_av_mouse_move_y)
            .sqrt();
        if r > MOUSE_HOLD_MAX_MOVE {
            self.mag_av_mouse_move_x = 0.0;
            self.mag_av_mouse_move_y = 0.0;
            self.mag_av_time = clock_ms;
        }
        self.magnetism_avoidance = clock_ms.saturating_sub(self.mag_av_time) >= MOUSE_HOLD_TIME;
    }

    /// Returns whether magnetism avoidance is active (mouse was held still before release).
    pub fn magnetism_avoidance(&self) -> bool {
        self.magnetism_avoidance
    }

    /// Whether kinetic grip coasting is active (post-release animation).
    pub fn is_grip_animating(&self) -> bool {
        self.grip_active
    }

    /// Advance grip animation by one frame.
    ///
    /// Handles two phases per C++ emSwipingViewAnimator + emKineticViewAnimator:
    ///
    /// **Gripped** (`panning`): Critically-damped spring converts accumulated
    /// mouse drag (spring extension) into smoothed velocity. The velocity is
    /// applied as scroll. This creates the springy drag feel.
    ///
    /// **Coasting** (`grip_active && !panning`): Linear friction decays the
    /// velocity each tick until it stops.
    ///
    /// Returns `true` if animation should continue.
    pub fn animate_grip(
        &mut self,
        view: &mut emView,
        tree: &mut super::emPanelTree::PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if !self.grip_active {
            return false;
        }

        if self.panning {
            // ── Gripped phase: critically-damped spring (C++ emSwipingViewAnimator) ──
            //
            // Spring equation: e'' + 2ω·e' + ω²·e = 0  (critically damped)
            // Analytical solution: e(t) = (e₀ + (e₀·ω + v₀)·t) · exp(-ω·t)
            //                     v(t) = (v₀ - (e₀·ω + v₀)·ω·t) · exp(-ω·t)
            // where ω = √(spring_constant)
            let w = self.mouse_spring_const.sqrt();
            let decay = (-w * dt).exp();

            // emProcess X spring — snap to zero when extension is small (C++ parity)
            let e0x = self.grip_spring_x;
            let v0x = self.grip_inst_vel_x;
            if self.mouse_spring_const < 1e5 && (e0x / dt).abs() > 20.0 {
                let e1x = (e0x + (e0x * w + v0x) * dt) * decay;
                let v1x = (v0x - (e0x * w + v0x) * w * dt) * decay;
                self.grip_spring_x = e1x;
                self.grip_inst_vel_x = v1x;
                self.grip_velocity_x = (e0x - e1x) / dt;
            } else {
                self.grip_spring_x = 0.0;
                self.grip_inst_vel_x = 0.0;
                self.grip_velocity_x = e0x / dt;
            }

            // emProcess Y spring — same snap condition per-axis
            let e0y = self.grip_spring_y;
            let v0y = self.grip_inst_vel_y;
            if self.mouse_spring_const < 1e5 && (e0y / dt).abs() > 20.0 {
                let e1y = (e0y + (e0y * w + v0y) * dt) * decay;
                let v1y = (v0y - (e0y * w + v0y) * w * dt) * decay;
                self.grip_spring_y = e1y;
                self.grip_inst_vel_y = v1y;
                self.grip_velocity_y = (e0y - e1y) / dt;
            } else {
                self.grip_spring_y = 0.0;
                self.grip_inst_vel_y = 0.0;
                self.grip_velocity_y = e0y / dt;
            }

            // emProcess Z spring (zoom) — same snap condition per-axis
            let e0z = self.grip_spring_z;
            let v0z = self.grip_inst_vel_z;
            if self.mouse_spring_const < 1e5 && (e0z / dt).abs() > 20.0 {
                let e1z = (e0z + (e0z * w + v0z) * dt) * decay;
                let v1z = (v0z - (e0z * w + v0z) * w * dt) * decay;
                self.grip_spring_z = e1z;
                self.grip_inst_vel_z = v1z;
                self.grip_velocity_z = (e0z - e1z) / dt;
            } else {
                self.grip_spring_z = 0.0;
                self.grip_inst_vel_z = 0.0;
                self.grip_velocity_z = e0z / dt;
            }

            // Apply velocity as scroll+zoom (without friction during grip, per C++)
            let dx = self.grip_velocity_x * dt;
            let dy = self.grip_velocity_y * dt;
            let dz = self.grip_velocity_z * dt;
            if dx.abs() > 0.01 || dy.abs() > 0.01 || dz.abs() > 0.001 {
                view.RawScrollAndZoom(tree, self.grip_fix_x, self.grip_fix_y, dx, dy, dz, ctx);
            }
        } else {
            // ── Coasting phase: linear friction (C++ emKineticViewAnimator) ──
            let v = (self.grip_velocity_x * self.grip_velocity_x
                + self.grip_velocity_y * self.grip_velocity_y
                + self.grip_velocity_z * self.grip_velocity_z)
                .sqrt();
            let f = if self.mouse_friction_enabled && v > 1e-10 {
                let new_v = (v - self.mouse_friction * dt).max(0.0);
                new_v / v
            } else {
                1.0
            };

            let v0x = self.grip_velocity_x;
            let v0y = self.grip_velocity_y;
            let v0z = self.grip_velocity_z;
            self.grip_velocity_x *= f;
            self.grip_velocity_y *= f;
            self.grip_velocity_z *= f;

            // Average velocity over the tick for smooth integration
            let dx = (v0x + self.grip_velocity_x) * 0.5 * dt;
            let dy = (v0y + self.grip_velocity_y) * 0.5 * dt;
            let dz = (v0z + self.grip_velocity_z) * 0.5 * dt;

            if dx.abs() >= 0.01 || dy.abs() >= 0.01 || dz.abs() >= 0.001 {
                let done =
                    view.RawScrollAndZoom(tree, self.grip_fix_x, self.grip_fix_y, dx, dy, dz, ctx);
                // C++: stop axis if view bounced (done < 99% of requested)
                if done[0].abs() < 0.99 * dx.abs() {
                    self.grip_velocity_x = 0.0;
                }
                if done[1].abs() < 0.99 * dy.abs() {
                    self.grip_velocity_y = 0.0;
                }
            }

            // Stop when velocity is negligible
            let speed_sq = self.grip_velocity_x * self.grip_velocity_x
                + self.grip_velocity_y * self.grip_velocity_y
                + self.grip_velocity_z * self.grip_velocity_z;
            if speed_sq < 1.0 {
                self.grip_velocity_x = 0.0;
                self.grip_velocity_y = 0.0;
                self.grip_velocity_z = 0.0;
                self.grip_active = false;
                return false;
            }
        }

        true
    }

    /// Set a deterministic clock for testing. When set, `filter()` uses this
    /// value (in milliseconds) instead of wall-clock time.
    pub fn set_test_clock(&mut self, ms: u64) {
        self.test_clock_ms = Some(ms);
    }

    /// Whether wheel zoom spring animation is active.
    pub fn is_wheel_animating(&self) -> bool {
        self.wheel_active
    }

    /// Advance wheel zoom spring animation by one frame.
    ///
    /// Two phases mirror the C++ VIF → emMagneticViewAnimator handoff:
    ///
    /// **Spring phase** (wheel_coasting=false): Critically-damped spring
    /// decays the extension and produces velocity. When the VIF stop
    /// condition triggers (vel < 10, ext < 0.5), transitions to coast.
    ///
    /// **Coast phase** (wheel_coasting=true): Linear friction decays the
    /// velocity (matching C++ emKineticViewAnimator / emMagneticViewAnimator
    /// behavior after the VIF deactivates the wheel swiping animator).
    ///
    /// Returns true if animation should continue.
    pub fn animate_wheel(
        &mut self,
        view: &mut emView,
        tree: &mut super::emPanelTree::PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if !self.wheel_active {
            return false;
        }

        if self.wheel_coasting {
            // ── Coast phase: friction-decayed velocity ──
            // Matches C++ emMagneticViewAnimator coasting with friction
            // transferred from the WheelAnim.
            let v = self.wheel_velocity_z.abs();
            let f = if self.wheel_friction_enabled && v > 1e-10 {
                let new_v = (v - self.wheel_friction * dt).max(0.0);
                new_v / v
            } else {
                1.0
            };
            let v1 = self.wheel_velocity_z;
            self.wheel_velocity_z *= f;
            let dz = (v1 + self.wheel_velocity_z) * 0.5 * dt;
            if dz.abs() >= 0.01 {
                view.RawScrollAndZoom(tree, self.wheel_fix_x, self.wheel_fix_y, 0.0, 0.0, dz, ctx);
            }
            if self.wheel_velocity_z.abs() < 0.01 {
                self.wheel_velocity_z = 0.0;
                self.wheel_active = false;
                self.wheel_coasting = false;
                return false;
            }
            return true;
        }

        // ── Spring phase: critically-damped spring ──
        // C++ snaps extension to zero when |extension/dt| <= 20 — avoids
        // lingering tiny velocities from near-zero spring decay.
        let e0 = self.wheel_spring_z;
        let v0 = self.wheel_inst_vel_z;

        if self.wheel_spring_const < 1e5 && (e0 / dt).abs() > 20.0 {
            let w = self.wheel_spring_const.sqrt();
            let decay = (-w * dt).exp();
            let e1 = (e0 + (e0 * w + v0) * dt) * decay;
            let v1 = (v0 - (e0 * w + v0) * w * dt) * decay;
            self.wheel_spring_z = e1;
            self.wheel_inst_vel_z = v1;
            self.wheel_velocity_z = (e0 - e1) / dt;
        } else {
            self.wheel_spring_z = 0.0;
            self.wheel_inst_vel_z = 0.0;
            self.wheel_velocity_z = e0 / dt;
        }

        // Apply zoom velocity via raw_scroll_and_zoom
        let dz = self.wheel_velocity_z * dt;
        if dz.abs() > 0.001 {
            view.RawScrollAndZoom(tree, self.wheel_fix_x, self.wheel_fix_y, 0.0, 0.0, dz, ctx);
        }

        // C++ VIF stop condition: when velocity and extension are both low,
        // the VIF activates the emMagneticViewAnimator (which coasts with
        // friction). We replicate this as a transition to the coast phase.
        if self.wheel_velocity_z.abs() < 10.0 && self.wheel_spring_z.abs() < 0.5 {
            self.wheel_spring_z = 0.0;
            self.wheel_inst_vel_z = 0.0;
            self.wheel_coasting = true;
            return true;
        }

        // C++ UpdateBusyState: stop when BOTH extension AND velocity are tiny.
        let abs_ext = self.wheel_spring_z.abs();
        let abs_vel = self.wheel_velocity_z.abs();
        if abs_ext <= 0.01 && abs_vel <= 0.01 {
            self.wheel_spring_z = 0.0;
            self.wheel_inst_vel_z = 0.0;
            self.wheel_velocity_z = 0.0;
            self.wheel_active = false;
            return false;
        }

        true
    }
}

impl Default for emMouseZoomScrollVIF {
    fn default() -> Self {
        Self::new()
    }
}

impl emViewInputFilter for emMouseZoomScrollVIF {
    fn filter(
        &mut self,
        event: &emInputEvent,
        state: &emInputState,
        view: &mut emView,
        _tree: &mut PanelTree,
        _ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if view.flags.contains(ViewFlags::NO_USER_NAVIGATION) {
            return false;
        }

        if event.key == InputKey::MouseMiddle {
            match event.variant {
                InputVariant::Press => {
                    self.panning = true;
                    self.last_x = state.mouse_x;
                    self.last_y = state.mouse_y;
                    self.grip_fix_x = state.mouse_x;
                    self.grip_fix_y = state.mouse_y;
                    // C++ calls InitMagnetismAvoidance on first middle-button press.
                    let clock_ms = self
                        .test_clock_ms
                        .unwrap_or_else(|| self.clock_start.elapsed().as_millis() as u64);
                    self.init_magnetism_avoidance(clock_ms);
                    // C++ calls SetMouseAnimParams on every grip to track current zflpp.
                    let zflpp = view.GetZoomFactorLogarithmPerPixel();
                    self.refresh_mouse_anim_params(zflpp);
                    // Reset spring and velocity on new grip
                    self.grip_spring_x = 0.0;
                    self.grip_spring_y = 0.0;
                    self.grip_spring_z = 0.0;
                    self.grip_inst_vel_x = 0.0;
                    self.grip_inst_vel_y = 0.0;
                    self.grip_inst_vel_z = 0.0;
                    self.grip_velocity_x = 0.0;
                    self.grip_velocity_y = 0.0;
                    self.grip_velocity_z = 0.0;
                    self.grip_active = true; // Activate animation (gripped phase)
                    return true;
                }
                InputVariant::Release => {
                    self.panning = false;
                    // C++: on release, spring extensions zeroed, velocity transfers
                    // to coasting phase. If velocity is negligible, stop.
                    self.grip_spring_x = 0.0;
                    self.grip_spring_y = 0.0;
                    self.grip_spring_z = 0.0;
                    self.grip_inst_vel_x = self.grip_velocity_x;
                    self.grip_inst_vel_y = self.grip_velocity_y;
                    self.grip_inst_vel_z = self.grip_velocity_z;
                    if !self.magnetism_avoidance {
                        view.activate_magnetic_view_animator();
                    }
                    let speed_sq = self.grip_velocity_x * self.grip_velocity_x
                        + self.grip_velocity_y * self.grip_velocity_y
                        + self.grip_velocity_z * self.grip_velocity_z;
                    if !self.mouse_friction_enabled || speed_sq < 1.0 {
                        self.grip_velocity_x = 0.0;
                        self.grip_velocity_y = 0.0;
                        self.grip_velocity_z = 0.0;
                        self.grip_active = false;
                    }
                    // grip_active remains true for coasting if velocity is significant
                    return true;
                }
                _ => {}
            }
        }

        // Wheel zoom — route through spring physics (C++ emSwipingViewAnimator)
        // C++: only process wheel when no modifier or only Shift is held.
        if matches!(event.key, InputKey::WheelUp | InputKey::WheelDown)
            && event.variant == InputVariant::Press
            && (state.IsNoMod() || state.IsShiftMod())
        {
            let down = event.key == InputKey::WheelDown;
            let clock_ms = self
                .test_clock_ms
                .unwrap_or_else(|| self.clock_start.elapsed().as_millis() as u64);
            self.update_wheel_zoom_speed(
                down,
                state.GetShift(),
                clock_ms,
                1.0,  // MouseWheelZoomAcceleration default
                0.25, // min value
            );
            self.wheel_fix_x = state.mouse_x;
            self.wheel_fix_y = state.mouse_y;
            // C++ calls SetWheelAnimParams on every wheel event to track current zflpp.
            let zflpp = view.GetZoomFactorLogarithmPerPixel();
            self.refresh_wheel_anim_params(zflpp);
            self.wheel_spring_z += self.wheel_zoom_speed / zflpp;
            self.wheel_active = true;
            self.wheel_coasting = false;
            return true;
        }

        // Handle panning/zooming with mouse movement
        if self.panning {
            let dmx = state.mouse_x - self.last_x;
            let dmy = state.mouse_y - self.last_y;
            // C++ calls UpdateMagnetismAvoidance on every frame while gripped.
            let clock_ms = self
                .test_clock_ms
                .unwrap_or_else(|| self.clock_start.elapsed().as_millis() as u64);
            self.update_magnetism_avoidance(dmx, dmy, clock_ms);
            if dmx.abs() > 0.1 || dmy.abs() > 0.1 {
                // D-PANEL-12: Ctrl+middle vertical drag = zoom (C++ parity)
                // C++: MoveGrip(2, -dmy * GetMouseZoomSpeed(shift))
                // Routes through the grip spring z-axis, same as scroll uses x/y.
                if state.GetCtrl() {
                    let f = self.get_mouse_zoom_speed(state.GetShift());
                    self.grip_spring_z += -dmy * f;
                    self.grip_fix_x = state.mouse_x;
                    // C++ line ~142: stick mouse during zoom drag
                    if self.stick_mouse {
                        self.pending_warp.0 += -dmx;
                        self.pending_warp.1 += -dmy;
                    }
                } else {
                    // D-PANEL-10: Accumulate spring extension (C++ MoveGrip).
                    // The spring physics in animate_grip() convert this into
                    // smoothed velocity and scroll. No direct scroll here.
                    let f = self.get_mouse_scroll_speed(state.GetShift());
                    self.grip_spring_x += dmx * f;
                    self.grip_spring_y += dmy * f;
                    // C++ line ~156: stick mouse during scroll drag
                    // (only when PanFunction is NOT active, matching C++ guard)
                    if self.stick_mouse && !self.pan_function {
                        self.pending_warp.0 += -dmx;
                        self.pending_warp.1 += -dmy;
                    }
                }
                self.last_x = state.mouse_x;
                self.last_y = state.mouse_y;
            }
        }

        false
    }

    fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut super::emPanelTree::PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        let wheel = self.animate_wheel(view, tree, dt, ctx);
        let grip = self.animate_grip(view, tree, dt, ctx);
        wheel || grip
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

bitflags::bitflags! {
    /// Tracks which direction keys are currently held down for continuous animation.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub(crate) struct KeyState: u8 {
        const UP       = 0b0000_0001;
        const DOWN     = 0b0000_0010;
        const LEFT     = 0b0000_0100;
        const RIGHT    = 0b0000_1000;
        const ZOOM_IN  = 0b0001_0000;
        const ZOOM_OUT = 0b0010_0000;
    }
}

/// Keyboard zoom and scroll filter (arrow keys, Page Up/Down).
///
/// Supports continuous velocity-based animation: when a direction key is held,
/// scroll/zoom velocity ramps up via acceleration (emSpeedingViewAnimator pattern).
/// On release, velocity decelerates to zero.
pub struct emKeyboardZoomScrollVIF {
    /// Target scroll speed in pixels per second (used as velocity target).
    pub scroll_speed: f64,
    /// Zoom speed multiplier per key press (for discrete mode fallback).
    pub zoom_speed: f64,
    /// State machine for three-step programmatic navigation.
    /// 0 = waiting for Shift+Alt+End, 1 = waiting for letter,
    /// 2..27 = waiting for direction key (step = state - 1).
    nav_by_prog_state: u8,
    /// Current scroll velocity (pixels/second).
    scroll_velocity_x: f64,
    scroll_velocity_y: f64,
    /// Current zoom velocity (log-zoom units/second).
    zoom_velocity: f64,
    /// Which direction keys are held.
    key_state: KeyState,
    /// Acceleration rate (pixels/second^2).
    acceleration: f64,
    /// Deceleration rate when key released (pixels/second^2).
    deceleration: f64,
    /// Reverse acceleration for opposing-direction deceleration.
    reverse_acceleration: f64,
    /// Friction for above-target deceleration.
    friction: f64,
    /// Whether friction-based deceleration is enabled.
    friction_enabled: bool,
}

impl emKeyboardZoomScrollVIF {
    pub fn new() -> Self {
        Self {
            scroll_speed: 50.0,
            zoom_speed: 1.2,
            nav_by_prog_state: 0,
            scroll_velocity_x: 0.0,
            scroll_velocity_y: 0.0,
            zoom_velocity: 0.0,
            key_state: KeyState::empty(),
            acceleration: 200.0,
            deceleration: 400.0,
            reverse_acceleration: 400.0,
            friction: 200.0,
            friction_enabled: false,
        }
    }

    /// Set the acceleration rate (pixels/second^2).
    pub fn SetAcceleration(&mut self, accel: f64) {
        self.acceleration = accel;
    }

    /// Set the deceleration rate (pixels/second^2).
    pub fn set_deceleration(&mut self, decel: f64) {
        self.deceleration = decel;
    }

    /// Configure parameters matching C++ SetAnimatorParameters().
    /// `kinetic`: KineticZoomingAndScrolling config (default 1.0)
    /// `min_kinetic`: minimum config value (default 0.25)
    /// `keyboard_scroll_speed`: KeyboardScrollSpeed config (default 1.0)
    /// `keyboard_zoom_speed`: KeyboardZoomSpeed config (default 1.0)
    /// `zflpp`: zoom factor log per pixel from view
    pub fn set_animator_params(
        &mut self,
        kinetic: f64,
        min_kinetic: f64,
        keyboard_scroll_speed: f64,
        keyboard_zoom_speed: f64,
        zflpp: f64,
    ) {
        let mut k = kinetic;
        if k < min_kinetic * 1.0001 {
            k = 0.001;
        }
        let ss = keyboard_scroll_speed / zflpp * 2.0;
        let zs = keyboard_zoom_speed / zflpp * 2.0;
        let v = (ss + zs) * 0.5;
        self.scroll_speed = ss;
        self.zoom_speed = zs;
        self.acceleration = v / (k * 0.6);
        self.reverse_acceleration = v / (k * 0.2);
        self.deceleration = v / (k * 0.2);
        self.friction = v / (k * 1.6);
        self.friction_enabled = true;
    }

    /// Whether the continuous animation is currently active (has velocity or held keys).
    pub fn is_animating(&self) -> bool {
        !self.key_state.is_empty()
            || self.scroll_velocity_x.abs() > 0.01
            || self.scroll_velocity_y.abs() > 0.01
            || self.zoom_velocity.abs() > 0.001
    }

    /// Advance the continuous keyboard animation by one frame.
    ///
    /// Called each frame when `is_animating()` returns true.
    /// `dt` is the time delta in seconds.
    pub fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut super::emPanelTree::PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) {
        // Compute target velocities from held keys (in pixels/sec, matching C++)
        let target_vx = if self.key_state.contains(KeyState::RIGHT) {
            self.scroll_speed
        } else if self.key_state.contains(KeyState::LEFT) {
            -self.scroll_speed
        } else {
            0.0
        };

        let target_vy = if self.key_state.contains(KeyState::DOWN) {
            self.scroll_speed
        } else if self.key_state.contains(KeyState::UP) {
            -self.scroll_speed
        } else {
            0.0
        };

        let target_vz = if self.key_state.contains(KeyState::ZOOM_IN) {
            self.zoom_speed
        } else if self.key_state.contains(KeyState::ZOOM_OUT) {
            -self.zoom_speed
        } else {
            0.0
        };

        // Three-mode speeding step per dimension
        self.scroll_velocity_x = speeding_step(
            self.scroll_velocity_x,
            target_vx,
            self.acceleration,
            self.reverse_acceleration,
            self.friction,
            self.friction_enabled,
            dt,
        );
        self.scroll_velocity_y = speeding_step(
            self.scroll_velocity_y,
            target_vy,
            self.acceleration,
            self.reverse_acceleration,
            self.friction,
            self.friction_enabled,
            dt,
        );
        self.zoom_velocity = speeding_step(
            self.zoom_velocity,
            target_vz,
            self.acceleration,
            self.reverse_acceleration,
            self.friction,
            self.friction_enabled,
            dt,
        );

        // Apply motion via raw_scroll_and_zoom (matches C++ emKineticViewAnimator base).
        // dz = velocity * dt in the same units as C++ — raw_scroll_and_zoom applies zflpp.
        let dx = self.scroll_velocity_x * dt;
        let dy = self.scroll_velocity_y * dt;
        let dz = self.zoom_velocity * dt;
        if dx.abs() > 0.001 || dy.abs() > 0.001 || dz.abs() > 0.0001 {
            let (vw, vh) = view.viewport_size();
            view.RawScrollAndZoom(tree, vw * 0.5, vh * 0.5, dx, dy, dz, ctx);
        }
    }

    /// Implement a three-step key sequence for programmatic navigation.
    ///
    /// Mirrors C++ `emKeyboardZoomScrollVIF::NavigateByProgram`.
    /// 1. Shift+Alt+End initiates (enters state 1).
    /// 2. Shift+Alt+A-Z selects step strength (enters state 2..27).
    /// 3. Shift+Alt+Arrow/Page executes scroll or zoom.
    ///
    /// Returns true if the event was consumed.
    pub fn navigate_by_program(
        &mut self,
        event: &emInputEvent,
        state: &emInputState,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        const SCROLL_DELTA: f64 = 0.3;
        const ZOOM_FAC: f64 = 1.0015;

        match self.nav_by_prog_state {
            0 => {
                // State 0: wait for Shift+Alt+End
                if event.key == InputKey::End
                    && event.variant == InputVariant::Press
                    && state.GetShift()
                    && state.GetAlt()
                {
                    self.nav_by_prog_state = 1;
                    return true;
                }
                false
            }
            1 => {
                // State 1: wait for a letter key to determine step strength
                if event.variant != InputVariant::Press && event.variant != InputVariant::Repeat {
                    return false;
                }
                self.nav_by_prog_state = 0;

                if state.GetShift() && state.GetAlt() {
                    // Compute step from key code: A=1, B=2, ..., Z=26
                    let step = match event.key {
                        InputKey::Key(c) => {
                            let upper = c.to_ascii_uppercase();
                            if upper.is_ascii_uppercase() {
                                upper as u8 - b'A' + 1
                            } else {
                                return false;
                            }
                        }
                        _ => return false,
                    };
                    if (1..=26).contains(&step) {
                        self.nav_by_prog_state = 1 + step;
                        return true;
                    }
                }
                false
            }
            s if s >= 2 => {
                // State 2..27: execute the navigation command
                if event.variant != InputVariant::Press && event.variant != InputVariant::Repeat {
                    return false;
                }
                let step = (s - 1) as f64;
                self.nav_by_prog_state = 0;

                if !state.GetShift() || !state.GetAlt() {
                    return false;
                }

                let (vw, vh) = view.viewport_size();
                let cpt = (vh / vw.max(1.0)).max(0.001);

                match event.key {
                    InputKey::ArrowLeft => {
                        view.Scroll(tree, -SCROLL_DELTA * step * vw, 0.0, ctx);
                        true
                    }
                    InputKey::ArrowRight => {
                        view.Scroll(tree, SCROLL_DELTA * step * vw, 0.0, ctx);
                        true
                    }
                    InputKey::ArrowUp => {
                        view.Scroll(tree, 0.0, -SCROLL_DELTA * step * vh / cpt, ctx);
                        true
                    }
                    InputKey::ArrowDown => {
                        view.Scroll(tree, 0.0, SCROLL_DELTA * step * vh / cpt, ctx);
                        true
                    }
                    InputKey::PageUp => {
                        let factor = ZOOM_FAC.powf(step);
                        view.Zoom(tree, factor, vw * 0.5, vh * 0.5, ctx);
                        true
                    }
                    InputKey::PageDown => {
                        let factor = 1.0 / ZOOM_FAC.powf(step);
                        view.Zoom(tree, factor, vw * 0.5, vh * 0.5, ctx);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl Default for emKeyboardZoomScrollVIF {
    fn default() -> Self {
        Self::new()
    }
}

impl emViewInputFilter for emKeyboardZoomScrollVIF {
    fn filter(
        &mut self,
        event: &emInputEvent,
        state: &emInputState,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if view.flags.contains(ViewFlags::NO_USER_NAVIGATION) {
            return false;
        }

        // D-PANEL-13: Ignore keyboard input when window not focused (C++ parity)
        if !view.IsFocused() {
            return false;
        }

        // Try programmatic navigation first
        if self.navigate_by_program(event, state, view, tree, ctx) {
            return true;
        }

        // Track key-down/key-up state for continuous animation
        if state.GetAlt() {
            match event.variant {
                InputVariant::Press => match event.key {
                    InputKey::ArrowUp => {
                        self.key_state.insert(KeyState::UP);
                        return true;
                    }
                    InputKey::ArrowDown => {
                        self.key_state.insert(KeyState::DOWN);
                        return true;
                    }
                    InputKey::ArrowLeft => {
                        self.key_state.insert(KeyState::LEFT);
                        return true;
                    }
                    InputKey::ArrowRight => {
                        self.key_state.insert(KeyState::RIGHT);
                        return true;
                    }
                    InputKey::PageUp => {
                        self.key_state.insert(KeyState::ZOOM_IN);
                        return true;
                    }
                    InputKey::PageDown => {
                        self.key_state.insert(KeyState::ZOOM_OUT);
                        return true;
                    }
                    _ => {}
                },
                InputVariant::Release => match event.key {
                    InputKey::ArrowUp => {
                        self.key_state.remove(KeyState::UP);
                        return true;
                    }
                    InputKey::ArrowDown => {
                        self.key_state.remove(KeyState::DOWN);
                        return true;
                    }
                    InputKey::ArrowLeft => {
                        self.key_state.remove(KeyState::LEFT);
                        return true;
                    }
                    InputKey::ArrowRight => {
                        self.key_state.remove(KeyState::RIGHT);
                        return true;
                    }
                    InputKey::PageUp => {
                        self.key_state.remove(KeyState::ZOOM_IN);
                        return true;
                    }
                    InputKey::PageDown => {
                        self.key_state.remove(KeyState::ZOOM_OUT);
                        return true;
                    }
                    _ => {}
                },
                InputVariant::Repeat => {
                    // Repeats are handled by continuous animation, consume them
                    if matches!(
                        event.key,
                        InputKey::ArrowUp
                            | InputKey::ArrowDown
                            | InputKey::ArrowLeft
                            | InputKey::ArrowRight
                            | InputKey::PageUp
                            | InputKey::PageDown
                    ) {
                        return true;
                    }
                }
                _ => {}
            }
        }

        // Release Alt clears all key state (modifier gone)
        if event.key == InputKey::Alt && event.variant == InputVariant::Release {
            self.key_state = KeyState::empty();
        }

        false
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Three-mode velocity step matching C++ emSpeedingViewAnimator::CycleAnimation.
///
/// Mode 1 (reverse): v and target in opposite directions -> use reverse_accel
/// Mode 2 (accelerate): |v| < |target| -> use accel with dt capped at 0.1
/// Mode 3 (friction): |v| >= |target| with friction -> use friction
fn speeding_step(
    v: f64,
    target: f64,
    accel: f64,
    reverse_accel: f64,
    friction: f64,
    friction_enabled: bool,
    dt: f64,
) -> f64 {
    let adt = if v * target < -0.1 {
        reverse_accel * dt
    } else if v.abs() < target.abs() {
        accel * dt.min(0.1)
    } else if friction_enabled {
        friction * dt
    } else {
        0.0
    };
    if v - adt > target {
        v - adt
    } else if v + adt < target {
        v + adt
    } else {
        target
    }
}

/// State for a tracked touch point (simple version for existing code).
#[derive(Copy, Clone, Debug)]
pub struct TouchPoint {
    /// Touch identifier.
    id: u64,
    /// Current position.
    x: f64,
    y: f64,
    /// Previous position (for delta computation).
    prev_x: f64,
    prev_y: f64,
}

/// C++ parity Touch struct for the full gesture state machine.
///
/// Port of C++ `emDefaultTouchVIF::Touch` (emViewInputFilter.h:286-298).
#[derive(Copy, Clone, Debug)]
pub struct Touch {
    pub id: u64,
    pub ms_total: i32,
    pub ms_since_prev: i32,
    pub down: bool,
    pub x: f64,
    pub y: f64,
    pub prev_down: bool,
    pub prev_x: f64,
    pub prev_y: f64,
    pub down_x: f64,
    pub down_y: f64,
}

impl Default for Touch {
    fn default() -> Self {
        Self {
            id: 0,
            ms_total: 0,
            ms_since_prev: 0,
            down: false,
            x: 0.0,
            y: 0.0,
            prev_down: false,
            prev_x: 0.0,
            prev_y: 0.0,
            down_x: 0.0,
            down_y: 0.0,
        }
    }
}

/// Maximum number of tracked touches (C++ MAX_TOUCH_COUNT).
pub const MAX_TOUCH_COUNT: usize = 16;

/// Gesture state machine — all 17 states matching C++ emDefaultTouchVIF::DoGesture.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GestureState {
    Ready,
    FirstDown,
    Scroll,
    ZoomIn,
    ZoomOut,
    FirstDownUp,
    DoubleDown,
    DoubleDownUp,
    TripleDown,
    TripleDownUp,
    SecondDown,
    EmuMouse1,
    EmuMouse2,
    EmuMouse3,
    EmuMouse4,
    ThirdDown,
    FourthDown,
    Finish,
}

/// Touch tracking infrastructure for the full C++ gesture state machine.
/// Actions produced by the gesture state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum GestureAction {
    /// Inject Menu key press+release.
    InjectMenuKey,
    /// Toggle soft keyboard.
    ToggleSoftKeyboard,
    /// Forward a synthetic mouse event (for two-finger emulation).
    /// (key, variant, mouse_x, mouse_y, shift, ctrl)
    ForwardInput {
        key: InputKey,
        variant: InputVariant,
        mouse_x: f64,
        mouse_y: f64,
        shift: bool,
        ctrl: bool,
    },
}

pub struct TouchTracker {
    pub touches: [Touch; MAX_TOUCH_COUNT],
    pub touch_count: usize,
    pub touches_time: u64,
    pub gesture_state: GestureState,
    /// Pending actions from the gesture state machine.
    pub pending_actions: Vec<GestureAction>,
}

impl Default for TouchTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchTracker {
    pub fn new() -> Self {
        Self {
            touches: [Touch::default(); MAX_TOUCH_COUNT],
            touch_count: 0,
            touches_time: 0,
            gesture_state: GestureState::Ready,
            pending_actions: Vec::new(),
        }
    }

    /// Reset all touches. C++ ResetTouches.
    pub fn reset_touches(&mut self) {
        self.touch_count = 0;
        self.touches_time = 0;
    }

    /// Advance to next frame: copy current to prev, update ms_since_prev.
    /// C++ NextTouches.
    pub fn next_touches(&mut self, delta_ms: i32) {
        for i in 0..self.touch_count {
            let t = &mut self.touches[i];
            t.prev_down = t.down;
            t.prev_x = t.x;
            t.prev_y = t.y;
            t.ms_since_prev = delta_ms;
            t.ms_total += delta_ms;
        }
    }

    /// Remove touch at index, shifting remaining touches down.
    /// C++ RemoveTouch.
    pub fn RemoveTouch(&mut self, index: usize) {
        if index >= self.touch_count {
            return;
        }
        for i in index..self.touch_count - 1 {
            self.touches[i] = self.touches[i + 1];
        }
        self.touch_count -= 1;
        self.touches[self.touch_count] = Touch::default();
    }

    /// Whether any touch is currently down.
    pub fn is_any_touch_down(&self) -> bool {
        (0..self.touch_count).any(|i| self.touches[i].down)
    }

    /// Get per-frame move delta for touch at index (current - prev).
    pub fn get_touch_move_x(&self, index: usize) -> f64 {
        if index >= self.touch_count {
            return 0.0;
        }
        self.touches[index].x - self.touches[index].prev_x
    }

    pub fn get_touch_move_y(&self, index: usize) -> f64 {
        if index >= self.touch_count {
            return 0.0;
        }
        self.touches[index].y - self.touches[index].prev_y
    }

    /// Get total move since touch-down (current - down).
    pub fn get_total_touch_move_x(&self, index: usize) -> f64 {
        if index >= self.touch_count {
            return 0.0;
        }
        self.touches[index].x - self.touches[index].down_x
    }

    pub fn get_total_touch_move_y(&self, index: usize) -> f64 {
        if index >= self.touch_count {
            return 0.0;
        }
        self.touches[index].y - self.touches[index].down_y
    }

    /// Run one step of the gesture state machine. Returns true if state changed
    /// (caller should loop until stable). `view` is used for scroll/zoom actions.
    pub fn do_gesture(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        let old_state = self.gesture_state;

        match self.gesture_state {
            GestureState::Ready => {
                if self.touch_count > 0 {
                    self.gesture_state = GestureState::FirstDown;
                }
            }

            GestureState::FirstDown => {
                if self.touch_count > 1 {
                    self.gesture_state = GestureState::SecondDown;
                } else if self.touch_count > 0 && !self.touches[0].down {
                    self.gesture_state = GestureState::FirstDownUp;
                } else if self.touch_count > 0 {
                    let total_move = (self.get_total_touch_move_x(0).powi(2)
                        + self.get_total_touch_move_y(0).powi(2))
                    .sqrt();
                    if total_move > 20.0 {
                        self.gesture_state = GestureState::Scroll;
                    } else if self.touches[0].ms_total > 250 {
                        self.gesture_state = GestureState::ZoomIn;
                    }
                }
            }

            GestureState::Scroll => {
                if self.touch_count == 0 || !self.touches[0].down {
                    self.gesture_state = GestureState::Finish;
                } else {
                    let mx = -self.get_touch_move_x(0);
                    let my = -self.get_touch_move_y(0);
                    view.Scroll(tree, mx, my, ctx);
                }
            }

            GestureState::ZoomIn => {
                if self.touch_count == 0 || !self.touches[0].down {
                    self.gesture_state = GestureState::Finish;
                } else {
                    let ms = self.touches[0].ms_since_prev as f64;
                    let factor = (0.002 * ms).exp();
                    let x = self.touches[0].x;
                    let y = self.touches[0].y;
                    // C++: scroll + zoom combined
                    let mx = -self.get_touch_move_x(0);
                    let my = -self.get_touch_move_y(0);
                    view.Scroll(tree, mx, my, ctx);
                    view.Zoom(tree, factor, x, y, ctx);
                    view.Update(tree, ctx);
                }
            }

            GestureState::ZoomOut => {
                if self.touch_count == 0 || !self.touches[0].down {
                    self.gesture_state = GestureState::Finish;
                } else {
                    let ms = self.touches[0].ms_since_prev as f64;
                    let factor = (-0.002 * ms).exp();
                    let x = self.touches[0].x;
                    let y = self.touches[0].y;
                    // C++: scroll + zoom combined
                    let mx = -self.get_touch_move_x(0);
                    let my = -self.get_touch_move_y(0);
                    view.Scroll(tree, mx, my, ctx);
                    view.Zoom(tree, factor, x, y, ctx);
                    view.Update(tree, ctx);
                }
            }

            GestureState::Finish => {
                if !self.is_any_touch_down() {
                    self.reset_touches();
                    self.gesture_state = GestureState::Ready;
                }
            }

            GestureState::FirstDownUp => {
                if self.touch_count > 1 {
                    // New touch arrived — remove old, transition to DoubleDown
                    self.RemoveTouch(0);
                    self.gesture_state = GestureState::DoubleDown;
                } else if self.touch_count > 0 && self.touches[0].ms_total > 250 {
                    // Single-tap timeout
                    self.gesture_state = GestureState::Finish;
                }
            }

            GestureState::DoubleDown => {
                if self.touch_count > 0 && !self.touches[0].down {
                    self.gesture_state = GestureState::DoubleDownUp;
                } else if self.touch_count > 0 && self.touches[0].ms_total > 250 {
                    self.gesture_state = GestureState::ZoomOut;
                }
            }

            GestureState::DoubleDownUp => {
                if self.touch_count > 1 {
                    self.RemoveTouch(0);
                    self.gesture_state = GestureState::TripleDown;
                } else if self.touch_count > 0 && self.touches[0].ms_total > 250 {
                    // Double-tap action: visit_fullsized(panel, animated=true, fill=false)
                    let x = self.touches[0].down_x;
                    let y = self.touches[0].down_y;
                    if let Some(panel) = view.GetFocusablePanelAt(tree, x, y) {
                        view.VisitFullsized(tree, panel, true, false);
                    }
                    self.gesture_state = GestureState::Finish;
                }
            }

            GestureState::TripleDown => {
                if self.touch_count > 0 && !self.touches[0].down {
                    self.gesture_state = GestureState::TripleDownUp;
                } else if self.touch_count > 0 && self.touches[0].ms_total > 250 {
                    self.gesture_state = GestureState::ZoomIn;
                }
            }

            GestureState::TripleDownUp => {
                if self.touch_count > 1 {
                    self.RemoveTouch(0);
                    self.gesture_state = GestureState::DoubleDown;
                } else if self.touch_count > 0 && self.touches[0].ms_total > 250 {
                    // Triple-tap action: visit_fullsized(panel, animated=true, fill=true)
                    let x = self.touches[0].down_x;
                    let y = self.touches[0].down_y;
                    if let Some(panel) = view.GetFocusablePanelAt(tree, x, y) {
                        view.VisitFullsized(tree, panel, true, true);
                    }
                    self.gesture_state = GestureState::Finish;
                }
            }

            GestureState::SecondDown => {
                if self.touch_count > 2 {
                    self.gesture_state = GestureState::ThirdDown;
                } else if self.touch_count >= 2
                    && (self.touches[0].ms_total > 250 || !self.is_any_touch_down())
                {
                    // Compute direction between two touch points
                    let dx = self.touches[1].down_x - self.touches[0].down_x;
                    let dy = self.touches[1].down_y - self.touches[0].down_y;
                    let (new_state, key, shift, ctrl) = if dx.abs() >= dy.abs() {
                        if dx > 0.0 {
                            (GestureState::EmuMouse1, InputKey::MouseLeft, false, false)
                        } else {
                            (GestureState::EmuMouse2, InputKey::MouseRight, false, false)
                        }
                    } else if dy > 0.0 {
                        (GestureState::EmuMouse3, InputKey::MouseLeft, true, false)
                    } else {
                        (GestureState::EmuMouse4, InputKey::MouseLeft, false, true)
                    };
                    // Inject initial press event at first finger position
                    let mx = self.touches[0].x;
                    let my = self.touches[0].y;
                    self.pending_actions.push(GestureAction::ForwardInput {
                        key,
                        variant: InputVariant::Press,
                        mouse_x: mx,
                        mouse_y: my,
                        shift,
                        ctrl,
                    });
                    self.gesture_state = new_state;
                }
            }

            GestureState::EmuMouse1
            | GestureState::EmuMouse2
            | GestureState::EmuMouse3
            | GestureState::EmuMouse4 => {
                // Determine which button/modifiers to emulate
                let (key, shift, ctrl) = match self.gesture_state {
                    GestureState::EmuMouse1 => (InputKey::MouseLeft, false, false),
                    GestureState::EmuMouse2 => (InputKey::MouseRight, false, false),
                    GestureState::EmuMouse3 => (InputKey::MouseLeft, true, false),
                    GestureState::EmuMouse4 => (InputKey::MouseLeft, false, true),
                    _ => unreachable!(),
                };

                if self.touch_count > 0 {
                    let mx = self.touches[0].x;
                    let my = self.touches[0].y;

                    if !self.is_any_touch_down() {
                        // Touch release: forward release event, clear state, go to Finish
                        self.pending_actions.push(GestureAction::ForwardInput {
                            key,
                            variant: InputVariant::Release,
                            mouse_x: mx,
                            mouse_y: my,
                            shift,
                            ctrl,
                        });
                        self.gesture_state = GestureState::Finish;
                    } else {
                        // Touch held: forward move event with button+modifiers
                        self.pending_actions.push(GestureAction::ForwardInput {
                            key,
                            variant: InputVariant::Move,
                            mouse_x: mx,
                            mouse_y: my,
                            shift,
                            ctrl,
                        });
                    }
                } else {
                    self.gesture_state = GestureState::Finish;
                }
            }

            GestureState::ThirdDown => {
                if self.touch_count > 3 {
                    self.gesture_state = GestureState::FourthDown;
                } else if !self.is_any_touch_down() {
                    // Three-finger release → inject Menu key
                    self.pending_actions.push(GestureAction::InjectMenuKey);
                    self.gesture_state = GestureState::Finish;
                }
            }

            GestureState::FourthDown => {
                if self.touch_count > 4 {
                    self.gesture_state = GestureState::Finish;
                } else if !self.is_any_touch_down() {
                    // Four-finger release → toggle soft keyboard
                    self.pending_actions.push(GestureAction::ToggleSoftKeyboard);
                    self.gesture_state = GestureState::Finish;
                }
            }
        }

        self.gesture_state != old_state
    }
}

/// Touch interaction state machine.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TouchState {
    /// No active touches.
    Idle,
    /// Single finger panning.
    SingleTouch { id: u64 },
    /// Two-finger pinch zoom.
    PinchZoom { id1: u64, id2: u64 },
    /// Released with velocity, coasting.
    Fling,
}

/// Default touch input filter — handles single-touch pan and two-touch pinch zoom.
///
/// Mirrors C++ `emDefaultTouchVIF`. Tracks up to 16 touch points and
/// implements a state machine for pan, pinch-zoom, and fling gestures.
pub struct emDefaultTouchVIF {
    touches: [Option<TouchPoint>; 16],
    active_count: usize,
    state: TouchState,
    last_pinch_distance: f64,
    fling_velocity_x: f64,
    fling_velocity_y: f64,
    /// Friction factor for fling deceleration (0..1).
    fling_friction: f64,
    /// Velocity threshold below which fling stops.
    fling_threshold: f64,
    /// Smoothed velocity for fling detection.
    smoothed_vx: f64,
    smoothed_vy: f64,
    /// C++ parity gesture tracker with 17-state machine.
    pub gesture_tracker: TouchTracker,
    /// Synthetic input events from ForwardInput/InjectMenuKey that the window
    /// must dispatch through its input pipeline.
    pending_forward_events: Vec<emInputEvent>,
}

impl emDefaultTouchVIF {
    pub fn new() -> Self {
        Self {
            touches: [None; 16],
            active_count: 0,
            state: TouchState::Idle,
            last_pinch_distance: 0.0,
            fling_velocity_x: 0.0,
            fling_velocity_y: 0.0,
            fling_friction: 0.95,
            fling_threshold: 0.5,
            smoothed_vx: 0.0,
            smoothed_vy: 0.0,
            gesture_tracker: TouchTracker::new(),
            pending_forward_events: Vec::new(),
        }
    }

    /// Current touch state.
    pub fn state(&self) -> TouchState {
        self.state
    }

    /// Number of active touch points.
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Set the fling friction factor.
    pub fn set_fling_friction(&mut self, friction: f64) {
        self.fling_friction = friction.clamp(0.0, 1.0);
    }

    /// Find a touch point by ID, returning its slot index.
    fn find_touch(&self, id: u64) -> Option<usize> {
        self.touches
            .iter()
            .position(|t| t.is_some_and(|tp| tp.id == id))
    }

    /// Find the first empty slot.
    fn find_empty_slot(&self) -> Option<usize> {
        self.touches.iter().position(|t| t.is_none())
    }

    /// Add a touch point. Returns false if no slots available.
    fn add_touch(&mut self, id: u64, x: f64, y: f64) -> bool {
        dlog!("touch add: id={} x={:.1} y={:.1}", id, x, y);
        if let Some(slot) = self.find_empty_slot() {
            self.touches[slot] = Some(TouchPoint {
                id,
                x,
                y,
                prev_x: x,
                prev_y: y,
            });
            self.active_count += 1;
            true
        } else {
            false
        }
    }

    /// Update a touch point position.
    fn update_touch(&mut self, id: u64, x: f64, y: f64) {
        if let Some(slot) = self.find_touch(id) {
            if let Some(ref mut tp) = self.touches[slot] {
                tp.prev_x = tp.x;
                tp.prev_y = tp.y;
                tp.x = x;
                tp.y = y;
            }
        }
    }

    /// Remove a touch point. Returns the removed touch if found.
    fn RemoveTouch(&mut self, id: u64) -> Option<TouchPoint> {
        if let Some(slot) = self.find_touch(id) {
            let tp = self.touches[slot].take();
            if tp.is_some() {
                self.active_count -= 1;
            }
            tp
        } else {
            None
        }
    }

    /// Get a touch point by ID.
    fn get_touch(&self, id: u64) -> Option<&TouchPoint> {
        if let Some(slot) = self.find_touch(id) {
            self.touches[slot].as_ref()
        } else {
            None
        }
    }

    /// Compute the distance between two touch points.
    fn pinch_distance(&self, id1: u64, id2: u64) -> f64 {
        let t1 = self.get_touch(id1);
        let t2 = self.get_touch(id2);
        match (t1, t2) {
            (Some(a), Some(b)) => {
                let dx = a.x - b.x;
                let dy = a.y - b.y;
                (dx * dx + dy * dy).sqrt()
            }
            _ => 0.0,
        }
    }

    /// Compute the center of two touch points.
    fn pinch_center(&self, id1: u64, id2: u64) -> (f64, f64) {
        let t1 = self.get_touch(id1);
        let t2 = self.get_touch(id2);
        match (t1, t2) {
            (Some(a), Some(b)) => ((a.x + b.x) * 0.5, (a.y + b.y) * 0.5),
            _ => (0.0, 0.0),
        }
    }

    /// Handle a touch start event. Returns true if consumed.
    pub fn touch_start(
        &mut self,
        id: u64,
        x: f64,
        y: f64,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        // Cancel any active fling
        if self.state == TouchState::Fling {
            self.fling_velocity_x = 0.0;
            self.fling_velocity_y = 0.0;
        }

        if !self.add_touch(id, x, y) {
            return false;
        }

        match self.active_count {
            1 => {
                self.state = TouchState::SingleTouch { id };
                self.smoothed_vx = 0.0;
                self.smoothed_vy = 0.0;
            }
            2 => {
                // Don't enter PinchZoom if gesture machine is handling two-finger input.
                // The gesture machine transitions FirstDown→SecondDown on second touch,
                // and SecondDown handles swipe direction detection and EmuMouse.
                let gesture_active = !matches!(
                    self.gesture_tracker.gesture_state,
                    GestureState::Ready | GestureState::Finish
                );
                if !gesture_active {
                    let mut ids = Vec::new();
                    for tp in self.touches.iter().flatten() {
                        ids.push(tp.id);
                        if ids.len() == 2 {
                            break;
                        }
                    }
                    if ids.len() == 2 {
                        self.state = TouchState::PinchZoom {
                            id1: ids[0],
                            id2: ids[1],
                        };
                        self.last_pinch_distance = self.pinch_distance(ids[0], ids[1]);
                    }
                }
            }
            _ => {
                // 3+ touches: remain in current state
            }
        }

        // Sync gesture tracker: add new touch
        let tc = self.gesture_tracker.touch_count;
        if tc < MAX_TOUCH_COUNT {
            self.gesture_tracker.touches[tc] = Touch {
                id,
                down: true,
                x,
                y,
                down_x: x,
                down_y: y,
                prev_x: x,
                prev_y: y,
                ..Touch::default()
            };
            self.gesture_tracker.touch_count += 1;
        }

        self.run_gesture_loop(view, tree, ctx);
        let events = self.drain_gesture_actions(view);
        self.pending_forward_events.extend(events);
        true
    }

    /// Handle a touch move event. Applies pan or pinch-zoom to the view.
    /// `dt` is the frame delta in seconds. Returns true if consumed.
    #[allow(clippy::too_many_arguments)]
    pub fn touch_move(
        &mut self,
        id: u64,
        x: f64,
        y: f64,
        dt: f64,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        self.update_touch(id, x, y);

        // Sync gesture tracker: update touch position (prev before current for frame delta)
        for i in 0..self.gesture_tracker.touch_count {
            if self.gesture_tracker.touches[i].id == id {
                self.gesture_tracker.touches[i].prev_x = self.gesture_tracker.touches[i].x;
                self.gesture_tracker.touches[i].prev_y = self.gesture_tracker.touches[i].y;
                self.gesture_tracker.touches[i].x = x;
                self.gesture_tracker.touches[i].y = y;
                break;
            }
        }

        // The gesture machine handles input in all active states.
        // Only Ready and Finish allow the old 4-state system to handle moves.
        let gesture_handles_move = !matches!(
            self.gesture_tracker.gesture_state,
            GestureState::Ready | GestureState::Finish
        );

        let consumed = match self.state {
            TouchState::SingleTouch { id: touch_id } if touch_id == id => {
                if let Some(tp) = self.get_touch(id) {
                    let dx = tp.x - tp.prev_x;
                    let dy = tp.y - tp.prev_y;
                    if dx.abs() > 0.001 || dy.abs() > 0.001 {
                        if !gesture_handles_move {
                            view.Scroll(tree, dx, dy, ctx);
                        }
                        // Update smoothed velocity for fling detection regardless
                        let dt_safe = dt.max(1e-6);
                        let ivx = dx / dt_safe;
                        let ivy = dy / dt_safe;
                        let alpha = 0.3;
                        self.smoothed_vx += alpha * (ivx - self.smoothed_vx);
                        self.smoothed_vy += alpha * (ivy - self.smoothed_vy);
                    }
                }
                true
            }
            TouchState::PinchZoom { id1, id2 } if id == id1 || id == id2 => {
                if !gesture_handles_move {
                    let new_dist = self.pinch_distance(id1, id2);
                    if self.last_pinch_distance > 0.1 && new_dist > 0.1 {
                        let factor = new_dist / self.last_pinch_distance;
                        let (cx, cy) = self.pinch_center(id1, id2);
                        view.Zoom(tree, factor, cx, cy, ctx);
                    }
                    self.last_pinch_distance = new_dist;
                }
                true
            }
            _ => false,
        };

        if consumed {
            self.run_gesture_loop(view, tree, ctx);
            let events = self.drain_gesture_actions(view);
            self.pending_forward_events.extend(events);
        }
        consumed
    }

    /// Handle a touch end event. May trigger fling. Returns true if consumed.
    pub fn touch_end(
        &mut self,
        id: u64,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        let removed = self.RemoveTouch(id);
        if removed.is_none() {
            return false;
        }

        match self.state {
            TouchState::SingleTouch { id: touch_id } if touch_id == id => {
                // Check for fling
                let speed = (self.smoothed_vx * self.smoothed_vx
                    + self.smoothed_vy * self.smoothed_vy)
                    .sqrt();
                if speed > self.fling_threshold {
                    self.fling_velocity_x = self.smoothed_vx;
                    self.fling_velocity_y = self.smoothed_vy;
                    self.state = TouchState::Fling;
                } else {
                    self.state = TouchState::Idle;
                }
            }
            TouchState::PinchZoom { id1, id2 } => {
                let remaining_id = if id == id1 { id2 } else { id1 };
                let gesture_active = !matches!(
                    self.gesture_tracker.gesture_state,
                    GestureState::Ready | GestureState::Finish
                );
                if !gesture_active && self.get_touch(remaining_id).is_some() {
                    self.state = TouchState::SingleTouch { id: remaining_id };
                    self.smoothed_vx = 0.0;
                    self.smoothed_vy = 0.0;
                } else {
                    self.state = TouchState::Idle;
                }
            }
            _ => {
                if self.active_count == 0 {
                    self.state = TouchState::Idle;
                }
            }
        }

        // Sync gesture tracker: mark touch as up
        for i in 0..self.gesture_tracker.touch_count {
            if self.gesture_tracker.touches[i].id == id {
                self.gesture_tracker.touches[i].down = false;
                break;
            }
        }

        self.run_gesture_loop(view, tree, ctx);
        let events = self.drain_gesture_actions(view);
        self.pending_forward_events.extend(events);
        true
    }

    /// Animate fling deceleration. Call each frame. Returns true if still animating.
    pub fn animate_fling(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if self.state != TouchState::Fling {
            return false;
        }

        self.fling_velocity_x *= self.fling_friction;
        self.fling_velocity_y *= self.fling_friction;

        let dx = self.fling_velocity_x * dt;
        let dy = self.fling_velocity_y * dt;

        if dx.abs() > 0.001 || dy.abs() > 0.001 {
            view.Scroll(tree, dx, dy, ctx);
        }

        let speed = (self.fling_velocity_x * self.fling_velocity_x
            + self.fling_velocity_y * self.fling_velocity_y)
            .sqrt();
        if speed < self.fling_threshold {
            self.fling_velocity_x = 0.0;
            self.fling_velocity_y = 0.0;
            self.state = TouchState::Idle;
            return false;
        }
        true
    }
}

impl Default for emDefaultTouchVIF {
    fn default() -> Self {
        Self::new()
    }
}

impl emDefaultTouchVIF {
    /// Run the gesture state machine until stable (C++ DoGesture loop in Input/Cycle).
    fn run_gesture_loop(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) {
        for _ in 0..100 {
            if !self.gesture_tracker.do_gesture(view, tree, ctx) {
                break;
            }
        }
    }

    /// C++ Cycle(): advance touch timers and loop DoGesture for time-based
    /// transitions (e.g. hold timeouts, tap chain timeouts).
    pub fn cycle_gesture(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt_ms: i32,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) {
        self.gesture_tracker.next_touches(dt_ms);
        self.run_gesture_loop(view, tree, ctx);
        let events = self.drain_gesture_actions(view);
        self.pending_forward_events.extend(events);
    }

    /// Process pending gesture actions that aren't handled inline by do_gesture.
    /// Returns synthetic input events that must be dispatched through the
    /// window's input pipeline (ForwardInput and InjectMenuKey actions).
    fn drain_gesture_actions(&mut self, _view: &mut emView) -> Vec<emInputEvent> {
        let mut forward_events = Vec::new();
        for action in self.gesture_tracker.pending_actions.drain(..) {
            match action {
                GestureAction::InjectMenuKey => {
                    dlog!("Touch gesture: inject menu key");
                    // Menu key press
                    forward_events.push(emInputEvent {
                        key: InputKey::Menu,
                        variant: InputVariant::Press,
                        chars: String::new(),
                        repeat: 0,
                        source_variant: 0,
                        mouse_x: 0.0,
                        mouse_y: 0.0,
                        shift: false,
                        ctrl: false,
                        alt: false,
                        meta: false,
                        eaten: false,
                    });
                    // Menu key release
                    forward_events.push(emInputEvent {
                        key: InputKey::Menu,
                        variant: InputVariant::Release,
                        chars: String::new(),
                        repeat: 0,
                        source_variant: 0,
                        mouse_x: 0.0,
                        mouse_y: 0.0,
                        shift: false,
                        ctrl: false,
                        alt: false,
                        meta: false,
                        eaten: false,
                    });
                }
                GestureAction::ToggleSoftKeyboard => {
                    dlog!("Touch gesture: toggle soft keyboard");
                    _view.ShowSoftKeyboard(!_view.IsSoftKeyboardShown());
                }
                GestureAction::ForwardInput {
                    key,
                    variant,
                    mouse_x,
                    mouse_y,
                    shift,
                    ctrl,
                } => {
                    dlog!(
                        "Touch gesture: forward input {:?} {:?} at ({:.0}, {:.0})",
                        key,
                        variant,
                        mouse_x,
                        mouse_y
                    );
                    forward_events.push(emInputEvent {
                        key,
                        variant,
                        chars: String::new(),
                        repeat: 0,
                        source_variant: 0,
                        mouse_x,
                        mouse_y,
                        shift,
                        ctrl,
                        alt: false,
                        meta: false,
                        eaten: false,
                    });
                }
            }
        }
        forward_events
    }

    /// Drain buffered synthetic input events for the window to dispatch.
    pub fn drain_forward_events(&mut self) -> Vec<emInputEvent> {
        std::mem::take(&mut self.pending_forward_events)
    }
}

impl emViewInputFilter for emDefaultTouchVIF {
    fn filter(
        &mut self,
        event: &emInputEvent,
        _state: &emInputState,
        _view: &mut emView,
        _tree: &mut PanelTree,
        _ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        // Touch events are handled via touch_start/touch_move/touch_end,
        // not through the generic filter. This filter only handles
        // cancellation of fling on any key/button event.
        if self.state == TouchState::Fling && event.variant == InputVariant::Press {
            self.fling_velocity_x = 0.0;
            self.fling_velocity_y = 0.0;
            self.state = TouchState::Idle;
            return true;
        }
        false
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// emCheatVIF — cheat code input filter
// ---------------------------------------------------------------------------

/// Actions produced by emCheatVIF that the window must apply to other VIFs or
/// global config. The caller drains these after each filter pass.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CheatAction {
    /// Toggle PanFunction on the emMouseZoomScrollVIF.
    PanFunction,
    /// Toggle EmulateMiddleButton on the emMouseZoomScrollVIF.
    EmulateMiddleButton,
    /// Toggle StickMouseWhenNavigating config.
    StickMouseWhenNavigating,
    /// Dump the panel tree to disk.
    TreeDump,
    /// Take a screenshot.
    Screenshot,
}

/// Cheat code input filter.
///
/// Port of C++ `emCheatVIF`. Maintains a rolling buffer of typed characters
/// and recognizes `:command!` sequences to trigger debug/developer actions.
/// Unless easy cheats are enabled, the prefix `chEat:` is required.
///
/// Always forwards events (never consumes them).
pub(crate) struct emCheatVIF {
    /// Rolling buffer of recent typed characters (mirrors C++ `CheatBuffer[64]`).
    buffer: [u8; 64],
    /// Whether easy cheats are enabled (no `chEat:` prefix needed).
    easy_cheats: bool,
    /// Pending actions for the window to apply.
    pending_actions: Vec<CheatAction>,
}

impl emCheatVIF {
    pub(crate) fn new() -> Self {
        Self {
            buffer: [0u8; 64],
            easy_cheats: false,
            pending_actions: Vec::new(),
        }
    }

    /// Drain pending actions. The caller applies these to the appropriate
    /// VIFs or config objects.
    pub(crate) fn drain_actions(&mut self) -> Vec<CheatAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Execute a recognized cheat command.
    fn execute_cheat(&mut self, func: &str, view: &mut emView) {
        match func {
            // Enable easy cheats for the whole process: chEat:easy!
            "easy" => {
                self.easy_cheats = true;
            }

            // Stress test on/off: chEat:st!
            "st" => {
                view.flags ^= ViewFlags::STRESS_TEST;
            }

            // Popup-zoom on/off: chEat:pz!
            "pz" => {
                let flags = view.flags ^ ViewFlags::POPUP_ZOOM;
                view.flags = flags;
            }

            // Ego mode on/off: chEat:egomode!
            "egomode" => {
                view.flags ^= ViewFlags::EGO_MODE;
                // emCursor override changes with EGO_MODE, so invalidate.
                view.mark_cursor_invalid();
            }

            // StickMouseWhenNavigating on/off: chEat:smwn!
            "smwn" => {
                self.pending_actions
                    .push(CheatAction::StickMouseWhenNavigating);
            }

            // EmulateMiddleButton on/off: chEat:emb!
            "emb" => {
                self.pending_actions.push(CheatAction::EmulateMiddleButton);
            }

            // PanFunction on/off: chEat:pan!
            "pan" => {
                self.pending_actions.push(CheatAction::PanFunction);
            }

            // Tree dump: chEat:td!
            "td" => {
                self.pending_actions.push(CheatAction::TreeDump);
            }

            // Debug log on/off: chEat:dlog!
            "dlog" => {
                let enabled = !crate::emStd1::emIsDLogEnabled();
                crate::emStd1::emEnableDLog(enabled);
                eprintln!(
                    "[CheatVIF] debug log {}",
                    if enabled { "enabled" } else { "disabled" }
                );
            }

            // Screenshot: chEat:ss!
            "ss" => {
                self.pending_actions.push(CheatAction::Screenshot);
            }

            // Crash by segfault: chEat:segfault!
            "segfault" => {
                // Deliberate crash for testing — port of C++ `*(volatile char*)NULL=0`
                panic!("CheatVIF: deliberate segfault cheat code triggered");
            }

            // Crash by division by zero: chEat:divzero!
            "divzero" => {
                // Deliberate crash for testing — port of C++ `emSleepMS(255/func[strlen(func)])`
                panic!("CheatVIF: deliberate divzero cheat code triggered");
            }

            // Fatal error: chEat:fatal!
            "fatal" => {
                panic!("CheatVIF: You entered that cheat code!");
            }

            // Unknown command — custom cheat fallthrough
            _ => {
                view.DoCustomCheat(func);
            }
        }
    }
}

impl emViewInputFilter for emCheatVIF {
    fn filter(
        &mut self,
        event: &emInputEvent,
        _state: &emInputState,
        view: &mut emView,
        _tree: &mut PanelTree,
        _ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        // C++: skip if NO_USER_NAVIGATION
        if view.flags.contains(ViewFlags::NO_USER_NAVIGATION) {
            return false;
        }

        // Only process events that produce characters
        let chars = &event.chars;
        if chars.is_empty() {
            return false;
        }

        // Shift buffer left and append new chars (C++ memmove + memcpy pattern)
        let bytes = chars.as_bytes();
        let sz = bytes.len().min(64);
        self.buffer.rotate_left(sz);
        self.buffer[64 - sz..].copy_from_slice(&bytes[..sz]);

        // Check if the last character is '!'
        if self.buffer[63] != b'!' {
            return false;
        }

        // Clear the '!' so we don't re-trigger
        self.buffer[63] = 0;

        // Scan backward for ':' to extract the command
        let mut colon_pos = None;
        for i in (0..63).rev() {
            if self.buffer[i] == b':' {
                colon_pos = Some(i);
                break;
            }
            if self.buffer[i] == 0 {
                break;
            }
        }

        let colon_pos = match colon_pos {
            Some(p) => p,
            None => return false,
        };

        let func_bytes = &self.buffer[colon_pos + 1..63];
        let func = match std::str::from_utf8(func_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => return false,
        };

        // Unless easy cheats are enabled, require "chEat" before the ':'
        if !self.easy_cheats {
            // Need at least 5 bytes before the ':' for "chEat"
            if colon_pos < 5 {
                return false;
            }
            if &self.buffer[colon_pos - 5..colon_pos] != b"chEat" {
                return false;
            }
        }

        self.execute_cheat(&func, view);

        // C++ always forwards (never eats the event)
        false
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;
    use crate::emScheduler::EngineScheduler;

    /// Test helper: owns the data needed to construct a `SchedCtx`.
    struct TestSched {
        sched: EngineScheduler,
        fw: Vec<crate::emEngineCtx::DeferredAction>,
        ctx: std::rc::Rc<crate::emContext::emContext>,
    }
    impl TestSched {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                ctx: crate::emContext::emContext::NewRoot(),
            }
        }
        fn with<R>(&mut self, f: impl FnOnce(&mut crate::emEngineCtx::SchedCtx<'_>) -> R) -> R {
            let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                std::cell::RefCell::new(None);
            let __pa: std::rc::Rc<
                std::cell::RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>,
            > = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.ctx,
                framework_clipboard: &__cb,
                current_engine: None,
                pending_actions: &__pa,
            };
            f(&mut sc)
        }
    }

    fn setup() -> (PanelTree, emView) {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
        let view = emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        (tree, view)
    }

    #[test]
    fn test_drain_gesture_actions_returns_forward_input() {
        let mut vif = emDefaultTouchVIF::new();
        // Manually push a ForwardInput action to simulate gesture machine output
        vif.gesture_tracker
            .pending_actions
            .push(GestureAction::ForwardInput {
                key: InputKey::MouseLeft,
                variant: InputVariant::Press,
                mouse_x: 100.0,
                mouse_y: 200.0,
                shift: false,
                ctrl: false,
            });
        let (_tree, mut view) = setup();
        let forward_events = vif.drain_gesture_actions(&mut view);
        assert_eq!(forward_events.len(), 1);
        assert_eq!(forward_events[0].key, InputKey::MouseLeft);
        assert_eq!(forward_events[0].variant, InputVariant::Press);
        assert!((forward_events[0].mouse_x - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_gesture_dead_zone_no_scroll_under_20px() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let root = view.GetRootPanel();
        tree.set_focusable(root, true);
        ts.with(|sc| view.Update(&mut tree, sc));
        let (_, rx_before, ry_before, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist before gesture");

        let mut vif = emDefaultTouchVIF::new();
        ts.with(|sc| vif.touch_start(1, 100.0, 100.0, &mut view, &mut tree, sc));
        // Move 10px — under the gesture machine's 20px dead zone
        ts.with(|sc| vif.touch_move(1, 110.0, 100.0, 0.016, &mut view, &mut tree, sc));

        // With old system suppressed, view should NOT have scrolled
        let (_, rx_after, ry_after, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist after gesture");
        assert!(
            (rx_after - rx_before).abs() < 1e-12 && (ry_after - ry_before).abs() < 1e-12,
            "View scrolled during dead zone — old SingleTouch not suppressed \
             (dx={:.6}, dy={:.6})",
            rx_after - rx_before,
            ry_after - ry_before,
        );
    }

    #[test]
    fn test_inject_menu_key_produces_press_release() {
        let mut vif = emDefaultTouchVIF::new();
        vif.gesture_tracker
            .pending_actions
            .push(GestureAction::InjectMenuKey);
        let (_tree, mut view) = setup();
        let events = vif.drain_gesture_actions(&mut view);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].key, InputKey::Menu);
        assert_eq!(events[0].variant, InputVariant::Press);
        assert_eq!(events[1].key, InputKey::Menu);
        assert_eq!(events[1].variant, InputVariant::Release);
    }

    #[test]
    fn test_emulate_middle_button() {
        let mut vif = emMouseZoomScrollVIF::new();
        let mut state = emInputState::new();

        // Disabled by default
        let event = emInputEvent::press(InputKey::Alt);
        assert!(vif
            .emulate_middle_button_event(&event, &mut state, 100)
            .is_none());

        // Enable and test — first press at 1000ms (well past initial time 0)
        vif.set_emulate_middle_button(true);
        let result = vif.emulate_middle_button_event(&event, &mut state, 1000);
        assert!(result.is_some());
        let synth = result.unwrap();
        assert_eq!(synth.key, InputKey::MouseMiddle);
        assert_eq!(synth.variant, InputVariant::Press);
        assert_eq!(synth.repeat, 0);
        // State should now have middle button set
        assert!(state.Get(InputKey::MouseMiddle));

        // Reset state for next test (simulate release cycle)
        state.release(InputKey::MouseMiddle);

        // Double-click within 330ms
        let event2 = emInputEvent::press(InputKey::Alt);
        let result2 = vif.emulate_middle_button_event(&event2, &mut state, 1200);
        assert!(result2.is_some());
        assert!(result2.unwrap().repeat > 0);
        state.release(InputKey::MouseMiddle);

        // After timeout, repeat resets
        let event3 = emInputEvent::press(InputKey::Alt);
        let result3 = vif.emulate_middle_button_event(&event3, &mut state, 2000);
        assert!(result3.is_some());
        assert_eq!(result3.unwrap().repeat, 0);
    }

    #[test]
    fn test_emulate_middle_button_alt_held_propagation() {
        let mut vif = emMouseZoomScrollVIF::new();
        vif.set_emulate_middle_button(true);
        let mut state = emInputState::new();
        state.press(InputKey::Alt);

        // Non-Alt event while Alt is held — should set middle-button state
        let move_event = emInputEvent::press(InputKey::MouseLeft);
        let result = vif.emulate_middle_button_event(&move_event, &mut state, 500);
        assert!(result.is_none(), "No synthetic event for non-Alt press");
        assert!(
            state.Get(InputKey::MouseMiddle),
            "Middle button should be set when Alt is held"
        );
    }

    #[test]
    fn test_update_wheel_zoom_speed() {
        let mut vif = emMouseZoomScrollVIF::new();

        // Basic zoom in
        vif.update_wheel_zoom_speed(false, false, 1000, 0.0, 0.0);
        assert!(vif.wheel_zoom_speed() > 0.0);

        // Zoom out negates
        vif.update_wheel_zoom_speed(true, false, 1100, 0.0, 0.0);
        assert!(vif.wheel_zoom_speed() < 0.0);

        // Fine mode reduces speed
        vif.update_wheel_zoom_speed(false, true, 1200, 0.0, 0.0);
        let fine_speed = vif.wheel_zoom_speed();
        vif.update_wheel_zoom_speed(false, false, 1300, 0.0, 0.0);
        let normal_speed = vif.wheel_zoom_speed();
        assert!(fine_speed.abs() < normal_speed.abs());

        // Acceleration curve
        vif.update_wheel_zoom_speed(false, false, 2000, 5.0, 1.0);
        let accel_speed = vif.wheel_zoom_speed();
        assert!(accel_speed.abs() > 0.0);
    }

    #[test]
    fn test_set_mouse_anim_params() {
        let mut vif = emMouseZoomScrollVIF::new();

        vif.set_mouse_anim_params(1.0, 0.5, 0.01);
        let (sc, fr, enabled) = vif.mouse_anim_params();
        assert!((sc - 2500.0).abs() < 0.1);
        assert!(fr > 0.0);
        assert!(enabled);

        // At minimum kinetic, clamps to 0.001
        vif.set_mouse_anim_params(0.5, 0.5, 0.01);
        let (sc2, _fr2, enabled2) = vif.mouse_anim_params();
        assert!(sc2 > 1e6); // 2500/(0.001^2) = very large
        assert!(!enabled2);
    }

    #[test]
    fn test_set_wheel_anim_params() {
        let mut vif = emMouseZoomScrollVIF::new();

        vif.set_wheel_anim_params(1.0, 0.5, 0.01);
        let (sc, fr, enabled) = vif.wheel_anim_params();
        assert!((sc - 480.0).abs() < 0.1);
        assert!(fr > 0.0);
        assert!(enabled);
    }

    #[test]
    fn test_navigate_by_program() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        // Zoom in so Scroll is not clamped by zoom-out root-centering.
        ts.with(|sc| view.Zoom(&mut tree, 4.0, 400.0, 300.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut vif = emKeyboardZoomScrollVIF::new();

        let mut state = emInputState::new();
        state.press(InputKey::Shift);
        state.press(InputKey::Alt);

        // Step 1: Shift+Alt+End
        let event = emInputEvent::press(InputKey::End);
        assert!(ts.with(|sc| vif.navigate_by_program(&event, &state, &mut view, &mut tree, sc)));

        // Step 2: Shift+Alt+C (step = 3)
        let event2 = emInputEvent::press(InputKey::Key('c'));
        assert!(ts.with(|sc| vif.navigate_by_program(&event2, &state, &mut view, &mut tree, sc)));

        // Step 3: Shift+Alt+Right (scroll right)
        let (_, before, _, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel before scroll");
        let event3 = emInputEvent::press(InputKey::ArrowRight);
        assert!(ts.with(|sc| vif.navigate_by_program(&event3, &state, &mut view, &mut tree, sc)));
        let (_, after, _, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel after scroll");
        assert!(after > before, "Should have scrolled right");
    }

    #[test]
    fn test_keyboard_continuous_key_state() {
        let mut ts = TestSched::new();
        let mut vif = emKeyboardZoomScrollVIF::new();

        // Initially not animating
        assert!(!vif.is_animating());

        // Simulate pressing right arrow with Alt
        let mut state = emInputState::new();
        state.press(InputKey::Alt);

        let event = emInputEvent::press(InputKey::ArrowRight);
        let (mut tree, mut view) = setup();
        assert!(ts.with(|sc| vif.filter(&event, &state, &mut view, &mut tree, sc)));
        assert!(vif.key_state.contains(KeyState::RIGHT));
        assert!(vif.is_animating());

        // Release right arrow
        let release_event = emInputEvent::release(InputKey::ArrowRight);
        assert!(ts.with(|sc| vif.filter(&release_event, &state, &mut view, &mut tree, sc)));
        assert!(!vif.key_state.contains(KeyState::RIGHT));
    }

    #[test]
    fn test_keyboard_continuous_animation() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        // Zoom in so the per-frame scroll deltas are observable via
        // get_visited_panel_idiom (otherwise root-centering clamps rel_x=0).
        ts.with(|sc| view.Zoom(&mut tree, 4.0, 400.0, 300.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emKeyboardZoomScrollVIF::new();
        vif.key_state.insert(KeyState::RIGHT);

        let (_, before, _, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel before animate");

        // Animate several frames
        for _ in 0..10 {
            ts.with(|sc| vif.animate(&mut view, &mut tree, 0.016, sc));
        }

        let (_, after, _, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel after animate");
        assert!(after > before, "Continuous animation should scroll right");
    }

    #[test]
    fn test_keyboard_deceleration() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emKeyboardZoomScrollVIF::new();
        // Enable friction so release decelerates (C++ parity requires set_animator_params)
        vif.friction_enabled = true;
        // Ramp up velocity
        vif.key_state.insert(KeyState::DOWN);
        for _ in 0..20 {
            ts.with(|sc| vif.animate(&mut view, &mut tree, 0.016, sc));
        }
        assert!(vif.scroll_velocity_y.abs() > 0.1, "Should have velocity");

        // Release key — should decelerate via friction
        vif.key_state.remove(KeyState::DOWN);
        for _ in 0..100 {
            ts.with(|sc| vif.animate(&mut view, &mut tree, 0.016, sc));
        }
        assert!(
            vif.scroll_velocity_y.abs() < 0.1,
            "Should decelerate to near zero"
        );
    }

    #[test]
    fn test_keyboard_zoom_continuous() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emKeyboardZoomScrollVIF::new();
        // Use set_animator_params to compute zoom_speed in correct units for
        // the zflpp-based raw_scroll_and_zoom path (C++ parity).
        let zflpp = view.GetZoomFactorLogarithmPerPixel();
        vif.set_animator_params(1.0, 0.25, 1.0, 1.0, zflpp);
        vif.key_state.insert(KeyState::ZOOM_IN);

        let (_, _, _, before) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist before zoom animation");
        for _ in 0..20 {
            ts.with(|sc| vif.animate(&mut view, &mut tree, 0.016, sc));
        }

        let (_, _, _, after) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist after zoom animation");
        // rel_a uses C++ convention: HomeW*HomeH/(vw*vh). Zooming IN enlarges vw*vh,
        // so rel_a DECREASES when zooming in.
        assert!(after < before, "Should have zoomed in");
    }

    #[test]
    fn test_keyboard_alt_release_clears() {
        let mut ts = TestSched::new();
        let mut vif = emKeyboardZoomScrollVIF::new();
        let mut state = emInputState::new();
        state.press(InputKey::Alt);
        let (mut tree, mut view) = setup();

        // Press some keys
        let e1 = emInputEvent::press(InputKey::ArrowUp);
        ts.with(|sc| vif.filter(&e1, &state, &mut view, &mut tree, sc));
        let e2 = emInputEvent::press(InputKey::PageUp);
        ts.with(|sc| vif.filter(&e2, &state, &mut view, &mut tree, sc));
        assert!(!vif.key_state.is_empty());

        // Release Alt — should clear all key state
        state.release(InputKey::Alt);
        let alt_release = emInputEvent::release(InputKey::Alt);
        ts.with(|sc| vif.filter(&alt_release, &state, &mut view, &mut tree, sc));
        assert!(vif.key_state.is_empty());
    }

    #[test]
    fn test_touch_single_pan() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        // Zoom in so the pan motion is observable via get_visited_panel_idiom
        // (otherwise root-centering clamps rel_x=0).
        ts.with(|sc| view.Zoom(&mut tree, 4.0, 400.0, 300.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emDefaultTouchVIF::new();
        assert_eq!(vif.state(), TouchState::Idle);

        // Touch start
        ts.with(|sc| assert!(vif.touch_start(1, 100.0, 100.0, &mut view, &mut tree, sc)));
        assert_eq!(vif.state(), TouchState::SingleTouch { id: 1 });
        assert_eq!(vif.active_count(), 1);

        // Touch move past 20px dead zone — gesture machine transitions
        // FirstDown→Scroll and handles scrolling via do_gesture.
        let (_, before, _, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel before touch_move");
        ts.with(|sc| vif.touch_move(1, 130.0, 100.0, 0.016, &mut view, &mut tree, sc));
        let (_, after, _, _) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel after touch_move");
        assert!(
            after != before,
            "Single touch should pan after 20px dead zone"
        );

        // Touch end with low velocity — should go idle
        ts.with(|sc| vif.touch_end(1, &mut view, &mut tree, sc));
        assert_eq!(vif.active_count(), 0);
    }

    #[test]
    fn test_touch_pinch_zoom() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emDefaultTouchVIF::new();

        // Two touches — gesture machine is active (FirstDown→SecondDown),
        // so old PinchZoom state is suppressed.
        ts.with(|sc| vif.touch_start(1, 100.0, 200.0, &mut view, &mut tree, sc));
        ts.with(|sc| vif.touch_start(2, 200.0, 200.0, &mut view, &mut tree, sc));
        // Gesture machine handles two-finger input; old PinchZoom suppressed
        assert!(
            !matches!(vif.state(), TouchState::PinchZoom { .. }),
            "PinchZoom should be suppressed when gesture machine is active"
        );
        assert_eq!(vif.active_count(), 2);
    }

    #[test]
    fn test_touch_fling() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emDefaultTouchVIF::new();
        vif.set_fling_friction(0.95);

        // Rapid drag
        ts.with(|sc| vif.touch_start(1, 100.0, 100.0, &mut view, &mut tree, sc));
        for i in 1..10 {
            ts.with(|sc| {
                vif.touch_move(
                    1,
                    100.0 + i as f64 * 50.0,
                    100.0,
                    0.016,
                    &mut view,
                    &mut tree,
                    sc,
                )
            });
        }
        ts.with(|sc| vif.touch_end(1, &mut view, &mut tree, sc));
        assert_eq!(vif.state(), TouchState::Fling);

        // Animate fling until stopped
        let mut frames = 0;
        while ts.with(|sc| vif.animate_fling(&mut view, &mut tree, 0.016, sc)) {
            frames += 1;
            if frames > 1000 {
                break;
            }
        }
        assert_eq!(vif.state(), TouchState::Idle);
    }

    #[test]
    fn test_touch_fling_cancel_on_press() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emDefaultTouchVIF::new();

        // Create fling
        ts.with(|sc| vif.touch_start(1, 100.0, 100.0, &mut view, &mut tree, sc));
        for i in 1..10 {
            ts.with(|sc| {
                vif.touch_move(
                    1,
                    100.0 + i as f64 * 50.0,
                    100.0,
                    0.016,
                    &mut view,
                    &mut tree,
                    sc,
                )
            });
        }
        ts.with(|sc| vif.touch_end(1, &mut view, &mut tree, sc));
        assert_eq!(vif.state(), TouchState::Fling);

        // Press any key cancels fling
        let state = emInputState::new();
        let event = emInputEvent::press(InputKey::Escape);
        assert!(ts.with(|sc| vif.filter(&event, &state, &mut view, &mut tree, sc)));
        assert_eq!(vif.state(), TouchState::Idle);
    }

    #[test]
    fn test_touch_pinch_to_single() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emDefaultTouchVIF::new();

        // Two touches — gesture machine is active, PinchZoom suppressed
        ts.with(|sc| vif.touch_start(1, 100.0, 200.0, &mut view, &mut tree, sc));
        ts.with(|sc| vif.touch_start(2, 200.0, 200.0, &mut view, &mut tree, sc));
        assert!(
            !matches!(vif.state(), TouchState::PinchZoom { .. }),
            "PinchZoom should be suppressed"
        );

        // Lift one finger — stays in SingleTouch (entered at first touch)
        // since PinchZoom was never entered
        ts.with(|sc| vif.touch_end(1, &mut view, &mut tree, sc));
        assert_eq!(vif.active_count(), 1);
    }

    #[test]
    fn test_speeding_step_function() {
        // Mode 2: accelerating toward target (|v| < |target|)
        let v = speeding_step(0.0, 100.0, 200.0, 400.0, 200.0, false, 0.1);
        assert!((v - 20.0).abs() < 0.01); // 200 * min(0.1, 0.1) = 20

        // Mode 1: reverse direction (v * target < -0.1)
        let v2 = speeding_step(50.0, -50.0, 200.0, 400.0, 200.0, false, 0.1);
        assert!((v2 - 10.0).abs() < 0.01); // 50 - 400*0.1 = 10

        // Already at target
        let v3 = speeding_step(100.0, 100.0, 200.0, 400.0, 200.0, false, 0.1);
        assert!((v3 - 100.0).abs() < 0.01);

        // Mode 3: friction enabled, |v| >= |target|
        let v4 = speeding_step(100.0, 0.0, 200.0, 400.0, 200.0, true, 0.1);
        assert!((v4 - 80.0).abs() < 0.01); // 100 - 200*0.1 = 80
    }

    #[test]
    fn test_grip_not_animating_by_default() {
        let vif = emMouseZoomScrollVIF::new();
        assert!(!vif.is_grip_animating());
    }

    #[test]
    fn test_grip_kinetic_coasting_after_drag() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        // Zoom in so the panel is larger than the viewport and scroll isn't clamped.
        ts.with(|sc| view.Zoom(&mut tree, 4.0, 400.0, 300.0, sc));

        let mut vif = emMouseZoomScrollVIF::new();
        // Enable kinetic behavior with realistic parameters
        vif.set_mouse_anim_params(1.0, 0.5, 0.01);

        let mut state = emInputState::new();

        // Press middle button — starts gripped phase
        let press = emInputEvent::press(InputKey::MouseMiddle);
        state.mouse_x = 100.0;
        state.mouse_y = 100.0;
        assert!(ts.with(|sc| vif.filter(&press, &state, &mut view, &mut tree, sc)));
        assert!(
            vif.is_grip_animating(),
            "Should be animating during grip phase"
        );

        // Simulate rapid drag: several move events + animate_grip ticks.
        // Use small (1px) per-frame moves so scroll doesn't hit the panel boundary.
        for i in 1..=10 {
            state.mouse_x = 100.0 + i as f64 * 1.0;
            let move_event = emInputEvent {
                key: InputKey::MouseMiddle,
                variant: InputVariant::Move,
                chars: String::new(),
                repeat: 0,
                source_variant: 0,
                mouse_x: state.mouse_x,
                mouse_y: state.mouse_y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
                eaten: false,
            };
            ts.with(|sc| vif.filter(&move_event, &state, &mut view, &mut tree, sc));
            // Tick spring physics between move events
            ts.with(|sc| vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0, sc));
        }

        // Release — should transition to coasting
        let release = emInputEvent::release(InputKey::MouseMiddle);
        assert!(ts.with(|sc| vif.filter(&release, &state, &mut view, &mut tree, sc)));
        assert!(
            vif.is_grip_animating(),
            "Should be coasting after kinetic drag"
        );

        // Animate coasting until it stops
        let mut frames = 0;
        while ts.with(|sc| vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0, sc)) {
            frames += 1;
            if frames > 10_000 {
                panic!("Coasting did not stop within 10000 frames");
            }
        }
        assert!(!vif.is_grip_animating());
        assert!(frames > 0, "Should have coasted for at least one frame");
    }

    #[test]
    fn test_grip_no_coasting_when_kinetic_disabled() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emMouseZoomScrollVIF::new();
        // Explicitly disable kinetic (k=0 → friction_enabled=false)
        vif.set_mouse_anim_params(0.0, 0.25, 0.01);

        let mut state = emInputState::new();

        // Press middle button
        let press = emInputEvent::press(InputKey::MouseMiddle);
        state.mouse_x = 100.0;
        state.mouse_y = 100.0;
        ts.with(|sc| vif.filter(&press, &state, &mut view, &mut tree, sc));

        // Drag with animation ticks
        for i in 1..=10 {
            state.mouse_x = 100.0 + i as f64 * 20.0;
            let move_event = emInputEvent {
                key: InputKey::MouseMiddle,
                variant: InputVariant::Move,
                chars: String::new(),
                repeat: 0,
                source_variant: 0,
                mouse_x: state.mouse_x,
                mouse_y: state.mouse_y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
                eaten: false,
            };
            ts.with(|sc| vif.filter(&move_event, &state, &mut view, &mut tree, sc));
            ts.with(|sc| vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0, sc));
        }

        // Release — should NOT coast when kinetic is disabled
        let release = emInputEvent::release(InputKey::MouseMiddle);
        ts.with(|sc| vif.filter(&release, &state, &mut view, &mut tree, sc));
        assert!(
            !vif.is_grip_animating(),
            "Should not coast when kinetic is disabled"
        );
    }

    #[test]
    fn test_grip_press_cancels_coasting() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut vif = emMouseZoomScrollVIF::new();
        vif.set_mouse_anim_params(1.0, 0.5, 0.01);

        let mut state = emInputState::new();

        // Start coasting via drag+release
        let press = emInputEvent::press(InputKey::MouseMiddle);
        state.mouse_x = 100.0;
        state.mouse_y = 100.0;
        ts.with(|sc| vif.filter(&press, &state, &mut view, &mut tree, sc));

        for i in 1..=10 {
            state.mouse_x = 100.0 + i as f64 * 20.0;
            let move_event = emInputEvent {
                key: InputKey::MouseMiddle,
                variant: InputVariant::Move,
                chars: String::new(),
                repeat: 0,
                source_variant: 0,
                mouse_x: state.mouse_x,
                mouse_y: state.mouse_y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
                eaten: false,
            };
            ts.with(|sc| vif.filter(&move_event, &state, &mut view, &mut tree, sc));
            ts.with(|sc| vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0, sc));
        }

        let release = emInputEvent::release(InputKey::MouseMiddle);
        ts.with(|sc| vif.filter(&release, &state, &mut view, &mut tree, sc));
        assert!(vif.is_grip_animating(), "Should be coasting");

        // New press resets to gripped phase (still animating, but gripped)
        let press2 = emInputEvent::press(InputKey::MouseMiddle);
        ts.with(|sc| vif.filter(&press2, &state, &mut view, &mut tree, sc));
        // Now in gripped phase — animation is active for spring, velocity zeroed
        assert!(
            vif.is_grip_animating(),
            "New grip starts fresh grip animation"
        );
    }

    #[test]
    fn test_grip_animate_returns_false_when_inactive() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emMouseZoomScrollVIF::new();
        assert!(!ts.with(|sc| vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0, sc)));
    }

    #[test]
    fn test_vif_animate_trait_delegates_wheel() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emMouseZoomScrollVIF::new();
        let state = emInputState::new();

        // Feed a wheel event to activate wheel spring
        let event = emInputEvent::press(InputKey::WheelUp);
        let consumed = ts.with(|sc| vif.filter(&event, &state, &mut view, &mut tree, sc));
        assert!(consumed);

        // Call animate via the trait — should return true (animation active)
        let active = ts
            .with(|sc| emViewInputFilter::animate(&mut vif, &mut view, &mut tree, 1.0 / 60.0, sc));
        assert!(active, "animate() should return true when wheel is active");
    }

    #[test]
    fn test_vif_animate_returns_false_when_idle() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emMouseZoomScrollVIF::new();

        // No events fed — animate should return false
        let active = ts
            .with(|sc| emViewInputFilter::animate(&mut vif, &mut view, &mut tree, 1.0 / 60.0, sc));
        assert!(!active, "animate() should return false when idle");
    }

    /// Helper: create a key event with characters for cheat code testing.
    fn cheat_key_event(chars: &str) -> emInputEvent {
        let mut event = emInputEvent::press(InputKey::Key('a'));
        event.chars = chars.to_string();
        event
    }

    /// Helper: type a sequence of characters into a emCheatVIF one char at a time.
    fn type_cheat(vif: &mut emCheatVIF, view: &mut emView, tree: &mut PanelTree, text: &str) {
        let mut ts = TestSched::new();
        let state = emInputState::new();
        for ch in text.chars() {
            let event = cheat_key_event(&ch.to_string());
            ts.with(|sc| vif.filter(&event, &state, view, tree, sc));
        }
    }

    #[test]
    fn cheat_vif_pan_toggle() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        // Type "chEat:pan!" — should produce TogglePanFunction action
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:pan!");

        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::PanFunction]);

        // Typing it again should produce another action
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:pan!");
        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::PanFunction]);
    }

    #[test]
    fn cheat_vif_easy_mode() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        // Without easy cheats, ":pan!" alone should not work
        type_cheat(&mut vif, &mut view, &mut tree, ":pan!");
        let actions = vif.drain_actions();
        assert!(actions.is_empty());

        // Enable easy cheats
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:easy!");

        // Now ":pan!" should work without "chEat" prefix
        type_cheat(&mut vif, &mut view, &mut tree, ":pan!");
        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::PanFunction]);
    }

    #[test]
    fn cheat_vif_escape_cancels() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();
        let state = emInputState::new();

        // Start typing a cheat code, then insert a non-char event (e.g. escape)
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:pa");

        // An event with no chars (e.g. Escape press) doesn't affect the buffer
        let escape_event = emInputEvent::press(InputKey::Escape);
        ts.with(|sc| vif.filter(&escape_event, &state, &mut view, &mut tree, sc));

        // Continue typing — the buffer still has the previous chars
        type_cheat(&mut vif, &mut view, &mut tree, "n!");
        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::PanFunction]);
    }

    #[test]
    fn cheat_vif_unknown_command_ignored() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        // Enable easy cheats for simpler testing
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:easy!");

        // Unknown command produces no actions
        type_cheat(&mut vif, &mut view, &mut tree, ":bogus!");
        let actions = vif.drain_actions();
        assert!(actions.is_empty());
    }

    #[test]
    fn cheat_vif_view_flags_toggle() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        // Enable easy cheats
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:easy!");

        // Toggle popup zoom
        assert!(!view.flags.contains(ViewFlags::POPUP_ZOOM));
        type_cheat(&mut vif, &mut view, &mut tree, ":pz!");
        assert!(view.flags.contains(ViewFlags::POPUP_ZOOM));
        type_cheat(&mut vif, &mut view, &mut tree, ":pz!");
        assert!(!view.flags.contains(ViewFlags::POPUP_ZOOM));

        // Toggle stress test
        assert!(!view.flags.contains(ViewFlags::STRESS_TEST));
        type_cheat(&mut vif, &mut view, &mut tree, ":st!");
        assert!(view.flags.contains(ViewFlags::STRESS_TEST));

        // Toggle ego mode
        assert!(!view.flags.contains(ViewFlags::EGO_MODE));
        type_cheat(&mut vif, &mut view, &mut tree, ":egomode!");
        assert!(view.flags.contains(ViewFlags::EGO_MODE));
    }

    #[test]
    fn cheat_vif_no_user_navigation_skips() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        // Set NO_USER_NAVIGATION — cheat codes should be skipped
        view.flags |= ViewFlags::NO_USER_NAVIGATION;
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:easy!");

        // easy should not have been activated
        type_cheat(&mut vif, &mut view, &mut tree, ":pan!");
        let actions = vif.drain_actions();
        assert!(actions.is_empty());
    }

    #[test]
    fn cheat_vif_emb_toggle() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        type_cheat(&mut vif, &mut view, &mut tree, "chEat:emb!");
        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::EmulateMiddleButton]);
    }

    #[test]
    fn cheat_vif_egomode_toggle() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        assert!(!view.flags.contains(ViewFlags::EGO_MODE));
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:egomode!");
        assert!(view.flags.contains(ViewFlags::EGO_MODE));
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:egomode!");
        assert!(!view.flags.contains(ViewFlags::EGO_MODE));
    }

    #[test]
    fn cheat_vif_stresstest_toggle() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        assert!(!view.flags.contains(ViewFlags::STRESS_TEST));
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:st!");
        assert!(view.flags.contains(ViewFlags::STRESS_TEST));
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:st!");
        assert!(!view.flags.contains(ViewFlags::STRESS_TEST));
    }

    #[test]
    fn cheat_vif_dlog_toggle() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        // Ensure dlog starts disabled
        crate::emStd1::emEnableDLog(false);
        assert!(!crate::emStd1::emIsDLogEnabled());

        // Toggle dlog on via cheat
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:dlog!");
        assert!(crate::emStd1::emIsDLogEnabled());

        // Toggle dlog off (need full prefix since easy cheats not enabled)
        type_cheat(&mut vif, &mut view, &mut tree, "chEat:dlog!");
        assert!(!crate::emStd1::emIsDLogEnabled());
    }

    #[test]
    fn dlog_integration_captures_call_site() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let root = view.GetRootPanel();

        // Enable dlog and start capturing
        crate::emStd1::emEnableDLog(true);
        crate::emStd1::start_capture();

        // Trigger a known dlog call site: set_active_panel logs
        // "active panel changed to ..."
        let child = tree.create_child(root, "dlog_test_child", None);
        tree.set_focusable(child, true);
        tree.Layout(child, 0.0, 0.0, 0.5, 1.0, 1.0, None);
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut h = crate::test_view_harness::TestViewHarness::new();
        view.set_active_panel(&mut tree, child, false, &mut h.sched_ctx());

        let lines = crate::emStd1::stop_capture();
        crate::emStd1::emEnableDLog(false);

        // Verify captured output contains the expected call site message
        assert!(
            lines.iter().any(|l| l.contains("active panel changed")),
            "dlog should capture 'active panel changed' from set_active_panel call site, got: {:?}",
            lines
        );
    }

    #[test]
    fn cheat_vif_smwn_toggle() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        type_cheat(&mut vif, &mut view, &mut tree, "chEat:smwn!");
        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::StickMouseWhenNavigating]);
    }

    #[test]
    fn cheat_vif_td_triggers_dump() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        type_cheat(&mut vif, &mut view, &mut tree, "chEat:td!");
        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::TreeDump]);
    }

    #[test]
    fn cheat_vif_ss_triggers_screenshot() {
        let (mut tree, mut view) = setup();
        let mut vif = emCheatVIF::new();

        type_cheat(&mut vif, &mut view, &mut tree, "chEat:ss!");
        let actions = vif.drain_actions();
        assert_eq!(actions, vec![CheatAction::Screenshot]);
    }

    #[test]
    fn touch_tracker_move_calculations() {
        let mut tracker = TouchTracker::new();

        // Add a touch at (100, 200)
        tracker.touches[0] = Touch {
            id: 1,
            ms_total: 0,
            ms_since_prev: 0,
            down: true,
            x: 100.0,
            y: 200.0,
            prev_down: false,
            prev_x: 100.0,
            prev_y: 200.0,
            down_x: 100.0,
            down_y: 200.0,
        };
        tracker.touch_count = 1;

        // Advance frame
        tracker.next_touches(16);

        // Move to (120, 230)
        tracker.touches[0].x = 120.0;
        tracker.touches[0].y = 230.0;

        assert!((tracker.get_touch_move_x(0) - 20.0).abs() < 1e-12);
        assert!((tracker.get_touch_move_y(0) - 30.0).abs() < 1e-12);
        assert!((tracker.get_total_touch_move_x(0) - 20.0).abs() < 1e-12);
        assert!((tracker.get_total_touch_move_y(0) - 30.0).abs() < 1e-12);
    }

    #[test]
    fn three_finger_release_injects_menu_key() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = TouchTracker::new();

        // Three fingers down
        for i in 0..3u64 {
            tracker.touches[i as usize] = Touch {
                id: i + 1,
                down: true,
                x: 100.0 + i as f64 * 50.0,
                y: 200.0,
                down_x: 100.0 + i as f64 * 50.0,
                down_y: 200.0,
                ..Touch::default()
            };
        }
        tracker.touch_count = 3;
        tracker.gesture_state = GestureState::ThirdDown;

        // Release all fingers
        for i in 0..3 {
            tracker.touches[i].down = false;
        }
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}

        assert!(
            tracker
                .pending_actions
                .contains(&GestureAction::InjectMenuKey),
            "three-finger release should inject Menu key"
        );
    }

    #[test]
    fn four_finger_release_toggles_soft_keyboard() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = TouchTracker::new();

        // Four fingers down
        for i in 0..4u64 {
            tracker.touches[i as usize] = Touch {
                id: i + 1,
                down: true,
                x: 100.0 + i as f64 * 50.0,
                y: 200.0,
                down_x: 100.0 + i as f64 * 50.0,
                down_y: 200.0,
                ..Touch::default()
            };
        }
        tracker.touch_count = 4;
        tracker.gesture_state = GestureState::FourthDown;

        // Release all
        for i in 0..4 {
            tracker.touches[i].down = false;
        }
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}

        assert!(
            tracker
                .pending_actions
                .contains(&GestureAction::ToggleSoftKeyboard),
            "four-finger release should toggle soft keyboard"
        );
    }

    #[test]
    fn gesture_finish_returns_to_ready() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = TouchTracker::new();

        // Set up Finish state with no touches down
        tracker.gesture_state = GestureState::Finish;
        tracker.touch_count = 0;
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}

        assert_eq!(
            tracker.gesture_state,
            GestureState::Ready,
            "Finish should return to Ready when all fingers lifted"
        );
    }

    fn setup_two_finger_tracker(x0: f64, y0: f64, x1: f64, y1: f64) -> TouchTracker {
        let mut tracker = TouchTracker::new();
        tracker.touches[0] = Touch {
            id: 1,
            down: true,
            x: x0,
            y: y0,
            down_x: x0,
            down_y: y0,
            ms_total: 260,
            ..Touch::default()
        };
        tracker.touches[1] = Touch {
            id: 2,
            down: true,
            x: x1,
            y: y1,
            down_x: x1,
            down_y: y1,
            ms_total: 260,
            ..Touch::default()
        };
        tracker.touch_count = 2;
        tracker.gesture_state = GestureState::SecondDown;
        tracker
    }

    #[test]
    fn two_finger_horizontal_right_emu_mouse_1() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = setup_two_finger_tracker(100.0, 200.0, 200.0, 200.0);
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::EmuMouse1);
    }

    #[test]
    fn two_finger_horizontal_left_emu_mouse_2() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = setup_two_finger_tracker(200.0, 200.0, 100.0, 200.0);
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::EmuMouse2);
    }

    #[test]
    fn two_finger_vertical_down_emu_mouse_3() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = setup_two_finger_tracker(200.0, 100.0, 200.0, 200.0);
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::EmuMouse3);
    }

    #[test]
    fn two_finger_vertical_up_emu_mouse_4() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = setup_two_finger_tracker(200.0, 200.0, 200.0, 100.0);
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::EmuMouse4);
    }

    #[test]
    fn gesture_double_tap_visits_fullsized() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let root = view.GetRootPanel();
        tree.set_focusable(root, true);
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = TouchTracker::new();

        // First down
        tracker.touches[0] = Touch {
            id: 1,
            down: true,
            x: 400.0,
            y: 300.0,
            down_x: 400.0,
            down_y: 300.0,
            ..Touch::default()
        };
        tracker.touch_count = 1;
        tracker.gesture_state = GestureState::FirstDown;

        // First up (within 250ms)
        tracker.touches[0].down = false;
        tracker.touches[0].ms_total = 100;
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::FirstDownUp);

        // Second down (new touch)
        tracker.touches[1] = Touch {
            id: 2,
            down: true,
            x: 400.0,
            y: 300.0,
            down_x: 400.0,
            down_y: 300.0,
            ..Touch::default()
        };
        tracker.touch_count = 2;
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::DoubleDown);

        // Second up
        tracker.touches[0].down = false;
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::DoubleDownUp);

        // Wait > 250ms — should trigger visit_fullsized and transition through Finish → Ready
        tracker.touches[0].ms_total = 260;
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        // After the loop settles, gesture completes and returns to Ready
        assert_eq!(tracker.gesture_state, GestureState::Ready);
    }

    #[test]
    fn gesture_single_tap_timeout() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = TouchTracker::new();

        // Touch down
        tracker.touches[0] = Touch {
            id: 1,
            down: true,
            x: 400.0,
            y: 300.0,
            down_x: 400.0,
            down_y: 300.0,
            ..Touch::default()
        };
        tracker.touch_count = 1;
        tracker.gesture_state = GestureState::FirstDown;

        // Touch up quickly
        tracker.touches[0].down = false;
        tracker.touches[0].ms_total = 50;
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        assert_eq!(tracker.gesture_state, GestureState::FirstDownUp);

        // Wait > 250ms — single tap timeout → Finish → Ready
        tracker.touches[0].ms_total = 260;
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}
        // After loop settles, should be back at Ready (timeout, no action taken)
        assert_eq!(tracker.gesture_state, GestureState::Ready);
    }

    #[test]
    fn gesture_scroll_on_drag_over_20px() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = TouchTracker::new();

        // Simulate touch down
        tracker.touches[0] = Touch {
            id: 1,
            down: true,
            x: 100.0,
            y: 100.0,
            down_x: 100.0,
            down_y: 100.0,
            ..Touch::default()
        };
        tracker.touch_count = 1;
        tracker.gesture_state = GestureState::FirstDown;

        // Drag > 20px
        tracker.next_touches(16);
        tracker.touches[0].x = 130.0; // 30px total move

        // Run gesture until stable
        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}

        assert_eq!(
            tracker.gesture_state,
            GestureState::Scroll,
            "should enter Scroll state on drag > 20px"
        );
    }

    #[test]
    fn gesture_zoom_in_on_hold_over_250ms() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let mut tracker = TouchTracker::new();

        // Simulate touch down held for > 250ms
        tracker.touches[0] = Touch {
            id: 1,
            down: true,
            x: 100.0,
            y: 100.0,
            ms_total: 260,
            down_x: 100.0,
            down_y: 100.0,
            ..Touch::default()
        };
        tracker.touch_count = 1;
        tracker.gesture_state = GestureState::FirstDown;

        while ts.with(|sc| tracker.do_gesture(&mut view, &mut tree, sc)) {}

        assert_eq!(
            tracker.gesture_state,
            GestureState::ZoomIn,
            "should enter ZoomIn state on hold > 250ms"
        );
    }

    #[test]
    fn touch_tracker_remove_shifts() {
        let mut tracker = TouchTracker::new();

        for i in 0..3u64 {
            tracker.touches[i as usize] = Touch {
                id: i + 1,
                down: true,
                x: (i as f64) * 10.0,
                ..Touch::default()
            };
        }
        tracker.touch_count = 3;

        // Remove middle touch (index 1, id=2)
        tracker.RemoveTouch(1);
        assert_eq!(tracker.touch_count, 2);
        assert_eq!(tracker.touches[0].id, 1);
        assert_eq!(tracker.touches[1].id, 3);
    }

    fn input_state_at(x: f64, y: f64) -> emInputState {
        let mut s = emInputState::new();
        s.mouse_x = x;
        s.mouse_y = y;
        s
    }

    #[test]
    fn stick_mouse_accumulates_warp_during_drag() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emMouseZoomScrollVIF::new();
        vif.set_stick_mouse(true);

        let state_press = input_state_at(100.0, 100.0);

        // Start middle-button press to initiate panning
        let press = emInputEvent::press(InputKey::MouseMiddle).with_mouse(100.0, 100.0);
        ts.with(|sc| vif.filter(&press, &state_press, &mut view, &mut tree, sc));
        assert!(vif.panning);

        // Move mouse (simulating drag)
        let state_move = input_state_at(120.0, 110.0);
        let move_ev = emInputEvent::mouse_move(InputKey::MouseMiddle, 120.0, 110.0);
        ts.with(|sc| vif.filter(&move_ev, &state_move, &mut view, &mut tree, sc));

        // Pending warp should have accumulated (-dmx, -dmy) = (-20, -10)
        let (wx, wy) = vif.drain_pending_warp();
        assert!(
            (wx - (-20.0)).abs() < 0.01 && (wy - (-10.0)).abs() < 0.01,
            "pending_warp should be (-20, -10), got ({}, {})",
            wx,
            wy
        );

        // After drain, should be zero
        let (wx2, wy2) = vif.drain_pending_warp();
        assert!(wx2.abs() < 0.01 && wy2.abs() < 0.01);
    }

    #[test]
    fn stick_mouse_no_warp_when_disabled() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emMouseZoomScrollVIF::new();
        // stick_mouse defaults to false

        let state_press = input_state_at(100.0, 100.0);
        let press = emInputEvent::press(InputKey::MouseMiddle).with_mouse(100.0, 100.0);
        ts.with(|sc| vif.filter(&press, &state_press, &mut view, &mut tree, sc));

        let state_move = input_state_at(120.0, 110.0);
        let move_ev = emInputEvent::mouse_move(InputKey::MouseMiddle, 120.0, 110.0);
        ts.with(|sc| vif.filter(&move_ev, &state_move, &mut view, &mut tree, sc));

        let (wx, wy) = vif.drain_pending_warp();
        assert!(
            wx.abs() < 0.01 && wy.abs() < 0.01,
            "no warp when stick_mouse disabled"
        );
    }

    #[test]
    fn test_magnetism_avoidance_basic() {
        let mut vif = emMouseZoomScrollVIF::new();

        // After init, magnetism avoidance is false
        vif.init_magnetism_avoidance(1000);
        assert!(!vif.magnetism_avoidance());

        // Small mouse movement, under 750ms — no avoidance
        vif.update_magnetism_avoidance(0.5, 0.5, 1100);
        assert!(!vif.magnetism_avoidance());

        // Still under 750ms hold time
        vif.update_magnetism_avoidance(0.0, 0.0, 1600);
        assert!(!vif.magnetism_avoidance());

        // After 750ms of holding still, avoidance activates
        vif.update_magnetism_avoidance(0.0, 0.0, 1851);
        assert!(vif.magnetism_avoidance());
    }

    #[test]
    fn test_magnetism_avoidance_reset_on_large_move() {
        let mut vif = emMouseZoomScrollVIF::new();
        vif.init_magnetism_avoidance(1000);

        // Large movement (> 2.0 px) resets the timer
        vif.update_magnetism_avoidance(3.0, 0.0, 1600);
        assert!(!vif.magnetism_avoidance());

        // 750ms from original init would be 1750, but timer was reset at 1600
        vif.update_magnetism_avoidance(0.0, 0.0, 1750);
        assert!(!vif.magnetism_avoidance());

        // 750ms from reset point (1600) = 2350
        vif.update_magnetism_avoidance(0.0, 0.0, 2350);
        assert!(vif.magnetism_avoidance());
    }

    #[test]
    fn test_magnetism_avoidance_wired_into_filter() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        let mut vif = emMouseZoomScrollVIF::new();
        vif.set_test_clock(1000);

        // Start grip — inits magnetism avoidance
        let press = emInputEvent::press(InputKey::MouseMiddle);
        let state = emInputState::new();
        ts.with(|sc| vif.filter(&press, &state, &mut view, &mut tree, sc));
        assert!(vif.panning);
        assert!(!vif.magnetism_avoidance());

        // Move mouse a tiny bit at 1100ms
        let mut move_state = emInputState::new();
        move_state.mouse_x = 1.0;
        move_state.mouse_y = 0.0;
        move_state.press(InputKey::MouseMiddle);
        let move_event = emInputEvent::mouse_move(InputKey::MouseLeft, 1.0, 0.0);
        vif.set_test_clock(1100);
        ts.with(|sc| vif.filter(&move_event, &move_state, &mut view, &mut tree, sc));
        assert!(!vif.magnetism_avoidance());

        // Hold still for 750ms
        vif.set_test_clock(1851);
        ts.with(|sc| vif.filter(&move_event, &move_state, &mut view, &mut tree, sc));
        assert!(vif.magnetism_avoidance());
    }
}

#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_speeding_step() {
        let mut p_v: f64 = kani::any::<f64>();
        kani::assume(p_v.is_finite());
        let mut p_target: f64 = kani::any::<f64>();
        kani::assume(p_target.is_finite());
        let mut p_accel: f64 = kani::any::<f64>();
        kani::assume(p_accel.is_finite());
        let mut p_reverse_accel: f64 = kani::any::<f64>();
        kani::assume(p_reverse_accel.is_finite());
        let mut p_friction: f64 = kani::any::<f64>();
        kani::assume(p_friction.is_finite());
        let mut p_friction_enabled: bool = kani::any::<bool>();
        let mut p_dt: f64 = kani::any::<f64>();
        kani::assume(p_dt.is_finite());
        let _r = speeding_step(
            p_v,
            p_target,
            p_accel,
            p_reverse_accel,
            p_friction,
            p_friction_enabled,
            p_dt,
        );
        assert!(_r.is_finite());
    }
}
