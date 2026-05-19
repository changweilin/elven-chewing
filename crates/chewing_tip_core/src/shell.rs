use std::error::Error;
use std::fmt::Display;
use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use exn::{Result, ResultExt};
use windows::Foundation::Uri;
use windows::System::Launcher;
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{BSTR, HSTRING, PCWSTR, w};

pub fn user_dir() -> Result<PathBuf, ShellError> {
    let err = || ShellError("unable to determine user dir".to_string());
    // Independent user data location so 精靈語輸入法 and 新酷音輸入法 do not
    // share learning dictionaries even when both are installed.
    let user_dir = PathBuf::from(std::env::var("APPDATA").or_raise(err)?)
        .join("ElvenIME")
        .join("ChewingTextService");

    // NB: chewing might be loaded into a low mandatory integrity level process (SearchHost.exe).
    // In that case, it might not be able to check if a file exists using CreateFile
    // If the file exists, it will get the PermissionDenied error instead.
    let user_dir_exists = match std::fs::exists(&user_dir) {
        Ok(true) => true,
        Err(e) => matches!(e.kind(), ErrorKind::PermissionDenied),
        _ => false,
    };

    if !user_dir_exists {
        std::fs::create_dir_all(&user_dir).or_raise(err)?;
        let metadata = user_dir.metadata().or_raise(err)?;
        let attributes = metadata.file_attributes();
        let user_dir_w: Vec<u16> = user_dir.as_os_str().encode_wide().collect();
        unsafe {
            SetFileAttributesW(
                &BSTR::from_wide(&user_dir_w),
                FILE_FLAGS_AND_ATTRIBUTES(attributes | FILE_ATTRIBUTE_HIDDEN.0),
            )
            .or_raise(err)?;
        };
    }

    Ok(user_dir)
}

pub fn program_dir() -> Result<PathBuf, ShellError> {
    let err = || ShellError("failed to determine Program Files path".to_string());
    Ok(PathBuf::from(
        std::env::var("ProgramW6432")
            .or_else(|_| std::env::var("ProgramFiles"))
            .or_else(|_| std::env::var("ProgramFiles(x86)"))
            .or_raise(err)?,
    )
    .join("ElvenIME"))
}

pub fn open_url(url: &str) {
    if let Ok(uri) = Uri::CreateUri(&url.into()) {
        let _ = Launcher::LaunchUriAsync(&uri);
    }
}

/// Launch a local file via the Windows shell's default file association
/// (e.g. `.msi` -> msiexec). Use for invoking installers; the user will see
/// a UAC prompt and msiexec's own UI.
pub fn launch_file(path: &Path) {
    let wide: HSTRING = path.as_os_str().into();
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

#[derive(Debug)]
pub struct ShellError(String);
impl Error for ShellError {}
impl Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShellError: {}", self.0)
    }
}
