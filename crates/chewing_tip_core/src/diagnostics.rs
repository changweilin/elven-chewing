use std::{
    env,
    path::{Path, PathBuf},
};

use serde::Serialize;
use windows::Win32::System::Registry::KEY_WOW64_64KEY;
use windows_registry::CURRENT_USER;

use crate::{
    config::{ChewingTsfConfig, Config, LEGACY_REGISTRY_ROOT, REGISTRY_ROOT},
    ipc::{client::ChewingIpcClient, named_pipe::named_pipe_path},
    shell::program_dir,
};

#[derive(Debug, Serialize)]
pub struct DiagnosticsReport {
    pub schema_version: u32,
    pub package_version: &'static str,
    pub process: ProcessDiagnostics,
    pub registry: RegistryDiagnostics,
    pub paths: PathDiagnostics,
    pub ipc: IpcDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct ProcessDiagnostics {
    pub current_exe: Option<String>,
    pub current_exe_error: Option<String>,
    pub arch: Option<String>,
    pub arch_wow64: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegistryDiagnostics {
    pub root: &'static str,
    pub legacy_root: &'static str,
    pub root_exists: bool,
    pub legacy_root_exists: bool,
    pub config_loaded: bool,
    pub config_error: Option<String>,
    pub config: Option<ChewingTsfConfig>,
}

#[derive(Debug, Serialize)]
pub struct PathDiagnostics {
    pub program_dir: PathProbe,
    pub host_exe: PathProbe,
    pub tip_dll: PathProbe,
    pub user_data_dir: PathProbe,
}

#[derive(Debug, Serialize)]
pub struct PathProbe {
    pub path: Option<String>,
    pub exists: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IpcDiagnostics {
    pub named_pipe_path: Option<String>,
    pub named_pipe_error: Option<String>,
    pub host_connectable: bool,
    pub host_connect_error: Option<String>,
}

pub fn collect_diagnostics() -> DiagnosticsReport {
    let current_exe = env::current_exe();
    let (current_exe, current_exe_error) = match current_exe {
        Ok(path) => (Some(path_to_string(&path)), None),
        Err(error) => (None, Some(error.to_string())),
    };

    let cfg = Config::from_reg();
    let (config_loaded, config_error, config) = match cfg {
        Ok(cfg) => (true, None, Some(cfg.chewing_tsf)),
        Err(error) => (false, Some(error.to_string()), None),
    };

    let program_dir_result = program_dir();
    let program_dir_path = program_dir_result.as_ref().ok().cloned();
    let program_dir = probe_result(program_dir_result);
    let host_exe = probe_optional_path(
        program_dir_path
            .as_ref()
            .map(|path| path.join("chewing_tip_host.exe")),
    );
    let tip_dll = probe_optional_path(
        program_dir_path
            .as_ref()
            .map(|path| path.join("chewing_tip.dll")),
    );

    let user_data_dir = probe_optional_path(env::var_os("APPDATA").map(|path| {
        PathBuf::from(path)
            .join("ElvenIME")
            .join("ChewingTextService")
    }));

    let pipe_path = named_pipe_path();
    let (named_pipe_path, named_pipe_error) = match pipe_path {
        Ok(path) => (Some(path), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let host_connect = ChewingIpcClient::connect();
    let (host_connectable, host_connect_error) = match host_connect {
        Ok(_) => (true, None),
        Err(error) => (false, Some(error.to_string())),
    };

    DiagnosticsReport {
        schema_version: 1,
        package_version: env!("CARGO_PKG_VERSION"),
        process: ProcessDiagnostics {
            current_exe,
            current_exe_error,
            arch: env::var("PROCESSOR_ARCHITECTURE").ok(),
            arch_wow64: env::var("PROCESSOR_ARCHITEW6432").ok(),
        },
        registry: RegistryDiagnostics {
            root: REGISTRY_ROOT,
            legacy_root: LEGACY_REGISTRY_ROOT,
            root_exists: registry_key_exists(REGISTRY_ROOT),
            legacy_root_exists: registry_key_exists(LEGACY_REGISTRY_ROOT),
            config_loaded,
            config_error,
            config,
        },
        paths: PathDiagnostics {
            program_dir,
            host_exe,
            tip_dll,
            user_data_dir,
        },
        ipc: IpcDiagnostics {
            named_pipe_path,
            named_pipe_error,
            host_connectable,
            host_connect_error,
        },
    }
}

fn registry_key_exists(path: &str) -> bool {
    CURRENT_USER
        .options()
        .read()
        .access(KEY_WOW64_64KEY.0)
        .open(path)
        .is_ok()
}

fn probe_result<E>(result: Result<PathBuf, E>) -> PathProbe
where
    E: std::fmt::Display,
{
    match result {
        Ok(path) => probe_path(path),
        Err(error) => PathProbe {
            path: None,
            exists: false,
            error: Some(error.to_string()),
        },
    }
}

fn probe_optional_path(path: Option<PathBuf>) -> PathProbe {
    match path {
        Some(path) => probe_path(path),
        None => PathProbe {
            path: None,
            exists: false,
            error: Some("path is unavailable".to_string()),
        },
    }
}

fn probe_path(path: PathBuf) -> PathProbe {
    let exists = path.exists();
    PathProbe {
        path: Some(path_to_string(&path)),
        exists,
        error: None,
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
