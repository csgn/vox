//! Asking the AI about the focused unit (agent mode). The answer replaces the unit's lines;
//! it is shown in the buffer as a diff to accept or decline hunk by hunk, or applied at
//! once (`ai_apply = "direct"`). The AI itself is an [`Assistant`] set by the binary; core
//! knows nothing about any particular model or API.

use std::sync::Arc;

use vox_core::{Document, Position};

use crate::command::{Args, CommandResult, Commands, Param};
use crate::editor::{Editor, Mode};
use crate::keymap::MapMode;
use crate::owner;
use crate::structure::UnitKind;
use crate::ui::Input;

/// What the AI is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistRequest {
    /// What the user wants, e.g. "add error handling".
    pub instruction: String,
    /// File path, if the buffer has one.
    pub path: Option<String>,
    /// The whole buffer, for context.
    pub text: String,
    /// First line the answer replaces (0-based).
    pub start: usize,
    /// Last line the answer replaces (0-based, inclusive).
    pub end: usize,
    /// What the range is: a focused unit's kind, or `Selection` / `Block` for a selection or
    /// the cursor line.
    pub kind: UnitKind,
    /// Cursor position.
    pub cursor: Position,
    /// Model to ask (`ai_model`); empty: the backend's default.
    pub model: String,
    /// How hard the model works (`ai_effort`): one of [`EFFORTS`].
    pub effort: String,
}

/// Values of `ai_effort`, least to most work.
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

impl AssistRequest {
    /// The lines the answer replaces.
    pub fn target(&self) -> String {
        self.text
            .lines()
            .skip(self.start)
            .take(self.end + 1 - self.start)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The AI's answer: new text for the requested lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistReply {
    /// Replacement for lines `start..=end`, lines separated by `\n`.
    pub text: String,
}

/// An AI backend. Called on a background thread.
pub trait Assistant: Send + Sync {
    /// Answer `req`, or say why not.
    fn ask(&self, req: &AssistRequest) -> Result<AssistReply, String>;
}

/// How an AI answer lands in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApplyMode {
    /// As an inline diff to accept or decline.
    #[default]
    Review,
    /// At once, as one undo step.
    Direct,
}

impl ApplyMode {
    /// Config name: `review`, `direct`.
    pub fn name(self) -> &'static str {
        match self {
            ApplyMode::Review => "review",
            ApplyMode::Direct => "direct",
        }
    }

    /// Parse a config name.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "review" => Some(ApplyMode::Review),
            "direct" => Some(ApplyMode::Direct),
            _ => None,
        }
    }
}

/// One change: `old` lines starting at line `at` become `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// First line (in the buffer as it is now).
    pub at: usize,
    /// Lines removed (empty for a pure insertion before `at`).
    pub old: Vec<String>,
    /// Lines added in their place.
    pub new: Vec<String>,
}

impl Hunk {
    /// Lines this hunk covers in the buffer (an insertion covers line `at`).
    fn covers(&self, line: usize) -> bool {
        line >= self.at && line < self.at + self.old.len().max(1)
    }
}

/// An AI answer waiting to be accepted or declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Buffer it belongs to.
    pub buffer: usize,
    /// The instruction it answers.
    pub instruction: String,
    /// Changes not decided yet, top to bottom.
    pub hunks: Vec<Hunk>,
}

/// Line diff of `old` → `new` as hunks starting at line `offset`: longest common
/// subsequence of lines, then each run of differences becomes one hunk.
pub fn diff(old: &[String], new: &[String], offset: usize) -> Vec<Hunk> {
    let (n, m) = (old.len(), new.len());
    // lcs[i][j]: length of the LCS of old[i..] and new[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old[i] == new[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut hunks = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut cur: Option<Hunk> = None;
    let flush = |cur: &mut Option<Hunk>, hunks: &mut Vec<Hunk>| {
        if let Some(h) = cur.take() {
            hunks.push(h);
        }
    };
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            flush(&mut cur, &mut hunks);
            i += 1;
            j += 1;
            continue;
        }
        let h = cur.get_or_insert_with(|| Hunk {
            at: offset + i,
            old: Vec::new(),
            new: Vec::new(),
        });
        if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            h.new.push(new[j].clone());
            j += 1;
        } else {
            h.old.push(old[i].clone());
            i += 1;
        }
    }
    flush(&mut cur, &mut hunks);
    hunks
}

/// The end of `line` (after its last char).
fn end_of(doc: &Document, line: usize) -> Position {
    Position::new(line, doc.line(line).map_or(0, |l| l.chars().count()))
}

/// Replace `count` lines from line `at` with `new` (whole lines) in `doc`.
fn replace_lines(doc: &mut Document, at: usize, count: usize, new: &[String]) {
    let lines = doc.line_count();
    let joined = new.join("\n");
    if at + count < lines {
        // Followed by more lines: work on "line\n" units.
        let from = Position::new(at, 0);
        doc.delete(from, Position::new(at + count, 0));
        if !new.is_empty() {
            doc.insert(from, &format!("{joined}\n"));
        }
    } else if at == 0 {
        // Everything.
        let to = end_of(doc, lines - 1);
        doc.delete(Position::new(0, 0), to);
        doc.insert(Position::new(0, 0), &joined);
    } else {
        // Up to the end: also take the line break before `at`.
        let from = end_of(doc, at - 1);
        let to = end_of(doc, lines - 1);
        doc.delete(from, to);
        if !new.is_empty() {
            doc.insert(from, &format!("\n{joined}"));
        }
    }
}

impl Editor {
    /// Set the AI backend used by `:ai`.
    pub fn set_assistant(&mut self, assistant: Arc<dyn Assistant>) {
        self.assistant = Some(assistant);
    }

    /// The AI change waiting for a decision, if any.
    pub fn proposal(&self) -> Option<&Proposal> {
        self.proposal.as_ref()
    }

    /// Lines the AI would change now: the visual selection, else the focused unit, else
    /// the cursor line. Returns (start, end inclusive, kind).
    pub fn ai_target(&self) -> (usize, usize, UnitKind) {
        if let Some((a, b, _)) = self.selection() {
            return (a.line, b.line, UnitKind::Selection);
        }
        if let Some(u) = self.focused_unit() {
            return (u.start, u.end, u.kind);
        }
        (self.cursor.line, self.cursor.line, UnitKind::Block)
    }

    /// Ask the AI to change the target lines (see [`Editor::ai_target`]) as `instruction`
    /// says. Runs in the background; the answer becomes a proposal or is applied.
    pub fn ask_ai(&mut self, instruction: &str) -> Result<(), String> {
        let assistant = self
            .assistant
            .clone()
            .ok_or("E900: no AI backend is configured")?;
        if self.proposal.is_some() {
            return Err("E900: accept or decline the pending AI change first (ga / gx)".into());
        }
        let (start, end, kind) = self.ai_target();
        if matches!(self.mode, Mode::Visual { .. }) {
            self.mode = Mode::Normal;
        }
        let text = (0..self.doc.line_count())
            .map(|i| self.doc.line(i).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        let req = AssistRequest {
            instruction: instruction.to_string(),
            path: self.doc.path().map(|p| p.display().to_string()),
            text,
            start,
            end,
            kind,
            cursor: self.cursor,
            model: self.config().ai_model.clone(),
            effort: self.config().ai_effort.clone(),
        };
        let (buffer, revision) = (self.buffer_id, self.doc.revision());
        let title = format!("AI: {instruction}");
        self.spawn_job(&title, move |_progress| {
            let answer = assistant.ask(&req);
            Box::new(move |ed: &mut Editor| ed.take_answer(req, answer, buffer, revision))
        });
        Ok(())
    }

    /// Turn an answer into a proposal (or apply it), if the text is still as it was asked.
    fn take_answer(
        &mut self,
        req: AssistRequest,
        answer: Result<AssistReply, String>,
        buffer: usize,
        revision: u64,
    ) {
        let reply = match answer {
            Ok(r) => r,
            Err(e) => return self.set_message(format!("E900: AI: {e}")),
        };
        if self.buffer_id != buffer || self.doc.revision() != revision {
            return self.set_message("E900: AI: the text changed while waiting; ask again");
        }
        let old: Vec<String> = req.target().lines().map(String::from).collect();
        let new: Vec<String> = reply.text.lines().map(String::from).collect();
        let hunks = diff(&old, &new, req.start);
        if hunks.is_empty() {
            return self.set_message("AI: no changes");
        }
        let proposal = Proposal {
            buffer,
            instruction: req.instruction,
            hunks,
        };
        match self.config().ai_apply {
            ApplyMode::Direct => {
                let n = proposal.hunks.len();
                self.proposal = Some(proposal);
                self.decide(true, true);
                self.set_message(format!("AI: applied {n} change(s) (u to undo)"));
            }
            ApplyMode::Review => {
                let n = proposal.hunks.len();
                let first = proposal.hunks[0].at;
                self.proposal = Some(proposal);
                self.proposal_revision = self.doc.revision();
                self.set_cursor(Position::new(first, 0));
                self.set_message(format!(
                    "AI: {n} change(s) — ga accept · gx decline · gA / gX all · ]c / [c next / previous"
                ));
            }
        }
    }

    /// Index of the hunk at the cursor, else the next one below it, else the last.
    fn hunk_at_cursor(&self) -> Option<usize> {
        let p = self.proposal.as_ref()?;
        let line = self.cursor.line;
        p.hunks
            .iter()
            .position(|h| h.covers(line))
            .or_else(|| p.hunks.iter().position(|h| h.at >= line))
            .or(p.hunks.len().checked_sub(1))
    }

    /// Accept (`accept`) or decline the hunk at the cursor, or all hunks (`all`). Accepted
    /// hunks are one undo step.
    pub fn decide(&mut self, accept: bool, all: bool) {
        let Some(mut p) = self.proposal.take() else {
            return;
        };
        let chosen: Vec<usize> = if all {
            (0..p.hunks.len()).collect()
        } else {
            self.proposal = Some(p.clone());
            let Some(i) = self.hunk_at_cursor() else {
                return;
            };
            self.proposal = None;
            vec![i]
        };
        let mut cursor = None;
        // Bottom up, so earlier hunks keep their line numbers.
        for &i in chosen.iter().rev() {
            let h = p.hunks.remove(i);
            if accept {
                replace_lines(&mut self.doc, h.at, h.old.len(), &h.new);
                let delta = h.new.len() as isize - h.old.len() as isize;
                for later in p.hunks.iter_mut().skip(i) {
                    later.at = (later.at as isize + delta) as usize;
                }
                cursor = Some(h.at);
            } else {
                cursor = cursor.or(Some(h.at));
            }
        }
        if accept {
            self.doc.commit();
        }
        if let Some(line) = cursor {
            self.set_cursor(Position::new(line, 0));
        }
        if !p.hunks.is_empty() {
            self.proposal = Some(p);
            self.proposal_revision = self.doc.revision();
        }
    }

    /// Move the cursor to the next (`forward`) or previous hunk.
    pub fn jump_hunk(&mut self, forward: bool) {
        let Some(p) = &self.proposal else {
            return;
        };
        let line = self.cursor.line;
        let target = if forward {
            p.hunks.iter().find(|h| h.at > line).or(p.hunks.first())
        } else {
            p.hunks
                .iter()
                .rev()
                .find(|h| h.at < line)
                .or(p.hunks.last())
        };
        if let Some(h) = target {
            self.set_cursor(Position::new(h.at, 0));
        }
    }

    /// Drop the proposal if the text was edited (its line numbers no longer fit).
    pub(crate) fn check_proposal(&mut self) {
        let here = self
            .proposal
            .as_ref()
            .is_some_and(|p| p.buffer == self.buffer_id);
        if here && self.doc.revision() != self.proposal_revision {
            self.proposal = None;
            self.set_message("AI change dropped: the text was edited");
        }
    }

    /// Register agent mode's AI commands and keys (owner [`owner::AGENT`]).
    pub(crate) fn register_agent(&mut self) {
        self.remove_owned(owner::AGENT);
        self.with_owner(owner::AGENT, |ed| {
            register(ed.commands_mut());
            for (keys, cmd) in [
                ("<leader>a", ":ai"),
                ("ga", ":ai accept"),
                ("gx", ":ai decline"),
                ("gA", ":ai accept all"),
                ("gX", ":ai decline all"),
                ("]c", ":ai next"),
                ("[c", ":ai prev"),
            ] {
                ed.map_key(MapMode::Normal, keys, cmd);
            }
            ed.map_key(MapMode::Visual, "<leader>a", ":ai");
        });
    }
}

/// The `:ai` commands.
fn register(c: &mut Commands) {
    use crate::command::category::AI;
    c.register(
        "ai",
        "Ask the AI to change the focused unit (else the selection, else the line): `:ai INSTRUCTION`; no argument opens a dialog.",
        ai,
    )
    .category(AI)
    .param(Param::optional("INSTRUCTION", "What to change, in your words."))
    .example("ai add error handling", "the AI's change appears as a diff to accept or decline");
    let all =
        || Param::optional("all", "Every change, not only the one at the cursor.").values(&["all"]);
    c.register(
        "ai accept",
        "Accept the AI change at the cursor (or `all`).",
        |ed, a| {
            need_proposal(ed)?;
            ed.decide(true, a.arg == Some("all"));
            Ok(())
        },
    )
    .category(AI)
    .param(all());
    c.register(
        "ai decline",
        "Decline the AI change at the cursor (or `all`).",
        |ed, a| {
            need_proposal(ed)?;
            ed.decide(false, a.arg == Some("all"));
            Ok(())
        },
    )
    .category(AI)
    .param(all());
    c.register("ai next", "Go to the next AI change.", |ed, _| {
        need_proposal(ed)?;
        ed.jump_hunk(true);
        Ok(())
    })
    .category(AI);
    c.register("ai prev", "Go to the previous AI change.", |ed, _| {
        need_proposal(ed)?;
        ed.jump_hunk(false);
        Ok(())
    })
    .category(AI);
}

/// Error unless an AI change is pending.
fn need_proposal(ed: &Editor) -> CommandResult {
    match ed.proposal() {
        Some(_) => Ok(()),
        None => Err("E900: no AI change is pending".into()),
    }
}

/// `:ai [INSTRUCTION]`.
fn ai(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if let Some(instruction) = a.arg {
        return ed.ask_ai(instruction);
    }
    if ed.assistant.is_none() {
        return ed.ask_ai("");
    }
    // Keep a selection as the target: focus it, since the dialog ends visual mode.
    if ed.selection().is_some() {
        ed.focus_kind(UnitKind::Selection)?;
    }
    let (start, end, kind) = ed.ai_target();
    let what = match ed.focused_unit() {
        Some(u) => u.label(),
        None => format!("{} (L{}–{})", kind.name(), start + 1, end + 1),
    };
    let input = Input::new(format!("Ask AI · {what}"), "> ", "ai {}");
    ed.open_modal(Box::new(input));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<String> {
        s.lines().map(String::from).collect()
    }

    #[test]
    fn diff_hunks() {
        let h = diff(&v("a\nb\nc\nd"), &v("a\nB\nc\nd\ne"), 10);
        assert_eq!(
            h,
            [
                Hunk {
                    at: 11,
                    old: v("b"),
                    new: v("B")
                },
                Hunk {
                    at: 14,
                    old: vec![],
                    new: v("e")
                },
            ]
        );
        assert!(diff(&v("x\ny"), &v("x\ny"), 0).is_empty());
        let h = diff(&v("x\ny"), &v(""), 0);
        assert_eq!(h[0].old, v("x\ny"));
    }

    #[test]
    fn replace_whole_lines() {
        let s = vox_core::storage::SimpleStorage::boxed;
        let mut d = Document::new("a\nb\nc", s);
        replace_lines(&mut d, 1, 1, &v("B1\nB2"));
        assert_eq!(d.to_string(), "a\nB1\nB2\nc");
        replace_lines(&mut d, 3, 1, &v("C"));
        assert_eq!(d.to_string(), "a\nB1\nB2\nC");
        replace_lines(&mut d, 4, 0, &v("D"));
        assert_eq!(d.to_string(), "a\nB1\nB2\nC\nD");
        replace_lines(&mut d, 1, 2, &[]);
        assert_eq!(d.to_string(), "a\nC\nD");
        replace_lines(&mut d, 2, 1, &[]);
        assert_eq!(d.to_string(), "a\nC");
        replace_lines(&mut d, 0, 2, &v("only"));
        assert_eq!(d.to_string(), "only");
        replace_lines(&mut d, 0, 0, &v("first"));
        assert_eq!(d.to_string(), "first\nonly");
    }
}
