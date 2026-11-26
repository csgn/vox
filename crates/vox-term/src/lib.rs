//! Terminal adapter backed by [`crossterm`]: raw mode, key input as [`vox_editor::Key`],
//! and drawing a [`Surface`] with per-row diffing.

use std::io::{self, Stdout, Write};

use crossterm::cursor::{MoveTo, SetCursorStyle, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use vox_editor::{CursorShape, Editor, Key, KeyCode, Modifiers, Style, Surface};

/// An active terminal session. Restores the terminal when dropped.
pub struct Terminal {
    out: Stdout,
    /// Last drawn surface, for diffing.
    prev: Option<Surface>,
}

impl Terminal {
    /// Enter raw mode and the alternate screen. Installs a panic hook that restores the
    /// terminal before the panic message is printed.
    pub fn enter() -> io::Result<Self> {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            hook(info);
        }));
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen)?;
        Ok(Self { out, prev: None })
    }

    /// Terminal size (columns, rows).
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Draw `s`, rewriting only rows that changed since the last draw.
    pub fn draw(&mut self, s: &Surface) -> io::Result<()> {
        let full = self
            .prev
            .as_ref()
            .is_none_or(|p| p.width() != s.width() || p.height() != s.height());
        if full {
            queue!(self.out, Clear(ClearType::All))?;
        }
        for y in 0..s.height() {
            if !full && self.prev.as_ref().is_some_and(|p| p.row(y) == s.row(y)) {
                continue;
            }
            queue!(self.out, MoveTo(0, y))?;
            let mut style = None;
            for cell in s.row(y) {
                if cell.symbol.is_empty() {
                    continue; // second half of a wide char
                }
                if style != Some(cell.style) {
                    set_style(&mut self.out, cell.style)?;
                    style = Some(cell.style);
                }
                queue!(self.out, Print(&cell.symbol))?;
            }
            queue!(self.out, SetAttribute(Attribute::Reset))?;
        }
        let shape = match s.cursor_shape {
            CursorShape::Block => SetCursorStyle::SteadyBlock,
            CursorShape::Bar => SetCursorStyle::SteadyBar,
        };
        queue!(self.out, MoveTo(s.cursor.0, s.cursor.1), shape, Show)?;
        self.out.flush()?;
        self.prev = Some(s.clone());
        Ok(())
    }

    /// Wait for the next event. Returns a key for key presses, `None` for anything else
    /// (resize, key release, mouse).
    pub fn read_key(&mut self) -> io::Result<Option<Key>> {
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => Ok(convert(k)),
            Event::Resize(..) => {
                self.prev = None;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        restore();
    }
}

/// Best-effort terminal restore.
fn restore() {
    let mut out = io::stdout();
    let _ = execute!(
        out,
        SetAttribute(Attribute::Reset),
        SetCursorStyle::DefaultUserShape,
        Show,
        LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
}

/// Queue terminal attributes for `style`.
fn set_style(out: &mut Stdout, style: Style) -> io::Result<()> {
    queue!(out, SetAttribute(Attribute::Reset))?;
    if style.dim {
        queue!(out, SetAttribute(Attribute::Dim))?;
    }
    if style.reversed {
        queue!(out, SetAttribute(Attribute::Reverse))?;
    }
    Ok(())
}

/// Convert a crossterm key event into a vox key. `None` for keys vox does not use.
pub fn convert(k: event::KeyEvent) -> Option<Key> {
    use crossterm::event::{KeyCode as C, KeyModifiers as M};
    let code = match k.code {
        C::Char(c) => KeyCode::Char(c),
        C::Esc => KeyCode::Esc,
        C::Enter => KeyCode::Enter,
        C::Backspace => KeyCode::Backspace,
        C::Tab => KeyCode::Tab,
        C::Up => KeyCode::Up,
        C::Down => KeyCode::Down,
        C::Left => KeyCode::Left,
        C::Right => KeyCode::Right,
        C::Home => KeyCode::Home,
        C::End => KeyCode::End,
        C::PageUp => KeyCode::PageUp,
        C::PageDown => KeyCode::PageDown,
        C::Delete => KeyCode::Delete,
        _ => return None,
    };
    Some(Key {
        code,
        mods: Modifiers {
            ctrl: k.modifiers.contains(M::CONTROL),
            alt: k.modifiers.contains(M::ALT),
            shift: k.modifiers.contains(M::SHIFT),
        },
    })
}

/// Run `editor` in the terminal until it quits.
pub fn run(editor: &mut Editor) -> io::Result<()> {
    let mut term = Terminal::enter()?;
    while !editor.should_quit() {
        let (w, h) = term.size()?;
        editor.resize(w, h);
        term.draw(&editor.render())?;
        if let Some(key) = term.read_key()? {
            editor.handle_key(key);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode as C, KeyEvent, KeyModifiers as M};

    #[test]
    fn converts_keys() {
        assert_eq!(
            convert(KeyEvent::new(C::Char('r'), M::CONTROL)),
            Some(Key::ctrl('r'))
        );
        assert_eq!(
            convert(KeyEvent::new(C::Esc, M::NONE)),
            Some(Key::new(KeyCode::Esc))
        );
        let shifted = convert(KeyEvent::new(C::Char('A'), M::SHIFT)).unwrap();
        assert_eq!(shifted.code, KeyCode::Char('A'));
        assert!(shifted.mods.shift && !shifted.mods.ctrl);
        assert_eq!(convert(KeyEvent::new(C::F(1), M::NONE)), None);
    }
}
