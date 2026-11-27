//! Units of code around a line — block, function, class, file — for focus mode. The
//! built-in [`Heuristic`] needs no parser: it matches braces (skipping strings and
//! comments), indentation blocks whose header ends with `:`, and keywords on the first
//! line. A real parser (e.g. tree-sitter in an adapter) can replace it via
//! [`Structure`].

/// What kind of unit a range is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnitKind {
    /// Lines chosen in visual mode.
    Selection,
    /// A `{ }` or indented block that is not a function or class.
    Block,
    /// A function or method.
    Function,
    /// A class, struct, impl, trait, interface, enum, or module.
    Class,
    /// The whole file.
    File,
}

impl UnitKind {
    /// Name used in commands and the status line: `block`, `function`, ….
    pub fn name(self) -> &'static str {
        match self {
            UnitKind::Selection => "selection",
            UnitKind::Block => "block",
            UnitKind::Function => "function",
            UnitKind::Class => "class",
            UnitKind::File => "file",
        }
    }

    /// Parse a name from [`UnitKind::name`].
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Selection,
            Self::Block,
            Self::Function,
            Self::Class,
            Self::File,
        ]
        .into_iter()
        .find(|k| k.name() == s)
    }
}

/// A range of whole lines (inclusive) with what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// What it is.
    pub kind: UnitKind,
    /// First line (0-based).
    pub start: usize,
    /// Last line (0-based, inclusive).
    pub end: usize,
    /// Name of a function or class, if found (`foo` in `fn foo()`).
    pub name: Option<String>,
}

impl Unit {
    /// Number of lines.
    pub fn len(&self) -> usize {
        self.end + 1 - self.start
    }

    /// Always false: a unit has at least one line.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Whether `line` is inside.
    pub fn contains(&self, line: usize) -> bool {
        (self.start..=self.end).contains(&line)
    }

    /// `function foo (L12–40)`, lines 1-based.
    pub fn label(&self) -> String {
        let name = self
            .name
            .as_ref()
            .map_or(String::new(), |n| format!(" {n}"));
        format!(
            "{}{name} (L{}–{})",
            self.kind.name(),
            self.start + 1,
            self.end + 1
        )
    }
}

/// Finds the units around a line.
pub trait Structure {
    /// Units containing `line`, innermost first; the last one is the whole file.
    fn units(&self, lines: &[String], line: usize) -> Vec<Unit>;
}

/// The built-in [`Structure`]: braces, indentation, and keywords.
#[derive(Debug, Clone, Copy, Default)]
pub struct Heuristic;

/// Words that may come before the keyword on a header line.
const MODIFIERS: &[&str] = &[
    "pub",
    "pub(crate)",
    "pub(super)",
    "async",
    "unsafe",
    "const",
    "extern",
    "export",
    "default",
    "static",
    "public",
    "private",
    "protected",
    "abstract",
    "final",
    "override",
    "virtual",
    "inline",
    "open",
    "internal",
    "sealed",
    "data",
    "partial",
];
/// First words of a function header.
const FUNCTION: &[&str] = &["fn", "def", "function", "func", "fun", "sub", "method"];
/// First words of a class-like header.
const CLASS: &[&str] = &[
    "class",
    "struct",
    "impl",
    "trait",
    "interface",
    "enum",
    "mod",
    "module",
    "object",
    "namespace",
    "protocol",
    "extension",
    "union",
];
/// First words of control-flow blocks (never functions).
const CONTROL: &[&str] = &[
    "if", "else", "for", "while", "loop", "match", "switch", "case", "do", "try", "catch",
    "finally", "with", "return", "let", "var", "const", "unsafe",
];

/// Leading spaces (a tab counts as 4).
fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Matched `{ }` pairs as (line of `{`, line of `}`), skipping strings and comments.
fn brace_pairs(lines: &[String]) -> Vec<(usize, usize)> {
    let mut stack = Vec::new();
    let mut pairs = Vec::new();
    let mut in_block_comment = false;
    for (n, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        let mut quote: Option<char> = None;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if in_block_comment {
                if c == '*' && next == Some('/') {
                    in_block_comment = false;
                    i += 1;
                }
            } else if let Some(q) = quote {
                if c == '\\' {
                    i += 1;
                } else if c == q {
                    quote = None;
                }
            } else {
                match (c, next) {
                    ('/', Some('/')) => break,
                    ('/', Some('*')) => {
                        in_block_comment = true;
                        i += 1;
                    }
                    ('"' | '`', _) => quote = Some(c),
                    // A char literal ('x', '\n'), not a lifetime ('a).
                    ('\'', Some('\\')) => {
                        if let Some(end) = chars[i + 2..].iter().position(|&x| x == '\'') {
                            i += end + 2;
                        }
                    }
                    ('\'', Some(_)) if chars.get(i + 2) == Some(&'\'') => i += 2,
                    ('{', _) => stack.push(n),
                    ('}', _) => {
                        if let Some(open) = stack.pop() {
                            pairs.push((open, n));
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
    }
    pairs
}

/// Where the statement owning a `{` on line `open` starts: the line before a lone `{`
/// (brace on its own line), else the line where the brackets closed on the `{` line were
/// opened (multi-line signatures like `fn f(\n  a,\n) {`).
fn header_start(lines: &[String], open: usize) -> usize {
    if lines[open].trim() == "{" {
        return (0..open)
            .rev()
            .find(|&i| !lines[i].trim().is_empty())
            .unwrap_or(open);
    }
    // Brackets still open, counting from the `{` line upward.
    let balance = |l: &str| {
        l.chars().fold(0i32, |n, c| match c {
            '(' | '[' => n + 1,
            ')' | ']' => n - 1,
            _ => n,
        })
    };
    let mut depth = balance(&lines[open]);
    let mut i = open;
    while depth < 0 && i > 0 {
        i -= 1;
        depth += balance(&lines[i]);
    }
    i
}

/// Indentation blocks around `line`: each header ending with `:` (Python and alike), with
/// the more-indented lines after it.
fn indent_blocks(lines: &[String], line: usize) -> Vec<(usize, usize)> {
    let nonblank = |i: usize| !lines[i].trim().is_empty();
    // Indentation of the line (or of the next non-blank line, for a blank one).
    let Some(here) = (line..lines.len()).find(|&i| nonblank(i)) else {
        return Vec::new();
    };
    let mut level = indent(&lines[here]) + 1;
    let mut out = Vec::new();
    for h in (0..=line.min(here)).rev() {
        if !nonblank(h) {
            continue;
        }
        let ih = indent(&lines[h]);
        if ih >= level {
            continue;
        }
        level = ih;
        if !lines[h].trim_end().ends_with(':') {
            continue;
        }
        let mut end = h;
        for (i, l) in lines.iter().enumerate().skip(h + 1) {
            if !l.trim().is_empty() {
                if indent(l) <= ih {
                    break;
                }
                end = i;
            }
        }
        if end > h {
            out.push((h, end));
        }
        if ih == 0 {
            break;
        }
    }
    out
}

/// Kind and name of a unit from its first line.
fn classify(header: &str) -> (UnitKind, Option<String>) {
    let mut words = header
        .split(|c: char| c.is_whitespace() || c == '(' || c == '<' || c == ':' || c == '{')
        .filter(|w| !w.is_empty())
        .peekable();
    while words
        .peek()
        .is_some_and(|w| MODIFIERS.contains(w) || w.starts_with('"'))
    {
        words.next();
    }
    let ident = |w: &str| {
        let name: String = w
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        (!name.is_empty()).then_some(name)
    };
    let Some(first) = words.next() else {
        return (UnitKind::Block, None);
    };
    if FUNCTION.contains(&first) {
        return (UnitKind::Function, words.next().and_then(ident));
    }
    if CLASS.contains(&first) {
        // `impl<T> Foo for Bar`: name the type after `for` if there is one.
        let rest: Vec<&str> = words.collect();
        let name = match rest.iter().position(|w| *w == "for") {
            Some(i) => rest.get(i + 1).copied(),
            None => rest.first().copied(),
        };
        return (UnitKind::Class, name.and_then(ident));
    }
    // C-like `int foo(...) {` / JS `foo(a) {`: a name right before `(`, not control flow.
    let trimmed = header.trim_start();
    if !CONTROL.contains(&first)
        && let Some(paren) = trimmed.find('(')
    {
        let before = trimmed[..paren].trim_end();
        let name: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let assigned = before.contains('=');
        if !name.is_empty() && !assigned && !CONTROL.contains(&name.as_str()) {
            return (UnitKind::Function, Some(name));
        }
    }
    (UnitKind::Block, None)
}

impl Structure for Heuristic {
    fn units(&self, lines: &[String], line: usize) -> Vec<Unit> {
        let last = lines.len().saturating_sub(1);
        let line = line.min(last);
        let mut ranges: Vec<(usize, usize)> = brace_pairs(lines)
            .into_iter()
            .map(|(open, close)| (header_start(lines, open), close))
            .filter(|&(s, e)| s < e && (s..=e).contains(&line))
            .collect();
        ranges.extend(indent_blocks(lines, line));
        ranges.sort_by_key(|&(s, e)| (e - s, s));
        ranges.dedup();
        let mut units: Vec<Unit> = ranges
            .into_iter()
            .map(|(start, end)| {
                let (kind, name) = classify(&lines[start]);
                Unit {
                    kind,
                    start,
                    end,
                    name,
                }
            })
            // A block as big as the file adds nothing; a function or class still counts.
            .filter(|u| !(u.kind == UnitKind::Block && u.start == 0 && u.end == last))
            .collect();
        units.push(Unit {
            kind: UnitKind::File,
            start: 0,
            end: last,
            name: None,
        });
        units
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(String::from).collect()
    }

    fn kinds(units: &[Unit]) -> Vec<(UnitKind, usize, usize, Option<&str>)> {
        units
            .iter()
            .map(|u| (u.kind, u.start, u.end, u.name.as_deref()))
            .collect()
    }

    const RUST: &str = r#"use std::fmt;

pub struct Point {
    x: i32,
}

impl fmt::Display for Point {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if self.x > 0 {
            write!(f, "{{}}", self.x)
        } else {
            write!(f, "}")
        }
    }
}
"#;

    #[test]
    fn rust_nested_units() {
        let l = lines(RUST);
        // Inside the `if` branch.
        let u = Heuristic.units(&l, 12);
        assert_eq!(
            kinds(&u),
            [
                (UnitKind::Block, 11, 13, None),
                (UnitKind::Function, 7, 16, Some("fmt")),
                (UnitKind::Class, 6, 17, Some("Point")),
                (UnitKind::File, 0, 17, None),
            ]
        );
        assert_eq!(u[1].label(), "function fmt (L8–17)");
        let u = Heuristic.units(&l, 3);
        assert_eq!(kinds(&u)[0], (UnitKind::Class, 2, 4, Some("Point")));
        // Outside any block: the file only.
        assert_eq!(Heuristic.units(&l, 0).len(), 1);
    }

    #[test]
    fn python_indentation() {
        let l = lines(
            "import os\n\nclass Shape:\n    def area(self):\n        if self.w:\n            return 1\n        return 0\n\n    def name(self):\n        return 'x'\n",
        );
        let u = Heuristic.units(&l, 5);
        assert_eq!(
            kinds(&u),
            [
                (UnitKind::Block, 4, 5, None),
                (UnitKind::Function, 3, 6, Some("area")),
                (UnitKind::Class, 2, 9, Some("Shape")),
                (UnitKind::File, 0, 9, None),
            ]
        );
    }

    #[test]
    fn javascript_functions_and_methods() {
        let l = lines(
            "export class Cart {\n  total(items) {\n    return items.length;\n  }\n}\n\nfunction add(a, b) {\n  return a + b;\n}\n",
        );
        let u = Heuristic.units(&l, 2);
        assert_eq!(
            kinds(&u),
            [
                (UnitKind::Function, 1, 3, Some("total")),
                (UnitKind::Class, 0, 4, Some("Cart")),
                (UnitKind::File, 0, 8, None),
            ]
        );
        let u = Heuristic.units(&l, 7);
        assert_eq!(kinds(&u)[0], (UnitKind::Function, 6, 8, Some("add")));
    }

    #[test]
    fn braces_in_strings_and_comments_are_skipped() {
        let l = lines("fn a() {\n    let s = \"{\"; // {\n    /* { */ let c = '{';\n}\n");
        let u = Heuristic.units(&l, 1);
        assert_eq!(kinds(&u)[0], (UnitKind::Function, 0, 3, Some("a")));
        assert_eq!(u.len(), 2);
    }

    #[test]
    fn kinds_parse_and_names() {
        assert_eq!(UnitKind::parse("class"), Some(UnitKind::Class));
        assert_eq!(UnitKind::parse("x"), None);
        assert_eq!(classify("    int main(void) {").1.as_deref(), Some("main"));
        assert_eq!(classify("if (x) {").0, UnitKind::Block);
        assert_eq!(classify("const f = (a) => {").0, UnitKind::Block);
    }
}
