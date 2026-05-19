//! Browser-facing wrapper around `chewing::editor::Editor`.
//!
//! Exposes the minimum surface JS needs to drive the engine: feed key bytes,
//! read back the preedit, walk candidate windows, and reconfigure the engine
//! through a [`DemoConfig`] that mirrors the user-visible fields of the
//! desktop IME's `ChewingTsfConfig`. Dictionary data ships as the built-in
//! `mini.dat` libchewing falls back to when no filesystem dicts are visible
//! — there is no filesystem in `wasm32-unknown-unknown`.
//!
//! `DemoConfig` is a superset of [`chewing_engine_kit::EngineConfig`]: engine
//! options are forwarded to libchewing through `build_editor`, while
//! state-machine settings (language mode, caps lock behavior, sel keys, simp
//! Chinese output, ...) are interpreted in this crate, the same way
//! `tip::text_service::chewing` does on Windows.

use chewing::editor::{BasicEditor, CharacterForm, Editor, LanguageMode};
use chewing::input::KeyboardEvent;
use chewing::input::keycode::{self, Keycode};
use chewing::input::keysym::{self, Keysym};
use chewing_engine_kit::keysim::{Special, qwerty};
use chewing_engine_kit::{EngineConfig, EnginePaths, build_editor};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use zhconv::{Variant, zhconv};

#[wasm_bindgen(start)]
pub fn _start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// `KeyState` bit constants exposed to JS via `MOD.*`. Kept in sync with
/// `chewing::input::KeyState` (libchewing's `#[repr(u32)]` enum).
const SHIFT: u32 = 1 << 0;
const CAPSLOCK_BIT: u32 = 1 << 1;
const CONTROL: u32 = 1 << 2;

/// Six well-known selection-key sets, matching `SEL_KEYS` in
/// `tip/src/text_service/chewing.rs`.
const SEL_KEYS: [&str; 6] = [
    "1234567890",
    "asdfghjkl;",
    "asdfzxcv89",
    "asdfjkl789",
    "aoeuhtn789",
    "1234qweras",
];

/// Web-facing settings — every user-visible knob from `ChewingTsfConfig` that
/// has a meaningful effect inside the sandbox. Engine-only fields are
/// forwarded to [`EngineConfig`]; the rest drive the state machine in
/// [`ChewingDemo`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DemoConfig {
    // --- 引擎: 直接餵給 libchewing 的 EditorOptions ---
    pub keyboard_layout: u8,
    pub conv_engine: u8,
    pub cand_per_page: u8,
    pub cand_per_row: u8,
    pub easy_symbols_with_shift: bool,
    pub easy_symbols_with_shift_ctrl: bool,
    pub add_phrase_forward: bool,
    pub phrase_choice_rearward: bool,
    pub advance_after_selection: bool,
    pub esc_clean_all_buf: bool,
    pub show_cand_with_space_key: bool,
    pub enable_auto_learn: bool,
    pub enable_fullwidth_toggle_key: bool,
    pub sort_candidates_by_frequency: bool,
    pub partial_syllable_match: bool,
    pub cursor_cand_list: bool,

    // --- 狀態機: 由 web_demo 自己解釋 ---
    pub default_english: bool,
    pub default_full_space: bool,
    pub output_simp_chinese: bool,
    pub full_shape_symbols: bool,
    pub upper_case_with_shift: bool,
    pub switch_lang_with_shift: bool,
    pub shift_key_sensitivity: u32,
    pub enable_caps_lock: bool,
    pub lock_chinese_on_caps_lock: bool,
    pub sel_key_type: u8,
    pub simulate_english_layout: u8,
    pub show_notification: bool,
    pub sync_lang_mode_openclose: bool,

    // --- 顯示: 套用到 web 候選字面板 ---
    pub font_size: u32,
    pub font_family: String,
    pub font_fg_color: String,
    pub font_bg_color: String,
    pub font_highlight_fg_color: String,
    pub font_highlight_bg_color: String,
    pub font_number_fg_color: String,
    pub cand_list_border_color: String,
    pub notify_fg_color: String,
    pub notify_bg_color: String,
    pub notify_border_color: String,
}

impl Default for DemoConfig {
    fn default() -> Self {
        // Match ChewingTsfConfig::default() in crates/chewing_tip_core/src/config.rs
        // so the web form starts with the same defaults the desktop IME does.
        Self {
            keyboard_layout: 0,
            conv_engine: 1,
            cand_per_page: 9,
            cand_per_row: 3,
            easy_symbols_with_shift: true,
            easy_symbols_with_shift_ctrl: false,
            add_phrase_forward: true,
            phrase_choice_rearward: false,
            advance_after_selection: true,
            esc_clean_all_buf: false,
            show_cand_with_space_key: false,
            enable_auto_learn: true,
            enable_fullwidth_toggle_key: false,
            sort_candidates_by_frequency: false,
            partial_syllable_match: false,
            cursor_cand_list: true,

            default_english: false,
            default_full_space: false,
            output_simp_chinese: false,
            full_shape_symbols: true,
            upper_case_with_shift: false,
            switch_lang_with_shift: true,
            shift_key_sensitivity: 200,
            enable_caps_lock: false,
            lock_chinese_on_caps_lock: true,
            sel_key_type: 0,
            simulate_english_layout: 0,
            show_notification: true,
            sync_lang_mode_openclose: false,

            font_size: 16,
            font_family: "Segoe UI".to_owned(),
            font_fg_color: "000000FF".to_owned(),
            font_bg_color: "FAFAFAFF".to_owned(),
            font_highlight_fg_color: "FFFFFFFF".to_owned(),
            font_highlight_bg_color: "000000FF".to_owned(),
            font_number_fg_color: "0000FFFF".to_owned(),
            cand_list_border_color: "D6D9DBFF".to_owned(),
            notify_fg_color: "000000FF".to_owned(),
            notify_bg_color: "FCFBDAFF".to_owned(),
            notify_border_color: "D6D9DBFF".to_owned(),
        }
    }
}

impl DemoConfig {
    fn engine(&self) -> EngineConfig {
        EngineConfig {
            keyboard_layout: self.keyboard_layout,
            conv_engine: self.conv_engine,
            cand_per_page: self.cand_per_page,
            easy_symbols_with_shift: self.easy_symbols_with_shift,
            easy_symbols_with_shift_ctrl: self.easy_symbols_with_shift_ctrl,
            add_phrase_forward: self.add_phrase_forward,
            phrase_choice_rearward: self.phrase_choice_rearward,
            advance_after_selection: self.advance_after_selection,
            esc_clean_all_buf: self.esc_clean_all_buf,
            show_cand_with_space_key: self.show_cand_with_space_key,
            enable_auto_learn: self.enable_auto_learn,
            enable_fullwidth_toggle_key: self.enable_fullwidth_toggle_key,
            sort_candidates_by_frequency: self.sort_candidates_by_frequency,
            partial_syllable_match: self.partial_syllable_match,
        }
    }
}

#[wasm_bindgen]
pub struct ChewingDemo {
    editor: Editor,
    cfg: DemoConfig,
    lang_mode: LangMode,
    /// Cached `output_simp_chinese`. The user-visible toggle lives on the
    /// instance so Ctrl+F12 can flip it without re-applying the whole config.
    output_simp_chinese: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LangMode {
    Chinese,
    English,
}

impl From<LangMode> for LanguageMode {
    fn from(m: LangMode) -> Self {
        match m {
            LangMode::Chinese => LanguageMode::Chinese,
            LangMode::English => LanguageMode::English,
        }
    }
}

#[wasm_bindgen]
impl ChewingDemo {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ChewingDemo {
        let cfg = DemoConfig::default();
        ChewingDemo::with_config(cfg)
    }

    fn with_config(cfg: DemoConfig) -> ChewingDemo {
        let mut editor = fresh_editor(&cfg.engine());
        let lang_mode = if cfg.default_english {
            LangMode::English
        } else {
            LangMode::Chinese
        };
        let char_form = if cfg.default_full_space {
            CharacterForm::Fullwidth
        } else {
            CharacterForm::Halfwidth
        };
        editor.set_editor_options(|opt| {
            opt.language_mode = lang_mode.into();
            opt.character_form = char_form;
        });
        let output_simp_chinese = cfg.output_simp_chinese;
        ChewingDemo {
            editor,
            cfg,
            lang_mode,
            output_simp_chinese,
        }
    }

    /// Replace the underlying editor with one built from `config_json`
    /// (must be the JSON serialisation of [`DemoConfig`]). Acts as both
    /// settings-apply and full reset of in-flight composition state.
    #[wasm_bindgen(js_name = applyConfig)]
    pub fn apply_config(&mut self, config_json: &str) -> Result<(), JsValue> {
        let cfg: DemoConfig =
            serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        *self = ChewingDemo::with_config(cfg);
        Ok(())
    }

    /// JSON for the default [`DemoConfig`]. Lets JS render the settings
    /// form without duplicating the schema.
    #[wasm_bindgen(js_name = defaultConfig)]
    pub fn default_config() -> String {
        serde_json::to_string(&DemoConfig::default()).expect("default config should serialise")
    }

    /// Drop any in-flight composition / candidate state without touching the
    /// engine config.
    #[wasm_bindgen(js_name = clear)]
    pub fn clear(&mut self) {
        self.editor.clear();
    }

    /// Toggle 中/英 mode. Used by the lang-bar button on the web UI;
    /// equivalent to a short-shift press when `switch_lang_with_shift` is on.
    #[wasm_bindgen(js_name = toggleLangMode)]
    pub fn toggle_lang_mode(&mut self) {
        self.lang_mode = match self.lang_mode {
            LangMode::Chinese => LangMode::English,
            LangMode::English => LangMode::Chinese,
        };
        self.editor
            .set_editor_options(|opt| opt.language_mode = self.lang_mode.into());
    }

    /// Toggle 全/半 shape. Mirrors libchewing's own toggle on Shift+Space,
    /// but reachable from the lang-bar button as well.
    #[wasm_bindgen(js_name = toggleShapeMode)]
    pub fn toggle_shape_mode(&mut self) {
        self.editor.set_editor_options(|opt| {
            opt.character_form = match opt.character_form {
                CharacterForm::Fullwidth => CharacterForm::Halfwidth,
                CharacterForm::Halfwidth => CharacterForm::Fullwidth,
            };
        });
    }

    /// Toggle simplified Chinese output. Equivalent to Ctrl+F12 on the
    /// desktop IME.
    #[wasm_bindgen(js_name = toggleSimpChinese)]
    pub fn toggle_simp_chinese(&mut self) {
        self.output_simp_chinese = !self.output_simp_chinese;
        self.cfg.output_simp_chinese = self.output_simp_chinese;
    }

    /// Current language mode as a short string ("chi" / "eng").
    #[wasm_bindgen(js_name = langMode)]
    pub fn lang_mode_str(&self) -> String {
        match self.lang_mode {
            LangMode::Chinese => "chi".into(),
            LangMode::English => "eng".into(),
        }
    }

    /// Current shape mode as a short string ("half" / "full").
    #[wasm_bindgen(js_name = shapeMode)]
    pub fn shape_mode_str(&self) -> String {
        match self.editor.editor_options().character_form {
            CharacterForm::Halfwidth => "half".into(),
            CharacterForm::Fullwidth => "full".into(),
        }
    }

    /// Whether simplified Chinese output is on.
    #[wasm_bindgen(js_name = simpChinese)]
    pub fn simp_chinese(&self) -> bool {
        self.output_simp_chinese
    }

    /// Feed a single ASCII byte through the QWERTY keymap. `modifiers` is an
    /// OR of `KeyState` bit values (see JS `MOD` constants).
    #[wasm_bindgen(js_name = feedAscii)]
    pub fn feed_ascii(&mut self, byte: u8, modifiers: u32) {
        let shift_on = modifiers & SHIFT != 0;
        let caps_on = modifiers & CAPSLOCK_BIT != 0;
        let ctrl_on = modifiers & CONTROL != 0;

        // CapsLock 行為 (enable_caps_lock + !lock_chinese_on_caps_lock):
        // CapsLock 開啟時直接送英數字, 不經過注音轉換.
        if self.cfg.enable_caps_lock && !self.cfg.lock_chinese_on_caps_lock && caps_on {
            let mut out = byte as char;
            if out.is_ascii_alphabetic() {
                // CapsLock + Shift = 小寫; CapsLock 單獨 = 大寫.
                out = if shift_on {
                    out.to_ascii_lowercase()
                } else {
                    out.to_ascii_uppercase()
                };
            }
            self.editor.set_editor_options(|opt| {
                opt.language_mode = LanguageMode::English;
            });
            self.feed_event_with_mods(byte_to_event(out as u8), modifiers);
            self.editor
                .set_editor_options(|opt| opt.language_mode = self.lang_mode.into());
            return;
        }

        // 純英文 + 半形, 直接送 ASCII (走 libchewing 的 English path 會 echo char).
        let in_english_mode = self.lang_mode == LangMode::English;
        let half_shape =
            self.editor.editor_options().character_form == CharacterForm::Halfwidth;
        if in_english_mode && half_shape && !self.is_selecting() {
            self.feed_event_with_mods(byte_to_event(byte), modifiers);
            return;
        }

        // Shift + 字母 (在中文模式下) -> momentary English mode,
        // 由 upper_case_with_shift 決定是否大寫.
        let mut momentary_english = false;
        let mut effective_byte = byte;
        if self.lang_mode == LangMode::Chinese
            && shift_on
            && !self.cfg.easy_symbols_with_shift
            && !(ctrl_on && self.cfg.easy_symbols_with_shift_ctrl)
        {
            let is_letter = byte.is_ascii_alphabetic();
            // full_shape_symbols 開啟 -> 非字母 Shift+符號交給 libchewing 作全形.
            if is_letter || !self.cfg.full_shape_symbols {
                momentary_english = true;
                effective_byte = if is_letter && self.cfg.upper_case_with_shift {
                    byte.to_ascii_uppercase()
                } else if is_letter {
                    byte.to_ascii_lowercase()
                } else {
                    byte
                };
            }
        }

        let mut evt = byte_to_event(effective_byte);
        // 選字模式: 把按下的 sel-key 對應到數字 1-9, 0.
        if self.is_selecting() {
            evt = self.map_sel_key(evt, effective_byte);
        }

        if momentary_english {
            let old = self.editor.editor_options().language_mode;
            self.editor
                .set_editor_options(|opt| opt.language_mode = LanguageMode::English);
            self.feed_event_with_mods(evt, modifiers);
            self.editor
                .set_editor_options(|opt| opt.language_mode = old);
        } else {
            self.feed_event_with_mods(evt, modifiers);
        }
    }

    /// Inject one of the named special keys (return, escape, arrows, F-keys,
    /// insert/delete, ...). `modifiers` is OR'd into the event state.
    #[wasm_bindgen(js_name = feedSpecial)]
    pub fn feed_special(&mut self, name: &str, modifiers: u32) -> bool {
        // Ctrl+F12: toggle simplified Chinese output (matches the default
        // keybind list in ChewingTsfConfig::default()).
        if name == "F12" && (modifiers & CONTROL) != 0 {
            self.toggle_simp_chinese();
            return true;
        }
        let Some(evt) = special_event(name) else {
            return false;
        };
        self.feed_event_with_mods(evt, modifiers);
        true
    }

    /// Current preedit. Combines the converted intervals with the partial
    /// bopomofo syllable being entered, inserting the latter at the editor's
    /// cursor position. Mirrors the TSF preedit builder in
    /// `tip::text_service::chewing::update_preedit` (minus the per-segment
    /// display attributes the sandbox does not render).
    #[wasm_bindgen(js_name = display)]
    pub fn display(&self) -> String {
        let bopomofo = self.editor.syllable_buffer_display();
        let out = if bopomofo.is_empty() {
            self.editor.display()
        } else {
            let cursor = self.editor.cursor();
            let mut out = String::new();
            let mut inserted = false;
            for it in self.editor.intervals() {
                if !inserted && it.start <= cursor && it.end >= cursor {
                    let head_len = cursor - it.start;
                    out.extend(it.text.chars().take(head_len));
                    out.push_str(&bopomofo);
                    out.extend(it.text.chars().skip(head_len));
                    inserted = true;
                } else {
                    out.push_str(&it.text);
                }
            }
            if !inserted {
                out.push_str(&bopomofo);
            }
            out
        };
        self.maybe_simp(out)
    }

    /// Text the engine wants the host application to commit. Empty when there
    /// is nothing pending. Calling this does NOT clear the buffer.
    #[wasm_bindgen(js_name = displayCommit)]
    pub fn display_commit(&self) -> String {
        self.maybe_simp(self.editor.display_commit().to_string())
    }

    /// Candidate window contents for the current page, or empty if the
    /// engine is not in a candidate-selection state.
    #[wasm_bindgen(js_name = candidates)]
    pub fn candidates(&self) -> Vec<JsValue> {
        self.editor
            .paginated_candidates()
            .unwrap_or_default()
            .into_iter()
            .map(|c| JsValue::from(self.maybe_simp(c)))
            .collect()
    }

    /// Selection-key labels (1234567890 / asdfghjkl; / ...) for the current
    /// sel_key_type. JS uses this to render the index above each candidate.
    #[wasm_bindgen(js_name = selKeys)]
    pub fn sel_keys(&self) -> String {
        let n = self.cfg.cand_per_page as usize;
        SEL_KEYS[self.cfg.sel_key_type.min(5) as usize]
            .chars()
            .take(n)
            .collect()
    }

    fn feed_event_with_mods(&mut self, mut evt: KeyboardEvent, modifiers: u32) {
        evt.state |= modifiers;
        self.editor.process_keyevent(evt);
    }

    fn maybe_simp(&self, s: String) -> String {
        if self.output_simp_chinese && !s.is_empty() {
            zhconv(&s, Variant::ZhHans)
        } else {
            s
        }
    }

    fn is_selecting(&self) -> bool {
        self.editor.is_selecting()
    }

    fn map_sel_key(&self, mut evt: KeyboardEvent, byte: u8) -> KeyboardEvent {
        let ch = byte as char;
        let set = SEL_KEYS[self.cfg.sel_key_type.min(5) as usize];
        if let Some(idx) = set.chars().position(|c| c == ch) {
            match idx {
                0..9 => {
                    evt.code = Keycode(keycode::KEY_1.0 + idx as u8);
                    evt.ksym = Keysym(keysym::SYM_1.0 + idx as u32);
                }
                _ => {
                    evt.code = keycode::KEY_0;
                    evt.ksym = keysym::SYM_0;
                }
            }
        }
        evt
    }
}

/// Translate a single ASCII byte into a KeyboardEvent using the QWERTY map.
/// Falls back to a bare keysym event for bytes the keymap drops (e.g. space
/// when it shouldn't go through the bopomofo layout — caller takes care of
/// that via [`special_event`] / [`Special::Space`]).
fn byte_to_event(byte: u8) -> KeyboardEvent {
    qwerty(&[byte])
        .into_iter()
        .next()
        .unwrap_or_else(|| {
            let mut b = KeyboardEvent::builder();
            b.ksym(Keysym(byte as u32));
            b.build()
        })
}

fn fresh_editor(cfg: &EngineConfig) -> Editor {
    let paths = EnginePaths {
        search_dirs: &[],
        user_dict: None,
        enabled_dicts: EnginePaths::DEFAULT_DICTS,
    };
    build_editor(cfg, &paths)
}

/// Translate a string name from the JS side into a `KeyboardEvent`. Handles
/// the names produced by both the on-screen keyboard's `data-special` attrs
/// and `window.keydown`'s `event.key` values. Returns `None` for unknown names.
fn special_event(name: &str) -> Option<KeyboardEvent> {
    let mapped = match name {
        "Return" | "Enter" => Some(Special::Return),
        "Escape" | "Esc" => Some(Special::Escape),
        "Backspace" => Some(Special::Backspace),
        "Tab" => Some(Special::Tab),
        "ArrowUp" => Some(Special::Up),
        "ArrowDown" => Some(Special::Down),
        "ArrowLeft" => Some(Special::Left),
        "ArrowRight" => Some(Special::Right),
        "Home" => Some(Special::Home),
        "End" => Some(Special::End),
        "PageUp" => Some(Special::PageUp),
        "PageDown" => Some(Special::PageDown),
        "Space" | " " => Some(Special::Space),
        _ => None,
    };
    if let Some(s) = mapped {
        return Some(s.event());
    }

    let (code, ksym) = match name {
        "Insert" => (keycode::KEY_INSERT, keysym::SYM_NONE),
        "Delete" => (keycode::KEY_DELETE, keysym::SYM_DELETE),
        "F1" => (keycode::KEY_F1, keysym::SYM_F1),
        "F2" => (keycode::KEY_F2, keysym::SYM_F2),
        "F3" => (keycode::KEY_F3, keysym::SYM_F3),
        "F4" => (keycode::KEY_F4, keysym::SYM_F4),
        "F5" => (keycode::KEY_F5, keysym::SYM_F5),
        "F6" => (keycode::KEY_F6, keysym::SYM_F6),
        "F7" => (keycode::KEY_F7, keysym::SYM_F7),
        "F8" => (keycode::KEY_F8, keysym::SYM_F8),
        "F9" => (keycode::KEY_F9, keysym::SYM_F9),
        "F10" => (keycode::KEY_F10, keysym::SYM_F10),
        "F11" => (keycode::KEY_F11, keysym::SYM_F11),
        "F12" => (keycode::KEY_F12, keysym::SYM_F12),
        _ => return None,
    };
    let mut b = KeyboardEvent::builder();
    b.code(code).ksym(ksym);
    Some(b.build())
}

