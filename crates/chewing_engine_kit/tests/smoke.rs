//! Engine smoke tests. Use libchewing's own seed dictionaries (vendored under
//! `fixtures/`) to verify the adapter wiring on every supported platform.
//!
//! The seed dict only contains ㄘㄜˋ→{測,策}, ㄕˋ→試, and the phrase 測試, so
//! the assertions stay tight on that vocabulary.

use std::path::{Path, PathBuf};

use chewing::editor::BasicEditor;
use chewing_engine_kit::keysim::qwerty;
use chewing_engine_kit::{EngineConfig, EnginePaths, build_editor};
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
