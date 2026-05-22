//! Browser-facing wrapper around `chewing::editor::Editor`.
//!
//! Exposes the minimum surface JS needs to drive the engine: feed key bytes,
//! read back the preedit, walk candidate windows, and reconfigure the engine
//! through a [`DemoConfig`] that mirrors the user-visible fields of the
//! desktop IME's `ChewingTsfConfig`. There is no filesystem in
//! `wasm32-unknown-unknown`, so the real `word.dat` / `tsi.dat` / `symbols.dat`
//! are `include_bytes!`d into the bundle (staged by `build.rs`) and fed to the
//! engine via [`chewing_engine_kit::build_editor_embedded`]. That gives the
//! sandbox desktop-equivalent vocabulary — including multi-syllable phrase
//! selection — instead of libchewing's tiny built-in `mini.dat` fallback.
//!
//! `DemoConfig` is a superset of [`chewing_engine_kit::EngineConfig`]: engine
//! options are forwarded to libchewing through `build_editor_embedded`, while
//! state-machine settings (language mode, caps lock behavior, sel keys, simp
//! Chinese output, ...) are interpreted in this crate, the same way
//! `tip::text_service::chewing` does on Windows.

use chewing::dictionary::LookupStrategy;
use chewing::editor::{BasicEditor, CharacterForm, Editor, EditorKeyBehavior, LanguageMode};
use chewing::input::KeyboardEvent;
use chewing::input::keycode::{self, Keycode};
use chewing::input::keysym::{self, Keysym};
use chewing_engine_kit::keysim::{Special, keypad, qwerty};
use chewing_engine_kit::{
    EmbeddedDicts, EngineConfig, build_editor_embedded, settle_partial_syllable_before_navigation,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use zhconv::{Variant, zhconv};

/// Real libchewing dictionaries compiled into the wasm bundle. `build.rs`
/// stages these from `build/installer/Dictionary/` into `OUT_DIR`; embedding
/// them is what gives the browser the same vocabulary and phrase selection as
/// the desktop IME (there is no filesystem to load `.dat` files from).
const WORD_DAT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/word.dat"));
const TSI_DAT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tsi.dat"));
const SYMBOLS_DAT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/symbols.dat"));

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl DemoConfig {
    fn new_chewing_defaults() -> Self {
        // Match ChewingTsfConfig::new_chewing_defaults() in
        // crates/chewing_tip_core/src/config.rs so the web form starts with
        // the same defaults the desktop IME does where fields overlap.
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
            partial_syllable_match: true,
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

    fn microsoft_new_phonetic_defaults() -> Self {
        let mut cfg = Self::new_chewing_defaults();
        cfg.enable_fullwidth_toggle_key = true;
        cfg.esc_clean_all_buf = true;
        cfg.full_shape_symbols = false;
        cfg.easy_symbols_with_shift = false;
        cfg.easy_symbols_with_shift_ctrl = true;
        cfg.show_cand_with_space_key = true;
        cfg
    }
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self::new_chewing_defaults()
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
    pending_key_events: Vec<KeyboardEvent>,
    last_reconvert: Option<LastCommitSnapshot>,
    reconvert_break_pending: bool,
    active_preview: Option<ActivePreview>,
    pending_preview_commit: Option<LastCommitSnapshot>,
}

#[derive(Debug, Clone)]
struct LastCommitSnapshot {
    key_events: Vec<KeyboardEvent>,
    committed_text: String,
    mode: LanguageMode,
}

#[derive(Debug, Clone)]
struct ActivePreview {
    key_events: Vec<KeyboardEvent>,
    text: String,
    mode: LanguageMode,
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

impl From<LanguageMode> for LangMode {
    fn from(m: LanguageMode) -> Self {
        match m {
            LanguageMode::Chinese => LangMode::Chinese,
            LanguageMode::English => LangMode::English,
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
            pending_key_events: Vec::new(),
            last_reconvert: None,
            reconvert_break_pending: false,
            active_preview: None,
            pending_preview_commit: None,
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
        Self::new_chewing_default_config()
    }

    /// JSON for the New Chewing-style default [`DemoConfig`].
    #[wasm_bindgen(js_name = newChewingDefaultConfig)]
    pub fn new_chewing_default_config() -> String {
        serde_json::to_string(&DemoConfig::new_chewing_defaults())
            .expect("new chewing default config should serialise")
    }

    /// JSON for the Microsoft New Phonetic-style default [`DemoConfig`].
    #[wasm_bindgen(js_name = microsoftNewPhoneticDefaultConfig)]
    pub fn microsoft_new_phonetic_default_config() -> String {
        serde_json::to_string(&DemoConfig::microsoft_new_phonetic_defaults())
            .expect("microsoft new phonetic default config should serialise")
    }

    /// Drop any in-flight composition / candidate state without touching the
    /// engine config.
    #[wasm_bindgen(js_name = clear)]
    pub fn clear(&mut self) {
        self.editor.clear();
        self.clear_reconvert_scope();
        self.active_preview = None;
        self.pending_preview_commit = None;
    }

    /// Toggle 中/英 mode. Used by the lang-bar button on the web UI;
    /// equivalent to a short-shift press when `switch_lang_with_shift` is on.
    #[wasm_bindgen(js_name = toggleLangMode)]
    pub fn toggle_lang_mode(&mut self) {
        self.clear_reconvert_scope();
        self.active_preview = None;
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
        self.clear_reconvert_scope();
        self.active_preview = None;
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
        self.clear_reconvert_scope();
        self.active_preview = None;
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
        self.active_preview = None;
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
            let evt = byte_to_event(out as u8);
            self.track_key_event(evt);
            self.feed_event_with_mods(evt, modifiers);
            self.snapshot_commit_if_needed(LanguageMode::English);
            self.editor
                .set_editor_options(|opt| opt.language_mode = self.lang_mode.into());
            return;
        }

        // 純英文 + 半形, 直接送 ASCII (走 libchewing 的 English path 會 echo char).
        let in_english_mode = self.lang_mode == LangMode::English;
        let half_shape = self.editor.editor_options().character_form == CharacterForm::Halfwidth;
        if in_english_mode && half_shape && !self.is_selecting() {
            let evt = byte_to_event(byte);
            self.track_key_event(evt);
            self.feed_event_with_mods(evt, modifiers);
            self.snapshot_commit_if_needed(LanguageMode::English);
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
        } else {
            self.track_key_event(evt);
        }

        if momentary_english {
            let old = self.editor.editor_options().language_mode;
            self.editor
                .set_editor_options(|opt| opt.language_mode = LanguageMode::English);
            self.feed_event_with_mods(evt, modifiers);
            self.snapshot_commit_if_needed(LanguageMode::English);
            self.editor
                .set_editor_options(|opt| opt.language_mode = old);
        } else {
            self.feed_event_with_mods(evt, modifiers);
            self.snapshot_commit_if_needed(self.current_language_mode());
        }
    }

    /// Feed one numeric-keypad key (`0`–`9`, `+`, `-`, `*`, `/`, `.`, `=`).
    /// Unlike [`Self::feed_ascii`], keypad bytes are never reinterpreted as
    /// 注音 or selection keys: the engine commits the literal character when the
    /// buffer is empty, inserts it mid-composition, or — if a candidate window
    /// is open — selects by that number, exactly like a physical numpad on the
    /// desktop IME. Bytes outside the keypad set are ignored.
    #[wasm_bindgen(js_name = feedKeypad)]
    pub fn feed_keypad(&mut self, byte: u8, modifiers: u32) {
        self.active_preview = None;
        if let Some(evt) = keypad(byte) {
            self.feed_event_with_mods(evt, modifiers);
            self.snapshot_commit_if_needed(self.current_language_mode());
        }
    }

    /// Inject one of the named special keys (return, escape, arrows, F-keys,
    /// insert/delete, ...). `modifiers` is OR'd into the event state.
    #[wasm_bindgen(js_name = feedSpecial)]
    pub fn feed_special(&mut self, name: &str, modifiers: u32) -> bool {
        if self.handle_active_preview_special(name) {
            return true;
        }
        // Ctrl+F12: toggle simplified Chinese output (matches the default
        // keybind list in ChewingTsfConfig::default()).
        if name == "F12" && (modifiers & CONTROL) != 0 {
            self.toggle_simp_chinese();
            return true;
        }
        let Some(evt) = special_event(name) else {
            return false;
        };
        if name == "Backspace" {
            self.pending_key_events.pop();
        } else if !self.is_selecting() {
            self.track_key_event(evt);
        }
        let behavior = self.feed_event_with_mods(evt, modifiers);
        self.snapshot_commit_if_needed(self.current_language_mode());
        if behavior != EditorKeyBehavior::Commit && Self::is_reconvert_break_special(name) {
            self.clear_reconvert_scope();
        }
        true
    }

    /// Current preedit. Combines the converted intervals with the partial
    /// bopomofo syllable being entered, inserting the latter at the editor's
    /// cursor position. Mirrors the TSF preedit builder in
    /// `tip::text_service::chewing::update_preedit` (minus the per-segment
    /// display attributes the sandbox does not render).
    #[wasm_bindgen(js_name = display)]
    pub fn display(&self) -> String {
        if let Some(preview) = &self.active_preview {
            return preview.text.clone();
        }
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
        if let Some(commit) = &self.pending_preview_commit {
            return commit.committed_text.clone();
        }
        self.maybe_simp(self.editor.display_commit().to_string())
    }

    /// Clear the commit buffer after JS has appended it to the fake document.
    #[wasm_bindgen(js_name = ackCommit)]
    pub fn ack_commit(&mut self) {
        if let Some(commit) = self.pending_preview_commit.take() {
            self.last_reconvert = Some(commit);
            self.reconvert_break_pending = false;
            return;
        }
        self.editor.ack();
    }

    /// Swap the most recent committed span between raw English keystrokes and
    /// the Chinese output produced by replaying those same key events.
    ///
    /// Returns `{"delete_chars":N,"replacement":"..."}` as JSON, or an empty
    /// string when there is nothing safe to replace.
    #[wasm_bindgen(js_name = reconvertLastCommit)]
    pub fn reconvert_last_commit(&mut self) -> String {
        if let Some(result) = self.toggle_active_preview() {
            return result;
        }
        if let Some(result) = self.reconvert_active_composition_to_preview() {
            return result;
        }

        let Some(last) = self.last_reconvert.take() else {
            return String::new();
        };
        let replacement = match last.mode {
            LanguageMode::English => self.simulate_chinese_from_key_events(&last.key_events),
            LanguageMode::Chinese => Some(Self::event_text(&last.key_events)),
        };
        let Some(replacement) = replacement.filter(|s| !s.is_empty()) else {
            self.last_reconvert = Some(last);
            return String::new();
        };
        let delete_chars = last.committed_text.chars().count();
        if delete_chars == 0 {
            self.last_reconvert = Some(last);
            return String::new();
        }
        let new_mode = match last.mode {
            LanguageMode::English => LanguageMode::Chinese,
            LanguageMode::Chinese => LanguageMode::English,
        };
        self.last_reconvert = Some(LastCommitSnapshot {
            key_events: last.key_events,
            committed_text: replacement.clone(),
            mode: new_mode,
        });
        self.mark_reconvert_break_after_current();
        self.set_lang_mode(new_mode);
        self.editor.ack();
        Self::reconvert_result_json(delete_chars, &replacement)
    }

    fn preview_result_json(preview: &ActivePreview) -> String {
        serde_json::json!({
            "preview": true,
            "mode": match preview.mode {
                LanguageMode::Chinese => "chi",
                LanguageMode::English => "eng",
            },
            "replacement": preview.text.as_str(),
            "delete_chars": 0,
        })
        .to_string()
    }

    fn reconvert_result_json(delete_chars: usize, replacement: &str) -> String {
        serde_json::json!({
            "delete_chars": delete_chars,
            "replacement": replacement,
        })
        .to_string()
    }

    /// Candidate window contents for the current page, or empty if the
    /// engine is not in a candidate-selection state.
    ///
    /// `paginated_candidates()` can hand back more entries than the configured
    /// page size, so cap it at `cand_per_page` — mirroring the desktop TSF path
    /// (`text_service::chewing::update_candidates`, which does `items.truncate`)
    /// and keeping the list aligned with the `cand_per_page`-length sel keys.
    #[wasm_bindgen(js_name = candidates)]
    pub fn candidates(&self) -> Vec<JsValue> {
        let n = self.cfg.cand_per_page as usize;
        self.editor
            .paginated_candidates()
            .unwrap_or_default()
            .into_iter()
            .take(n)
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

    fn feed_event_with_mods(
        &mut self,
        mut evt: KeyboardEvent,
        modifiers: u32,
    ) -> EditorKeyBehavior {
        evt.state |= modifiers;
        settle_partial_syllable_before_navigation(&mut self.editor, &evt);
        self.editor.process_keyevent(evt)
    }

    fn toggle_active_preview(&mut self) -> Option<String> {
        let mut preview = self.active_preview.take()?;
        let (mode, text) = match preview.mode {
            LanguageMode::Chinese => (LanguageMode::English, Self::event_text(&preview.key_events)),
            LanguageMode::English => (
                LanguageMode::Chinese,
                self.simulate_chinese_from_key_events(&preview.key_events)?,
            ),
        };
        if text.is_empty() {
            self.active_preview = Some(preview);
            return None;
        }
        preview.mode = mode;
        preview.text = text;
        self.set_lang_mode(mode);
        let result = Self::preview_result_json(&preview);
        self.active_preview = Some(preview);
        Some(result)
    }

    fn reconvert_active_composition_to_preview(&mut self) -> Option<String> {
        if self.current_language_mode() != LanguageMode::Chinese
            || self.pending_key_events.is_empty()
            || self.is_selecting()
            || (self.editor.is_empty() && !self.editor.entering_syllable())
        {
            return None;
        }

        let key_events = std::mem::take(&mut self.pending_key_events);
        let replacement = Self::event_text(&key_events);
        if replacement.is_empty() {
            self.pending_key_events = key_events;
            return None;
        }

        self.editor.clear();
        let preview = ActivePreview {
            key_events,
            mode: LanguageMode::English,
            text: replacement,
        };
        self.set_lang_mode(LanguageMode::English);
        let result = Self::preview_result_json(&preview);
        self.active_preview = Some(preview);
        self.pending_key_events.clear();
        Some(result)
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

    fn handle_active_preview_special(&mut self, name: &str) -> bool {
        let Some(preview) = self.active_preview.take() else {
            return false;
        };
        match name {
            "Return" | "Enter" => {
                self.pending_preview_commit = Some(LastCommitSnapshot {
                    key_events: preview.key_events,
                    committed_text: preview.text,
                    mode: preview.mode,
                });
            }
            "Escape" | "Esc" => {
                // Drop the preview.
            }
            _ => {
                self.active_preview = Some(preview);
                return false;
            }
        }
        true
    }

    fn track_key_event(&mut self, evt: KeyboardEvent) {
        if Self::event_text_char(evt).is_some() {
            self.start_new_reconvert_segment_if_needed();
            self.pending_key_events.push(evt);
        }
    }

    fn clear_reconvert_scope(&mut self) {
        self.pending_key_events.clear();
        self.last_reconvert = None;
        self.reconvert_break_pending = false;
    }

    fn mark_reconvert_break_after_current(&mut self) {
        self.pending_key_events.clear();
        self.reconvert_break_pending = true;
    }

    fn start_new_reconvert_segment_if_needed(&mut self) {
        if self.reconvert_break_pending {
            self.last_reconvert = None;
            self.reconvert_break_pending = false;
        }
    }

    fn is_reconvert_break_special(name: &str) -> bool {
        !matches!(name, "Backspace" | "Space" | " ")
    }

    fn snapshot_commit_if_needed(&mut self, mode: LanguageMode) {
        if self.editor.last_key_behavior() != EditorKeyBehavior::Commit {
            return;
        }
        let committed_text = self.maybe_simp(self.editor.display_commit().to_string());
        if committed_text.is_empty() {
            return;
        }
        let key_events = std::mem::take(&mut self.pending_key_events);
        if key_events.is_empty() {
            return;
        }
        self.reconvert_break_pending = false;
        if mode == LanguageMode::English
            && let Some(last) = self.last_reconvert.as_mut()
            && last.mode == LanguageMode::English
        {
            last.key_events.extend(key_events);
            last.committed_text.push_str(&committed_text);
            return;
        }
        self.last_reconvert = Some(LastCommitSnapshot {
            key_events,
            committed_text,
            mode,
        });
    }

    fn event_text_char(evt: KeyboardEvent) -> Option<char> {
        evt.ksym
            .is_unicode()
            .then(|| evt.ksym.to_unicode())
            .filter(|c| c.is_ascii() && !c.is_ascii_control() && *c != '\0')
    }

    fn event_text(events: &[KeyboardEvent]) -> String {
        events
            .iter()
            .filter_map(|&evt| Self::event_text_char(evt))
            .collect()
    }

    fn normalize_replay_event(mut evt: KeyboardEvent) -> KeyboardEvent {
        evt.state &= 1 << 4;
        if evt.ksym.is_unicode() {
            let c = evt.ksym.to_unicode();
            if c.is_ascii_alphabetic() {
                evt.ksym = Keysym::from(c.to_ascii_lowercase());
            }
        }
        evt
    }

    fn simulate_chinese_from_key_events(&mut self, events: &[KeyboardEvent]) -> Option<String> {
        let old_mode = self.editor.editor_options().language_mode;
        self.editor
            .set_editor_options(|opt| opt.language_mode = LanguageMode::Chinese);
        self.editor.clear_syllable_editor();
        self.editor.clear_composition_editor();

        for &evt in events {
            self.editor
                .process_keyevent(Self::normalize_replay_event(evt));
        }
        Self::settle_partial_syllable_for_commit(&mut self.editor);
        let out = if self.editor.commit().is_ok() {
            Some(self.maybe_simp(self.editor.display_commit().to_string()))
        } else {
            None
        };

        self.editor.ack();
        self.editor.clear_syllable_editor();
        self.editor.clear_composition_editor();
        self.editor
            .set_editor_options(|opt| opt.language_mode = old_mode);
        out
    }

    fn settle_partial_syllable_for_commit(editor: &mut Editor) {
        if editor.editor_options().lookup_strategy != LookupStrategy::FuzzyPartialPrefix
            || !editor.entering_syllable()
        {
            return;
        }

        let down = KeyboardEvent::builder()
            .code(keycode::KEY_DOWN)
            .ksym(keysym::SYM_DOWN)
            .build();
        editor.process_keyevent(down);
        if editor.is_selecting() {
            let _ = editor.cancel_selecting();
        }
    }

    fn current_language_mode(&self) -> LanguageMode {
        self.lang_mode.into()
    }

    fn set_lang_mode(&mut self, mode: LanguageMode) {
        self.lang_mode = mode.into();
        self.editor
            .set_editor_options(|opt| opt.language_mode = mode);
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
    qwerty(&[byte]).into_iter().next().unwrap_or_else(|| {
        let mut b = KeyboardEvent::builder();
        b.ksym(Keysym(byte as u32));
        b.build()
    })
}

fn fresh_editor(cfg: &EngineConfig) -> Editor {
    build_editor_embedded(
        cfg,
        &EmbeddedDicts {
            // word.dat then tsi.dat, matching EnginePaths::DEFAULT_DICTS order.
            system_dicts: &[WORD_DAT, TSI_DAT],
            symbols: Some(SYMBOLS_DAT),
        },
    )
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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{ChewingDemo, DemoConfig};

    fn drain_commit(demo: &mut ChewingDemo, out: &mut String) {
        let committed = demo.display_commit();
        if !committed.is_empty() {
            out.push_str(&committed);
            demo.ack_commit();
        }
    }

    #[test]
    fn default_config_uses_new_chewing_preset() {
        let cfg: DemoConfig = serde_json::from_str(&ChewingDemo::default_config()).unwrap();
        assert_eq!(cfg, DemoConfig::new_chewing_defaults());
        assert!(cfg.partial_syllable_match);
    }

    #[test]
    fn microsoft_new_phonetic_preset_keeps_new_features_enabled() {
        let cfg: DemoConfig =
            serde_json::from_str(&ChewingDemo::microsoft_new_phonetic_default_config()).unwrap();
        assert!(cfg.partial_syllable_match);
        assert!(cfg.enable_fullwidth_toggle_key);
        assert!(cfg.esc_clean_all_buf);
        assert!(!cfg.full_shape_symbols);
        assert!(!cfg.easy_symbols_with_shift);
        assert!(cfg.easy_symbols_with_shift_ctrl);
        assert!(cfg.show_cand_with_space_key);
    }

    #[test]
    fn reconvert_english_commit_uses_key_events() {
        let mut demo = ChewingDemo::new();
        demo.toggle_lang_mode();
        let mut doc = String::new();

        for byte in b"hk4" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("hk4", doc);

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(3), result["delete_chars"].as_u64());
        let replacement = result["replacement"].as_str().unwrap();
        assert!(!replacement.is_empty());
        assert_ne!("hk4", replacement);
        assert_eq!("chi", demo.lang_mode_str());

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!("hk4", result["replacement"].as_str().unwrap());
        assert_eq!("eng", demo.lang_mode_str());
    }

    #[test]
    fn reconvert_chinese_commit_restores_raw_keys() {
        let mut demo = ChewingDemo::new();
        let mut doc = String::new();

        for byte in b"hk4" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        demo.feed_special("Return", 0);
        drain_commit(&mut demo, &mut doc);
        assert!(!doc.is_empty());
        assert_ne!("hk4", doc);

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!("hk4", result["replacement"].as_str().unwrap());
        assert_eq!("eng", demo.lang_mode_str());
    }

    #[test]
    fn reconvert_english_to_chinese_keeps_following_input_chinese() {
        let mut demo = ChewingDemo::new();
        demo.toggle_lang_mode();
        let mut doc = String::new();

        for byte in b"hk4" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("hk4", doc);

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(3), result["delete_chars"].as_u64());
        assert_eq!("chi", demo.lang_mode_str());

        demo.feed_ascii(b'h', 0);
        assert!(demo.display_commit().is_empty());
        assert!(!demo.display().is_empty());
    }

    #[test]
    fn reconvert_active_chinese_composition_restores_raw_keys() {
        let mut demo = ChewingDemo::new();

        for byte in b"test" {
            demo.feed_ascii(*byte, 0);
        }

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(0), result["delete_chars"].as_u64());
        assert_eq!(Some(true), result["preview"].as_bool());
        assert_eq!("test", result["replacement"].as_str().unwrap());
        assert_eq!("test", demo.display());
        assert_eq!("eng", demo.lang_mode_str());

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(true), result["preview"].as_bool());
        assert_ne!("test", result["replacement"].as_str().unwrap());
        assert_eq!("chi", demo.lang_mode_str());
    }

    #[test]
    fn active_preview_commit_can_be_reconverted_after_ack() {
        let mut demo = ChewingDemo::new();
        let mut doc = String::new();

        for byte in b"test" {
            demo.feed_ascii(*byte, 0);
        }
        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!("test", result["replacement"].as_str().unwrap());

        assert!(demo.feed_special("Return", 0));
        drain_commit(&mut demo, &mut doc);
        assert_eq!("test", doc);
        assert!(demo.display().is_empty());

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(4), result["delete_chars"].as_u64());
        assert_ne!("test", result["replacement"].as_str().unwrap());
    }

    #[test]
    fn shift_breaks_before_active_chinese_reconvert() {
        let mut demo = ChewingDemo::new();
        demo.toggle_lang_mode();
        let mut doc = String::new();

        for byte in b"test" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("test", doc);

        demo.toggle_lang_mode();
        for byte in b"test" {
            demo.feed_ascii(*byte, 0);
        }

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(0), result["delete_chars"].as_u64());
        doc.push_str(result["replacement"].as_str().unwrap());
        assert_eq!("testtest", doc);
    }

    #[test]
    fn lang_toggle_breaks_reconvert_scope() {
        let mut demo = ChewingDemo::new();
        demo.toggle_lang_mode();
        let mut doc = String::new();

        for byte in b"hk4" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("hk4", doc);

        demo.toggle_lang_mode();
        assert!(demo.reconvert_last_commit().is_empty());
    }

    #[test]
    fn reconvert_breaks_before_following_english_input() {
        let mut demo = ChewingDemo::new();
        let mut doc = String::new();

        for byte in b"test" {
            demo.feed_ascii(*byte, 0);
        }
        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(0), result["delete_chars"].as_u64());
        doc.push_str(result["replacement"].as_str().unwrap());

        for byte in b"hk4" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("testhk4", doc);

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(3), result["delete_chars"].as_u64());
        assert!(!result["replacement"].as_str().unwrap().is_empty());
    }

    #[test]
    fn plain_space_does_not_break_english_reconvert_scope() {
        let mut demo = ChewingDemo::new();
        demo.toggle_lang_mode();
        let mut doc = String::new();

        for byte in b"hk4 hk4" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("hk4 hk4", doc);

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(7), result["delete_chars"].as_u64());
    }

    #[test]
    fn enter_breaks_english_reconvert_scope() {
        let mut demo = ChewingDemo::new();
        demo.toggle_lang_mode();
        let mut doc = String::new();

        for byte in b"test" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("test", doc);

        assert!(demo.feed_special("Return", 0));
        assert!(demo.reconvert_last_commit().is_empty());
    }

    #[test]
    fn reconvert_english_initials_phrase_settles_partial_prefix() {
        let mut demo = ChewingDemo::new();
        let mut cfg = super::DemoConfig::default();
        cfg.partial_syllable_match = true;
        demo.apply_config(&serde_json::to_string(&cfg).unwrap())
            .unwrap();
        demo.toggle_lang_mode();
        let mut doc = String::new();

        for byte in b"5cae" {
            demo.feed_ascii(*byte, 0);
            drain_commit(&mut demo, &mut doc);
        }
        assert_eq!("5cae", doc);

        let raw = demo.reconvert_last_commit();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(Some(4), result["delete_chars"].as_u64());
        assert_eq!("中華民國", result["replacement"].as_str().unwrap());
    }
}
