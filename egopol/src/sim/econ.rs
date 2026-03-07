use std::fmt;
use std::path::{Path, PathBuf};

use jlrs::prelude::*;
use jlrs::runtime::handle::local_handle::LocalHandle;
use world::{EconSnapshot, TaxRates};

#[derive(Debug)]
pub(crate) enum EconError {
    RuntimeInit(String),
    Eval(String),
    ModelInit { country: String, message: String },
    StepFailed { country: String, message: String },
    ReadResult(String),
}

impl fmt::Display for EconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EconError::RuntimeInit(msg) => write!(f, "Julia runtime init failed: {msg}"),
            EconError::Eval(msg) => write!(f, "Julia eval error: {msg}"),
            EconError::ModelInit { country, message } => {
                write!(f, "Model init failed for {country}: {message}")
            }
            EconError::StepFailed { country, message } => {
                write!(f, "Step failed for {country}: {message}")
            }
            EconError::ReadResult(msg) => write!(f, "Failed to read result: {msg}"),
        }
    }
}

impl std::error::Error for EconError {}

struct CountryModel {
    julia_id: i64,
    country_code: String,
}

pub(crate) struct EconBridge {
    julia: LocalHandle,
    models: Vec<CountryModel>,
}

fn find_julia_bindir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("JLRS_JULIA_DIR") {
        let p = PathBuf::from(&dir).join("bin");
        if p.exists() {
            return Ok(p);
        }
        let p = PathBuf::from(&dir);
        if p.exists() {
            return Ok(p);
        }
        return Err(format!(
            "JLRS_JULIA_DIR={dir} does not contain a bin/ directory"
        ));
    }

    let output = std::process::Command::new("which")
        .arg("julia")
        .output()
        .map_err(|e| format!("Failed to run `which julia`: {e}"))?;

    if !output.status.success() {
        return Err("Julia not found in PATH. Set JLRS_JULIA_DIR or install Julia 1.10+".into());
    }

    let julia_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let julia_bin = std::fs::canonicalize(&julia_path)
        .map_err(|e| format!("Failed to resolve julia path: {e}"))?;

    Ok(julia_bin
        .parent()
        .expect("julia binary has parent directory")
        .to_path_buf())
}

impl EconBridge {
    pub(crate) fn new(workspace_root: &Path, sysimage: Option<&Path>) -> Result<Self, EconError> {
        let builder = Builder::new();

        let julia = if let Some(sysimg) = sysimage {
            let bindir = find_julia_bindir().map_err(EconError::RuntimeInit)?;
            let sysimg_owned = sysimg.to_path_buf();
            // Safety: using our own sysimage built from PackageCompiler
            let builder = unsafe { builder.image(bindir, sysimg_owned) }.map_err(|_| {
                EconError::RuntimeInit("sysimage or bindir path does not exist".into())
            })?;
            builder
                .start_local()
                .map_err(|e| EconError::RuntimeInit(format!("{e}")))?
        } else {
            builder
                .start_local()
                .map_err(|e| EconError::RuntimeInit(format!("{e}")))?
        };

        // Activate the Julia project and include init.jl
        let project_dir = workspace_root.join("julia");
        let activate_cmd = format!(r#"import Pkg; Pkg.activate("{}")"#, project_dir.display());
        let init_path = project_dir.join("init.jl");
        let include_cmd = format!(r#"include("{}")"#, init_path.display());

        julia.local_scope::<_, 2>(|mut frame| -> Result<(), EconError> {
            // Safety: activating our own project directory
            unsafe { Value::eval_string(&mut frame, &activate_cmd) }.map_err(|e| {
                EconError::Eval(format!(
                    "Pkg.activate: {}",
                    e.error_string_or("unknown error")
                ))
            })?;
            // Safety: including our own init.jl
            unsafe { Value::eval_string(&mut frame, &include_cmd) }.map_err(|e| {
                EconError::Eval(format!(
                    "include init.jl: {}",
                    e.error_string_or("unknown error")
                ))
            })?;
            Ok(())
        })?;

        Ok(Self {
            julia,
            models: Vec::new(),
        })
    }

    pub(crate) fn init_country(
        &mut self,
        code: &str,
        rates: &TaxRates,
    ) -> Result<usize, EconError> {
        let mut params = serde_json::json!({ "country": code });
        let has_overrides = rates.income != 0.0
            || rates.corporate != 0.0
            || rates.vat != 0.0
            || rates.social_employer != 0.0
            || rates.social_employee != 0.0
            || rates.export != 0.0
            || rates.capital_formation != 0.0;
        if has_overrides {
            params["tax_rates"] = serde_json::json!({
                "income": rates.income,
                "corporate": rates.corporate,
                "vat": rates.vat,
                "social_employer": rates.social_employer,
                "social_employee": rates.social_employee,
                "export": rates.export,
                "capital_formation": rates.capital_formation,
            });
        }
        let params_json = serde_json::to_string(&params).map_err(|e| EconError::ModelInit {
            country: code.to_string(),
            message: format!("JSON serialize: {e}"),
        })?;

        let escaped = params_json.replace('\\', "\\\\").replace('"', "\\\"");
        let call_expr = format!(r#"egopol_init_model("{escaped}")"#);

        let julia_id = self.julia.local_scope::<_, 1>(|mut frame| {
            // Safety: calling our own init function with sanitized JSON
            match unsafe { Value::eval_string(&mut frame, &call_expr) } {
                Ok(val) => val.unbox::<i64>().map_err(|e| EconError::ModelInit {
                    country: code.to_string(),
                    message: format!("unbox id: {e}"),
                }),
                Err(e) => Err(EconError::ModelInit {
                    country: code.to_string(),
                    message: e.error_string_or("unknown Julia error"),
                }),
            }
        })?;

        let idx = self.models.len();
        self.models.push(CountryModel {
            julia_id,
            country_code: code.to_string(),
        });
        Ok(idx)
    }

    pub(crate) fn step(&mut self, idx: usize) -> Result<EconSnapshot, EconError> {
        let model = &self.models[idx];
        let call_expr = format!("egopol_step!({})", model.julia_id);
        let country = model.country_code.clone();

        let json_str = self.julia.local_scope::<_, 1>(|mut frame| {
            // Safety: calling our own step function with integer id
            match unsafe { Value::eval_string(&mut frame, &call_expr) } {
                Ok(val) => {
                    let unboxed = val.unbox::<String>().map_err(|e| EconError::StepFailed {
                        country: country.clone(),
                        message: format!("unbox string: {e}"),
                    })?;
                    unboxed.map_err(|bytes| EconError::StepFailed {
                        country: country.clone(),
                        message: format!("non-UTF8 string ({} bytes)", bytes.len()),
                    })
                }
                Err(e) => Err(EconError::StepFailed {
                    country,
                    message: e.error_string_or("unknown Julia error"),
                }),
            }
        })?;

        serde_json::from_str(&json_str).map_err(|e| EconError::ReadResult(format!("{e}")))
    }

    pub(crate) fn model_count(&self) -> usize {
        self.models.len()
    }
}
