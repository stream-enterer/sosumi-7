use crate::input::{InputEvent, InputKey, InputState, InputVariant};

use super::view::{View, ViewFlags};

/// Trait for view input filters that intercept input before it reaches panels.
pub trait ViewInputFilter {
    /// Process an input event. Returns true if the event was consumed.
    fn filter(&mut self, event: &InputEvent, state: &InputState, view: &mut View) -> bool;
}

/// Mouse wheel zoom and middle-button pan filter.
pub struct MouseZoomScrollVIF {
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
    /// Spring constant for the wheel swiping animator.
    wheel_spring_const: f64,
    /// Friction for the wheel swiping animator.
    wheel_friction: f64,
    /// Whether kinetic wheel behavior is enabled.
    wheel_friction_enabled: bool,
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
    /// Whether the grip animation is active (grip or coast phase).
    grip_active: bool,
    /// Zoom fix point for grip-drag operations.
    grip_fix_x: f64,
    grip_fix_y: f64,
}

impl MouseZoomScrollVIF {
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
            wheel_spring_const: 0.0,
            wheel_friction: 0.0,
            wheel_friction_enabled: false,
            grip_velocity_x: 0.0,
            grip_velocity_y: 0.0,
            grip_spring_x: 0.0,
            grip_spring_y: 0.0,
            grip_inst_vel_x: 0.0,
            grip_inst_vel_y: 0.0,
            grip_active: false,
            grip_fix_x: 0.0,
            grip_fix_y: 0.0,
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

    /// Translate Alt key presses into emulated middle mouse button events.
    ///
    /// Mirrors C++ `emMouseZoomScrollVIF::EmulateMiddleButton`.
    /// When emulation is enabled and the real middle button is not pressed,
    /// an Alt key press generates a synthetic middle-button event. Tracks
    /// timing for double/triple click emulation (330ms threshold).
    ///
    /// Returns `Some(synthetic_event)` if an emulated middle-button press
    /// should be generated, or `None` if no emulation occurred. The caller
    /// should process the returned event before normal input handling.
    pub fn emulate_middle_button_event(
        &mut self,
        event: &InputEvent,
        state: &InputState,
        clock_ms: u64,
    ) -> Option<InputEvent> {
        if !self.emulate_middle_button {
            return None;
        }
        // Don't emulate if the real middle button is already held
        if state.is_pressed(InputKey::MouseMiddle) {
            return None;
        }

        if event.key == InputKey::Alt && event.variant == InputVariant::Press && !event.is_repeat {
            // Compute repeat from timing
            let d = clock_ms.saturating_sub(self.emu_mid_button_time);
            if d < 330 {
                self.emu_mid_button_repeat += 1;
            } else {
                self.emu_mid_button_repeat = 0;
            }
            self.emu_mid_button_time = clock_ms;

            // Synthesize a middle button press event
            let mut synthetic = InputEvent::press(InputKey::MouseMiddle);
            synthetic.is_repeat = self.emu_mid_button_repeat > 0;
            synthetic.mouse_x = event.mouse_x;
            synthetic.mouse_y = event.mouse_y;
            return Some(synthetic);
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
            let f1: f64 = 2.2;
            let f2: f64 = 0.4;

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
    /// `zoom_factor_log_per_pixel` is from `View::get_zoom_factor_log_per_pixel`.
    pub fn set_mouse_anim_params(
        &mut self,
        kinetic_factor: f64,
        min_kinetic: f64,
        zoom_factor_log_per_pixel: f64,
    ) {
        let mut k = kinetic_factor;
        if k < min_kinetic * 1.0001 {
            k = 0.001;
        }
        let zflpp = zoom_factor_log_per_pixel.max(1e-10);
        self.mouse_spring_const = 2500.0 / (k * k);
        self.mouse_friction = 2.0 / zflpp / (k * k);
        self.mouse_friction_enabled = k > 0.001;
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
        let mut k = kinetic_factor;
        if k < min_kinetic * 1.0001 {
            k = 0.001;
        }
        let zflpp = zoom_factor_log_per_pixel.max(1e-10);
        self.wheel_spring_const = 480.0 / (k * k);
        self.wheel_friction = 2.0 / zflpp / (k * k);
        self.wheel_friction_enabled = k > 0.001;
    }

    /// Returns the wheel animator parameters (spring_const, friction, friction_enabled).
    pub fn wheel_anim_params(&self) -> (f64, f64, bool) {
        (
            self.wheel_spring_const,
            self.wheel_friction,
            self.wheel_friction_enabled,
        )
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
        view: &mut View,
        tree: &mut super::tree::PanelTree,
        dt: f64,
    ) -> bool {
        if !self.grip_active {
            return false;
        }

        if self.panning {
            // ── Gripped phase: critically-damped spring (C++ SwipingViewAnimator) ──
            //
            // Spring equation: e'' + 2ω·e' + ω²·e = 0  (critically damped)
            // Analytical solution: e(t) = (e₀ + (e₀·ω + v₀)·t) · exp(-ω·t)
            //                     v(t) = (v₀ - (e₀·ω + v₀)·ω·t) · exp(-ω·t)
            // where ω = √(spring_constant)
            let w = self.mouse_spring_const.sqrt();
            let decay = (-w * dt).exp();

            // Process X spring
            let e0x = self.grip_spring_x;
            let v0x = self.grip_inst_vel_x;
            let e1x = (e0x + (e0x * w + v0x) * dt) * decay;
            let v1x = (v0x - (e0x * w + v0x) * w * dt) * decay;
            self.grip_spring_x = e1x;
            self.grip_inst_vel_x = v1x;
            // Output velocity = spring displacement per dt
            self.grip_velocity_x = (e0x - e1x) / dt;

            // Process Y spring
            let e0y = self.grip_spring_y;
            let v0y = self.grip_inst_vel_y;
            let e1y = (e0y + (e0y * w + v0y) * dt) * decay;
            let v1y = (v0y - (e0y * w + v0y) * w * dt) * decay;
            self.grip_spring_y = e1y;
            self.grip_inst_vel_y = v1y;
            self.grip_velocity_y = (e0y - e1y) / dt;

            // Apply velocity as scroll (without friction during grip, per C++)
            let dx = self.grip_velocity_x * dt;
            let dy = self.grip_velocity_y * dt;
            if dx.abs() > 0.01 || dy.abs() > 0.01 {
                view.raw_scroll_and_zoom(tree, self.grip_fix_x, self.grip_fix_y, dx, dy, 0.0);
            }
        } else {
            // ── Coasting phase: linear friction (C++ KineticViewAnimator) ──
            let v = (self.grip_velocity_x * self.grip_velocity_x
                + self.grip_velocity_y * self.grip_velocity_y)
                .sqrt();
            let f = if self.mouse_friction_enabled && v > 1e-10 {
                let new_v = (v - self.mouse_friction * dt).max(0.0);
                new_v / v
            } else {
                1.0
            };

            let v0x = self.grip_velocity_x;
            let v0y = self.grip_velocity_y;
            self.grip_velocity_x *= f;
            self.grip_velocity_y *= f;

            // Average velocity over the tick for smooth integration
            let dx = (v0x + self.grip_velocity_x) * 0.5 * dt;
            let dy = (v0y + self.grip_velocity_y) * 0.5 * dt;

            if dx.abs() >= 0.01 || dy.abs() >= 0.01 {
                let done =
                    view.raw_scroll_and_zoom(tree, self.grip_fix_x, self.grip_fix_y, dx, dy, 0.0);
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
                + self.grip_velocity_y * self.grip_velocity_y;
            if speed_sq < 1.0 {
                self.grip_velocity_x = 0.0;
                self.grip_velocity_y = 0.0;
                self.grip_active = false;
                return false;
            }
        }

        true
    }
}

impl Default for MouseZoomScrollVIF {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewInputFilter for MouseZoomScrollVIF {
    fn filter(&mut self, event: &InputEvent, state: &InputState, view: &mut View) -> bool {
        if view.flags.contains(ViewFlags::NO_USER_NAVIGATION) {
            return false;
        }

        // D-PANEL-13: Abort drag on window focus loss (C++ parity)
        if !view.window_focused() {
            self.panning = false;
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
                    // Reset spring and velocity on new grip
                    self.grip_spring_x = 0.0;
                    self.grip_spring_y = 0.0;
                    self.grip_inst_vel_x = 0.0;
                    self.grip_inst_vel_y = 0.0;
                    self.grip_velocity_x = 0.0;
                    self.grip_velocity_y = 0.0;
                    self.grip_active = true; // Activate animation (gripped phase)
                    return true;
                }
                InputVariant::Release => {
                    self.panning = false;
                    // C++: on release, spring extensions zeroed, velocity transfers
                    // to coasting phase. If velocity is negligible, stop.
                    self.grip_spring_x = 0.0;
                    self.grip_spring_y = 0.0;
                    self.grip_inst_vel_x = self.grip_velocity_x;
                    self.grip_inst_vel_y = self.grip_velocity_y;
                    let speed_sq = self.grip_velocity_x * self.grip_velocity_x
                        + self.grip_velocity_y * self.grip_velocity_y;
                    if !self.mouse_friction_enabled || speed_sq < 1.0 {
                        self.grip_velocity_x = 0.0;
                        self.grip_velocity_y = 0.0;
                        self.grip_active = false;
                    }
                    // grip_active remains true for coasting if velocity is significant
                    return true;
                }
                _ => {}
            }
        }

        // Wheel zoom
        if matches!(event.key, InputKey::WheelUp | InputKey::WheelDown)
            && event.variant == InputVariant::Press
        {
            let down = event.key == InputKey::WheelDown;
            let factor = if down {
                1.0 / self.zoom_speed
            } else {
                self.zoom_speed
            };
            view.zoom(factor, state.mouse_x, state.mouse_y);
            return true;
        }

        // Handle panning/zooming with mouse movement
        if self.panning {
            let dmx = state.mouse_x - self.last_x;
            let dmy = state.mouse_y - self.last_y;
            if dmx.abs() > 0.1 || dmy.abs() > 0.1 {
                // D-PANEL-12: Ctrl+middle vertical drag = zoom (C++ parity)
                if state.ctrl() {
                    let zoom_factor = (1.0 + dmy * 0.005).clamp(0.1, 10.0);
                    view.zoom(zoom_factor, self.grip_fix_x, self.grip_fix_y);
                } else {
                    // D-PANEL-10: Accumulate spring extension (C++ MoveGrip).
                    // The spring physics in animate_grip() convert this into
                    // smoothed velocity and scroll. No direct scroll here.
                    self.grip_spring_x += dmx;
                    self.grip_spring_y += dmy;
                }
                self.last_x = state.mouse_x;
                self.last_y = state.mouse_y;
            }
        }

        false
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
/// scroll/zoom velocity ramps up via acceleration (SpeedingViewAnimator pattern).
/// On release, velocity decelerates to zero.
pub struct KeyboardZoomScrollVIF {
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
}

impl KeyboardZoomScrollVIF {
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
        }
    }

    /// Set the acceleration rate (pixels/second^2).
    pub fn set_acceleration(&mut self, accel: f64) {
        self.acceleration = accel;
    }

    /// Set the deceleration rate (pixels/second^2).
    pub fn set_deceleration(&mut self, decel: f64) {
        self.deceleration = decel;
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
    pub fn animate(&mut self, view: &mut View, dt: f64) {
        // Compute target velocities from held keys
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

        let zoom_log_speed = self.zoom_speed.ln().max(0.1);
        let target_vz = if self.key_state.contains(KeyState::ZOOM_IN) {
            zoom_log_speed
        } else if self.key_state.contains(KeyState::ZOOM_OUT) {
            -zoom_log_speed
        } else {
            0.0
        };

        // Accelerate/decelerate toward target velocity
        self.scroll_velocity_x = approach(
            self.scroll_velocity_x,
            target_vx,
            self.acceleration,
            self.deceleration,
            dt,
        );
        self.scroll_velocity_y = approach(
            self.scroll_velocity_y,
            target_vy,
            self.acceleration,
            self.deceleration,
            dt,
        );
        self.zoom_velocity = approach(
            self.zoom_velocity,
            target_vz,
            self.acceleration * 0.01,
            self.deceleration * 0.01,
            dt,
        );

        // Apply motion
        let dx = self.scroll_velocity_x * dt;
        let dy = self.scroll_velocity_y * dt;
        if dx.abs() > 0.001 || dy.abs() > 0.001 {
            view.scroll(dx, dy);
        }
        if self.zoom_velocity.abs() > 0.001 {
            let factor = (self.zoom_velocity * dt).exp();
            let (vw, vh) = view.viewport_size();
            view.zoom(factor, vw * 0.5, vh * 0.5);
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
        event: &InputEvent,
        state: &InputState,
        view: &mut View,
    ) -> bool {
        const SCROLL_DELTA: f64 = 0.3;
        const ZOOM_FAC: f64 = 1.0015;

        match self.nav_by_prog_state {
            0 => {
                // State 0: wait for Shift+Alt+End
                if event.key == InputKey::End
                    && event.variant == InputVariant::Press
                    && state.shift()
                    && state.alt()
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

                if state.shift() && state.alt() {
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

                if !state.shift() || !state.alt() {
                    return false;
                }

                let (vw, vh) = view.viewport_size();
                let cpt = (vh / vw.max(1.0)).max(0.001);

                match event.key {
                    InputKey::ArrowLeft => {
                        view.scroll(-SCROLL_DELTA * step * vw, 0.0);
                        true
                    }
                    InputKey::ArrowRight => {
                        view.scroll(SCROLL_DELTA * step * vw, 0.0);
                        true
                    }
                    InputKey::ArrowUp => {
                        view.scroll(0.0, -SCROLL_DELTA * step * vh / cpt);
                        true
                    }
                    InputKey::ArrowDown => {
                        view.scroll(0.0, SCROLL_DELTA * step * vh / cpt);
                        true
                    }
                    InputKey::PageUp => {
                        let factor = ZOOM_FAC.powf(step);
                        view.zoom(factor, vw * 0.5, vh * 0.5);
                        true
                    }
                    InputKey::PageDown => {
                        let factor = 1.0 / ZOOM_FAC.powf(step);
                        view.zoom(factor, vw * 0.5, vh * 0.5);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl Default for KeyboardZoomScrollVIF {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewInputFilter for KeyboardZoomScrollVIF {
    fn filter(&mut self, event: &InputEvent, state: &InputState, view: &mut View) -> bool {
        if view.flags.contains(ViewFlags::NO_USER_NAVIGATION) {
            return false;
        }

        // D-PANEL-13: Ignore keyboard input when window not focused (C++ parity)
        if !view.window_focused() {
            return false;
        }

        // Try programmatic navigation first
        if self.navigate_by_program(event, state, view) {
            return true;
        }

        // Track key-down/key-up state for continuous animation
        if state.alt() {
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
                    InputKey::Home => {
                        view.go_home();
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
}

/// Accelerate or decelerate a value toward a target.
///
/// Uses `accel` rate when moving toward the target (speeding up or
/// changing direction), `decel` rate when the target is zero and we're
/// slowing down. Returns the updated value.
fn approach(current: f64, target: f64, accel: f64, decel: f64, dt: f64) -> f64 {
    let diff = target - current;
    if diff.abs() < 0.001 {
        return target;
    }
    let rate = if target.abs() < 0.001 {
        // Decelerating toward zero
        decel
    } else {
        accel
    };
    let step = rate * dt;
    if diff > 0.0 {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

/// State for a tracked touch point.
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
pub struct DefaultTouchVIF {
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
}

impl DefaultTouchVIF {
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
    fn remove_touch(&mut self, id: u64) -> Option<TouchPoint> {
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
    pub fn touch_start(&mut self, id: u64, x: f64, y: f64) -> bool {
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
                // Find the two active touch IDs
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
            _ => {
                // 3+ touches: remain in current state
            }
        }
        true
    }

    /// Handle a touch move event. Applies pan or pinch-zoom to the view.
    /// `dt` is the frame delta in seconds. Returns true if consumed.
    pub fn touch_move(&mut self, id: u64, x: f64, y: f64, dt: f64, view: &mut View) -> bool {
        self.update_touch(id, x, y);

        match self.state {
            TouchState::SingleTouch { id: touch_id } if touch_id == id => {
                if let Some(tp) = self.get_touch(id) {
                    let dx = tp.x - tp.prev_x;
                    let dy = tp.y - tp.prev_y;
                    if dx.abs() > 0.001 || dy.abs() > 0.001 {
                        view.scroll(dx, dy);
                        // Update smoothed velocity for fling
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
                let new_dist = self.pinch_distance(id1, id2);
                if self.last_pinch_distance > 0.1 && new_dist > 0.1 {
                    let factor = new_dist / self.last_pinch_distance;
                    let (cx, cy) = self.pinch_center(id1, id2);
                    view.zoom(factor, cx, cy);
                }
                self.last_pinch_distance = new_dist;
                true
            }
            _ => false,
        }
    }

    /// Handle a touch end event. May trigger fling. Returns true if consumed.
    pub fn touch_end(&mut self, id: u64) -> bool {
        let removed = self.remove_touch(id);
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
                // One finger lifted — revert to single touch with remaining finger
                let remaining_id = if id == id1 { id2 } else { id1 };
                if self.get_touch(remaining_id).is_some() {
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
        true
    }

    /// Animate fling deceleration. Call each frame. Returns true if still animating.
    pub fn animate_fling(&mut self, view: &mut View, dt: f64) -> bool {
        if self.state != TouchState::Fling {
            return false;
        }

        self.fling_velocity_x *= self.fling_friction;
        self.fling_velocity_y *= self.fling_friction;

        let dx = self.fling_velocity_x * dt;
        let dy = self.fling_velocity_y * dt;

        if dx.abs() > 0.001 || dy.abs() > 0.001 {
            view.scroll(dx, dy);
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

impl Default for DefaultTouchVIF {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewInputFilter for DefaultTouchVIF {
    fn filter(&mut self, event: &InputEvent, _state: &InputState, _view: &mut View) -> bool {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::PanelTree;

    fn setup() -> (PanelTree, View) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
        let view = View::new(root, 800.0, 600.0);
        (tree, view)
    }

    #[test]
    fn test_emulate_middle_button() {
        let mut vif = MouseZoomScrollVIF::new();
        let state = InputState::new();

        // Disabled by default
        let event = InputEvent::press(InputKey::Alt);
        assert!(vif
            .emulate_middle_button_event(&event, &state, 100)
            .is_none());

        // Enable and test — first press at 1000ms (well past initial time 0)
        vif.set_emulate_middle_button(true);
        let result = vif.emulate_middle_button_event(&event, &state, 1000);
        assert!(result.is_some());
        let synth = result.unwrap();
        assert_eq!(synth.key, InputKey::MouseMiddle);
        assert_eq!(synth.variant, InputVariant::Press);
        assert!(!synth.is_repeat);

        // Double-click within 330ms
        let event2 = InputEvent::press(InputKey::Alt);
        let result2 = vif.emulate_middle_button_event(&event2, &state, 1200);
        assert!(result2.is_some());
        assert!(result2.unwrap().is_repeat);

        // After timeout, repeat resets
        let event3 = InputEvent::press(InputKey::Alt);
        let result3 = vif.emulate_middle_button_event(&event3, &state, 2000);
        assert!(result3.is_some());
        assert!(!result3.unwrap().is_repeat);
    }

    #[test]
    fn test_update_wheel_zoom_speed() {
        let mut vif = MouseZoomScrollVIF::new();

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
        let mut vif = MouseZoomScrollVIF::new();

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
        let mut vif = MouseZoomScrollVIF::new();

        vif.set_wheel_anim_params(1.0, 0.5, 0.01);
        let (sc, fr, enabled) = vif.wheel_anim_params();
        assert!((sc - 480.0).abs() < 0.1);
        assert!(fr > 0.0);
        assert!(enabled);
    }

    #[test]
    fn test_navigate_by_program() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);
        let mut vif = KeyboardZoomScrollVIF::new();

        let mut state = InputState::new();
        state.press(InputKey::Shift);
        state.press(InputKey::Alt);

        // Step 1: Shift+Alt+End
        let event = InputEvent::press(InputKey::End);
        assert!(vif.navigate_by_program(&event, &state, &mut view));

        // Step 2: Shift+Alt+C (step = 3)
        let event2 = InputEvent::press(InputKey::Key('c'));
        assert!(vif.navigate_by_program(&event2, &state, &mut view));

        // Step 3: Shift+Alt+Right (scroll right)
        let before = view.current_visit().rel_x;
        let event3 = InputEvent::press(InputKey::ArrowRight);
        assert!(vif.navigate_by_program(&event3, &state, &mut view));
        let after = view.current_visit().rel_x;
        assert!(after > before, "Should have scrolled right");
    }

    #[test]
    fn test_keyboard_continuous_key_state() {
        let mut vif = KeyboardZoomScrollVIF::new();

        // Initially not animating
        assert!(!vif.is_animating());

        // Simulate pressing right arrow with Alt
        let mut state = InputState::new();
        state.press(InputKey::Alt);

        let event = InputEvent::press(InputKey::ArrowRight);
        let (_, mut view) = setup();
        assert!(vif.filter(&event, &state, &mut view));
        assert!(vif.key_state.contains(KeyState::RIGHT));
        assert!(vif.is_animating());

        // Release right arrow
        let release_event = InputEvent::release(InputKey::ArrowRight);
        assert!(vif.filter(&release_event, &state, &mut view));
        assert!(!vif.key_state.contains(KeyState::RIGHT));
    }

    #[test]
    fn test_keyboard_continuous_animation() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut vif = KeyboardZoomScrollVIF::new();
        vif.key_state.insert(KeyState::RIGHT);

        let before = view.current_visit().rel_x;

        // Animate several frames
        for _ in 0..10 {
            vif.animate(&mut view, 0.016);
        }

        let after = view.current_visit().rel_x;
        assert!(after > before, "Continuous animation should scroll right");
    }

    #[test]
    fn test_keyboard_deceleration() {
        let (mut _tree, mut view) = setup();
        view.update_viewing(&mut _tree);

        let mut vif = KeyboardZoomScrollVIF::new();
        // Ramp up velocity
        vif.key_state.insert(KeyState::DOWN);
        for _ in 0..20 {
            vif.animate(&mut view, 0.016);
        }
        assert!(vif.scroll_velocity_y.abs() > 0.1, "Should have velocity");

        // Release key — should decelerate
        vif.key_state.remove(KeyState::DOWN);
        for _ in 0..100 {
            vif.animate(&mut view, 0.016);
        }
        assert!(
            vif.scroll_velocity_y.abs() < 0.1,
            "Should decelerate to near zero"
        );
    }

    #[test]
    fn test_keyboard_zoom_continuous() {
        let (mut _tree, mut view) = setup();
        view.update_viewing(&mut _tree);

        let mut vif = KeyboardZoomScrollVIF::new();
        vif.key_state.insert(KeyState::ZOOM_IN);

        let before = view.current_visit().rel_a;
        for _ in 0..20 {
            vif.animate(&mut view, 0.016);
        }

        let after = view.current_visit().rel_a;
        assert!(after > before, "Should have zoomed in");
    }

    #[test]
    fn test_keyboard_alt_release_clears() {
        let mut vif = KeyboardZoomScrollVIF::new();
        let mut state = InputState::new();
        state.press(InputKey::Alt);
        let (_, mut view) = setup();

        // Press some keys
        let e1 = InputEvent::press(InputKey::ArrowUp);
        vif.filter(&e1, &state, &mut view);
        let e2 = InputEvent::press(InputKey::PageUp);
        vif.filter(&e2, &state, &mut view);
        assert!(!vif.key_state.is_empty());

        // Release Alt — should clear all key state
        state.release(InputKey::Alt);
        let alt_release = InputEvent::release(InputKey::Alt);
        vif.filter(&alt_release, &state, &mut view);
        assert!(vif.key_state.is_empty());
    }

    #[test]
    fn test_touch_single_pan() {
        let (mut _tree, mut view) = setup();
        view.update_viewing(&mut _tree);

        let mut vif = DefaultTouchVIF::new();
        assert_eq!(vif.state(), TouchState::Idle);

        // Touch start
        assert!(vif.touch_start(1, 100.0, 100.0));
        assert_eq!(vif.state(), TouchState::SingleTouch { id: 1 });
        assert_eq!(vif.active_count(), 1);

        // Touch move — should pan
        let before = view.current_visit().rel_x;
        vif.touch_move(1, 120.0, 100.0, 0.016, &mut view);
        let after = view.current_visit().rel_x;
        assert!(after > before, "Single touch should pan");

        // Touch end with low velocity — should go idle
        vif.touch_end(1);
        assert_eq!(vif.active_count(), 0);
    }

    #[test]
    fn test_touch_pinch_zoom() {
        let (mut _tree, mut view) = setup();
        view.update_viewing(&mut _tree);

        let mut vif = DefaultTouchVIF::new();

        // Two touches
        vif.touch_start(1, 100.0, 200.0);
        vif.touch_start(2, 200.0, 200.0);
        assert!(matches!(vif.state(), TouchState::PinchZoom { .. }));
        assert_eq!(vif.active_count(), 2);

        // Move touches apart — should zoom in
        let before = view.current_visit().rel_a;
        vif.touch_move(1, 50.0, 200.0, 0.016, &mut view);
        vif.touch_move(2, 250.0, 200.0, 0.016, &mut view);
        let after = view.current_visit().rel_a;
        assert!(after > before, "Spreading touches should zoom in");
    }

    #[test]
    fn test_touch_fling() {
        let (mut _tree, mut view) = setup();
        view.update_viewing(&mut _tree);

        let mut vif = DefaultTouchVIF::new();
        vif.set_fling_friction(0.95);

        // Rapid drag
        vif.touch_start(1, 100.0, 100.0);
        for i in 1..10 {
            vif.touch_move(1, 100.0 + i as f64 * 50.0, 100.0, 0.016, &mut view);
        }
        vif.touch_end(1);
        assert_eq!(vif.state(), TouchState::Fling);

        // Animate fling until stopped
        let mut frames = 0;
        while vif.animate_fling(&mut view, 0.016) {
            frames += 1;
            if frames > 1000 {
                break;
            }
        }
        assert_eq!(vif.state(), TouchState::Idle);
    }

    #[test]
    fn test_touch_fling_cancel_on_press() {
        let (mut _tree, mut view) = setup();
        view.update_viewing(&mut _tree);

        let mut vif = DefaultTouchVIF::new();

        // Create fling
        vif.touch_start(1, 100.0, 100.0);
        for i in 1..10 {
            vif.touch_move(1, 100.0 + i as f64 * 50.0, 100.0, 0.016, &mut view);
        }
        vif.touch_end(1);
        assert_eq!(vif.state(), TouchState::Fling);

        // Press any key cancels fling
        let state = InputState::new();
        let event = InputEvent::press(InputKey::Escape);
        assert!(vif.filter(&event, &state, &mut view));
        assert_eq!(vif.state(), TouchState::Idle);
    }

    #[test]
    fn test_touch_pinch_to_single() {
        let mut vif = DefaultTouchVIF::new();

        // Two touches
        vif.touch_start(1, 100.0, 200.0);
        vif.touch_start(2, 200.0, 200.0);
        assert!(matches!(vif.state(), TouchState::PinchZoom { .. }));

        // Lift one finger — should revert to single touch
        vif.touch_end(1);
        assert_eq!(vif.state(), TouchState::SingleTouch { id: 2 });
        assert_eq!(vif.active_count(), 1);
    }

    #[test]
    fn test_approach_function() {
        // Accelerating toward target
        let v = approach(0.0, 100.0, 200.0, 400.0, 0.1);
        assert!((v - 20.0).abs() < 0.01); // 200 * 0.1 = 20

        // Decelerating toward zero
        let v2 = approach(50.0, 0.0, 200.0, 400.0, 0.1);
        assert!((v2 - 10.0).abs() < 0.01); // 50 - 400*0.1 = 10

        // Already at target
        let v3 = approach(100.0, 100.0, 200.0, 400.0, 0.1);
        assert!((v3 - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_grip_not_animating_by_default() {
        let vif = MouseZoomScrollVIF::new();
        assert!(!vif.is_grip_animating());
    }

    #[test]
    fn test_grip_kinetic_coasting_after_drag() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut vif = MouseZoomScrollVIF::new();
        // Enable kinetic behavior with realistic parameters
        vif.set_mouse_anim_params(1.0, 0.5, 0.01);

        let mut state = InputState::new();

        // Press middle button — starts gripped phase
        let press = InputEvent::press(InputKey::MouseMiddle);
        state.mouse_x = 100.0;
        state.mouse_y = 100.0;
        assert!(vif.filter(&press, &state, &mut view));
        assert!(
            vif.is_grip_animating(),
            "Should be animating during grip phase"
        );

        // Simulate rapid drag: several move events + animate_grip ticks
        for i in 1..=10 {
            state.mouse_x = 100.0 + i as f64 * 20.0;
            let move_event = InputEvent {
                key: InputKey::MouseMiddle,
                variant: InputVariant::Move,
                chars: String::new(),
                is_repeat: false,
                mouse_x: state.mouse_x,
                mouse_y: state.mouse_y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
            };
            vif.filter(&move_event, &state, &mut view);
            // Tick spring physics between move events
            vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0);
        }

        // Release — should transition to coasting
        let release = InputEvent::release(InputKey::MouseMiddle);
        assert!(vif.filter(&release, &state, &mut view));
        assert!(
            vif.is_grip_animating(),
            "Should be coasting after kinetic drag"
        );

        // Animate coasting until it stops
        let mut frames = 0;
        while vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0) {
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
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut vif = MouseZoomScrollVIF::new();
        // Kinetic disabled (default: mouse_friction_enabled = false)

        let mut state = InputState::new();

        // Press middle button
        let press = InputEvent::press(InputKey::MouseMiddle);
        state.mouse_x = 100.0;
        state.mouse_y = 100.0;
        vif.filter(&press, &state, &mut view);

        // Drag with animation ticks
        for i in 1..=10 {
            state.mouse_x = 100.0 + i as f64 * 20.0;
            let move_event = InputEvent {
                key: InputKey::MouseMiddle,
                variant: InputVariant::Move,
                chars: String::new(),
                is_repeat: false,
                mouse_x: state.mouse_x,
                mouse_y: state.mouse_y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
            };
            vif.filter(&move_event, &state, &mut view);
            vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0);
        }

        // Release — should NOT coast when kinetic is disabled
        let release = InputEvent::release(InputKey::MouseMiddle);
        vif.filter(&release, &state, &mut view);
        assert!(
            !vif.is_grip_animating(),
            "Should not coast when kinetic is disabled"
        );
    }

    #[test]
    fn test_grip_press_cancels_coasting() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut vif = MouseZoomScrollVIF::new();
        vif.set_mouse_anim_params(1.0, 0.5, 0.01);

        let mut state = InputState::new();

        // Start coasting via drag+release
        let press = InputEvent::press(InputKey::MouseMiddle);
        state.mouse_x = 100.0;
        state.mouse_y = 100.0;
        vif.filter(&press, &state, &mut view);

        for i in 1..=10 {
            state.mouse_x = 100.0 + i as f64 * 20.0;
            let move_event = InputEvent {
                key: InputKey::MouseMiddle,
                variant: InputVariant::Move,
                chars: String::new(),
                is_repeat: false,
                mouse_x: state.mouse_x,
                mouse_y: state.mouse_y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
            };
            vif.filter(&move_event, &state, &mut view);
            vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0);
        }

        let release = InputEvent::release(InputKey::MouseMiddle);
        vif.filter(&release, &state, &mut view);
        assert!(vif.is_grip_animating(), "Should be coasting");

        // New press resets to gripped phase (still animating, but gripped)
        let press2 = InputEvent::press(InputKey::MouseMiddle);
        vif.filter(&press2, &state, &mut view);
        // Now in gripped phase — animation is active for spring, velocity zeroed
        assert!(
            vif.is_grip_animating(),
            "New grip starts fresh grip animation"
        );
    }

    #[test]
    fn test_grip_animate_returns_false_when_inactive() {
        let (mut tree, mut view) = setup();
        let mut vif = MouseZoomScrollVIF::new();
        assert!(!vif.animate_grip(&mut view, &mut tree, 1.0 / 60.0));
    }
}
