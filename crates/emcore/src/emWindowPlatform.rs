// SPLIT: Split from emWindow.h — platform abstraction extracted
// Platform-specific implementations for system beep (libcanberra) and
// screensaver inhibition (D-Bus). Gracefully degrade when APIs are unavailable.
//
// Manual verification only — these require a running display server, audio
// daemon, and D-Bus session bus respectively.

#[cfg(all(target_os = "linux", not(kani)))]
mod inner {
    use std::cell::OnceCell;
    use std::ffi::c_void;

    /// Wrapper holding a loaded libcanberra context.
    struct CanberraCtx {
        _lib: libloading::Library,
        ctx: *mut c_void,
        play_fn: unsafe extern "C" fn(*mut c_void, u32, *const i8, *const i8, *const c_void) -> i32,
        destroy_fn: unsafe extern "C" fn(*mut c_void),
    }

    // Safety: CanberraCtx is only used from the main thread via thread_local.
    unsafe impl Send for CanberraCtx {}

    impl Drop for CanberraCtx {
        fn drop(&mut self) {
            unsafe {
                (self.destroy_fn)(self.ctx);
            }
        }
    }

    thread_local! {
        static CANBERRA: OnceCell<Option<CanberraCtx>> = const { OnceCell::new() };
    }

    fn try_init_canberra() -> Option<CanberraCtx> {
        unsafe {
            let lib = libloading::Library::new("libcanberra.so.0")
                .map_err(|e| log::debug!("libcanberra not available: {e}"))
                .ok()?;

            let create: libloading::Symbol<unsafe extern "C" fn(*mut *mut c_void) -> i32> = lib
                .get(b"ca_context_create")
                .map_err(|e| log::debug!("ca_context_create not found: {e}"))
                .ok()?;

            let play: libloading::Symbol<
                unsafe extern "C" fn(*mut c_void, u32, *const i8, *const i8, *const c_void) -> i32,
            > = lib
                .get(b"ca_context_play")
                .map_err(|e| log::debug!("ca_context_play not found: {e}"))
                .ok()?;

            let destroy: libloading::Symbol<unsafe extern "C" fn(*mut c_void)> = lib
                .get(b"ca_context_destroy")
                .map_err(|e| log::debug!("ca_context_destroy not found: {e}"))
                .ok()?;

            let play_fn = *play;
            let destroy_fn = *destroy;

            let mut ctx: *mut c_void = std::ptr::null_mut();
            let ret = create(&mut ctx);
            if ret != 0 {
                log::debug!("ca_context_create failed: {ret}");
                return None;
            }

            Some(CanberraCtx {
                _lib: lib,
                ctx,
                play_fn,
                destroy_fn,
            })
        }
    }

    pub(crate) fn system_beep() {
        CANBERRA.with(|cell| {
            let canberra = cell.get_or_init(try_init_canberra);
            if let Some(ctx) = canberra {
                unsafe {
                    let ret = (ctx.play_fn)(
                        ctx.ctx,
                        0,
                        c"event.id".as_ptr(),
                        c"bell-terminal".as_ptr(),
                        std::ptr::null::<c_void>(),
                    );
                    if ret != 0 {
                        log::debug!("ca_context_play failed: {ret}");
                    }
                }
            }
        });
    }

    pub(crate) fn InhibitScreensaver() -> Option<u32> {
        let conn = match zbus::blocking::Connection::session() {
            Ok(c) => c,
            Err(e) => {
                log::debug!("D-Bus session connect failed: {e}");
                return None;
            }
        };
        let reply = conn.call_method(
            Some("org.freedesktop.ScreenSaver"),
            "/org/freedesktop/ScreenSaver",
            Some("org.freedesktop.ScreenSaver"),
            "Inhibit",
            &("eaglemode-rs", "Active content display"),
        );
        match reply {
            Ok(msg) => match msg.body().deserialize::<u32>() {
                Ok(cookie) => {
                    log::debug!("screensaver inhibited (cookie={cookie})");
                    Some(cookie)
                }
                Err(e) => {
                    log::debug!("screensaver inhibit reply parse failed: {e}");
                    None
                }
            },
            Err(e) => {
                log::debug!("screensaver inhibit call failed: {e}");
                None
            }
        }
    }

    pub(crate) fn uninhibit_screensaver(cookie: u32) {
        let conn = match zbus::blocking::Connection::session() {
            Ok(c) => c,
            Err(e) => {
                log::debug!("D-Bus session connect failed: {e}");
                return;
            }
        };
        if let Err(e) = conn.call_method(
            Some("org.freedesktop.ScreenSaver"),
            "/org/freedesktop/ScreenSaver",
            Some("org.freedesktop.ScreenSaver"),
            "UnInhibit",
            &(cookie,),
        ) {
            log::debug!("screensaver uninhibit failed: {e}");
        }
    }

    /// DIVERGED: (language-forced) C++ calls `system("xscreensaver-command -deactivate >&- 2>&- &")`
    /// in emX11Screen.cpp:711-765. Rust spawns the process directly.
    fn poke_xscreensaver() {
        use std::process::Command;
        // Fire and forget — match C++ `system("xscreensaver-command -deactivate >&- 2>&- &")`
        let _ = Command::new("xscreensaver-command")
            .arg("-deactivate")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    /// Keepalive state for periodic screensaver re-inhibition.
    /// DIVERGED: (dependency-forced) C++ uses emX11Screen::ScreensaverUpdateTimer (59s) with
    /// XResetScreenSaver + xscreensaver-command. Rust uses thread-local state
    /// ticked from the event loop since we don't have an X11 display handle.
    /// `active_count` ref-counts callers so multi-window support is correct:
    /// keepalive stays active until all windows release it.
    struct ScreensaverKeepAlive {
        active_count: u32,
        last_poke: std::time::Instant,
    }

    thread_local! {
        static KEEPALIVE: std::cell::RefCell<ScreensaverKeepAlive> =
            std::cell::RefCell::new(ScreensaverKeepAlive {
                active_count: 0,
                last_poke: std::time::Instant::now(),
            });
    }

    const KEEPALIVE_INTERVAL_SECS: u64 = 59;

    pub(crate) fn start_screensaver_keepalive() {
        KEEPALIVE.with(|cell| {
            let mut ka = cell.borrow_mut();
            ka.active_count += 1;
            if ka.active_count == 1 {
                ka.last_poke = std::time::Instant::now();
                poke_xscreensaver();
                log::debug!("screensaver keepalive started");
            }
        });
    }

    pub(crate) fn stop_screensaver_keepalive() {
        KEEPALIVE.with(|cell| {
            let mut ka = cell.borrow_mut();
            if ka.active_count > 0 {
                ka.active_count -= 1;
                if ka.active_count == 0 {
                    log::debug!("screensaver keepalive stopped");
                }
            }
        });
    }

    pub(crate) fn tick_screensaver_keepalive() {
        KEEPALIVE.with(|cell| {
            let mut ka = cell.borrow_mut();
            if ka.active_count > 0 && ka.last_poke.elapsed().as_secs() >= KEEPALIVE_INTERVAL_SECS {
                ka.last_poke = std::time::Instant::now();
                poke_xscreensaver();
                log::debug!("screensaver keepalive poked (59s tick)");
            }
        });
    }
}

#[cfg(any(not(target_os = "linux"), kani))]
mod inner {
    pub(crate) fn system_beep() {}
    pub(crate) fn InhibitScreensaver() -> Option<u32> {
        None
    }
    pub(crate) fn uninhibit_screensaver(_cookie: u32) {}
    pub(crate) fn start_screensaver_keepalive() {}
    pub(crate) fn stop_screensaver_keepalive() {}
    pub(crate) fn tick_screensaver_keepalive() {}
}

pub(crate) use inner::start_screensaver_keepalive;
pub(crate) use inner::stop_screensaver_keepalive;
pub(crate) use inner::system_beep;
pub(crate) use inner::tick_screensaver_keepalive;
pub(crate) use inner::uninhibit_screensaver;
pub(crate) use inner::InhibitScreensaver;
