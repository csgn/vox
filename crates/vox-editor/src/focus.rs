//! Focus mode: work on one unit — selection, block, function, class, or file — with the
//! rest of the file dimmed. In agent mode it is also what the AI may change.

use crate::command::{Args, CommandResult, Commands, Param};
use crate::editor::{Editor, Mode};
use crate::structure::{Structure, Unit, UnitKind};

impl Editor {
    /// The focused unit, if any.
    pub fn focused_unit(&self) -> Option<&Unit> {
        self.unit_focus.as_ref()
    }

    /// Replace how units are found (the default is [`crate::structure::Heuristic`]).
    pub fn set_structure(&mut self, structure: Box<dyn Structure>) {
        self.structure = structure;
    }

    /// Units around `line`, innermost first, the file last.
    pub fn units_at(&self, line: usize) -> Vec<Unit> {
        let lines: Vec<String> = (0..self.doc.line_count())
            .map(|i| self.doc.line(i).unwrap_or_default())
            .collect();
        self.structure.units(&lines, line)
    }

    /// Focus `unit` (or nothing). The cursor moves inside it if it was outside.
    pub fn set_focus(&mut self, unit: Option<Unit>) {
        if let Some(u) = &unit
            && !u.contains(self.cursor.line)
        {
            self.set_cursor(vox_core::Position::new(u.start, 0));
        }
        self.unit_focus = unit;
    }

    /// Focus the innermost unit at the cursor, or the next larger unit around the focused
    /// one.
    pub fn focus_out(&mut self) {
        let units = self.units_at(self.cursor.line);
        let next = match &self.unit_focus {
            None => units.into_iter().next(),
            Some(cur) => units
                .into_iter()
                .find(|u| u.start <= cur.start && u.end >= cur.end && u.len() > cur.len()),
        };
        if let Some(u) = next {
            self.set_focus(Some(u));
        }
    }

    /// Focus the next smaller unit at the cursor inside the focused one.
    pub fn focus_in(&mut self) {
        let Some(cur) = self.unit_focus.clone() else {
            return self.focus_out();
        };
        let inner = self
            .units_at(self.cursor.line)
            .into_iter()
            .filter(|u| u.start >= cur.start && u.end <= cur.end && u.len() < cur.len())
            .max_by_key(Unit::len);
        if let Some(u) = inner {
            self.set_focus(Some(u));
        }
    }

    /// Focus the nearest unit of `kind` around the cursor; for a selection, the lines of
    /// the visual selection (leaving visual mode).
    pub fn focus_kind(&mut self, kind: UnitKind) -> Result<(), String> {
        if kind == UnitKind::Selection {
            let (a, b, _) = self
                .selection()
                .ok_or("E900: no selection: select lines in visual mode first")?;
            self.mode = Mode::Normal;
            self.set_focus(Some(Unit {
                kind,
                start: a.line,
                end: b.line,
                name: None,
            }));
            return Ok(());
        }
        let unit = self
            .units_at(self.cursor.line)
            .into_iter()
            .find(|u| u.kind == kind)
            .ok_or_else(|| format!("E900: no {} around the cursor", kind.name()))?;
        self.set_focus(Some(unit));
        Ok(())
    }

    /// Keep the focused range on the same text after an edit that changed the number of
    /// lines from `before` (made with the cursor on line `at`).
    pub(crate) fn follow_focus(&mut self, before: usize, at: usize) {
        let now = self.doc.line_count();
        let Some(f) = &mut self.unit_focus else {
            return;
        };
        if now == before {
            return;
        }
        let delta = now as isize - before as isize;
        let shift = |n: usize| (n as isize + delta).max(0) as usize;
        if at < f.start {
            f.start = shift(f.start);
            f.end = shift(f.end);
        } else if at <= f.end {
            f.end = shift(f.end).max(f.start);
        }
        let last = now.saturating_sub(1);
        f.end = f.end.min(last);
        f.start = f.start.min(f.end);
    }
}

/// Register `:narrow` and `:widen`.
pub(crate) fn register(c: &mut Commands) {
    use crate::command::category::FOCUS;
    c.register(
        "narrow",
        "Focus one unit, the rest dimmed: `:narrow [KIND|out|in]`; no argument: the innermost unit at the cursor.",
        narrow,
    )
    .category(FOCUS)
    .param(
        Param::optional("KIND", "What to focus, or `out` / `in` for the next larger / smaller unit.")
            .values(&["selection", "block", "function", "class", "file", "out", "in"]),
    )
    .example("narrow function", "the function around the cursor")
    .example("narrow out", "one unit larger (also <leader>f)");
    c.register(
        "widen",
        "End focus: show the whole file normally.",
        |ed, _| {
            ed.set_focus(None);
            Ok(())
        },
    )
    .category(FOCUS);
}

/// `:narrow [KIND|out|in]`.
fn narrow(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    match a.arg {
        None => {
            ed.set_focus(None);
            ed.focus_out();
        }
        Some("out") => ed.focus_out(),
        Some("in") => ed.focus_in(),
        Some(k) => {
            let kind = UnitKind::parse(k).ok_or_else(|| {
                format!("E474: Use :narrow selection|block|function|class|file|out|in, not {k}")
            })?;
            ed.focus_kind(kind)?;
        }
    }
    if let Some(u) = ed.focused_unit() {
        let label = u.label();
        ed.set_message(format!("focus: {label}"));
    }
    Ok(())
}
