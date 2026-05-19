//! Translate compact key descriptions into [`chewing::input::KeyboardEvent`]s.
//!
//! Two entry points:
//!
//! * [`ascii_seq`] — feed a UTF-8 byte string to a `QWERTY`/`DVORAK`/... map
//!   and get back a vector of events. Convenient for注音/英文一般字元.
//! * [`special`] — build a special-key event (arrows, return, esc, space) by
//!   name. These have no ASCII representation in libchewing's keymap.

use chewing::input::keymap::{Keymap, QWERTY_MAP, map_ascii};
use chewing::input::{KeyboardEvent, keycode, keysym};

/// Translate every byte of `text` through `keymap`, dropping bytes that yield
/// the default (empty) event.
pub fn ascii_seq(keymap: &Keymap, text: &[u8]) -> Vec<KeyboardEvent> {
    let blank = KeyboardEvent::default();
    text.iter()
        .map(|&b| map_ascii(keymap, b))
        .filter(|evt| *evt != blank)
        .collect()
}

/// Convenience wrapper that always uses the QWERTY keymap.
pub fn qwerty(text: &[u8]) -> Vec<KeyboardEvent> {
    ascii_seq(&QWERTY_MAP, text)
}

/// Named special keys used by the chewing state machine.
#[derive(Debug, Clone, Copy)]
pub enum Special {
    Return,
    Escape,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
}

impl Special {
    pub fn event(self) -> KeyboardEvent {
        let (code, ksym) = match self {
            Special::Return => (keycode::KEY_ENTER, keysym::SYM_RETURN),
            Special::Escape => (keycode::KEY_ESC, keysym::SYM_ESC),
            Special::Backspace => (keycode::KEY_BACKSPACE, keysym::SYM_BACKSPACE),
            Special::Tab => (keycode::KEY_TAB, keysym::SYM_TAB),
            Special::Up => (keycode::KEY_UP, keysym::SYM_UP),
            Special::Down => (keycode::KEY_DOWN, keysym::SYM_DOWN),
            Special::Left => (keycode::KEY_LEFT, keysym::SYM_LEFT),
            Special::Right => (keycode::KEY_RIGHT, keysym::SYM_RIGHT),
            Special::Home => (keycode::KEY_HOME, keysym::SYM_HOME),
            Special::End => (keycode::KEY_END, keysym::SYM_END),
            Special::PageUp => (keycode::KEY_PAGEUP, keysym::SYM_PAGEUP),
            Special::PageDown => (keycode::KEY_PAGEDOWN, keysym::SYM_PAGEDOWN),
            Special::Space => (keycode::KEY_SPACE, keysym::Keysym(b' ' as u32)),
        };
        let mut b = KeyboardEvent::builder();
        b.code(code).ksym(ksym);
        b.build()
    }
}
