// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

#![windows_subsystem = "windows"]

use std::{env, fs, process};

use chewing_tip_core::{
    config::Config,
    diagnostics::collect_diagnostics,
    ipc::{client::ChewingIpcClient, messages::Stop, varlink::MethodCall},
};
use serde_json::Value;
use windows::{
    Win32::{
        Globalization::*,
        System::{
            Com::*,
            Console::{ATTACH_PARENT_PROCESS, AttachConsole},
            Diagnostics::Debug::IsDebuggerPresent,
        },
        UI::{Input::KeyboardAndMouse::HKL, TextServices::*},
    },
    core::*,
};
#[cfg(feature = "nightly")]
use windows_registry::LOCAL_MACHINE;

// https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip
windows::core::link!("input.dll" "system" fn InstallLayoutOrTip(psz: *const u16, dwFlags: u32));
const ILOT_INSTALL: u32 = 0x00000000;
// const ILOT_UNINSTALL: u32 = 0x00000001;

const CHEWING_TSF_CLSID: GUID = GUID::from_u128(0xDE733D27_7EEB_4C3B_9EEC_715F05B5BA85);
const CHEWING_ZH_TW_PROFILE_GUID: GUID = GUID::from_u128(0x548A3D08_85CB_4CA4_880E_9250544F5FB8);
const CHEWING_ZH_CN_PROFILE_GUID: GUID = GUID::from_u128(0x7A4480B4_F40C_4002_A674_243A502EF40E);
const CHEWING_TIP_DESC: PCWSTR =
    w!("0x0404:{DE733D27-7EEB-4C3B-9EEC-715F05B5BA85}{548A3D08-85CB-4CA4-880E-9250544F5FB8}");

const CATEGORIES: [GUID; 7] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_COMLESS,
];

fn register(icon_path: String) -> Result<()> {
    unsafe {
        let input_processor_profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let pw_icon_path = icon_path.encode_utf16().collect::<Vec<_>>();

        // Register for zh_TW
        let mut lcid = LocaleNameToLCID(w!("zh-TW"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x404;
        }
        input_processor_profile_mgr.RegisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_TW_PROFILE_GUID,
            w!("精靈語輸入法").as_wide(),
            &pw_icon_path,
            0,
            HKL::default(),
            0,
            false,
            0,
        )?;
        // Register for zh_CN
        let mut lcid = LocaleNameToLCID(w!("zh-CN"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x804;
        }
        input_processor_profile_mgr.RegisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_CN_PROFILE_GUID,
            w!("精灵语输入法").as_wide(),
            &pw_icon_path,
            0,
            HKL::default(),
            0,
            false,
            0,
        )?;

        let category_manager: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for tfcat in &CATEGORIES {
            category_manager.RegisterCategory(&CHEWING_TSF_CLSID, tfcat, &CHEWING_TSF_CLSID)?;
        }
    }

    #[cfg(feature = "nightly")]
    {
        // Enable user-mode minidump for debug build
        if let Err(error) = LOCAL_MACHINE
            .create("SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting\\LocalDumps")
        {
            println!("Error: unable to enable user-mode minidump: {error}");
        }
    }
    Ok(())
}

fn unregister() -> Result<()> {
    unsafe {
        let input_processor_profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let category_manager: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for tfcat in &CATEGORIES {
            if let Err(error) =
                category_manager.UnregisterCategory(&CHEWING_TSF_CLSID, tfcat, &CHEWING_TSF_CLSID)
            {
                println!("Failed to unregister category {tfcat:?}: {error}");
            }
        }

        // Unregister zh_TW profile
        let mut lcid = LocaleNameToLCID(w!("zh-TW"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x404;
        }
        input_processor_profile_mgr.UnregisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_TW_PROFILE_GUID,
            0,
        )?;
        // Unregister zh_CN profile
        let mut lcid = LocaleNameToLCID(w!("zh-CN"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x804;
        }
        input_processor_profile_mgr.UnregisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_CN_PROFILE_GUID,
            0,
        )?;
    }

    Ok(())
}

fn enable() {
    unsafe {
        InstallLayoutOrTip(CHEWING_TIP_DESC.as_ptr(), ILOT_INSTALL);
    }
}

fn disable() {
    // Don't uninstall the layout for now. If the last layout of a language is
    // uninstalled then Windows changes the system locale to English or another
    // available language.
    //
    // Ref: https://github.com/chewing/windows-chewing-tsf/issues/553
    //
    // unsafe {
    //     InstallLayoutOrTip(CHEWING_TIP_DESC.as_ptr(), ILOT_UNINSTALL);
    // }
}

fn stop() {
    if let Ok(client) = ChewingIpcClient::connect() {
        if let Err(error) = client.send(MethodCall {
            method: Stop::METHOD.to_string(),
            parameters: Value::Null,
            oneway: Some(true),
            more: None,
            upgrade: None,
        }) {
            println!("Error: failed to stop chewing_tip_host: {error:?}");
        }
    }
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn export_profile(path: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::from_reg().unwrap_or_default();
    fs::write(path, cfg.to_profile_json()?)?;
    Ok(())
}

fn import_profile(path: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(path)?;
    let cfg = Config::from_profile_json(&raw)?;
    cfg.save_reg();
    Ok(())
}

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "-x" | "--diagnostics" | "--export-profile" | "--import-profile"
        )
    }) {
        unsafe {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }

    if args.iter().any(|arg| arg == "-x" || arg == "--diagnostics") {
        let report = collect_diagnostics();
        match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                println!("Error: failed to serialize diagnostics: {error}");
                process::exit(1);
            }
        }
        return Ok(());
    }

    if let Some(path) = option_value(&args, "--export-profile") {
        if let Err(error) = export_profile(&path) {
            println!("Error: failed to export profile: {error}");
            process::exit(1);
        }
        println!("Exported profile to {path}");
        return Ok(());
    }

    if let Some(path) = option_value(&args, "--import-profile") {
        if let Err(error) = import_profile(&path) {
            println!("Error: failed to import profile: {error}");
            process::exit(1);
        }
        println!("Imported profile from {path}");
        return Ok(());
    }

    unsafe {
        if IsDebuggerPresent().as_bool() {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        if args.len() == 1 {
            println!("Usage:");
            println!("  tsfreg -r <IconPath>    註冊輸入法");
            println!("  tsfreg -i           立即啟用輸入法");
            println!("  tsfreg -d           立即停用輸入法");
            println!("  tsfreg -u                 取消註冊");
            println!("  tsfreg -s   停止 chewing_tip_host");
            println!("  tsfreg -x           匯出診斷資訊(JSON)");
            println!("  tsfreg --export-profile <Path>  匯出設定 profile");
            println!("  tsfreg --import-profile <Path>  匯入設定 profile");
            process::exit(1);
        }

        if let Some("-r") = args.get(1).map(String::as_str) {
            let icon_path = args.get(2).expect("缺少 IconPath").clone();
            register(icon_path)?;
        } else if let Some("-i") = args.get(1).map(String::as_str) {
            enable();
        } else if let Some("-d") = args.get(1).map(String::as_str) {
            disable();
        } else if let Some("-s") = args.get(1).map(String::as_str) {
            stop();
        } else if let Err(err) = unregister() {
            println!("警告：無法解除輸入法註冊，反安裝可能無法正常完成。");
            println!("錯誤訊息：{:?}", err);
        }
    }

    Ok(())
}
