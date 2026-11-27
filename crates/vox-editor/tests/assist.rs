//! Asking the AI (agent mode) with a fake backend: the dialog, the inline diff, accepting
//! and declining, undo, and `ai_apply = "direct"`.

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use vox_core::Document;
use vox_editor::agent::EditorMode;
use vox_editor::assist::{ApplyMode, AssistReply, AssistRequest, Assistant};
use vox_editor::structure::UnitKind;
use vox_editor::{Config, Editor, parse_keys};

const CODE: &str = "fn area(w: u32, h: u32) -> u32 {
    let a = w * h;
    a
}

fn main() {}";

/// Answers with `answer(request)`; remembers the last request.
struct Fake {
    answer: fn(&AssistRequest) -> Result<String, String>,
    last: Mutex<Option<AssistRequest>>,
}

impl Assistant for Fake {
    fn ask(&self, req: &AssistRequest) -> Result<AssistReply, String> {
        *self.last.lock().unwrap() = Some(req.clone());
        (self.answer)(req).map(|text| AssistReply { text })
    }
}

fn fake(answer: fn(&AssistRequest) -> Result<String, String>) -> Arc<Fake> {
    Arc::new(Fake {
        answer,
        last: Mutex::new(None),
    })
}

/// Uppercase `let` lines and mark the closing brace: two changes apart (for `area`).
fn upper_a(req: &AssistRequest) -> Result<String, String> {
    Ok(req
        .target()
        .lines()
        .map(|l| match l {
            l if l.contains("let ") => l.to_uppercase(),
            "}" => "} // done".to_string(),
            l => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn editor(assistant: Option<Arc<Fake>>) -> Editor {
    let mut ed = Editor::new(Document::from_text(CODE));
    ed.resize(60, 14);
    ed.set_editor_mode(EditorMode::Agent);
    if let Some(a) = assistant {
        ed.set_assistant(a);
    }
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

fn wait(ed: &mut Editor) {
    while ed.jobs_running() > 0 {
        std::thread::sleep(Duration::from_millis(5));
        ed.tick();
    }
}

fn text(ed: &Editor) -> String {
    ed.document().to_string()
}

#[test]
fn only_in_agent_mode() {
    let mut ed = editor(None);
    ed.set_editor_mode(EditorMode::Text);
    assert!(ed.execute("ai hi").unwrap_err().starts_with("E492"));
    ed.set_editor_mode(EditorMode::Agent);
    let e = ed.execute("ai hi").unwrap_err();
    assert!(e.contains("no AI backend"), "{e}");
}

#[test]
fn request_is_the_focused_unit() {
    let f = fake(upper_a);
    let mut ed = editor(Some(f.clone()));
    feed(&mut ed, "2G:narrow function<CR>:ai shout<CR>");
    wait(&mut ed);
    let req = f.last.lock().unwrap().clone().unwrap();
    assert_eq!((req.start, req.end, req.kind), (0, 3, UnitKind::Function));
    assert_eq!(req.instruction, "shout");
    assert_eq!(req.target().lines().count(), 4);
    assert_eq!(req.text, CODE);
    assert_eq!((req.model.as_str(), req.effort.as_str()), ("", "medium"));
    // Without focus: the cursor line.
    feed(&mut ed, "gX<Esc>6G:ai x<CR>");
    wait(&mut ed);
    let req = f.last.lock().unwrap().clone().unwrap();
    assert_eq!((req.start, req.end, req.kind), (5, 5, UnitKind::Block));
}

#[test]
fn review_accept_and_undo() {
    let mut ed = editor(Some(fake(upper_a)));
    feed(&mut ed, "2G\\f\\f:ai shout<CR>");
    wait(&mut ed);
    let p = ed.proposal().expect("pending");
    assert_eq!(p.hunks.len(), 2, "two changed lines, apart");
    assert_eq!(text(&ed), CODE, "nothing changed yet");
    assert!(ed.message().unwrap().contains("2 change(s)"));
    // The cursor is on the first change; accept it.
    feed(&mut ed, "ga");
    assert!(text(&ed).contains("    LET A = W * H;"));
    assert!(
        !text(&ed).contains("// done"),
        "the second is still pending"
    );
    assert_eq!(ed.proposal().unwrap().hunks.len(), 1);
    // Next change, decline it: done.
    feed(&mut ed, "]cgx");
    assert!(ed.proposal().is_none());
    assert!(!text(&ed).contains("// done"));
    feed(&mut ed, "u");
    assert_eq!(text(&ed), CODE, "accepting is one undo step");
}

#[test]
fn accept_and_decline_all() {
    let mut ed = editor(Some(fake(upper_a)));
    feed(&mut ed, "2G:narrow function<CR>:ai shout<CR>");
    wait(&mut ed);
    feed(&mut ed, "gA");
    assert!(ed.proposal().is_none());
    assert!(text(&ed).contains("LET A") && text(&ed).contains("} // done"));
    assert!(
        text(&ed).ends_with("fn main() {}"),
        "outside the unit unchanged"
    );
    feed(&mut ed, "u");
    assert_eq!(text(&ed), CODE);
    feed(&mut ed, ":ai shout<CR>");
    wait(&mut ed);
    feed(&mut ed, "gX");
    assert!(ed.proposal().is_none());
    assert_eq!(text(&ed), CODE);
    assert!(
        ed.execute("ai accept")
            .unwrap_err()
            .contains("no AI change")
    );
}

#[test]
fn editing_drops_the_pending_change() {
    let mut ed = editor(Some(fake(upper_a)));
    feed(&mut ed, "2G:narrow function<CR>:ai shout<CR>");
    wait(&mut ed);
    feed(&mut ed, "x");
    assert!(ed.proposal().is_none());
    assert_eq!(ed.message(), Some("AI change dropped: the text was edited"));
}

#[test]
fn text_changed_while_waiting() {
    static GATE: Mutex<Option<mpsc::Receiver<()>>> = Mutex::new(None);
    let (tx, rx) = mpsc::channel();
    *GATE.lock().unwrap() = Some(rx);
    let slow = fake(|req| {
        let rx = GATE.lock().unwrap().take().unwrap();
        rx.recv().unwrap();
        upper_a(req)
    });
    let mut ed = editor(Some(slow));
    feed(&mut ed, "2G:ai shout<CR>x");
    tx.send(()).unwrap();
    wait(&mut ed);
    assert!(ed.proposal().is_none());
    assert!(ed.message().unwrap().contains("text changed while waiting"));
}

#[test]
fn model_and_effort_come_from_the_config() {
    let f = fake(upper_a);
    let mut ed = editor(Some(f.clone()));
    ed.apply_config(&Config {
        ai_model: "claude-sonnet-5-5".into(),
        ai_effort: "high".into(),
        editor_mode: EditorMode::Agent,
        ..Config::default()
    });
    feed(&mut ed, ":ai x<CR>");
    wait(&mut ed);
    let req = f.last.lock().unwrap().clone().unwrap();
    assert_eq!(
        (req.model.as_str(), req.effort.as_str()),
        ("claude-sonnet-5-5", "high")
    );
}

#[test]
fn errors_and_no_changes() {
    let mut ed = editor(Some(fake(|_| Err("quota".into()))));
    feed(&mut ed, ":ai x<CR>");
    wait(&mut ed);
    assert_eq!(ed.message(), Some("E900: AI: quota"));
    let mut ed = editor(Some(fake(|req| Ok(req.target()))));
    feed(&mut ed, ":ai x<CR>");
    wait(&mut ed);
    assert_eq!(ed.message(), Some("AI: no changes"));
}

#[test]
fn direct_apply() {
    let mut ed = editor(Some(fake(upper_a)));
    let cfg = Config {
        ai_apply: ApplyMode::Direct,
        editor_mode: EditorMode::Agent,
        ..Config::default()
    };
    ed.apply_config(&cfg);
    feed(&mut ed, "2G:narrow function<CR>:ai shout<CR>");
    wait(&mut ed);
    assert!(ed.proposal().is_none());
    assert!(text(&ed).contains("LET A"));
    assert_eq!(ed.message(), Some("AI: applied 2 change(s) (u to undo)"));
    feed(&mut ed, "u");
    assert_eq!(text(&ed), CODE);
}

#[test]
fn dialog_with_leader_a() {
    let mut ed = editor(Some(fake(upper_a)));
    feed(&mut ed, "2G:narrow function<CR>\\a");
    assert_eq!(
        ed.modal_title().as_deref(),
        Some("Ask AI · function area (L1–4)")
    );
    feed(&mut ed, "shout<CR>");
    wait(&mut ed);
    assert!(ed.proposal().is_some());
    // Visual: the selected lines become the focus and the target.
    feed(&mut ed, "gX6GV\\a");
    assert_eq!(
        ed.modal_title().as_deref(),
        Some("Ask AI · selection (L6–6)")
    );
    assert_eq!(
        ed.focused_unit().map(|u| (u.kind, u.start)),
        Some((UnitKind::Selection, 5))
    );
}

#[test]
fn leaving_agent_mode_drops_it() {
    let mut ed = editor(Some(fake(upper_a)));
    feed(&mut ed, "2G:ai shout<CR>");
    wait(&mut ed);
    assert!(ed.proposal().is_some());
    feed(&mut ed, ":mode text<CR>");
    assert!(ed.proposal().is_none());
    assert!(ed.commands().get("ai").is_none());
}

mod snapshots {
    use super::*;

    #[test]
    fn inline_diff() {
        let mut ed = editor(Some(fake(upper_a)));
        feed(&mut ed, "2G:narrow function<CR>:ai shout<CR>");
        wait(&mut ed);
        let s = ed.render();
        // The cursor is on the first removed line (row 1).
        assert_eq!(s.cursor.1, 1);
        // Removed lines are red, added lines green.
        assert_eq!(s.cell(8, 1).style.fg, vox_editor::Color::Red);
        assert_eq!(s.cell(8, 2).style.fg, vox_editor::Color::Green);
        assert_eq!(s.cell(8, 3).style.fg, vox_editor::Color::Default);
        insta::assert_snapshot!(s.to_string());
    }
}
