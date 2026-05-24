use std::{
    error::Error,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chewing_tip_core::{config::REGISTRY_ROOT, result::ResultExt};
use windows::Win32::System::Registry::KEY_WOW64_64KEY;
use windows_registry::{CURRENT_USER, Key};

use super::version;

pub(crate) struct CheckUpdateConfig {
    pub(crate) enabled: bool,
    pub(crate) channel: String,
}

pub(crate) fn get_check_update_config() -> Result<CheckUpdateConfig, CheckUpdateError> {
    let key = open_update_config_key().boxed()?;
    let channel = match key.get_string("AutoCheckUpdateChannel") {
        Ok(ch) => ch,
        Err(_) => {
            let dll_channel = version::chewing_dll_channel();
            let _ = key.set_string("AutoCheckUpdateChannel", &dll_channel);
            dll_channel
        }
    };
    let enabled = channel == "stable" || channel == "development";
    Ok(CheckUpdateConfig { enabled, channel })
}

pub(crate) fn set_update_info_url(url: &str) -> Result<(), SetUpdateInfoError> {
    let key = open_update_config_key().boxed()?;
    if url.is_empty() {
        key.remove_value("UpdateInfoUrl").boxed()?;
    } else {
        key.set_string("UpdateInfoUrl", &url).boxed()?;
    }
    Ok(())
}

pub(crate) fn set_last_update_check_time() -> Result<(), SetUpdateInfoError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .as_ref()
        .map(Duration::as_secs)
        .unwrap_or_default();
    let key = open_update_config_key().boxed()?;
    key.set_u64("LastUpdateCheckTime", now).boxed()?;
    Ok(())
}

fn open_update_config_key() -> windows_registry::Result<Key> {
    CURRENT_USER
        .options()
        .create()
        .access(KEY_WOW64_64KEY.0)
        .write()
        .open(REGISTRY_ROOT)
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to get update config")]
pub(crate) struct CheckUpdateError(#[from] Box<dyn Error + Send + Sync>);

#[derive(Debug, thiserror::Error)]
#[error("Failed to set update info")]
pub(crate) struct SetUpdateInfoError(#[from] Box<dyn Error + Send + Sync>);
