//! Key types (independent of any terminal library) and Vim-style key notation:
//! `"ihello<Esc>"`, `"<C-r>"`, `"<CR>"`.

/// A key, without modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A printable char.
    Char(char),
    /// Escape.
    Esc,
    /// Enter / Return.
    Enter,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Delete.
    Delete,
}

/// Modifier keys held with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    /// Control.
    pub ctrl: bool,
    /// Alt / Meta.
    pub alt: bool,
    /// Shift. For `Char` keys the char is already shifted (`'A'`), so this is informational.
    pub shift: bool,
}

/// A key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// Which key.
    pub code: KeyCode,
    /// Held modifiers.
    pub mods: Modifiers,
}

impl Key {
    /// A key without modifiers.
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            mods: Modifiers {
                ctrl: false,
                alt: false,
                shift: false,
            },
        }
    }

    /// Ctrl + `c`.
    pub const fn ctrl(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            mods: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
            },
        }
    }
}

/// Parse Vim key notation into keys.
///
/// Supported names (case-insensitive): `<Esc> <CR> <Enter> <BS> <Tab> <Space> <lt>
/// <Up> <Down> <Left> <Right> <Home> <End> <PageUp> <PageDown> <Del>`, and `<C-x>` for
/// Ctrl+char. A `<` that does not start a known name is a literal `<`.
pub fn parse_keys(s: &str) -> Vec<Key> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '<'
            && let Some(end) = rest.find('>')
            && let Some(key) = parse_name(&rest[1..end])
        {
            out.push(key);
            rest = &rest[end + 1..];
            continue;
        }
        out.push(Key::new(KeyCode::Char(c)));
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Parse the inside of `<...>` (e.g. `Esc`, `C-r`) into a key.
fn parse_name(name: &str) -> Option<Key> {
    let lower = name.to_ascii_lowercase();
    if let Some(ch) = lower.strip_prefix("c-") {
        let mut chars = ch.chars();
        return match (chars.next(), chars.next()) {
            (Some(c), None) => Some(Key::ctrl(c)),
            _ => None,
        };
    }
    let code = match lower.as_str() {
        "esc" => KeyCode::Esc,
        "cr" | "enter" => KeyCode::Enter,
        "bs" => KeyCode::Backspace,
        "tab" => KeyCode::Tab,
        "space" => KeyCode::Char(' '),
        "lt" => KeyCode::Char('<'),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "del" => KeyCode::Delete,
        _ => return None,
    };
    Some(Key::new(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(c: char) -> Key {
        Key::new(KeyCode::Char(c))
    }

    #[test]
    fn parses_plain_and_named() {
        assert_eq!(
            parse_keys("ia<Esc><C-r><cr>"),
            vec![
                ch('i'),
                ch('a'),
                Key::new(KeyCode::Esc),
                Key::ctrl('r'),
                Key::new(KeyCode::Enter),
            ]
        );
    }

    #[test]
    fn unknown_angle_is_literal() {
        assert_eq!(parse_keys("a<b"), vec![ch('a'), ch('<'), ch('b')]);
        assert_eq!(
            parse_keys("<foo>"),
            "<foo>".chars().map(ch).collect::<Vec<_>>()
        );
        assert_eq!(parse_keys("<lt>"), vec![ch('<')]);
    }
}
