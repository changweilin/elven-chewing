// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    ffi::{c_int, c_void},
    sync::atomic::{AtomicUsize, Ordering},
};

use logforth::record::{Level, LevelFilter};
use windows::Win32::System::{
    Com::{CoLockObjectExternal, IClassFactory, IClassFactory_Impl},
    Console::{ATTACH_PARENT_PROCESS, AttachConsole},
};
use windows::Win32::{Foundation::TRUE, System::SystemServices::DLL_PROCESS_ATTACH};
use windows::core::{
    BOOL, ComObjectInner, ComObjectInterface, GUID, HRESULT, IUnknown, Interface, Ref, Result,
    implement,
};

use crate::{
    logging::{self, DiagFile, WinDbg},
    text_service::TextService,
};

pub(crate) static G_HINSTANCE: AtomicUsize = AtomicUsize::new(0);

#[unsafe(no_mangle)]
extern "system" fn DllMain(
    hmodule: *mut c_void,
    ul_reason_for_call: u32,
    _reserved: *const c_void,
) -> c_int {
    if let DLL_PROCESS_ATTACH = ul_reason_for_call {
        let g_hinstance = G_HINSTANCE.load(Ordering::Relaxed);
        if g_hinstance == 0 {
            G_HINSTANCE.store(hmodule as usize, Ordering::Relaxed);
            if logging::is_debugger_present() {
                unsafe {
                    let _ = AttachConsole(ATTACH_PARENT_PROCESS);
                }
            }
            // Always-on logging to TEMP\chewing_tip_diag.log + OutputDebugString.
            // Temporary diagnostic for activation failure debugging.
            logforth::starter_log::builder()
                .dispatch(|d| {
                    let mut d = d
                        .filter(LevelFilter::MoreSevereEqual(Level::Debug))
                        .append(WinDbg::default());
                    if let Some(diag) = DiagFile::try_new() {
                        d = d.append(diag);
                    }
                    if logging::is_debugger_present() {
                        d = d.append(logforth::append::Stderr::default());
                    }
                    d
                })
                .apply();
            // Install a global panic hook so chewing_tip.dll panics get logged.
            std::panic::set_hook(Box::new(|info| {
                let loc = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_else(|| "<unknown>".to_string());
                let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = info.payload().downcast_ref::<String>() {
                    s.clone()
                } else {
                    "<non-string panic>".to_string()
                };
                log::error!("PANIC at {loc}: {payload}");
            }));
            log::info!("chewing_tip.dll loaded");
        }
    }
    TRUE.0
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(
    _rclsid: *const c_void,
    riid: *const GUID,
    ppv_obj: *mut *mut c_void,
) -> HRESULT {
    let factory: IUnknown = CClassFactory::new().into_object().into_interface();
    unsafe { factory.query(riid, ppv_obj) }
}

#[implement(IClassFactory)]
struct CClassFactory;

impl CClassFactory {
    fn new() -> CClassFactory {
        CClassFactory
    }
}

impl IClassFactory_Impl for CClassFactory_Impl {
    fn CreateInstance(
        &self,
        _punkouter: Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        let text_service: IUnknown = TextService::new().into_object().into_interface();
        unsafe {
            text_service.query(riid, ppvobject).ok()?;
        }
        Ok(())
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        unsafe { CoLockObjectExternal(self.as_interface_ref(), flock.as_bool(), true) }
    }
}
