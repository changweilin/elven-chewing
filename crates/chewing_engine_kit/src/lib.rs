//! Portable libchewing engine adapter.
//!
//! Mirrors [`tip::text_service::chewing::build_editor_from_cfg`] without any
//! Windows or filesystem dependency, so engine behaviour can be exercised by
//! `cargo test` on any platform and compiled to wasm for the web demo.
//!
//! Two layers:
//!
//! * [`EngineConfig`] — POD subset of [`ChewingTsfConfig`] that actually
//!   influences engine behaviour (layout, conv engine, options).
//! * [`build_editor`] — turns a config plus a set of dictionary paths into a
//!   ready-to-use [`chewing::editor::Editor`].
//!
//! The [`keysim`] module bundles helpers for feeding ASCII / virtual-key
//! sequences without dragging in the TSF event plumbing.

#![deny(unsafe_code)]

use std::path::Path;

use chewing::conversion::{ChewingEngine, FuzzyChewingEngine, SimpleEngine};
use chewing::dictionary::{Dictionary, Layered, LookupStrategy, Trie};
use chewing::editor::zhuyin_layout::{KeyboardLayoutCompat, SyllableEditor};
use chewing::editor::{
    AbbrevTable, BasicEditor, ConversionEngineKind, Editor, LaxUserFreqEstimate, SymbolSelector,
    UserPhraseAddDirection,
};
use chewing::input::{KeyboardEvent, keycode, keysym};

pub mod keysim;

/// Engine-relevant subset of `ChewingTsfConfig`. Keep field names in sync with
/// the registry-backed struct so a future migration can simply forward fields.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub keyboard_layout: u8,
    pub conv_engine: u8,
    pub cand_per_page: u8,
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
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            keyboard_layout: 0,
            conv_engine: 1,
            cand_per_page: 9,
            easy_symbols_with_shift: false,
            easy_symbols_with_shift_ctrl: false,
            add_phrase_forward: false,
            phrase_choice_rearward: false,
            advance_after_selection: true,
            esc_clean_all_buf: false,
            show_cand_with_space_key: false,
            enable_auto_learn: true,
            enable_fullwidth_toggle_key: false,
            sort_candidates_by_frequency: false,
            partial_syllable_match: true,
        }
    }
}

/// Filesystem paths the engine needs. Caller is responsible for providing them
/// — this crate intentionally does not touch the registry, AppContainer, or
/// Windows shell.
#[derive(Debug, Clone)]
pub struct EnginePaths<'a> {
    /// One or more directories holding `.dat` system dictionaries. Separator
    /// follows libchewing convention (`;` on Windows, `:` elsewhere).
    pub search_dirs: &'a [&'a Path],
    /// File path for the user phrase dictionary. `None` lets libchewing pick
    /// the default location, which on hosted CI is typically the user's home.
    pub user_dict: Option<&'a Path>,
    /// Names of dictionaries to enable, in load order.
    pub enabled_dicts: &'a [&'a str],
}

impl<'a> EnginePaths<'a> {
    pub const DEFAULT_DICTS: &'a [&'a str] =
        &["word.dat", "tsi.dat", "chewing.dat", "chewing-deleted.dat"];
}

/// Construct an `Editor` from `config` + `paths`. Mirrors the logic in
/// `tip/src/text_service/chewing.rs::build_editor_from_cfg`.
pub fn build_editor(config: &EngineConfig, paths: &EnginePaths<'_>) -> Editor {
    let sep = if cfg!(target_family = "windows") {
        ';'
    } else {
        ':'
    };
    let search_path = if paths.search_dirs.is_empty() {
        None
    } else {
        Some(
            paths
                .search_dirs
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(&sep.to_string()),
        )
    };
    let user_dict = paths.user_dict.map(|p| p.to_string_lossy().into_owned());

    let editor = Editor::chewing(search_path, user_dict, paths.enabled_dicts);
    configure_editor(editor, config)
}

/// In partial-prefix mode, a second still-incomplete syllable can sit in the
/// syllable editor while the first one is already in the composition editor.
/// Before cursor navigation, materialize that partial syllable so navigation
/// operates on the same two visible syllables the user sees.
pub fn settle_partial_syllable_before_navigation(editor: &mut Editor, evt: &KeyboardEvent) -> bool {
    if editor.editor_options().lookup_strategy != LookupStrategy::FuzzyPartialPrefix
        || editor.is_selecting()
        || !editor.entering_syllable()
        || editor.is_empty()
        || evt.has_modifiers()
        || !is_cursor_navigation_key(evt)
    {
        return false;
    }

    let before_len = editor.len();
    let down = KeyboardEvent::builder()
        .code(keycode::KEY_DOWN)
        .ksym(keysym::SYM_DOWN)
        .build();
    editor.process_keyevent(down);
    if editor.is_selecting() {
        let _ = editor.cancel_selecting();
    }

    editor.len() > before_len
}

fn is_cursor_navigation_key(evt: &KeyboardEvent) -> bool {
    matches!(
        evt.ksym,
        keysym::SYM_LEFT
            | keysym::SYM_RIGHT
            | keysym::SYM_HOME
            | keysym::SYM_END
            | keysym::SYM_PAGEUP
            | keysym::SYM_PAGEDOWN
    )
}

/// Dictionary payloads already resident in memory, for targets without a
/// filesystem — notably `wasm32-unknown-unknown` in the web demo. Each slice is
/// a libchewing `.dat` Trie image, i.e. the raw bytes of `word.dat` / `tsi.dat`.
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedDicts<'a> {
    /// System dictionaries in load order; typically `[word.dat, tsi.dat]` to
    /// match the desktop `EnginePaths::DEFAULT_DICTS` ordering.
    pub system_dicts: &'a [&'a [u8]],
    /// Optional symbol-selector table (`symbols.dat`) backing the `` ` `` menu.
    /// `None` yields an empty table, matching the desktop fallback when no
    /// `symbols.dat` is found.
    pub symbols: Option<&'a [u8]>,
}

/// Construct an `Editor` from `config` plus dictionaries already in memory,
/// bypassing the filesystem paths `Editor::chewing` requires.
///
/// The web demo `include_bytes!`s the real `word.dat` / `tsi.dat` and calls
/// this so the browser sandbox shares desktop's full vocabulary — including
/// multi-syllable phrase selection (e.g. 台灣 as one phrase) — instead of
/// falling back to libchewing's tiny built-in `mini.dat`, which only knows a
/// handful of single characters.
///
/// # Panics
/// Panics if an embedded `.dat` image fails to parse. These are compiled-in
/// assets, so a malformed image is a build mistake, not a runtime condition.
pub fn build_editor_embedded(config: &EngineConfig, dicts: &EmbeddedDicts<'_>) -> Editor {
    let system: Vec<Box<dyn Dictionary>> = dicts
        .system_dicts
        .iter()
        .map(|bytes| {
            let trie = Trie::new(*bytes)
                .expect("embedded system dictionary should be a valid libchewing .dat image");
            Box::new(trie) as Box<dyn Dictionary>
        })
        .collect();
    let layered = Layered::new(system);
    let estimate = LaxUserFreqEstimate::max_from(layered.user_dict());
    let symbols = match dicts.symbols {
        Some(bytes) => {
            SymbolSelector::new(bytes).expect("embedded symbols.dat should be a valid table")
        }
        None => SymbolSelector::new(b"".as_slice()).expect("empty symbol table is always valid"),
    };
    let editor = Editor::new(
        Box::new(ChewingEngine::new()),
        layered,
        estimate,
        AbbrevTable::new(),
        symbols,
    );
    configure_editor(editor, config)
}

/// Apply every `EngineConfig`-driven knob to a freshly built `editor`,
/// regardless of how its dictionaries were sourced. This is the shared tail of
/// [`build_editor`] and [`build_editor_embedded`], mirroring the option block in
/// `tip/src/text_service/chewing.rs::build_editor_from_cfg`.
fn configure_editor(mut editor: Editor, config: &EngineConfig) -> Editor {
    let (conv_kind, lookup) = match config.conv_engine {
        0 => (ConversionEngineKind::SimpleEngine, LookupStrategy::Standard),
        2 => (
            ConversionEngineKind::FuzzyChewingEngine,
            LookupStrategy::FuzzyPartialPrefix,
        ),
        _ => (
            ConversionEngineKind::ChewingEngine,
            LookupStrategy::Standard,
        ),
    };
    let (conv_kind, lookup) = if config.partial_syllable_match {
        (
            ConversionEngineKind::FuzzyChewingEngine,
            LookupStrategy::FuzzyPartialPrefix,
        )
    } else {
        (conv_kind, lookup)
    };

    editor.set_editor_options(|opt| {
        opt.easy_symbol_input =
            config.easy_symbols_with_shift || config.easy_symbols_with_shift_ctrl;
        opt.user_phrase_add_dir = if config.add_phrase_forward {
            UserPhraseAddDirection::Backward
        } else {
            UserPhraseAddDirection::Forward
        };
        opt.phrase_choice_rearward = config.phrase_choice_rearward;
        opt.auto_shift_cursor = config.advance_after_selection;
        opt.candidates_per_page = config.cand_per_page as usize;
        opt.esc_clear_all_buffer = config.esc_clean_all_buf;
        opt.space_is_select_key = config.show_cand_with_space_key;
        opt.disable_auto_learn_phrase = !config.enable_auto_learn;
        opt.enable_fullwidth_toggle_key = config.enable_fullwidth_toggle_key;
        opt.sort_candidates_by_frequency = config.sort_candidates_by_frequency;
        opt.conversion_engine = conv_kind;
        opt.lookup_strategy = lookup;
        opt.auto_snapshot_selections = true;
    });

    let kbtype = KeyboardLayoutCompat::try_from(config.keyboard_layout)
        .unwrap_or(KeyboardLayoutCompat::Default);
    editor.set_syllable_editor(syllable_editor_for(kbtype));

    match conv_kind {
        ConversionEngineKind::SimpleEngine => {
            editor.set_conversion_engine(Box::new(SimpleEngine::new()));
        }
        ConversionEngineKind::ChewingEngine => {
            editor.set_conversion_engine(Box::new(ChewingEngine::new()));
        }
        ConversionEngineKind::FuzzyChewingEngine => {
            editor.set_conversion_engine(Box::new(FuzzyChewingEngine::new()));
        }
    }
    editor
}

fn syllable_editor_for(kb: KeyboardLayoutCompat) -> Box<dyn SyllableEditor> {
    use chewing::editor::zhuyin_layout::{
        DaiChien26, Et, Et26, GinYieh, Hsu, Ibm, Pinyin, Standard,
    };
    match kb {
        KeyboardLayoutCompat::Hsu | KeyboardLayoutCompat::DvorakHsu => Box::new(Hsu::new()),
        KeyboardLayoutCompat::Ibm => Box::new(Ibm::new()),
        KeyboardLayoutCompat::GinYieh => Box::new(GinYieh::new()),
        KeyboardLayoutCompat::Et => Box::new(Et::new()),
        KeyboardLayoutCompat::Et26 => Box::new(Et26::new()),
        KeyboardLayoutCompat::DachenCp26 => Box::new(DaiChien26::new()),
        KeyboardLayoutCompat::HanyuPinyin => Box::new(Pinyin::hanyu()),
        KeyboardLayoutCompat::ThlPinyin => Box::new(Pinyin::thl()),
        KeyboardLayoutCompat::Mps2Pinyin => Box::new(Pinyin::mps2()),
        _ => Box::new(Standard::new()),
    }
}
