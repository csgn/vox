//! UI layer: panels, modal, focus, notifications.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use vox_core::Document;
use vox_editor::ui::{Confirm, Input, NotifyPosition, SidePosition};
use vox_editor::{Config, Editor, Focus, Level, PanelSlot, Services, parse_keys};

/// Editor with a controllable clock.
fn editor(text: &str) -> (Editor, Rc<Cell<Instant>>) {
    let now = Rc::new(Cell::new(Instant::now()));
    let clock = now.clone();
    let services = Services {
        clock: Box::new(move || clock.get()),
        ..Services::default()
    };
    let mut ed = Editor::with_services(Document::from_text(text), services);
    ed.resize(60, 16);
    (ed, now)
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

fn ui(cfg: impl FnOnce(&mut vox_editor::UiConfig)) -> Config {
    let mut c = Config::default();
    cfg(&mut c.ui);
    c
}

#[test]
fn commands_modal_takes_all_keys_and_runs_selection() {
    let (mut ed, _) = editor("abc");
    feed(&mut ed, ":commands<CR>");
    assert!(ed.has_modal());
    feed(&mut ed, "x"); // goes to the modal, not the text
    assert_eq!(ed.document().to_string(), "abc");
    feed(&mut ed, "<Esc>");
    assert!(!ed.has_modal());
    // Select `messages` and run it: the modal closes, the bottom panel opens.
    feed(&mut ed, ":commands<CR>");
    let names: Vec<String> = ed.commands().iter().map(|c| c.name.clone()).collect();
    let idx = names.iter().position(|n| n == "messages").unwrap();
    feed(&mut ed, &"j".repeat(idx));
    feed(&mut ed, "<CR>");
    assert!(!ed.has_modal());
    assert!(ed.has_panel(PanelSlot::Bottom));
    assert_eq!(ed.focused(), Focus::Bottom);
}

#[test]
fn focus_moves_with_ctrl_w() {
    let (mut ed, _) = editor("abc");
    feed(&mut ed, ":keymaps<CR>");
    assert_eq!(ed.focused(), Focus::Side);
    feed(&mut ed, "x"); // panel key, ignored by the list; text unchanged
    assert_eq!(ed.document().to_string(), "abc");
    feed(&mut ed, "<C-w>h"); // side panel is on the right: h goes back to the text
    assert_eq!(ed.focused(), Focus::Editor);
    feed(&mut ed, "<C-w>l");
    assert_eq!(ed.focused(), Focus::Side);
    feed(&mut ed, ":messages<CR>");
    assert_eq!(ed.focused(), Focus::Bottom);
    feed(&mut ed, "<C-w>k");
    assert_eq!(ed.focused(), Focus::Editor);
    feed(&mut ed, "<C-w>w<C-w>w");
    assert_eq!(ed.focused(), Focus::Bottom);
    feed(&mut ed, "<C-w>c");
    assert!(!ed.has_panel(PanelSlot::Bottom));
    assert_eq!(ed.focused(), Focus::Editor);
    feed(&mut ed, ":close<CR>");
    assert!(!ed.has_panel(PanelSlot::Side));
}

#[test]
fn left_side_panel_uses_h() {
    let (mut ed, _) = editor("abc");
    ed.apply_config(&ui(|u| u.side_panel = SidePosition::Left));
    feed(&mut ed, ":keymaps<CR><C-w>l");
    assert_eq!(ed.focused(), Focus::Editor);
    feed(&mut ed, "<C-w>h");
    assert_eq!(ed.focused(), Focus::Side);
    feed(&mut ed, ":focus editor<CR>");
    assert_eq!(ed.focused(), Focus::Editor);
}

#[test]
fn q_closes_focused_panel() {
    let (mut ed, _) = editor("abc");
    feed(&mut ed, ":messages<CR>q");
    assert!(!ed.has_panel(PanelSlot::Bottom));
    assert_eq!(ed.focused(), Focus::Editor);
    feed(&mut ed, "x"); // back in the text
    assert_eq!(ed.document().to_string(), "bc");
}

#[test]
fn confirm_and_input_through_api() {
    let (mut ed, _) = editor("abc");
    ed.open_modal(Box::new(Confirm::new("Quit", "Discard changes?", "q!")));
    feed(&mut ed, "n");
    assert!(!ed.should_quit() && !ed.has_modal());
    ed.open_modal(Box::new(Confirm::new("Quit", "Discard changes?", "q!")));
    feed(&mut ed, "y");
    assert!(ed.should_quit());

    let (mut ed, _) = editor("abc");
    ed.open_modal(Box::new(Input::new("Notify", "text: ", "notify warn {}")));
    feed(&mut ed, "hi there<CR>");
    assert_eq!(ed.notifications()[0].level, Level::Warn);
    assert_eq!(ed.notifications()[0].text, "hi there");
}

#[test]
fn notifications_expire_with_time() {
    let (mut ed, now) = editor("abc");
    let start = now.get();
    feed(&mut ed, ":notify error disk full<CR>");
    assert_eq!(ed.notifications().len(), 1);
    assert_eq!(
        ed.next_deadline(),
        Some(start + Duration::from_millis(4000))
    );
    now.set(start + Duration::from_millis(3999));
    ed.tick();
    assert_eq!(ed.notifications().len(), 1);
    now.set(start + Duration::from_millis(4000));
    ed.tick();
    assert!(ed.notifications().is_empty());
    assert_eq!(ed.next_deadline(), None);
    assert_eq!(ed.history(), &[(Level::Error, "disk full".to_string())]);
}

#[test]
fn messages_go_to_history_with_levels() {
    let (mut ed, _) = editor("abc");
    feed(&mut ed, ":nope<CR>:notify hello<CR>");
    assert_eq!(
        ed.history(),
        &[
            (
                Level::Error,
                "E492: Not an editor command: nope".to_string()
            ),
            (Level::Info, "hello".to_string()),
        ]
    );
}

#[test]
fn pending_mapping_times_out_on_tick() {
    let (mut ed, now) = editor("");
    let mut cfg = Config::default();
    cfg.keymaps
        .push((vox_editor::MapMode::Insert, "jk".into(), "<Esc>".into()));
    ed.apply_config(&cfg);
    let start = now.get();
    feed(&mut ed, "ij");
    assert_eq!(
        ed.next_deadline(),
        Some(start + Duration::from_millis(1000))
    );
    now.set(start + Duration::from_millis(1000));
    ed.tick();
    assert_eq!(ed.document().to_string(), "j");
    assert_eq!(ed.next_deadline(), None);
}

mod snapshots {
    use super::*;

    fn screen(ed: &mut Editor, w: u16, h: u16) -> String {
        ed.resize(w, h);
        ed.render().to_string()
    }

    #[test]
    fn side_panel_right() {
        let (mut ed, _) = editor("hello\nworld");
        let mut cfg = Config::default();
        cfg.keymaps
            .push((vox_editor::MapMode::Normal, "H".into(), "^".into()));
        cfg.ui.side_width = 24;
        ed.apply_config(&cfg);
        feed(&mut ed, ":keymaps<CR>");
        insta::assert_snapshot!(screen(&mut ed, 50, 8));
        assert!(!ed.render().cursor_visible, "list has no cursor");
    }

    #[test]
    fn side_panel_left_and_bottom() {
        let (mut ed, _) = editor("hello\nworld");
        ed.apply_config(&ui(|u| {
            u.side_panel = SidePosition::Left;
            u.side_width = 16;
            u.bottom_height = 4;
        }));
        feed(&mut ed, ":notify one<CR>:keymaps<CR>:messages<CR><C-w>k");
        let s = {
            ed.resize(50, 12);
            ed.render()
        };
        assert_eq!(s.cursor, (20, 0), "text cursor right of the side panel");
        insta::assert_snapshot!(s.to_string());
    }

    #[test]
    fn modal_and_notification() {
        let (mut ed, _) = editor("hello");
        feed(
            &mut ed,
            ":notify warn careful: this is a long warning text<CR>",
        );
        ed.open_modal(Box::new(Confirm::new(
            "Delete",
            "Delete plugin foo?",
            "close",
        )));
        insta::assert_snapshot!(screen(&mut ed, 60, 14));
    }

    #[test]
    fn notifications_bottom_right_stack() {
        let (mut ed, _) = editor("hello");
        ed.apply_config(&ui(|u| u.notify_position = NotifyPosition::BottomRight));
        feed(&mut ed, ":notify first<CR>:notify error second<CR>");
        insta::assert_snapshot!(screen(&mut ed, 60, 12));
    }

    #[test]
    fn input_modal_cursor() {
        let (mut ed, _) = editor("hello");
        ed.open_modal(Box::new(Input::new("Open", "file: ", "e {}")));
        feed(&mut ed, "ab");
        ed.resize(40, 10);
        let s = ed.render();
        assert!(s.cursor_visible);
        insta::assert_snapshot!(s.to_string());
        let (x, y) = s.cursor;
        assert_eq!(s.cell(x - 1, y).symbol, "b");
    }
}

mod devmode {
    use super::*;
    use vox_editor::owner;

    fn log_view_lines(ed: &mut Editor) -> String {
        ed.resize(80, 20);
        ed.render().to_string()
    }

    #[test]
    fn toggle_adds_and_removes_shortcuts() {
        let (mut ed, _) = editor("abc");
        assert!(!ed.developer_mode());
        feed(&mut ed, ":devmode<CR>");
        assert!(ed.developer_mode());
        assert!(ed.render().to_string().contains(" DEV "));
        let dev_maps = ed.keymap().iter().filter(|m| m.3 == owner::DEVMODE).count();
        assert_eq!(dev_maps, 3);
        feed(&mut ed, "\\dl"); // default leader is backslash
        assert!(ed.has_panel(PanelSlot::Bottom));
        feed(&mut ed, "q:devmode off<CR>");
        assert!(!ed.developer_mode());
        assert_eq!(ed.keymap().iter().count(), 0);
    }

    #[test]
    fn config_turns_it_on() {
        let (mut ed, _) = editor("abc");
        ed.apply_config(&Config {
            developer_mode: true,
            ..Config::default()
        });
        assert!(ed.developer_mode());
    }

    #[test]
    fn log_view_filters_and_updates_live() {
        let (mut ed, _) = editor("abc");
        ed.log(Level::Debug, "test", "quiet detail");
        ed.log(Level::Warn, "test", "first warning");
        feed(&mut ed, ":log warn<CR>");
        let screen = log_view_lines(&mut ed);
        assert!(screen.contains("first warning") && !screen.contains("quiet detail"));
        ed.log(Level::Error, "other", "arrives later");
        assert!(
            log_view_lines(&mut ed).contains("arrives later"),
            "live update"
        );
        feed(&mut ed, "f"); // warn -> error only
        let screen = log_view_lines(&mut ed);
        assert!(!screen.contains("first warning") && screen.contains("arrives later"));
        feed(&mut ed, "c");
        assert!(ed.log_entries().is_empty());
    }

    #[test]
    fn commands_are_logged_with_owner() {
        let (mut ed, _) = editor("abc");
        feed(&mut ed, ":nope<CR>:notify hi<CR>");
        let msgs: Vec<_> = ed
            .log_entries()
            .entries()
            .map(|e| e.message.clone())
            .collect();
        assert!(
            msgs.contains(&":notify hi (builtin)".to_string()),
            "{msgs:?}"
        );
    }

    #[test]
    fn remove_owned_drops_subscriptions_and_restores_builtins() {
        let (mut ed, _) = editor("abc");
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        ed.with_owner("plugin:t", |ed| {
            ed.subscribe(vox_editor::EventKind::TextChanged, move |_, _| {
                h.set(h.get() + 1)
            });
            ed.commands_mut().register("write", "fake", |ed, _| {
                ed.set_message("fake write");
                Ok(())
            });
            ed.map_key(vox_editor::MapMode::Normal, "Q", ":q!");
        });
        feed(&mut ed, "x:w<CR>");
        assert_eq!(hits.get(), 1);
        assert_eq!(ed.message(), Some("fake write"));
        ed.remove_owned("plugin:t");
        feed(&mut ed, "x:w<CR>");
        assert_eq!(hits.get(), 1, "subscription removed");
        assert_eq!(
            ed.message(),
            Some("E32: No file name"),
            "built-in write is back"
        );
        assert_eq!(ed.keymap().iter().count(), 0);
    }
}
