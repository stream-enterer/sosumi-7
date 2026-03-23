use std::path::PathBuf;
use std::sync::Mutex;

use zuicchini::emCore::emInstallInfo::{emGetConfigDirOverloadable, emGetInstallPath, InstallDirType, InstallInfoError};

/// Serialize all tests that mutate environment variables.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Save, set, run, restore env vars under the global lock.
fn with_envs<F, R>(vars: &[(&str, &str)], removes: &[&str], f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = ENV_LOCK.lock().unwrap();

    // Save originals
    let saved: Vec<(&str, Option<String>)> = vars
        .iter()
        .map(|&(k, _)| (k, std::env::var(k).ok()))
        .chain(removes.iter().map(|&k| (k, std::env::var(k).ok())))
        .collect();

    // Apply
    for &(k, v) in vars {
        unsafe { std::env::set_var(k, v) };
    }
    for &k in removes {
        unsafe { std::env::remove_var(k) };
    }

    let result = f();

    // Restore
    for (k, original) in saved {
        match original {
            Some(v) => unsafe { std::env::set_var(k, v) },
            None => unsafe { std::env::remove_var(k) },
        }
    }

    result
}

#[test]
fn bin_path() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::Bin, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/bin"));
    });
}

#[test]
fn include_path_uses_prj() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::Include, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/include/emCore"));
    });
}

#[test]
fn lib_path() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::Lib, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/lib"));
    });
}

#[test]
fn html_doc_path() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::HtmlDoc, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/doc/html"));
    });
}

#[test]
fn pdf_doc_path() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::PdfDoc, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/doc/pdf"));
    });
}

#[test]
fn ps_doc_path() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::PsDoc, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/doc/ps"));
    });
}

#[test]
fn host_config_path() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::HostConfig, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/etc/emCore"));
    });
}

#[test]
fn res_path() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::Res, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/res/emCore"));
    });
}

#[test]
fn user_config_default() {
    with_envs(
        &[("HOME", "/home/testuser"), ("EM_DIR", "/opt/eaglemode")],
        &["EM_USER_CONFIG_DIR"],
        || {
            let p = emGetInstallPath(InstallDirType::UserConfig, "emCore", None).unwrap();
            assert_eq!(p, PathBuf::from("/home/testuser/.eaglemode/emCore"));
        },
    );
}

#[test]
fn user_config_with_override() {
    with_envs(
        &[
            ("EM_USER_CONFIG_DIR", "/custom/config"),
            ("EM_DIR", "/opt/eaglemode"),
        ],
        &[],
        || {
            let p = emGetInstallPath(InstallDirType::UserConfig, "emCore", None).unwrap();
            assert_eq!(p, PathBuf::from("/custom/config/emCore"));
        },
    );
}

#[test]
fn tmp_path_with_tmpdir() {
    with_envs(
        &[("TMPDIR", "/my/tmp"), ("EM_DIR", "/opt/eaglemode")],
        &[],
        || {
            let p = emGetInstallPath(InstallDirType::Tmp, "emCore", None).unwrap();
            assert_eq!(p, PathBuf::from("/my/tmp"));
        },
    );
}

#[test]
fn tmp_path_default() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &["TMPDIR"], || {
        let p = emGetInstallPath(InstallDirType::Tmp, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/tmp"));
    });
}

#[test]
fn home_path() {
    with_envs(&[("HOME", "/home/testuser")], &[], || {
        let p = emGetInstallPath(InstallDirType::Home, "emCore", None).unwrap();
        assert_eq!(p, PathBuf::from("/home/testuser"));
    });
}

#[test]
fn sub_path_appended() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::Res, "emCore", Some("images/logo.png")).unwrap();
        assert_eq!(
            p,
            PathBuf::from("/opt/eaglemode/res/emCore/images/logo.png")
        );
    });
}

#[test]
fn empty_sub_path_ignored() {
    with_envs(&[("EM_DIR", "/opt/eaglemode")], &[], || {
        let p = emGetInstallPath(InstallDirType::Bin, "emCore", Some("")).unwrap();
        assert_eq!(p, PathBuf::from("/opt/eaglemode/bin"));
    });
}

#[test]
fn missing_em_dir_returns_error() {
    with_envs(&[], &["EM_DIR"], || {
        let result = emGetInstallPath(InstallDirType::Bin, "emCore", None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InstallInfoError::EnvNotSet(ref v) if v == "EM_DIR"
        ));
    });
}

#[test]
fn config_dir_overloadable_versions_match() {
    let tmp = std::env::temp_dir().join("zuicchini_test_install_info_match");
    let host_dir = tmp.join("etc").join("testprj");
    let user_dir = tmp.join("userconfig").join("testprj");
    std::fs::create_dir_all(&host_dir).unwrap();
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::write(host_dir.join("version"), "1.0.0\n").unwrap();
    std::fs::write(user_dir.join("version"), "1.0.0\n").unwrap();

    let uc = tmp.join("userconfig");
    with_envs(
        &[
            ("EM_DIR", tmp.to_str().unwrap()),
            ("EM_USER_CONFIG_DIR", uc.to_str().unwrap()),
            ("HOME", tmp.to_str().unwrap()),
        ],
        &[],
        || {
            let p = emGetConfigDirOverloadable("testprj", None).unwrap();
            assert_eq!(p, user_dir);
        },
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn config_dir_overloadable_versions_mismatch() {
    let tmp = std::env::temp_dir().join("zuicchini_test_install_info_mismatch");
    let host_dir = tmp.join("etc").join("testprj");
    let user_dir = tmp.join("userconfig").join("testprj");
    std::fs::create_dir_all(&host_dir).unwrap();
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::write(host_dir.join("version"), "2.0.0\n").unwrap();
    std::fs::write(user_dir.join("version"), "1.0.0\n").unwrap();

    let uc = tmp.join("userconfig");
    with_envs(
        &[
            ("EM_DIR", tmp.to_str().unwrap()),
            ("EM_USER_CONFIG_DIR", uc.to_str().unwrap()),
            ("HOME", tmp.to_str().unwrap()),
        ],
        &[],
        || {
            let p = emGetConfigDirOverloadable("testprj", None).unwrap();
            assert_eq!(p, host_dir);
        },
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn config_dir_overloadable_no_user_version() {
    let tmp = std::env::temp_dir().join("zuicchini_test_install_info_no_user");
    let host_dir = tmp.join("etc").join("testprj");
    let user_dir = tmp.join("userconfig").join("testprj");
    std::fs::create_dir_all(&host_dir).unwrap();
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::write(host_dir.join("version"), "1.0.0\n").unwrap();
    // No version file in user dir

    let uc = tmp.join("userconfig");
    with_envs(
        &[
            ("EM_DIR", tmp.to_str().unwrap()),
            ("EM_USER_CONFIG_DIR", uc.to_str().unwrap()),
            ("HOME", tmp.to_str().unwrap()),
        ],
        &[],
        || {
            let p = emGetConfigDirOverloadable("testprj", None).unwrap();
            assert_eq!(p, host_dir);
        },
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn config_dir_overloadable_with_sub_dir() {
    let tmp = std::env::temp_dir().join("zuicchini_test_install_info_subdir");
    let host_dir = tmp.join("etc").join("testprj");
    std::fs::create_dir_all(&host_dir).unwrap();
    std::fs::write(host_dir.join("version"), "1.0.0\n").unwrap();

    let uc = tmp.join("userconfig");
    with_envs(
        &[
            ("EM_DIR", tmp.to_str().unwrap()),
            ("EM_USER_CONFIG_DIR", uc.to_str().unwrap()),
            ("HOME", tmp.to_str().unwrap()),
        ],
        &[],
        || {
            let p = emGetConfigDirOverloadable("testprj", Some("themes")).unwrap();
            assert_eq!(p, host_dir.join("themes"));
        },
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
