//! Developer API: command registry, key mappings, config.

use std::cell::Cell;
use std::rc::Rc;

use vox_core::Document;
use vox_editor::{Binding, Config, Editor, MapMode, Mode, Value, parse_keys};

fn editor(text: &str) -> Editor {
    let mut ed = Editor::new(Document::from_text(text));
    ed.resize(80, 24);
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

fn config(toml_like: &[(&str, &str, &str)], leader: &str) -> Config {
    let mut modes = std::collections::BTreeMap::new();
    for (mode, lhs, rhs) in toml_like {
        let entry = modes
            .entry(mode.to_string())
            .or_insert_with(|| Value::Table(Default::default()));
        if let Value::Table(t) = entry {
            t.insert(lhs.to_string(), Value::String(rhs.to_string()));
        }
    }
    let mut top = std::collections::BTreeMap::new();
    top.insert("leader".to_string(), Value::String(leader.into()));
    top.insert("keymap".to_string(), Value::Table(modes));
    let (cfg, errors) = Config::from_value(&Value::Table(top));
    assert!(errors.is_empty(), "{errors:?}");
    cfg
}

#[test]
fn custom_command_runs_from_command_line() {
    let mut ed = editor("hello");
    ed.commands_mut()
        .register("upper", "Uppercase the line", |ed, args| {
            let msg = format!("upper called with {:?} force={}", args.arg, args.force);
            ed.set_message(msg);
            Ok(())
        });
    ed.commands_mut().alias("up", "upper");
    feed(&mut ed, ":up! now<CR>");
    assert_eq!(
        ed.message(),
        Some("upper called with Some(\"now\") force=true")
    );
}

#[test]
fn command_error_becomes_message() {
    let mut ed = editor("x");
    ed.commands_mut()
        .register("fail", "Always fails", |_, _| Err("E999: nope".into()));
    feed(&mut ed, ":fail<CR>");
    assert_eq!(ed.message(), Some("E999: nope"));
    feed(&mut ed, ":missing arg<CR>");
    assert_eq!(
        ed.message(),
        Some("E492: Not an editor command: missing arg")
    );
}

#[test]
fn builtins_are_registered_commands() {
    let ed = editor("");
    for name in [
        "w", "write", "q", "quit", "qa", "wq", "x", "xit", "e", "edit",
    ] {
        assert!(ed.commands().get(name).is_some(), "{name}");
    }
    assert_eq!(ed.commands().get("w").unwrap().name, "write");
}

#[test]
fn builtin_commands_can_be_replaced() {
    let mut ed = editor("x");
    let called = Rc::new(Cell::new(false));
    let c = called.clone();
    ed.commands_mut().register("quit", "custom", move |_, _| {
        c.set(true);
        Ok(())
    });
    feed(&mut ed, ":q<CR>");
    assert!(called.get());
    assert!(!ed.should_quit());
}

#[test]
fn insert_mapping_jk_escapes() {
    let mut ed = editor("");
    ed.apply_config(&config(&[("insert", "jk", "<Esc>")], "\\"));
    feed(&mut ed, "iab");
    assert_eq!(ed.mode(), &Mode::Insert);
    feed(&mut ed, "j");
    assert!(ed.has_pending_keys(), "j waits for k");
    assert_eq!(ed.document().to_string(), "ab");
    feed(&mut ed, "k");
    assert_eq!(ed.mode(), &Mode::Normal);
    assert_eq!(ed.document().to_string(), "ab");
}

#[test]
fn unfinished_mapping_falls_back_to_typed_keys() {
    let mut ed = editor("");
    ed.apply_config(&config(&[("insert", "jk", "<Esc>")], "\\"));
    feed(&mut ed, "ijx");
    assert_eq!(ed.document().to_string(), "jx");
    feed(&mut ed, "jj");
    assert!(ed.has_pending_keys(), "second j may start jk");
    assert_eq!(ed.document().to_string(), "jxj");
    ed.timeout();
    assert!(!ed.has_pending_keys());
    assert_eq!(ed.document().to_string(), "jxjj");
}

#[test]
fn leader_mapping_runs_command() {
    let mut ed = editor("x");
    let ran = Rc::new(Cell::new(0));
    let r = ran.clone();
    ed.commands_mut().register("hello", "", move |_, _| {
        r.set(r.get() + 1);
        Ok(())
    });
    ed.apply_config(&config(&[("normal", "<leader>h", ":hello")], " "));
    feed(&mut ed, " h");
    assert_eq!(ran.get(), 1);
}

#[test]
fn normal_mappings_remap_and_disable() {
    let mut ed = editor("  abc def");
    ed.apply_config(&config(&[("normal", "H", "^"), ("normal", "x", "")], "\\"));
    feed(&mut ed, "$H");
    assert_eq!(ed.cursor().col, 2);
    feed(&mut ed, "x");
    assert_eq!(ed.document().to_string(), "  abc def", "x is disabled");
    // Mapped keys are not mapped again: `H` → `^` uses the built-in `^`.
}

#[test]
fn exact_and_longer_mapping_waits_then_times_out() {
    let mut ed = editor("abcdef");
    ed.apply_config(&config(
        &[("normal", "g", "l"), ("normal", "gx", "$")],
        "\\",
    ));
    feed(&mut ed, "g");
    assert!(ed.has_pending_keys());
    feed(&mut ed, "x");
    assert_eq!(ed.cursor().col, 5, "gx → $");
    feed(&mut ed, "0g");
    ed.timeout();
    assert_eq!(ed.cursor().col, 1, "g alone → l after timeout");
}

#[test]
fn visual_mapping_and_prompt_is_unmapped() {
    let mut ed = editor("abc");
    ed.apply_config(&config(&[("visual", "q", "d"), ("normal", "q", "x")], "\\"));
    feed(&mut ed, "vlq");
    assert_eq!(ed.document().to_string(), "c");
    // In the `:` prompt, `q` is just a char.
    feed(&mut ed, ":q<CR>");
    assert!(!ed.should_quit(), "buffer is modified, so :q refuses");
}

#[test]
fn ctrl_mapping_to_write() {
    let mut ed = editor("x");
    ed.apply_config(&config(&[("normal", "<C-s>", ":w")], "\\"));
    feed(&mut ed, "<C-s>");
    assert_eq!(ed.message(), Some("E32: No file name"));
}

#[test]
fn keymap_api_directly() {
    let mut ed = editor("abc");
    ed.keymap_mut().set(
        MapMode::Normal,
        parse_keys("Q"),
        Binding::Command("q!".into()),
    );
    feed(&mut ed, "Q");
    assert!(ed.should_quit());
}

#[test]
fn timeout_value_from_config() {
    let mut ed = editor("");
    ed.apply_config(&Config {
        timeout_ms: 250,
        ..Config::default()
    });
    assert_eq!(ed.key_timeout().as_millis(), 250);
}

mod events {
    use super::*;
    use std::cell::RefCell;
    use vox_editor::{Event, EventKind, ModeKind};

    fn record(ed: &mut Editor, kinds: &[EventKind]) -> Rc<RefCell<Vec<Event>>> {
        let log = Rc::new(RefCell::new(Vec::new()));
        for &k in kinds {
            let l = log.clone();
            ed.subscribe(k, move |_, e| l.borrow_mut().push(e.clone()));
        }
        log
    }

    #[test]
    fn text_changed_once_per_key_or_command() {
        let mut ed = editor("abc");
        let log = record(&mut ed, &[EventKind::TextChanged]);
        feed(&mut ed, "l"); // motion: no change
        assert!(log.borrow().is_empty());
        feed(&mut ed, "3x"); // one key that changes text
        assert_eq!(log.borrow().len(), 1);
        ed.commands_mut().register("two", "", |ed, _| {
            ed.insert_text(vox_core::Position::new(0, 0), "a");
            ed.insert_text(vox_core::Position::new(0, 0), "b");
            Ok(())
        });
        feed(&mut ed, ":two<CR>"); // two edits in one command: one event
        assert_eq!(log.borrow().len(), 2);
        assert_eq!(ed.document().to_string(), "baa"); // `l3x` left "a"
        feed(&mut ed, "u"); // the command's edits undo together
        assert_eq!(ed.document().to_string(), "a");
    }

    #[test]
    fn mode_changed() {
        let mut ed = editor("abc");
        let log = record(&mut ed, &[EventKind::ModeChanged]);
        feed(&mut ed, "ix<Esc>v<Esc>");
        let modes: Vec<_> = log
            .borrow()
            .iter()
            .map(|e| match e {
                Event::ModeChanged { mode } => *mode,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            modes,
            [
                ModeKind::Insert,
                ModeKind::Normal,
                ModeKind::Visual,
                ModeKind::Normal
            ]
        );
    }

    #[test]
    fn buf_open_and_write() {
        let path = std::env::temp_dir().join(format!("vox-events-{}.txt", std::process::id()));
        std::fs::write(&path, "x\n").unwrap();
        let mut ed =
            Editor::new(Document::open(&path, vox_core::storage::SimpleStorage::boxed).unwrap());
        let log = record(&mut ed, &[EventKind::BufOpen, EventKind::BufWrite]);
        ed.start();
        feed(&mut ed, ":w<CR>:e<CR>");
        std::fs::remove_file(&path).unwrap();
        let p = Some(path.display().to_string());
        assert_eq!(
            *log.borrow(),
            vec![
                Event::BufOpen { path: p.clone() },
                Event::BufWrite {
                    path: path.display().to_string()
                },
                Event::BufOpen { path: p },
            ]
        );
    }

    #[test]
    fn handler_can_edit_without_event_loop() {
        let mut ed = editor("");
        ed.subscribe(EventKind::TextChanged, |ed, _| {
            if !ed.document().to_string().ends_with('!') {
                let end = vox_core::Position::new(0, usize::MAX);
                ed.insert_text(end, "!");
            }
        });
        feed(&mut ed, "ia<Esc>");
        assert_eq!(ed.document().to_string(), "a!");
    }

    #[test]
    fn plugin_style_map_key_uses_leader() {
        let mut ed = editor("abc");
        ed.apply_config(&Config {
            leader: ",".into(),
            ..Config::default()
        });
        ed.map_key(vox_editor::MapMode::Normal, "<leader>e", "$");
        feed(&mut ed, ",e");
        assert_eq!(ed.cursor().col, 2);
    }
}

#[test]
fn subcommands_share_a_root() {
    let mut ed = editor("x");
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    for name in ["demo a", "demo b"] {
        let s = seen.clone();
        ed.with_owner(&format!("plugin:{name}"), |ed| {
            ed.commands_mut().register(name, "", move |_, args| {
                s.borrow_mut()
                    .push(format!("{name}:{:?}:{}", args.arg, args.force));
                Ok(())
            });
        });
    }
    feed(&mut ed, ":demo<CR>");
    assert_eq!(ed.message(), Some("E471: Use :demo a|b"));
    feed(&mut ed, ":demo a<CR>:demo! b  some arg <CR>");
    assert_eq!(
        *seen.borrow(),
        ["demo a:None:false", "demo b:Some(\"some arg\"):true"]
    );
    // A root command can exist next to its subcommands; unknown words are its argument.
    let s = seen.clone();
    ed.commands_mut().register("demo", "", move |_, args| {
        s.borrow_mut().push(format!("demo:{:?}", args.arg));
        Ok(())
    });
    feed(&mut ed, ":demo c<CR>");
    assert_eq!(seen.borrow().last().unwrap(), "demo:Some(\"c\")");
    feed(&mut ed, ":nothing here<CR>");
    assert_eq!(
        ed.message(),
        Some("E492: Not an editor command: nothing here")
    );
}

#[test]
fn builtin_subcommands() {
    let mut ed = editor("x");
    for name in [
        "log clear",
        "log refresh",
        "scope setup",
        "scope init",
        "scope pick",
    ] {
        assert!(ed.commands().get(name).is_some(), "{name}");
    }
    assert!(
        ed.commands().iter().all(|c| !c.name.contains('-')),
        "no hyphenated names"
    );
    ed.log(vox_editor::Level::Info, "test", "x");
    feed(&mut ed, ":log clear<CR>");
    assert!(
        ed.log_entries().entries().all(|e| e.message != "x"),
        "cleared"
    );
}

#[test]
fn bang_after_a_subcommand_forces_it() {
    let mut ed = editor("x");
    let forced = Rc::new(Cell::new(None));
    let f = forced.clone();
    ed.commands_mut().register("thing drop", "", move |_, a| {
        f.set(Some((a.force, a.arg.map(String::from))));
        Ok(())
    });
    ed.run_command("thing drop! a");
    assert_eq!(forced.take(), Some((true, Some("a".into()))));
    ed.run_command("thing! drop a");
    assert_eq!(forced.take(), Some((true, Some("a".into()))));
    ed.run_command("thing drop a");
    assert_eq!(forced.take(), Some((false, Some("a".into()))));
}

#[test]
fn leader_p_opens_the_plugin_manager() {
    let mut ed = editor("x");
    feed(&mut ed, "\\p");
    assert!(ed.modal_title().unwrap().starts_with("Plugins — "));
    let cfg = Config {
        leader: " ".into(),
        ..Default::default()
    };
    ed.apply_config(&cfg);
    feed(&mut ed, "<Esc><Space>p");
    assert!(ed.has_modal(), "follows the leader");
}

#[test]
fn execute_returns_reply_or_error() {
    let mut ed = editor("x");
    ed.commands_mut().register("answer", "", |ed, a| {
        if a.force {
            return Err("E1: no".into());
        }
        ed.reply(Value::Integer(42));
        Ok(())
    });
    assert_eq!(ed.execute("answer"), Ok(Some(Value::Integer(42))));
    assert_eq!(ed.execute("answer!"), Err("E1: no".into()));
    assert_eq!(ed.execute("notify hi"), Ok(None));
    assert!(ed.execute("nope").unwrap_err().starts_with("E492"));
    assert!(ed.execute("log nope").is_ok(), "log takes a source name");
    assert!(
        ed.execute("plugin nope")
            .unwrap_err()
            .starts_with("E471: Use :plugin ")
    );
    assert_eq!(ed.message(), None, "execute shows nothing");
}

#[test]
fn run_command_shows_reply() {
    let mut ed = editor("x");
    ed.commands_mut().register("one", "", |ed, _| {
        ed.reply(Value::from("one line"));
        Ok(())
    });
    ed.commands_mut().register("many", "", |ed, _| {
        ed.reply(Value::table([("a", Value::from(1)), ("b", 2.into())]));
        Ok(())
    });
    feed(&mut ed, ":one<CR>");
    assert_eq!(ed.message(), Some("one line"));
    feed(&mut ed, ":many<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some(":many"));
    assert!(ed.render().to_string().contains("b: 2"));
}

#[test]
fn nested_commands_keep_their_own_reply() {
    let mut ed = editor("x");
    ed.commands_mut().register("inner", "", |ed, _| {
        ed.reply(Value::from("inner"));
        Ok(())
    });
    ed.commands_mut().register("outer", "", |ed, _| {
        ed.reply(Value::from("outer"));
        ed.run_command("inner");
        Ok(())
    });
    assert_eq!(ed.execute("outer"), Ok(Some(Value::from("outer"))));
}

#[test]
fn views_fail_when_not_interactive() {
    let mut ed = editor("x");
    assert!(ed.interactive());
    ed.set_interactive(false);
    for cmd in ["commands", "keymaps", "messages", "scope", "plugins"] {
        let e = ed.execute(cmd).unwrap_err();
        assert!(e.contains("needs the editor UI"), "{cmd}: {e}");
    }
    assert_eq!(ed.modal_title(), None);
    assert!(!ed.has_panel(vox_editor::PanelSlot::Side));
    // A reply is still returned; run_command does not open a view for it.
    ed.run_command("scope show");
    assert_eq!(ed.modal_title(), None);
}

#[test]
fn commands_list_and_scope_show() {
    let mut ed = editor("x");
    let Ok(Some(Value::Array(cmds))) = ed.execute("commands list") else {
        panic!("commands list returns a list")
    };
    let write = cmds
        .iter()
        .find(|c| c.to_text().starts_with("aliases:\n  - w\n"))
        .expect("write has alias w");
    assert!(write.to_text().contains("name: write\nowner: builtin"));
    ed.set_scope(vox_editor::Scope::global("/cfg".into()), None);
    assert_eq!(
        ed.execute("scope show"),
        Ok(Some(Value::table([
            ("kind", Value::from("global")),
            ("config", "/cfg/config.toml".into()),
            ("plugins", "/cfg/plugins".into()),
            ("lockfile", "/cfg/vox.lock".into()),
            ("exists", false.into()),
        ])))
    );
    ed.set_scope(vox_editor::Scope::vanilla(), None);
    assert_eq!(
        ed.execute("scope show"),
        Ok(Some(Value::table([
            ("kind", Value::from("vanilla")),
            ("exists", false.into()),
        ])))
    );
}

#[test]
fn plugin_list_and_info() {
    use vox_editor::plugin::{HealthItem, HealthStatus, PluginInfo, SettingValue};
    let mut ed = editor("x");
    let info = |name: &str, status| PluginInfo {
        name: name.into(),
        path: format!("/p/{name}").into(),
        wasm: format!("/p/{name}/{name}.wasm").into(),
        manifest: None,
        status,
        items: vec![
            HealthItem::ok("api 0.2"),
            HealthItem::problem(vox_editor::Level::Warn, "git not found"),
        ],
        settings: vec![("case".into(), SettingValue::String("upper".into()))],
    };
    ed.set_plugin_infos(vec![
        info("a", HealthStatus::Warning),
        info("b", HealthStatus::Disabled),
    ]);
    assert_eq!(
        ed.execute("plugin list").unwrap().unwrap().to_text(),
        "- name: a\n  path: /p/a\n  status: warning\n- name: b\n  path: /p/b\n  status: disabled"
    );
    assert_eq!(
        ed.execute("plugin info a").unwrap().unwrap().to_text(),
        "health:\n  - level: info\n    message: api 0.2\n    ok: true\n  \
         - level: warning\n    message: git not found\n    ok: false\n\
         name: a\npath: /p/a\nsettings:\n  case: upper\nstatus: warning"
    );
    assert!(
        ed.execute("plugin info c")
            .unwrap_err()
            .contains("no plugin named c")
    );
    assert!(ed.execute("plugin info").unwrap_err().starts_with("E471"));
}

#[test]
fn plugin_commands_do_not_ask_when_not_interactive() {
    let dir = std::env::temp_dir().join(format!("vox-api-headless-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("plugins/demo")).unwrap();
    let mut ed = editor("x");
    ed.set_scope(vox_editor::Scope::global(dir.clone()), None);
    ed.set_file_format(
        |_| Ok(Value::Table(Default::default())),
        |_| Ok(String::new()),
    );
    ed.set_interactive(false);
    assert!(
        ed.execute("plugin add")
            .unwrap_err()
            .starts_with("E471: Use :plugin add")
    );
    let e = ed.execute("plugin remove demo").unwrap_err();
    assert!(e.contains("use :plugin remove! demo"), "{e}");
    assert!(dir.join("plugins/demo").exists());
    assert_eq!(ed.execute("plugin remove! demo"), Ok(None));
    assert!(!dir.join("plugins/demo").exists());
    std::fs::remove_dir_all(&dir).ok();
}
