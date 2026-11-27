//! The `:` and `/` prompts: completion (`Tab`, menu), history, and where they are drawn.

use vox_core::Document;
use vox_editor::ui::{CompletionMode, PromptPlace};
use vox_editor::{Config, Editor, Mode, parse_keys};

fn editor() -> Editor {
    let mut ed = Editor::new(Document::from_text("hello"));
    ed.resize(80, 24);
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

/// The text typed at the `:` prompt.
fn line(ed: &Editor) -> &str {
    match ed.mode() {
        Mode::Command(s) | Mode::Search(s) => s,
        m => panic!("not in a prompt: {m:?}"),
    }
}

fn with_ui(ed: &mut Editor, f: impl FnOnce(&mut vox_editor::UiConfig)) {
    let mut cfg = Config::default();
    f(&mut cfg.ui);
    ed.apply_config(&cfg);
}

#[test]
fn tab_completes_the_only_match() {
    let mut ed = editor();
    feed(&mut ed, ":wri<Tab>");
    assert_eq!(line(&ed), "write ");
    // A subcommand, then a value.
    feed(&mut ed, "<Esc>:plugin ena<Tab>");
    assert_eq!(line(&ed), "plugin enable ");
    feed(&mut ed, "<Esc>:notify w<Tab>");
    assert_eq!(line(&ed), "notify warn ");
}

#[test]
fn tab_cycles_and_esc_restores() {
    let mut ed = editor();
    feed(&mut ed, ":plugin <Tab>");
    assert_eq!(line(&ed), "plugin add");
    feed(&mut ed, "<Tab>");
    assert_eq!(line(&ed), "plugin disable");
    feed(&mut ed, "<S-Tab><S-Tab>");
    assert_eq!(line(&ed), "plugin remove", "wraps to the last");
    feed(&mut ed, "<Down>");
    assert_eq!(line(&ed), "plugin add", "arrows move in the menu too");
    // Esc goes back to what was typed, a second Esc leaves.
    feed(&mut ed, "<Esc>");
    assert_eq!(line(&ed), "plugin ");
    feed(&mut ed, "<Esc>");
    assert_eq!(ed.mode(), &Mode::Normal);
    // Typing after a selection keeps it and goes on.
    feed(&mut ed, ":notify <Tab><Tab> hi<CR>");
    assert_eq!(ed.notifications().last().unwrap().text, "hi");
}

#[test]
fn esc_without_selection_leaves_at_once() {
    let mut ed = editor();
    feed(&mut ed, ":wr<Esc>");
    assert_eq!(ed.mode(), &Mode::Normal);
}

#[test]
fn history_with_up_and_down() {
    let mut ed = editor();
    feed(&mut ed, ":notify one<CR>:notify two<CR>:notify two<CR>");
    assert_eq!(ed.command_history(), ["notify one", "notify two"]);
    feed(&mut ed, ":<Up>");
    assert_eq!(line(&ed), "notify two");
    feed(&mut ed, "<Up><Up>");
    assert_eq!(line(&ed), "notify one", "stops at the oldest");
    feed(&mut ed, "<Down>");
    assert_eq!(line(&ed), "notify two");
    feed(&mut ed, "<Down>");
    assert_eq!(line(&ed), "", "back to what was typed");
    // Search has its own history.
    feed(&mut ed, "<Esc>/hel<CR>/<C-p>");
    assert_eq!(line(&ed), "hel");
}

#[test]
fn completion_modes() {
    let mut ed = editor();
    ed.resize(80, 24);
    feed(&mut ed, ":plugin ");
    assert!(
        ed.render().to_string().contains("disable"),
        "auto: menu while typing"
    );
    feed(&mut ed, "<Esc>");
    with_ui(&mut ed, |u| u.completion = CompletionMode::Tab);
    feed(&mut ed, ":plugin ");
    assert!(
        !ed.render().to_string().contains("disable"),
        "tab: no menu yet"
    );
    feed(&mut ed, "<Tab>");
    assert!(
        ed.render().to_string().contains("disable"),
        "tab: menu after Tab"
    );
    feed(&mut ed, "<Esc><Esc>");
    with_ui(&mut ed, |u| u.completion = CompletionMode::Off);
    feed(&mut ed, ":wri<Tab>");
    assert_eq!(line(&ed), "wri", "off: Tab does nothing");
}

#[test]
fn prompt_places_run_the_same() {
    for place in [PromptPlace::Float, PromptPlace::Modal] {
        let mut ed = editor();
        with_ui(&mut ed, |u| {
            u.cmdline = place;
            u.searchline = place;
        });
        feed(&mut ed, ":notify hi<CR>");
        assert_eq!(ed.notifications().last().unwrap().text, "hi");
        feed(&mut ed, "/llo<CR>");
        assert_eq!(ed.cursor().col, 2);
    }
}

mod snapshots {
    use super::*;

    #[test]
    fn statusline_menu() {
        let mut ed = editor();
        feed(&mut ed, ":plugin <Tab><Tab>");
        let s = ed.render();
        assert_eq!(s.cursor, (15, 23));
        // The selected entry (`disable`, row 15) is reversed, the others are not.
        assert!(s.cell(9, 15).style.reversed);
        assert!(!s.cell(9, 14).style.reversed);
        insta::assert_snapshot!(s.to_string());
    }

    #[test]
    fn statusline_usage_hint() {
        let mut ed = editor();
        feed(&mut ed, ":notify ");
        let s = ed.render();
        // The parameter being typed is highlighted in the usage title.
        assert_eq!(s.cell(19, 17).symbol, "L");
        assert!(s.cell(19, 17).style.reversed);
        assert!(
            !s.cell(27, 17).style.reversed,
            "TEXT is not the current parameter"
        );
        insta::assert_snapshot!(s.to_string());
    }

    #[test]
    fn float_prompt() {
        let mut ed = editor();
        with_ui(&mut ed, |u| u.cmdline = PromptPlace::Float);
        feed(&mut ed, ":set");
        let s = ed.render();
        insta::assert_snapshot!(s.to_string());
        let (x, y) = s.cursor;
        assert_eq!(s.cell(x - 1, y).symbol, "t");
    }

    #[test]
    fn modal_prompt() {
        let mut ed = editor();
        with_ui(&mut ed, |u| u.cmdline = PromptPlace::Modal);
        feed(&mut ed, ":settings g");
        insta::assert_snapshot!(ed.render().to_string());
    }

    #[test]
    fn float_search() {
        let mut ed = editor();
        with_ui(&mut ed, |u| u.searchline = PromptPlace::Float);
        feed(&mut ed, "/hel");
        insta::assert_snapshot!(ed.render().to_string());
    }
}
