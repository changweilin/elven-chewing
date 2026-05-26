#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::c_void;
use std::mem::size_of;

use chewing_tip_core::config::{ChewingTsfConfig, Config};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{DEFAULT_GUI_FONT, GetStockObject, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, CB_ADDSTRING,
    CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, GWLP_USERDATA, GetDlgItem, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HCURSOR, HMENU,
    IDC_ARROW, LoadCursorW, MB_ICONERROR, MB_ICONINFORMATION, MSG, MessageBoxW, PostQuitMessage,
    RegisterClassExW, SW_SHOW, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW, SetWindowPos,
    SetWindowTextW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND,
    WM_CREATE, WM_DESTROY, WM_NCCREATE, WM_SETFONT, WNDCLASSEXW, WS_BORDER, WS_CAPTION, WS_CHILD,
    WS_CLIPCHILDREN, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{HSTRING, PCWSTR, w};

const WINDOW_CLASS: PCWSTR = w!("ElvenImeConfigWindow");
const WINDOW_TITLE: PCWSTR = w!("設定精靈語輸入法");

const ID_OUTPUT_SIMP: i32 = 1001;
const ID_PARTIAL_MATCH: i32 = 1002;
const ID_DUAL_MODE: i32 = 1003;
const ID_SHOW_CAND_SPACE: i32 = 1004;
const ID_SWITCH_SHIFT: i32 = 1005;
const ID_SHOW_NOTIFICATION: i32 = 1006;
const ID_AUTO_LEARN: i32 = 1007;
const ID_CAPS_LOCK: i32 = 1008;
const ID_DEFAULT_ENGLISH: i32 = 1009;
const ID_SYNC_OPEN_CLOSE: i32 = 1010;
const ID_ESC_CLEAN_ALL: i32 = 1011;
const ID_FULL_SHAPE_SYMBOLS: i32 = 1012;
const ID_ADVANCE_AFTER_SELECTION: i32 = 1013;

const ID_CAND_PER_PAGE: i32 = 1101;
const ID_CAND_PER_ROW: i32 = 1102;
const ID_FONT_SIZE: i32 = 1103;
const ID_SHIFT_SENSITIVITY: i32 = 1104;
const ID_FONT_FAMILY: i32 = 1105;

const ID_CONV_ENGINE: i32 = 1201;
const ID_KEYBOARD_LAYOUT: i32 = 1202;
const ID_SEL_KEY_TYPE: i32 = 1203;
const ID_SIMULATE_LAYOUT: i32 = 1204;

const ID_SAVE: i32 = 2001;
const ID_RESET: i32 = 2002;
const ID_CANCEL: i32 = 2003;
const BST_CHECKED_VALUE: usize = 1;

const CONV_ENGINE_OPTIONS: &[(i32, &str)] =
    &[(0, "簡易引擎"), (1, "標準 libchewing"), (2, "模糊注音引擎")];

const KEYBOARD_LAYOUT_OPTIONS: &[(i32, &str)] = &[
    (0, "標準注音"),
    (1, "許氏鍵盤"),
    (2, "IBM 鍵盤"),
    (3, "精業鍵盤"),
    (4, "ET 鍵盤"),
    (5, "ET26 鍵盤"),
    (6, "標準注音於 Dvorak"),
    (7, "許氏鍵盤於 Dvorak"),
    (8, "大千 26 鍵"),
    (9, "漢語拼音"),
    (10, "通用拼音"),
    (11, "注音二式"),
    (12, "Carpalx"),
    (13, "Colemak-DH ANSI"),
    (14, "Colemak-DH 正交"),
    (15, "Workman"),
    (16, "Colemak"),
];

const SIMULATE_LAYOUT_OPTIONS: &[(i32, &str)] = &[
    (0, "不反向映射"),
    (1, "Dvorak"),
    (2, "QGMLWY"),
    (3, "Colemak"),
    (4, "Colemak-DH ANSI"),
    (5, "Colemak-DH 正交"),
    (6, "Workman"),
];

const SEL_KEY_OPTIONS: &[(i32, &str)] = &[
    (0, "1234567890"),
    (1, "asdfghjkl;"),
    (2, "asdfzxcv89"),
    (3, "asdfjkl789"),
    (4, "aoeuhtn789"),
    (5, "1234qweras"),
];

fn main() {
    if let Err(error) = run() {
        let text = HSTRING::from(format!("無法開啟設定：{error}"));
        unsafe {
            MessageBoxW(None, PCWSTR(text.as_ptr()), WINDOW_TITLE, MB_ICONERROR);
        }
    }
}

fn run() -> windows::core::Result<()> {
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None)? }.into();
    unsafe {
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_else(|_| HCURSOR::default()),
            hbrBackground: HBRUSH((windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as _),
            lpszClassName: WINDOW_CLASS,
            lpfnWndProc: Some(wnd_proc),
            ..Default::default()
        };
        RegisterClassExW(&wc);
    }

    let state = Box::new(AppState::load());
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WINDOW_CLASS,
            WINDOW_TITLE,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            650,
            None,
            None,
            Some(hinst),
            Some(Box::into_raw(state).cast::<c_void>()),
        )?
    };
    center_window(hwnd);
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.into() {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

struct AppState {
    config: Config,
    default_font: HFONT,
}

impl AppState {
    fn load() -> Self {
        let config = Config::from_reg().unwrap_or_default();
        let default_font = unsafe { HFONT(GetStockObject(DEFAULT_GUI_FONT).0) };
        Self {
            config,
            default_font,
        }
    }

    fn create_controls(&self, hwnd: HWND) {
        create_label(hwnd, 20, 18, 500, 24, "常用設定");

        let mut y = 50;
        for (id, label, checked) in [
            (
                ID_OUTPUT_SIMP,
                "輸出簡體中文",
                self.config.chewing_tsf.output_simp_chinese,
            ),
            (
                ID_PARTIAL_MATCH,
                "模糊注音猜詞",
                self.config.chewing_tsf.partial_syllable_match,
            ),
            (
                ID_DUAL_MODE,
                "雙排輸入模式",
                self.config.chewing_tsf.dual_input_mode,
            ),
            (
                ID_SHOW_CAND_SPACE,
                "空白鍵顯示候選字",
                self.config.chewing_tsf.show_cand_with_space_key,
            ),
            (
                ID_SWITCH_SHIFT,
                "短按 Shift 切換中文/英文",
                self.config.chewing_tsf.switch_lang_with_shift,
            ),
            (
                ID_SHOW_NOTIFICATION,
                "顯示狀態通知",
                self.config.chewing_tsf.show_notification,
            ),
            (
                ID_AUTO_LEARN,
                "啟用自動學習",
                self.config.chewing_tsf.enable_auto_learn,
            ),
            (
                ID_CAPS_LOCK,
                "啟用 Caps Lock 行為",
                self.config.chewing_tsf.enable_caps_lock,
            ),
            (
                ID_DEFAULT_ENGLISH,
                "預設英文模式",
                self.config.chewing_tsf.default_english,
            ),
            (
                ID_SYNC_OPEN_CLOSE,
                "同步 Windows 中文/英數模式",
                self.config.chewing_tsf.sync_lang_mode_openclose,
            ),
            (
                ID_ESC_CLEAN_ALL,
                "Esc 清除全部緩衝",
                self.config.chewing_tsf.esc_clean_all_buf,
            ),
            (
                ID_FULL_SHAPE_SYMBOLS,
                "中文模式使用全形符號",
                self.config.chewing_tsf.full_shape_symbols,
            ),
            (
                ID_ADVANCE_AFTER_SELECTION,
                "選字後游標往後移動",
                self.config.chewing_tsf.advance_after_selection,
            ),
        ] {
            let checkbox = create_checkbox(hwnd, id, 28, y, 230, 24, label);
            set_checked(checkbox, checked);
            y += 30;
        }

        create_label(hwnd, 288, 50, 220, 22, "轉換引擎");
        let conv = create_combo(hwnd, ID_CONV_ENGINE, 288, 74, 220, 120);
        fill_combo(
            conv,
            CONV_ENGINE_OPTIONS,
            self.config.chewing_tsf.conv_engine,
        );

        create_label(hwnd, 288, 112, 220, 22, "注音鍵盤");
        let keyboard = create_combo(hwnd, ID_KEYBOARD_LAYOUT, 288, 136, 220, 240);
        fill_combo(
            keyboard,
            KEYBOARD_LAYOUT_OPTIONS,
            self.config.chewing_tsf.keyboard_layout,
        );

        create_label(hwnd, 288, 174, 220, 22, "英文模式反向映射");
        let simulate = create_combo(hwnd, ID_SIMULATE_LAYOUT, 288, 198, 220, 180);
        fill_combo(
            simulate,
            SIMULATE_LAYOUT_OPTIONS,
            self.config.chewing_tsf.simulate_english_layout,
        );

        create_label(hwnd, 288, 236, 220, 22, "選字鍵");
        let sel_key = create_combo(hwnd, ID_SEL_KEY_TYPE, 288, 260, 220, 180);
        fill_combo(
            sel_key,
            SEL_KEY_OPTIONS,
            self.config.chewing_tsf.sel_key_type,
        );

        let mut y = 328;
        create_labeled_edit(
            hwnd,
            ID_CAND_PER_PAGE,
            288,
            y,
            "候選字每頁",
            &self.config.chewing_tsf.cand_per_page.to_string(),
        );
        y += 54;
        create_labeled_edit(
            hwnd,
            ID_CAND_PER_ROW,
            288,
            y,
            "候選字每列",
            &self.config.chewing_tsf.cand_per_row.to_string(),
        );
        y += 54;
        create_labeled_edit(
            hwnd,
            ID_FONT_SIZE,
            288,
            y,
            "候選窗字體大小",
            &self.config.chewing_tsf.font_size.to_string(),
        );
        y += 54;
        create_labeled_edit(
            hwnd,
            ID_SHIFT_SENSITIVITY,
            288,
            y,
            "Shift 靈敏度 ms",
            &self.config.chewing_tsf.shift_key_sensitivity.to_string(),
        );
        y += 54;
        create_labeled_edit(
            hwnd,
            ID_FONT_FAMILY,
            288,
            y,
            "候選窗字型",
            &self.config.chewing_tsf.font_family,
        );

        create_button(hwnd, ID_SAVE, 288, 570, 90, 32, "儲存", true);
        create_button(hwnd, ID_RESET, 386, 570, 90, 32, "重設", false);
        create_button(hwnd, ID_CANCEL, 484, 570, 56, 32, "關閉", false);
        self.apply_default_font(hwnd);
    }

    fn apply_default_font(&self, hwnd: HWND) {
        for id in ID_OUTPUT_SIMP..=ID_ADVANCE_AFTER_SELECTION {
            set_font(dlg_item(hwnd, id), self.default_font);
        }
        for id in [
            ID_CAND_PER_PAGE,
            ID_CAND_PER_ROW,
            ID_FONT_SIZE,
            ID_SHIFT_SENSITIVITY,
            ID_FONT_FAMILY,
            ID_CONV_ENGINE,
            ID_KEYBOARD_LAYOUT,
            ID_SEL_KEY_TYPE,
            ID_SIMULATE_LAYOUT,
            ID_SAVE,
            ID_RESET,
            ID_CANCEL,
        ] {
            set_font(dlg_item(hwnd, id), self.default_font);
        }
    }

    fn save_from_controls(&mut self, hwnd: HWND) -> Result<(), String> {
        let cfg = &mut self.config.chewing_tsf;
        cfg.output_simp_chinese = is_checked(hwnd, ID_OUTPUT_SIMP);
        cfg.partial_syllable_match = is_checked(hwnd, ID_PARTIAL_MATCH);
        cfg.dual_input_mode = is_checked(hwnd, ID_DUAL_MODE);
        cfg.show_cand_with_space_key = is_checked(hwnd, ID_SHOW_CAND_SPACE);
        cfg.switch_lang_with_shift = is_checked(hwnd, ID_SWITCH_SHIFT);
        cfg.show_notification = is_checked(hwnd, ID_SHOW_NOTIFICATION);
        cfg.enable_auto_learn = is_checked(hwnd, ID_AUTO_LEARN);
        cfg.enable_caps_lock = is_checked(hwnd, ID_CAPS_LOCK);
        cfg.default_english = is_checked(hwnd, ID_DEFAULT_ENGLISH);
        cfg.sync_lang_mode_openclose = is_checked(hwnd, ID_SYNC_OPEN_CLOSE);
        cfg.esc_clean_all_buf = is_checked(hwnd, ID_ESC_CLEAN_ALL);
        cfg.full_shape_symbols = is_checked(hwnd, ID_FULL_SHAPE_SYMBOLS);
        cfg.advance_after_selection = is_checked(hwnd, ID_ADVANCE_AFTER_SELECTION);
        cfg.conv_engine = selected_value(hwnd, ID_CONV_ENGINE, CONV_ENGINE_OPTIONS);
        cfg.keyboard_layout = selected_value(hwnd, ID_KEYBOARD_LAYOUT, KEYBOARD_LAYOUT_OPTIONS);
        cfg.simulate_english_layout =
            selected_value(hwnd, ID_SIMULATE_LAYOUT, SIMULATE_LAYOUT_OPTIONS);
        cfg.sel_key_type = selected_value(hwnd, ID_SEL_KEY_TYPE, SEL_KEY_OPTIONS);
        cfg.cand_per_page = bounded_i32(hwnd, ID_CAND_PER_PAGE, 1, 10, "候選字每頁")?;
        cfg.cand_per_row = bounded_i32(hwnd, ID_CAND_PER_ROW, 1, 10, "候選字每列")?;
        cfg.font_size = bounded_i32(hwnd, ID_FONT_SIZE, 8, 48, "候選窗字體大小")?;
        cfg.shift_key_sensitivity =
            bounded_i32(hwnd, ID_SHIFT_SENSITIVITY, 50, 1000, "Shift 靈敏度")?;
        cfg.font_family = get_text(hwnd, ID_FONT_FAMILY).trim().to_string();
        if cfg.font_family.is_empty() {
            return Err("候選窗字型不可空白".to_string());
        }

        self.config.save_reg();
        Ok(())
    }

    fn reset_controls(&mut self, hwnd: HWND) {
        self.config.chewing_tsf = ChewingTsfConfig::default();
        self.refresh_controls(hwnd);
    }

    fn refresh_controls(&self, hwnd: HWND) {
        let cfg = &self.config.chewing_tsf;
        for (id, value) in [
            (ID_OUTPUT_SIMP, cfg.output_simp_chinese),
            (ID_PARTIAL_MATCH, cfg.partial_syllable_match),
            (ID_DUAL_MODE, cfg.dual_input_mode),
            (ID_SHOW_CAND_SPACE, cfg.show_cand_with_space_key),
            (ID_SWITCH_SHIFT, cfg.switch_lang_with_shift),
            (ID_SHOW_NOTIFICATION, cfg.show_notification),
            (ID_AUTO_LEARN, cfg.enable_auto_learn),
            (ID_CAPS_LOCK, cfg.enable_caps_lock),
            (ID_DEFAULT_ENGLISH, cfg.default_english),
            (ID_SYNC_OPEN_CLOSE, cfg.sync_lang_mode_openclose),
            (ID_ESC_CLEAN_ALL, cfg.esc_clean_all_buf),
            (ID_FULL_SHAPE_SYMBOLS, cfg.full_shape_symbols),
            (ID_ADVANCE_AFTER_SELECTION, cfg.advance_after_selection),
        ] {
            set_checked(dlg_item(hwnd, id), value);
        }
        select_combo(
            dlg_item(hwnd, ID_CONV_ENGINE),
            CONV_ENGINE_OPTIONS,
            cfg.conv_engine,
        );
        select_combo(
            dlg_item(hwnd, ID_KEYBOARD_LAYOUT),
            KEYBOARD_LAYOUT_OPTIONS,
            cfg.keyboard_layout,
        );
        select_combo(
            dlg_item(hwnd, ID_SIMULATE_LAYOUT),
            SIMULATE_LAYOUT_OPTIONS,
            cfg.simulate_english_layout,
        );
        select_combo(
            dlg_item(hwnd, ID_SEL_KEY_TYPE),
            SEL_KEY_OPTIONS,
            cfg.sel_key_type,
        );
        set_text(hwnd, ID_CAND_PER_PAGE, &cfg.cand_per_page.to_string());
        set_text(hwnd, ID_CAND_PER_ROW, &cfg.cand_per_row.to_string());
        set_text(hwnd, ID_FONT_SIZE, &cfg.font_size.to_string());
        set_text(
            hwnd,
            ID_SHIFT_SENSITIVITY,
            &cfg.shift_key_sensitivity.to_string(),
        );
        set_text(hwnd, ID_FONT_FAMILY, &cfg.font_family);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let create = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            let state = unsafe { (*create).lpCreateParams as *mut AppState };
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
            }
            LRESULT(1)
        }
        WM_CREATE => {
            if let Some(state) = state(hwnd) {
                state.create_controls(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = loword(wparam.0 as u32) as i32;
            let notify = hiword(wparam.0 as u32);
            if notify == BN_CLICKED as u16 {
                match id {
                    ID_SAVE => {
                        if let Some(state) = state_mut(hwnd) {
                            match state.save_from_controls(hwnd) {
                                Ok(()) => show_info(
                                    hwnd,
                                    "設定已儲存。請切換輸入法，或重新開啟正在輸入的應用程式，讓設定生效。",
                                ),
                                Err(error) => show_error(hwnd, &error),
                            }
                        }
                    }
                    ID_RESET => {
                        if let Some(state) = state_mut(hwnd) {
                            state.reset_controls(hwnd);
                        }
                    }
                    ID_CANCEL => unsafe {
                        let _ = DestroyWindow(hwnd);
                    },
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState };
            if !ptr.is_null() {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
            }
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn state(hwnd: HWND) -> Option<&'static AppState> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppState };
    (!ptr.is_null()).then(|| unsafe { &*ptr })
}

fn state_mut(hwnd: HWND) -> Option<&'static mut AppState> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState };
    (!ptr.is_null()).then(|| unsafe { &mut *ptr })
}

fn loword(value: u32) -> u16 {
    (value & 0xffff) as u16
}

fn hiword(value: u32) -> u16 {
    ((value >> 16) & 0xffff) as u16
}

fn center_window(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let screen_x = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
            );
            let screen_y = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
            );
            let x = (screen_x - width).max(0) / 2;
            let y = (screen_y - height).max(0) / 2;
            let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOZORDER);
        }
    }
}

fn create_label(parent: HWND, x: i32, y: i32, w: i32, h: i32, text: &str) -> HWND {
    create_control(
        parent,
        w!("STATIC"),
        text,
        WINDOW_STYLE::default(),
        x,
        y,
        w,
        h,
        0,
    )
}

fn create_checkbox(parent: HWND, id: i32, x: i32, y: i32, w: i32, h: i32, text: &str) -> HWND {
    create_control(
        parent,
        w!("BUTTON"),
        text,
        WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        x,
        y,
        w,
        h,
        id,
    )
}

fn create_combo(parent: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    create_control(
        parent,
        w!("COMBOBOX"),
        "",
        WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        x,
        y,
        w,
        h,
        id,
    )
}

fn create_labeled_edit(parent: HWND, id: i32, x: i32, y: i32, label: &str, value: &str) {
    create_label(parent, x, y, 220, 22, label);
    create_control(
        parent,
        w!("EDIT"),
        value,
        WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
        x,
        y + 24,
        220,
        24,
        id,
    );
}

fn create_button(parent: HWND, id: i32, x: i32, y: i32, w: i32, h: i32, text: &str, default: bool) {
    let style = WS_TABSTOP
        | if default {
            WINDOW_STYLE(BS_DEFPUSHBUTTON as u32)
        } else {
            WINDOW_STYLE::default()
        };
    create_control(parent, w!("BUTTON"), text, style, x, y, w, h, id);
}

fn create_control(
    parent: HWND,
    class_name: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
) -> HWND {
    let title = HSTRING::from(text);
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap_or_default() }.into();
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            PCWSTR(title.as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            Some(parent),
            child_menu(id),
            Some(hinst),
            None,
        )
        .unwrap_or_default()
    }
}

fn child_menu(id: i32) -> Option<HMENU> {
    (id != 0).then_some(HMENU(id as isize as *mut c_void))
}

fn set_font(hwnd: HWND, font: HFONT) {
    if !hwnd.is_invalid() {
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }
}

fn set_checked(hwnd: HWND, checked: bool) {
    unsafe {
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            Some(WPARAM(if checked { BST_CHECKED_VALUE } else { 0 })),
            Some(LPARAM(0)),
        );
    }
}

fn is_checked(parent: HWND, id: i32) -> bool {
    let hwnd = dlg_item(parent, id);
    unsafe {
        SendMessageW(hwnd, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0))).0
            == BST_CHECKED_VALUE as isize
    }
}

fn fill_combo(hwnd: HWND, options: &[(i32, &str)], selected_value: i32) {
    let mut selected = 0usize;
    for (index, (value, label)) in options.iter().enumerate() {
        let text = HSTRING::from(*label);
        unsafe {
            SendMessageW(
                hwnd,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }
        if *value == selected_value {
            selected = index;
        }
    }
    unsafe {
        SendMessageW(hwnd, CB_SETCURSEL, Some(WPARAM(selected)), Some(LPARAM(0)));
    }
}

fn select_combo(hwnd: HWND, options: &[(i32, &str)], selected_value: i32) {
    let selected = options
        .iter()
        .position(|(value, _)| *value == selected_value)
        .unwrap_or(0);
    unsafe {
        SendMessageW(hwnd, CB_SETCURSEL, Some(WPARAM(selected)), Some(LPARAM(0)));
    }
}

fn selected_value(parent: HWND, id: i32, options: &[(i32, &str)]) -> i32 {
    let hwnd = dlg_item(parent, id);
    let index = unsafe { SendMessageW(hwnd, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0 };
    options
        .get(index.max(0) as usize)
        .map(|(value, _)| *value)
        .unwrap_or(options[0].0)
}

fn bounded_i32(parent: HWND, id: i32, min: i32, max: i32, label: &str) -> Result<i32, String> {
    let raw = get_text(parent, id);
    let value = raw
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("{label} 必須是數字"))?;
    if !(min..=max).contains(&value) {
        return Err(format!("{label} 必須介於 {min} 到 {max}"));
    }
    Ok(value)
}

fn get_text(parent: HWND, id: i32) -> String {
    let hwnd = dlg_item(parent, id);
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buf = vec![0u16; len as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..copied as usize])
}

fn set_text(parent: HWND, id: i32, text: &str) {
    let hwnd = dlg_item(parent, id);
    let text = HSTRING::from(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(text.as_ptr()));
    }
}

fn show_info(parent: HWND, text: &str) {
    let text = HSTRING::from(text);
    unsafe {
        MessageBoxW(
            Some(parent),
            PCWSTR(text.as_ptr()),
            WINDOW_TITLE,
            MB_ICONINFORMATION,
        );
    }
}

fn show_error(parent: HWND, text: &str) {
    let text = HSTRING::from(text);
    unsafe {
        MessageBoxW(
            Some(parent),
            PCWSTR(text.as_ptr()),
            WINDOW_TITLE,
            MB_ICONERROR,
        );
    }
}

fn dlg_item(parent: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(Some(parent), id).unwrap_or_default() }
}
