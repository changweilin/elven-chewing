//! Golden traces for input behaviour that tends to regress when libchewing or
//! TSF-facing state-machine code changes.
//!
//! These tests intentionally live in `chewing_engine_kit`: the same adapter
//! powers headless CI and the web demo, so the traces stay runnable without a
//! Windows desktop session.

use std::path::{Path, PathBuf};

use chewing::editor::BasicEditor;
use chewing_engine_kit::keysim::{Special, keypad, qwerty};
use chewing_engine_kit::{EngineConfig, EnginePaths, build_editor};
use tempfile::TempDir;

#[derive(Debug, PartialEq, Eq)]
struct Frame {
    label: &'static str,
    preedit: String,
    commit: String,
    selecting: bool,
    candidates: Vec<String>,
}

impl Frame {
    fn capture(label: &'static str, editor: &mut chewing::editor::Editor) -> Self {
        Self {
            label,
            preedit: editor.display(),
            commit: editor.display_commit().to_string(),
            selecting: editor.is_selecting(),
            candidates: editor.paginated_candidates().unwrap_or_default(),
        }
    }
}

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
fn phrase_commit_trace() {
    let tmp = TempDir::new().unwrap();
    let mut editor = make_editor_with(&EngineConfig::default(), &tmp.path().join("user.dat"));

    for evt in qwerty(b"hk4g4") {
        editor.process_keyevent(evt);
    }
    let before_commit = Frame::capture("after ㄘㄜˋㄕˋ", &mut editor);

    editor.process_keyevent(Special::Return.event());
    let after_commit = Frame::capture("after return", &mut editor);

    assert_eq!(
        vec![before_commit, after_commit],
        vec![
            Frame {
                label: "after ㄘㄜˋㄕˋ",
                preedit: "測試".to_string(),
                commit: String::new(),
                selecting: false,
                candidates: Vec::new(),
            },
            Frame {
                label: "after return",
                preedit: String::new(),
                commit: "測試".to_string(),
                selecting: false,
                candidates: Vec::new(),
            },
        ]
    );
}

#[test]
fn candidate_selection_trace() {
    let tmp = TempDir::new().unwrap();
    let mut editor = make_editor_with(&EngineConfig::default(), &tmp.path().join("user.dat"));

    for evt in qwerty(b"hk4") {
        editor.process_keyevent(evt);
    }
    editor.process_keyevent(Special::Down.event());
    let selecting = Frame::capture("candidate window", &mut editor);
    assert!(
        selecting.candidates.len() >= 2,
        "ㄘㄜˋ should expose 測/策 candidates, got {selecting:?}"
    );

    let selected = selecting.candidates[1].clone();
    editor.process_keyevent(keypad(b'2').expect("'2' is a keypad key"));
    let after_select = Frame::capture("after keypad 2", &mut editor);

    assert_eq!(selecting.label, "candidate window");
    assert!(selecting.selecting);
    assert_eq!(after_select.preedit, selected);
    assert_eq!(after_select.commit, "");
    assert!(!after_select.selecting);
}

#[test]
fn partial_prefix_lookup_trace() {
    let tmp = TempDir::new().unwrap();
    let mut cfg = EngineConfig::default();
    cfg.partial_syllable_match = true;
    let mut editor = make_editor_with(&cfg, &tmp.path().join("user.dat"));

    // 'h' is the ㄘ initial. In partial-prefix mode, Down should materialize
    // that incomplete syllable and open a candidate list instead of ringing the
    // bell or losing the buffer.
    for evt in qwerty(b"h") {
        editor.process_keyevent(evt);
    }
    editor.process_keyevent(Special::Down.event());
    let selecting = Frame::capture("partial ㄘ down", &mut editor);

    assert_eq!(selecting.label, "partial ㄘ down");
    assert!(
        selecting.selecting,
        "expected candidate mode, got {selecting:?}"
    );
    assert!(
        !selecting.candidates.is_empty(),
        "partial-prefix lookup should return candidates, got {selecting:?}"
    );
}
