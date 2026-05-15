// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    fs::OpenOptions,
    io::Write,
    sync::Mutex,
};

use logforth::{Append, Diagnostic, Error, Layout, layout::PlainTextLayout, record::Record};
use windows::Win32::System::Diagnostics::Debug::{IsDebuggerPresent, OutputDebugStringW};
use windows_core::HSTRING;

#[derive(Debug)]
pub(crate) struct WinDbg {
    layout: Box<dyn Layout>,
}

impl Default for WinDbg {
    fn default() -> Self {
        Self {
            layout: Box::new(PlainTextLayout::default()),
        }
    }
}

impl Append for WinDbg {
    fn append(&self, record: &Record, diags: &[Box<dyn Diagnostic>]) -> Result<(), Error> {
        let mut bytes = self.layout.format(record, diags)?;
        bytes.truncate(1999);
        bytes.push(b'\n');
        let text = String::from_utf8_lossy(&bytes);
        output_debug_string(&text);
        Ok(())
    }
    fn flush(&self) -> Result<(), Error> {
        Ok(())
    }
}

pub(crate) fn output_debug_string(text: &str) {
    unsafe {
        OutputDebugStringW(&HSTRING::from(text));
    }
}

pub(crate) fn is_debugger_present() -> bool {
    unsafe { IsDebuggerPresent().as_bool() }
}

#[derive(Debug)]
pub(crate) struct DiagFile {
    layout: Box<dyn Layout>,
    file: Mutex<std::fs::File>,
}

impl DiagFile {
    pub(crate) fn try_new() -> Option<Self> {
        let path = std::env::var_os("TEMP")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows\\Temp"))
            .join("chewing_tip_diag.log");
        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(Self {
            layout: Box::new(PlainTextLayout::default()),
            file: Mutex::new(f),
        })
    }
}

impl Append for DiagFile {
    fn append(&self, record: &Record, diags: &[Box<dyn Diagnostic>]) -> Result<(), Error> {
        let mut bytes = self.layout.format(record, diags)?;
        bytes.push(b'\n');
        if let Ok(mut f) = self.file.lock() {
            let _ = f.write_all(&bytes);
            let _ = f.flush();
        }
        Ok(())
    }
    fn flush(&self) -> Result<(), Error> {
        Ok(())
    }
}
