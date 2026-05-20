//! Engine smoke tests. Use libchewing's own seed dictionaries (vendored under
//! `fixtures/`) to verify the adapter wiring on every supported platform.
//!
//! The seed dict only contains ㄘㄜˋ→{測,策}, ㄕˋ→試, and the phrase 測試, so
//! the assertions stay tight on that vocabulary.

use std::path::{Path, PathBuf};

use chewing::editor::BasicEditor;
use chewing_engine_kit::keysim::qwerty;
use chewing_engine_kit::{EmbeddedDicts, EngineConfig, EnginePaths, build_editor, build_editor_embedded};
use tempfile::TempDir;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn make_editor_with(cfg: &EngineConfig, user_dict: &Path) -> chewing::editor::Editor {
    let dir = fixtures_dir();
    let paths = EnginePaths {
        search_dirs: &[dir.as_path()],
        user_dict: Some(user_dict),
        enabled_dicts: &["word.dat", "tsi.dat"],
    };
    build_editor(cfg, &paths)
}

#[test]
fn qwerty_zhuyin_produces_preedit() {
    let tmp = TempDir::new().unwrap();
    let mut editor = make_editor_with(&EngineConfig::default(), &tmp.path().join("user.dat"));

    // QWERTY 'h k 4' = ㄘ ㄜ ˋ → preedit shows 測 or 策.
    for evt in qwerty(b"hk4") {
        editor.process_keyevent(evt);
    }
    let preedit = editor.display();
    assert!(
        preedit == "測" || preedit == "策",
        "unexpected preedit {preedit:?}"
    );
}

#[test]
fn qwerty_zhuyin_phrase_test() {
    let tmp = TempDir::new().unwrap();
    let mut editor = make_editor_with(&EngineConfig::default(), &tmp.path().join("user.dat"));

    // ㄘㄜˋㄕˋ should resolve to the phrase 測試 from tsi.dat.
    for evt in qwerty(b"hk4g4") {
        editor.process_keyevent(evt);
    }
    assert_eq!("測試", editor.display());
}

#[test]
fn default_config_uses_chewing_engine() {
    use chewing::editor::ConversionEngineKind;
    let tmp = TempDir::new().unwrap();
    let editor = make_editor_with(&EngineConfig::default(), &tmp.path().join("user.dat"));
    assert!(matches!(
        editor.editor_options().conversion_engine,
        ConversionEngineKind::ChewingEngine
    ));
}

#[test]
fn partial_syllable_match_forces_fuzzy() {
    use chewing::editor::ConversionEngineKind;
    let mut cfg = EngineConfig::default();
    cfg.partial_syllable_match = true;
    let tmp = TempDir::new().unwrap();
    let editor = make_editor_with(&cfg, &tmp.path().join("user.dat"));
    assert!(matches!(
        editor.editor_options().conversion_engine,
        ConversionEngineKind::FuzzyChewingEngine
    ));
}

#[test]
fn partial_syllable_match_with_no_search_dirs_uses_minidat() {
    // 模擬 web sandbox: search_dirs 空,libchewing 會 fallback 到內建 mini.dat
    // (Trie 格式,支援 FuzzyPartialPrefix). 確認此情境下單按聲母 + Down 能進入
    // 候選字選擇模式而不是 spin_bell.
    use chewing::input::{KeyboardEvent, keycode, keysym};
    let mut cfg = EngineConfig::default();
    cfg.partial_syllable_match = true;
    let tmp = TempDir::new().unwrap();
    let user_dict = tmp.path().join("user.dat");
    let paths = EnginePaths {
        search_dirs: &[],
        user_dict: Some(user_dict.as_path()),
        enabled_dicts: EnginePaths::DEFAULT_DICTS,
    };
    let mut editor = build_editor(&cfg, &paths);

    // 'h' = ㄘ 聲母
    for evt in qwerty(b"h") {
        editor.process_keyevent(evt);
    }
    // 此時 syllable buffer 持有 ㄘ;Down 應 flush partial syllable 並開選字窗.
    let down = KeyboardEvent::builder()
        .code(keycode::KEY_DOWN)
        .ksym(keysym::SYM_DOWN)
        .build();
    editor.process_keyevent(down);

    let cands = editor.paginated_candidates().unwrap_or_default();
    assert!(
        !cands.is_empty(),
        "partial-prefix lookup on built-in mini.dat 應該回傳至少一個 ㄘ 開頭的候選字"
    );
}

#[test]
fn embedded_dicts_resolve_multisyllable_phrase() {
    // The web demo embeds real `.dat` images instead of reading from disk
    // (no filesystem in wasm). Drive the embedded path with the fixture dicts
    // and confirm a two-syllable phrase (測試) converts as one unit — the same
    // mechanism that lets 台灣 be selected as a phrase rather than 台/灣 alone.
    const WORD: &[u8] = include_bytes!("../fixtures/word.dat");
    const TSI: &[u8] = include_bytes!("../fixtures/tsi.dat");

    let mut editor = build_editor_embedded(
        &EngineConfig::default(),
        &EmbeddedDicts {
            system_dicts: &[WORD, TSI],
            symbols: None,
        },
    );

    // ㄘㄜˋㄕˋ → the phrase 測試 from the embedded tsi.dat.
    for evt in qwerty(b"hk4g4") {
        editor.process_keyevent(evt);
    }
    assert_eq!("測試", editor.display());
}
