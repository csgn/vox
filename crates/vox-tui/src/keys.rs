//! Vim-style key notation: `"ihello<Esc>"`, `"<C-r>"`, `"<CR>"`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Parse Vim key notation into key events.
///
/// Supported names (case-insensitive): `<Esc> <CR> <Enter> <BS> <Tab> <Space> <lt>
/// <Up> <Down> <Left> <Right> <Home> <End> <PageUp> <PageDown> <Del>`, and `<C-x>` for
/// Ctrl+char. A `<` that does not start a known name is a literal `<`.
pub fn parse_keys(s: &str) -> Vec<KeyEvent> {
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
        out.push(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn parse_name(name: &str) -> Option<KeyEvent> {
    let plain = |code| Some(KeyEvent::new(code, KeyModifiers::NONE));
    let lower = name.to_ascii_lowercase();
    if let Some(ch) = lower.strip_prefix("c-") {
        let mut chars = ch.chars();
        return match (chars.next(), chars.next()) {
            (Some(c), None) => Some(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
            _ => None,
        };
    }
    match lower.as_str() {
        "esc" => plain(KeyCode::Esc),
        "cr" | "enter" => plain(KeyCode::Enter),
        "bs" => plain(KeyCode::Backspace),
        "tab" => plain(KeyCode::Tab),
        "space" => plain(KeyCode::Char(' ')),
        "lt" => plain(KeyCode::Char('<')),
        "up" => plain(KeyCode::Up),
        "down" => plain(KeyCode::Down),
        "left" => plain(KeyCode::Left),
        "right" => plain(KeyCode::Right),
        "home" => plain(KeyCode::Home),
        "end" => plain(KeyCode::End),
        "pageup" => plain(KeyCode::PageUp),
        "pagedown" => plain(KeyCode::PageDown),
        "del" => plain(KeyCode::Delete),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn parses_plain_and_named() {
        assert_eq!(
            parse_keys("ia<Esc><C-r><cr>"),
            vec![
                ch('i'),
                ch('a'),
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
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
