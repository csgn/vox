//! Golden tests for the Vim grammar.
//!
//! Each case is `(before, keys, after)`. `|` marks the cursor: it sits just before the
//! char the cursor is on.

use std::cell::RefCell;
use std::rc::Rc;

use vox_core::Document;
use vox_tui::{App, Clipboard, Mode, parse_keys};

/// In-memory clipboard shared with the test.
#[derive(Clone, Default)]
struct MemClipboard(Rc<RefCell<String>>);

impl Clipboard for MemClipboard {
    fn get(&mut self) -> Result<String, String> {
        Ok(self.0.borrow().clone())
    }
    fn set(&mut self, text: String) -> Result<(), String> {
        *self.0.borrow_mut() = text;
        Ok(())
    }
}

fn app_from(marked: &str) -> App {
    let idx = marked.find('|').expect("cursor marker");
    let text = marked.replacen('|', "", 1);
    let doc = Document::from_text(&text);
    let pos = doc.pos_at(text[..idx].chars().count());
    let mut app = App::new(doc);
    app.set_clipboard(Box::new(MemClipboard::default()));
    app.resize(80, 24);
    // Move to the marker with motions so the app's own state stays consistent.
    let keys = format!("gg0{}{}", "j".repeat(pos.line), "l".repeat(pos.col));
    feed(&mut app, &keys);
    assert_eq!(app.cursor(), pos, "setup for {marked:?}");
    app
}

fn feed(app: &mut App, keys: &str) {
    for key in parse_keys(keys) {
        app.handle_key(key);
    }
}

fn marked(app: &App) -> String {
    let doc = app.document();
    let idx = doc.char_idx(app.cursor());
    let mut text = doc.text().to_string();
    let byte = doc.text().char_to_byte(idx);
    text.insert(byte, '|');
    text
}

fn check(cases: &[(&str, &str, &str)]) {
    let mut failures = Vec::new();
    for (before, keys, after) in cases {
        let mut app = app_from(before);
        feed(&mut app, keys);
        let got = marked(&app);
        if got != *after {
            failures.push(format!(
                "{before:?} + {keys:?}\n   want {after:?}\n    got {got:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn delete_with_motions() {
    check(&[
        ("|foo bar baz", "dw", "|bar baz"),
        ("foo |bar baz", "dw", "foo |baz"),
        ("foo |bar\nbaz", "dw", "foo| \nbaz"),
        ("|foo\n  bar", "dw", "|\n  bar"),
        ("|foo bar baz", "d2w", "|baz"),
        ("|foo bar baz", "2dw", "|baz"),
        ("|foo bar baz", "wdw", "foo |baz"),
        ("|foo bar", "de", "| bar"),
        ("foo |bar", "d$", "foo| "),
        ("foo b|ar", "d0", "|ar"),
        ("  foo |bar", "d^", "  |bar"),
        ("foo |bar", "db", "|bar"),
        ("a|bc", "dl", "a|c"),
        ("ab|c", "dh", "a|c"),
    ]);
}

#[test]
fn delete_lines() {
    check(&[
        ("a\n|b\nc", "dd", "a\n|c"),
        ("a\nb\n|c", "dd", "a\n|b"),
        ("|a\nb\nc", "2dd", "|c"),
        ("|a", "dd", "|"),
        ("a\n|b\nc\nd", "dj", "a\n|d"),
        ("a\nb\n|c", "dk", "|a"),
        ("|a\nb\nc", "dG", "|"),
        ("a\nb\n|c", "dgg", "|"),
        ("a\n  |b\nc", "dd", "a\n|c"),
        ("|a\n  b", "dd", "  |b"),
    ]);
}

#[test]
fn x_and_counts() {
    check(&[
        ("a|bc", "x", "a|c"),
        ("ab|c", "x", "a|b"),
        ("|abc", "3x", "|"),
        ("|abcd", "2x", "|cd"),
        ("|", "x", "|"),
    ]);
}

#[test]
fn change() {
    check(&[
        ("|foo bar", "cwxy<Esc>", "x|y bar"),
        ("  |a\nb", "cwX<Esc>", "  |X\nb"),
        ("|foo bar baz", "c2wX<Esc>", "|X baz"),
        ("|foo bar", "ccnew<Esc>", "ne|w"),
        ("a\n|b\nc", "2ccX<Esc>", "a\n|X"),
        ("|foo bar", "c$X<Esc>", "|X"),
        ("|foo bar", "ceX<Esc>", "|X bar"),
        ("|foo bar", "cwxy<Esc>u", "|foo bar"),
    ]);
}

#[test]
fn yank_and_paste() {
    check(&[
        ("|foo bar", "yyp", "foo bar\n|foo bar"),
        ("|foo bar", "yyP", "|foo bar\nfoo bar"),
        ("|foo bar", "ywP", "foo| foo bar"),
        ("|ab", "xp", "b|a"),
        ("|abc", "yl3p", "aaa|abc"),
        ("|a\nb", "ddp", "b\n|a"),
        ("|a\nb", "yj2p", "a\n|a\nb\na\nb\nb"),
        ("a\n|b", "yyggP", "|b\na\nb"),
        ("|foo", "ye$p", "foofo|o"),
    ]);
}

#[test]
fn undo_redo_operators() {
    check(&[
        ("a\n|b\nc", "ddu", "a\n|b\nc"),
        ("a\n|b\nc", "ddu<C-r>", "a\n|c"),
        ("|a b c", "dwdwuu", "|a b c"),
        ("|a b c", "dwdw2u", "|a b c"),
        ("|abc", "yyp2u", "ab|c"),
    ]);
}

#[test]
fn motions_with_counts() {
    check(&[
        ("|a\nb\nc\nd", "3j", "a\nb\nc\n|d"),
        ("a\nb\nc\n|d", "2k", "a\n|b\nc\nd"),
        ("|a\nb\nc\nd", "3G", "a\nb\n|c\nd"),
        ("a\nb\n|c", "1G", "|a\nb\nc"),
        ("a\nb\n|c", "2gg", "a\n|b\nc"),
        ("|a\nb\nc", "99G", "a\nb\n|c"),
        ("|abcdef", "3l", "abc|def"),
        ("|a b c d", "3w", "a b c |d"),
        ("a b c |d", "2b", "a |b c d"),
        ("|foo bar baz", "2e", "foo ba|r baz"),
        ("|ab\ncd", "2$", "ab\nc|d"),
        ("  |ab", "0^", "  |ab"),
    ]);
}

#[test]
fn insert_entry() {
    check(&[
        ("|abc", "AX<Esc>", "abc|X"),
        ("  a|b", "IX<Esc>", "  |Xab"),
        ("a|bc", "iX<Esc>", "a|Xbc"),
        ("a|bc", "aX<Esc>", "ab|Xc"),
    ]);
}

#[test]
fn visual_charwise() {
    check(&[
        ("|foo bar", "vlld", "| bar"),
        ("foo |bar", "vhhd", "fo|ar"),
        ("|abc", "vx", "|bc"),
        ("|foo bar", "veyP", "fo|ofoo bar"),
        ("|foo bar", "vecX<Esc>", "|X bar"),
        ("a|b\ncd", "vjd", "|a"),
        ("|foo bar", "v<Esc>x", "|oo bar"),
        ("|foo bar", "vvx", "|oo bar"),
    ]);
}

#[test]
fn visual_linewise() {
    check(&[
        ("a\n|b\nc", "Vjd", "|a"),
        ("|a\nb", "Vyp", "a\n|a\nb"),
        ("|a\nb\nc", "VjcX<Esc>", "|X\nc"),
        ("a\n|b\nc", "Vkd", "|c"),
        ("|ab\ncd", "vjVd", "|"),
    ]);
}

#[test]
fn search() {
    check(&[
        ("|foo bar foo", "/foo<CR>", "foo bar |foo"),
        ("|a b a b", "/b<CR>n", "a b a |b"),
        ("|a b a b", "/b<CR>nn", "a |b a b"),
        ("|a b a b", "/b<CR>N", "a b a |b"),
        ("|a b a b", "/b<CR>2n", "a |b a b"),
        ("|foo bar foo", "/foo<CR>0dn", "|foo"),
        ("|x a x", "/x<CR>/<CR>", "|x a x"),
        ("|abc", "/zz<CR>", "|abc"),
    ]);
}

#[test]
fn search_messages() {
    let mut app = app_from("|a b a");
    feed(&mut app, "n");
    assert_eq!(app.message(), Some("E35: No previous regular expression"));
    feed(&mut app, "/a<CR>");
    assert_eq!(app.message(), Some("/a"));
    feed(&mut app, "n");
    assert_eq!(app.message(), Some("search hit BOTTOM, continuing at TOP"));
    feed(&mut app, "/zz<CR>");
    assert_eq!(app.message(), Some("E486: Pattern not found: zz"));
}

#[test]
fn visual_mode_switching() {
    let mut app = app_from("|abc");
    feed(&mut app, "v");
    assert_eq!(app.mode(), &Mode::Visual { line: false });
    feed(&mut app, "V");
    assert_eq!(app.mode(), &Mode::Visual { line: true });
    feed(&mut app, "V");
    assert_eq!(app.mode(), &Mode::Normal);
    feed(&mut app, "vl<Esc>");
    assert_eq!(app.mode(), &Mode::Normal);
}

#[test]
fn clipboard_registers() {
    let clip = MemClipboard::default();
    let mut app = app_from("|foo\nbar");
    app.set_clipboard(Box::new(clip.clone()));
    feed(&mut app, "\"+yy");
    assert_eq!(*clip.0.borrow(), "foo\n");
    feed(&mut app, "j\"*yw");
    assert_eq!(*clip.0.borrow(), "bar");
    *clip.0.borrow_mut() = "hi".into();
    feed(&mut app, "\"+P");
    assert_eq!(marked(&app), "foo\nh|ibar");
    // Unnamed register is separate from the clipboard.
    feed(&mut app, "ggyy");
    assert_eq!(*clip.0.borrow(), "hi");
    // Clipboard text ending in a newline pastes linewise.
    *clip.0.borrow_mut() = "line\r\n".into();
    feed(&mut app, "\"+p");
    assert_eq!(marked(&app), "foo\n|line\nhibar");
}

#[test]
fn register_errors() {
    let mut app = app_from("|a");
    feed(&mut app, "p");
    assert_eq!(app.message(), Some("E353: Nothing in register \""));
    feed(&mut app, "\"ayy");
    assert_eq!(app.message(), Some("E354: Invalid register name: 'a'"));
}

#[test]
fn pending_keys_are_dropped_on_invalid() {
    check(&[("|abc", "dzx", "|bc"), ("|abc", "d<Esc>x", "|bc")]);
}
