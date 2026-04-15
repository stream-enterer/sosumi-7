// Port of C++ emMain (IPC server + window factory).

use emcore::emMiniIpc::emMiniIpcClient;

/// Compute the IPC server name for single-instance coordination.
///
/// Port of C++ `emMain::CalcServerName`.
/// Parses the DISPLAY env var (`[host]:display[.screen]`), normalises the host
/// (empty/"localhost"/"127.0.0.1"/current-hostname all collapse to ""), and
/// defaults missing parts to "0".  Output: `eaglemode_on_<host>:<display>.<screen>`.
pub fn CalcServerName() -> String {
    let display_env = std::env::var("DISPLAY").unwrap_or_default();
    let p = display_env.as_str();

    let (h, d, s) = if let Some(colon_pos) = p.rfind(':') {
        let h_raw = &p[..colon_pos];
        let rest = &p[colon_pos + 1..];
        let (d_raw, s_raw) = if let Some(dot_pos) = rest.rfind('.') {
            (&rest[..dot_pos], &rest[dot_pos + 1..])
        } else {
            (rest, "")
        };
        (h_raw, d_raw, s_raw)
    } else {
        (p, "", "")
    };

    // Normalise host: collapse local aliases to ""
    let local = get_hostname();
    let h = if h == "localhost" || h == "127.0.0.1" || h == local.as_str() {
        ""
    } else {
        h
    };

    let d = if d.is_empty() { "0" } else { d };
    let s = if s.is_empty() { "0" } else { s };

    format!("eaglemode_on_{h}:{d}.{s}")
}

/// Return the system hostname via `gethostname(2)`, falling back to "localhost".
fn get_hostname() -> String {
    // SAFETY: gethostname fills a stack buffer; we check the return code.
    let mut buf = [0u8; 512];
    let rc = unsafe {
        libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len())
    };
    if rc != 0 {
        return "localhost".to_string();
    }
    // Find NUL terminator
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

/// Try to send a command to an already-running instance via IPC.
///
/// Port of C++ single-instance coordination using emMiniIpc::TrySend.
pub fn try_ipc_client(server_name: &str, visit: Option<&str>) -> bool {
    let mut args: Vec<&str> = vec!["NewWindow"];
    if let Some(v) = visit {
        args.push("-visit");
        args.push(v);
    }
    match emMiniIpcClient::TrySend(server_name, &args) {
        Ok(()) => {
            log::info!("IPC: sent NewWindow to existing instance");
            true
        }
        Err(e) => {
            log::debug!("IPC: no existing instance ({e})");
            false
        }
    }
}

/// IPC server + window factory engine.
///
/// Port of C++ `emMain`.
pub struct emMain {
    server_name: String,
}

impl emMain {
    pub fn new(serve: bool) -> Self {
        let server_name = CalcServerName();
        if serve {
            log::info!("IPC server name: {server_name}");
        }
        Self { server_name }
    }

    pub fn on_reception(&self, args: &[String]) {
        if args.is_empty() {
            log::warn!("emMain: empty IPC message");
            return;
        }
        match args[0].as_str() {
            "NewWindow" => {
                log::info!("emMain: received NewWindow command");
                // Full wiring in Phase 3 when startup engine is ported.
            }
            "ReloadFiles" => {
                log::info!("emMain: received ReloadFiles command");
            }
            _ => {
                let joined: String = args.join(" ");
                log::warn!("emMain: illegal MiniIpc request: {joined}");
            }
        }
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_server_name() {
        let name = CalcServerName();
        // Format: eaglemode_on_<host>:<display>.<screen>
        assert!(name.starts_with("eaglemode_on_"));
        let suffix = &name["eaglemode_on_".len()..];
        // Must contain exactly one colon and at least one dot
        assert_eq!(suffix.matches(':').count(), 1);
        assert!(suffix.contains('.'));
    }

    #[test]
    fn test_calc_server_name_display_parsing() {
        // Verify that DISPLAY=:0 (no host, no screen) produces :0.0
        // We can't easily set env vars in parallel tests, but we can test
        // the logic indirectly: the name must have the <host>:<disp>.<screen> structure.
        let name = CalcServerName();
        let after_prefix = &name["eaglemode_on_".len()..];
        let colon_pos = after_prefix.find(':').expect("must have colon");
        let after_colon = &after_prefix[colon_pos + 1..];
        assert!(after_colon.contains('.'), "must have dot after colon");
    }

    #[test]
    fn test_get_hostname_non_empty() {
        let h = get_hostname();
        assert!(!h.is_empty());
    }

    #[test]
    fn test_try_ipc_client_no_server() {
        // Should return false when no server is running
        assert!(!try_ipc_client("nonexistent_test_server_12345", None));
        assert!(!try_ipc_client("nonexistent_test_server_12345", Some("/home")));
    }

    #[test]
    fn test_emMain_new() {
        let em = emMain::new(false);
        assert!(em.server_name().starts_with("eaglemode_on_"));
    }

    #[test]
    fn test_emMain_on_reception_new_window() {
        let em = emMain::new(false);
        // Should not panic
        em.on_reception(&["NewWindow".to_string()]);
        em.on_reception(&[
            "NewWindow".to_string(),
            "-visit".to_string(),
            "/home".to_string(),
        ]);
    }

    #[test]
    fn test_emMain_on_reception_empty() {
        let em = emMain::new(false);
        // Should not panic on empty args
        em.on_reception(&[]);
    }

    #[test]
    fn test_emMain_on_reception_reload_files() {
        let em = emMain::new(false);
        em.on_reception(&["ReloadFiles".to_string()]);
    }

    #[test]
    fn test_emMain_on_reception_unknown() {
        let em = emMain::new(false);
        em.on_reception(&["UnknownCommand".to_string(), "arg1".to_string()]);
    }
}
