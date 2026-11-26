//! Normal/visual mode grammar: `["x][count](operator[count](motion|operator) | motion | action)`.

use crate::keys::{Key, KeyCode};

/// Result of parsing pending keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Parse<T> {
    /// Need more keys.
    Incomplete,
    /// Keys do not form a command; discard them.
    Invalid,
    /// A complete command.
    Done(T),
}

/// A parsed normal/visual mode command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cmd {
    /// Register from a `"x` prefix.
    pub register: Option<char>,
    /// Count, if one was typed (operator and motion counts multiplied).
    pub count: Option<usize>,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operator {
    Delete,
    Change,
    Yank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Motion(Motion),
    /// `dd`, `cc`, `yy`.
    Lines,
    /// Visual selection.
    Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motion {
    Left,
    Right,
    Up,
    Down,
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    FirstNonBlank,
    LineEnd,
    FileStart,
    FileEnd,
    SearchNext,
    SearchPrev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InsertAt {
    Before,
    After,
    LineStart,
    LineEnd,
    Below,
    Above,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scroll {
    HalfDown,
    HalfUp,
    PageDown,
    PageUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Move(Motion),
    Operate(Operator, Target),
    Insert(InsertAt),
    Paste { before: bool },
    Undo,
    Redo,
    Scroll(Scroll),
    Visual { line: bool },
    CommandLine,
    Search,
}

/// The char of a key typed without Ctrl, if it is a char key.
fn plain_char(key: &Key) -> Option<char> {
    match key.code {
        KeyCode::Char(c) if !key.mods.ctrl => Some(c),
        _ => None,
    }
}

/// Parse a count starting at `keys[*i]`. A leading `0` is not a count (it is a motion).
fn parse_count(keys: &[Key], i: &mut usize) -> Option<usize> {
    let mut count: Option<usize> = None;
    while let Some(d) = keys
        .get(*i)
        .and_then(plain_char)
        .and_then(|c| c.to_digit(10))
    {
        if d == 0 && count.is_none() {
            break;
        }
        count = Some(
            count
                .unwrap_or(0)
                .saturating_mul(10)
                .saturating_add(d as usize),
        );
        *i += 1;
    }
    count
}

/// Parse one motion (`w`, `$`, `gg`, ...) from the start of `keys`.
fn parse_motion(keys: &[Key]) -> Parse<Motion> {
    let Some(key) = keys.first() else {
        return Parse::Incomplete;
    };
    let m = match key.code {
        KeyCode::Left => Motion::Left,
        KeyCode::Right => Motion::Right,
        KeyCode::Up => Motion::Up,
        KeyCode::Down => Motion::Down,
        KeyCode::Home => Motion::LineStart,
        KeyCode::End => Motion::LineEnd,
        _ => match plain_char(key) {
            Some('h') => Motion::Left,
            Some('l') | Some(' ') => Motion::Right,
            Some('k') => Motion::Up,
            Some('j') => Motion::Down,
            Some('w') => Motion::WordForward,
            Some('b') => Motion::WordBackward,
            Some('e') => Motion::WordEnd,
            Some('0') => Motion::LineStart,
            Some('^') => Motion::FirstNonBlank,
            Some('$') => Motion::LineEnd,
            Some('G') => Motion::FileEnd,
            Some('n') => Motion::SearchNext,
            Some('N') => Motion::SearchPrev,
            Some('g') => {
                return match keys.get(1).map(plain_char) {
                    None => Parse::Incomplete,
                    Some(Some('g')) => Parse::Done(Motion::FileStart),
                    Some(_) => Parse::Invalid,
                };
            }
            _ => return Parse::Invalid,
        },
    };
    Parse::Done(m)
}

/// Multiply two optional counts (`2d3w` = 6). `None` if neither was typed.
fn multiply(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    match (a, b) {
        (None, None) => None,
        _ => Some(a.unwrap_or(1).saturating_mul(b.unwrap_or(1))),
    }
}

/// Parse pending keys. `visual` selects visual-mode meanings (operators act on the selection).
pub(crate) fn parse(keys: &[Key], visual: bool) -> Parse<Cmd> {
    let mut i = 0;
    let mut register = None;
    if keys.first().and_then(plain_char) == Some('"') {
        match keys.get(1).map(plain_char) {
            None => return Parse::Incomplete,
            Some(Some(c)) => register = Some(c),
            Some(None) => return Parse::Invalid,
        }
        i = 2;
    }
    let count = parse_count(keys, &mut i);
    let Some(key) = keys.get(i) else {
        return Parse::Incomplete;
    };
    let done = |action| {
        Parse::Done(Cmd {
            register,
            count,
            action,
        })
    };

    if key.mods.ctrl {
        return match key.code {
            KeyCode::Char('d') => done(Action::Scroll(Scroll::HalfDown)),
            KeyCode::Char('u') => done(Action::Scroll(Scroll::HalfUp)),
            KeyCode::Char('f') => done(Action::Scroll(Scroll::PageDown)),
            KeyCode::Char('b') => done(Action::Scroll(Scroll::PageUp)),
            KeyCode::Char('r') if !visual => done(Action::Redo),
            _ => Parse::Invalid,
        };
    }
    match key.code {
        KeyCode::PageDown => return done(Action::Scroll(Scroll::PageDown)),
        KeyCode::PageUp => return done(Action::Scroll(Scroll::PageUp)),
        _ => {}
    }

    let op = match plain_char(key) {
        Some('d') => Some(Operator::Delete),
        Some('c') => Some(Operator::Change),
        Some('y') => Some(Operator::Yank),
        _ => None,
    };
    if let Some(op) = op {
        if visual {
            return done(Action::Operate(op, Target::Selection));
        }
        let op_char = plain_char(key);
        i += 1;
        let count2 = parse_count(keys, &mut i);
        let Some(next) = keys.get(i) else {
            return Parse::Incomplete;
        };
        let target = if plain_char(next) == op_char {
            Target::Lines
        } else {
            match parse_motion(&keys[i..]) {
                Parse::Done(m) => Target::Motion(m),
                Parse::Incomplete => return Parse::Incomplete,
                Parse::Invalid => return Parse::Invalid,
            }
        };
        return Parse::Done(Cmd {
            register,
            count: multiply(count, count2),
            action: Action::Operate(op, target),
        });
    }

    let action = match (plain_char(key), visual) {
        (Some('x'), false) => Action::Operate(Operator::Delete, Target::Motion(Motion::Right)),
        (Some('x'), true) => Action::Operate(Operator::Delete, Target::Selection),
        (Some('v'), _) => Action::Visual { line: false },
        (Some('V'), _) => Action::Visual { line: true },
        (Some('i'), false) => Action::Insert(InsertAt::Before),
        (Some('a'), false) => Action::Insert(InsertAt::After),
        (Some('I'), false) => Action::Insert(InsertAt::LineStart),
        (Some('A'), false) => Action::Insert(InsertAt::LineEnd),
        (Some('o'), false) => Action::Insert(InsertAt::Below),
        (Some('O'), false) => Action::Insert(InsertAt::Above),
        (Some('p'), false) => Action::Paste { before: false },
        (Some('P'), false) => Action::Paste { before: true },
        (Some('u'), false) => Action::Undo,
        (Some(':'), false) => Action::CommandLine,
        (Some('/'), false) => Action::Search,
        _ => {
            return match parse_motion(&keys[i..]) {
                Parse::Done(m) => done(Action::Move(m)),
                Parse::Incomplete => Parse::Incomplete,
                Parse::Invalid => Parse::Invalid,
            };
        }
    };
    done(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_keys;

    fn p(keys: &str) -> Parse<Cmd> {
        parse(&parse_keys(keys), false)
    }

    fn cmd(register: Option<char>, count: Option<usize>, action: Action) -> Parse<Cmd> {
        Parse::Done(Cmd {
            register,
            count,
            action,
        })
    }

    #[test]
    fn motions_and_counts() {
        assert_eq!(p("w"), cmd(None, None, Action::Move(Motion::WordForward)));
        assert_eq!(p("3j"), cmd(None, Some(3), Action::Move(Motion::Down)));
        assert_eq!(p("10G"), cmd(None, Some(10), Action::Move(Motion::FileEnd)));
        assert_eq!(p("0"), cmd(None, None, Action::Move(Motion::LineStart)));
        assert_eq!(p("g"), Parse::Incomplete);
        assert_eq!(p("gg"), cmd(None, None, Action::Move(Motion::FileStart)));
        assert_eq!(p("gx"), Parse::Invalid);
        assert_eq!(p("3"), Parse::Incomplete);
        assert_eq!(p("Z"), Parse::Invalid);
    }

    #[test]
    fn operators() {
        use Operator::*;
        assert_eq!(p("d"), Parse::Incomplete);
        assert_eq!(p("d2"), Parse::Incomplete);
        assert_eq!(
            p("dw"),
            cmd(
                None,
                None,
                Action::Operate(Delete, Target::Motion(Motion::WordForward))
            )
        );
        assert_eq!(
            p("2d3w"),
            cmd(
                None,
                Some(6),
                Action::Operate(Delete, Target::Motion(Motion::WordForward))
            )
        );
        assert_eq!(
            p("cc"),
            cmd(None, None, Action::Operate(Change, Target::Lines))
        );
        assert_eq!(
            p("3yy"),
            cmd(None, Some(3), Action::Operate(Yank, Target::Lines))
        );
        assert_eq!(
            p("dgg"),
            cmd(
                None,
                None,
                Action::Operate(Delete, Target::Motion(Motion::FileStart))
            )
        );
        assert_eq!(p("dy"), Parse::Invalid);
        assert_eq!(
            p("x"),
            cmd(
                None,
                None,
                Action::Operate(Delete, Target::Motion(Motion::Right))
            )
        );
    }

    #[test]
    fn registers() {
        assert_eq!(p("\""), Parse::Incomplete);
        assert_eq!(p("\"+"), Parse::Incomplete);
        assert_eq!(
            p("\"+p"),
            cmd(Some('+'), None, Action::Paste { before: false })
        );
        assert_eq!(
            p("\"+2yy"),
            cmd(
                Some('+'),
                Some(2),
                Action::Operate(Operator::Yank, Target::Lines)
            )
        );
    }

    #[test]
    fn visual_operators() {
        let v = |k: &str| parse(&parse_keys(k), true);
        assert_eq!(
            v("d"),
            cmd(
                None,
                None,
                Action::Operate(Operator::Delete, Target::Selection)
            )
        );
        assert_eq!(
            v("x"),
            cmd(
                None,
                None,
                Action::Operate(Operator::Delete, Target::Selection)
            )
        );
        assert_eq!(
            v("2w"),
            cmd(None, Some(2), Action::Move(Motion::WordForward))
        );
        assert_eq!(v("i"), Parse::Invalid);
    }
}
