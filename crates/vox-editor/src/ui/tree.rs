//! A drill-down tree view: one column per level, side by side when there is room, with a
//! breadcrumb path, a detail area for the selection, and a filter over all levels.

use std::cell::Cell;

use vox_view::CharWidth;

use super::{Outcome, View, wrap};
use crate::keys::{Key, KeyCode};
use crate::surface::{Color, Rect, Style, Surface};

/// Narrowest column; fewer columns are shown when the view is narrower.
const MIN_COLUMN: u16 = 24;
/// Rows of the detail area.
const DETAIL_ROWS: u16 = 3;

/// One entry of a [`TreeView`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeNode {
    /// Text in the column and the breadcrumb.
    pub label: String,
    /// Dimmed text after the label, when the column is wide enough.
    pub detail: String,
    /// Lines shown in the detail area while this entry is selected.
    pub info: Vec<String>,
    /// Command line run by `Enter` on an entry without children.
    pub run: Option<String>,
    /// Command line run by `?` (e.g. `help NAME`).
    pub help: Option<String>,
    /// Entries one level down.
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// An entry with a label only.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }
}

/// A filter match: the indices from the top level down, and the joined labels.
type Match = (Vec<usize>, String);

/// A tree browsed one level at a time. Keys: `j`/`k` move, `l`/`Enter` go in, `Enter` on an
/// entry without children runs it, `h`/`Backspace` go back up, `?` help, `/` filter,
/// `q`/`Esc` close.
#[derive(Debug, Clone)]
pub struct TreeView {
    title: String,
    /// First breadcrumb part, e.g. `Commands`.
    root: String,
    nodes: Vec<TreeNode>,
    /// Selected index at each level; the last one is the active column.
    path: Vec<usize>,
    /// Text typed after `/`; `None` when not filtering.
    filter: Option<String>,
    /// Selected filter match.
    filter_sel: usize,
    /// Shown in a panel (stays open while running commands) rather than the modal.
    in_panel: bool,
    /// Rows of the column area last drawn (for scrolling).
    rows: Cell<u16>,
}

impl TreeView {
    /// A tree titled `title` whose breadcrumb starts with `root`.
    pub fn new(title: impl Into<String>, root: impl Into<String>, nodes: Vec<TreeNode>) -> Self {
        Self {
            title: title.into(),
            root: root.into(),
            nodes,
            path: vec![0],
            filter: None,
            filter_sel: 0,
            in_panel: false,
            rows: Cell::new(0),
        }
    }

    /// Shown in a panel: commands run with the view kept open.
    pub fn in_panel(mut self) -> Self {
        self.in_panel = true;
        self
    }

    /// Entries at `level` (children of the selection one level up).
    fn level(&self, level: usize) -> &[TreeNode] {
        let mut nodes = self.nodes.as_slice();
        for &i in &self.path[..level.min(self.path.len())] {
            match nodes.get(i) {
                Some(n) => nodes = &n.children,
                None => return &[],
            }
        }
        nodes
    }

    /// The selected entry of the active column.
    pub fn current(&self) -> Option<&TreeNode> {
        let depth = self.path.len() - 1;
        self.level(depth).get(self.path[depth])
    }

    /// Labels from the top level down to the selection, e.g. `["Plugins", "plugin", "add"]`.
    pub fn selected_path(&self) -> Vec<&str> {
        (0..self.path.len())
            .filter_map(|d| self.level(d).get(self.path[d]))
            .map(|n| n.label.as_str())
            .collect()
    }

    /// Breadcrumb: `Commands › Plugins › plugin › add`.
    pub fn breadcrumb(&self) -> String {
        std::iter::once(self.root.as_str())
            .chain(self.selected_path())
            .collect::<Vec<_>>()
            .join(" › ")
    }

    /// Matches of the filter text at all levels, case-insensitive: entries whose path
    /// (joined labels) contains it, then entries whose detail does.
    pub fn matches(&self) -> Vec<Match> {
        let query = self.filter.as_deref().unwrap_or("").to_lowercase();
        let mut by_path = Vec::new();
        let mut by_detail = Vec::new();
        let mut stack: Vec<(Vec<usize>, String, &TreeNode)> = Vec::new();
        for (i, n) in self.nodes.iter().enumerate().rev() {
            stack.push((vec![i], n.label.clone(), n));
        }
        // Depth-first, in display order.
        while let Some((idx, joined, n)) = stack.pop() {
            if joined.to_lowercase().contains(&query) {
                by_path.push((idx.clone(), joined.clone()));
            } else if n.detail.to_lowercase().contains(&query) {
                by_detail.push((idx.clone(), joined.clone()));
            }
            for (i, c) in n.children.iter().enumerate().rev() {
                let mut ci = idx.clone();
                ci.push(i);
                stack.push((ci, format!("{joined} › {}", c.label), c));
            }
        }
        by_path.extend(by_detail);
        by_path
    }

    /// The node at `indices` from the top level.
    fn node_at(&self, indices: &[usize]) -> Option<&TreeNode> {
        let mut nodes = self.nodes.as_slice();
        let mut found = None;
        for &i in indices {
            let n = nodes.get(i)?;
            found = Some(n);
            nodes = &n.children;
        }
        found
    }

    /// Run a command line, keeping the view (panel) or remembering it (modal).
    fn run(&self, line: &str) -> Outcome {
        if self.in_panel {
            Outcome::Run(line.into())
        } else {
            Outcome::RunAndClose(line.into())
        }
    }

    /// Keys while typing a filter.
    fn filter_key(&mut self, key: Key) -> Outcome {
        let count = self.matches().len();
        let text = self.filter.as_mut().expect("filtering");
        match key.code {
            KeyCode::Esc => self.filter = None,
            KeyCode::Backspace => {
                if text.pop().is_none() {
                    self.filter = None;
                }
                self.filter_sel = 0;
            }
            KeyCode::Down => self.filter_sel = (self.filter_sel + 1).min(count.saturating_sub(1)),
            KeyCode::Up => self.filter_sel = self.filter_sel.saturating_sub(1),
            KeyCode::Char('n') if key.mods.ctrl => {
                self.filter_sel = (self.filter_sel + 1).min(count.saturating_sub(1));
            }
            KeyCode::Char('p') if key.mods.ctrl => {
                self.filter_sel = self.filter_sel.saturating_sub(1)
            }
            KeyCode::Enter => {
                // Jump to the match in the tree.
                if let Some((indices, _)) = self.matches().into_iter().nth(self.filter_sel) {
                    self.path = indices;
                }
                self.filter = None;
            }
            KeyCode::Char(c) if !key.mods.ctrl => {
                text.push(c);
                self.filter_sel = 0;
            }
            _ => {}
        }
        Outcome::Handled
    }

    /// Footer with the keys that work now.
    fn hint(&self) -> &'static str {
        if self.filter.is_some() {
            "type to filter · ↑/↓ move · Enter go · Esc cancel"
        } else if self.path.len() > 1 {
            "j/k move · l/Enter open · h back · ? help · / filter · Esc close"
        } else {
            "j/k move · l/Enter open · ? help · / filter · Esc close"
        }
    }

    /// Draw one column of entries.
    #[allow(clippy::too_many_arguments)]
    fn draw_column(
        &self,
        s: &mut Surface,
        area: Rect,
        nodes: &[TreeNode],
        selected: Option<usize>,
        active: bool,
        details: bool,
        width: &dyn CharWidth,
    ) {
        let h = area.h as usize;
        let top = selected.map_or(0, |sel| sel.saturating_sub(h.saturating_sub(1)));
        let cells = |s: &str| vox_view::display_col(width, s, s.chars().count());
        // Details start in one column, at most half way across.
        let label_w = nodes
            .iter()
            .map(|n| cells(&n.label) + if n.children.is_empty() { 3 } else { 5 })
            .max()
            .unwrap_or(0)
            .min(area.w as usize / 2);
        for (row, (i, n)) in nodes.iter().enumerate().skip(top).take(h).enumerate() {
            let line = Rect::new(area.x, area.y + row as u16, area.w, 1);
            let is_sel = selected == Some(i);
            let style = match (is_sel, active) {
                (true, true) => Style::REVERSED,
                (true, false) => Style::BOLD,
                _ => Style::default(),
            };
            if is_sel && active {
                s.clear(line, style);
            }
            let more = if n.children.is_empty() { "" } else { " ›" };
            let label = format!(" {}{more} ", n.label);
            s.put_in(line, 0, &label, style, width);
            let used = (cells(&label).max(label_w) + 1) as u16;
            if details && !n.detail.is_empty() && used < area.w {
                let rest = Rect::new(area.x + used, line.y, area.w - used, 1);
                let dim = Style {
                    dim: !(is_sel && active),
                    ..style
                };
                s.put_in(rest, 0, &n.detail, dim, width);
            }
        }
    }
}

impl View for TreeView {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        if area.h < 3 || area.w == 0 {
            return;
        }
        let accent = Style::BOLD.fg(Color::Blue);
        let top_line = match &self.filter {
            Some(q) => format!("/{q}"),
            None => self.breadcrumb(),
        };
        s.put_in(area, 0, &top_line, accent, width);
        let detail_h = if area.h >= 12 { DETAIL_ROWS } else { 0 };
        let foot_y = area.y + area.h - 1;
        // Columns between the top line (+ blank row) and the detail area (+ rule).
        let cols_y = area.y + 2;
        let cols_h = foot_y
            .saturating_sub(cols_y)
            .saturating_sub(if detail_h > 0 { detail_h + 1 } else { 0 });
        self.rows.set(cols_h);
        let cols_area = Rect::new(area.x, cols_y, area.w, cols_h);

        let info: Vec<String> = if self.filter.is_some() {
            let matches = self.matches();
            if matches.is_empty() {
                s.put_in(cols_area, 0, " no match", Style::DIM, width);
            }
            let nodes: Vec<TreeNode> = matches
                .iter()
                .map(|(idx, joined)| TreeNode {
                    label: joined.clone(),
                    detail: self
                        .node_at(idx)
                        .map(|n| n.detail.clone())
                        .unwrap_or_default(),
                    ..TreeNode::default()
                })
                .collect();
            self.draw_column(
                s,
                cols_area,
                &nodes,
                Some(self.filter_sel),
                true,
                true,
                width,
            );
            matches
                .get(self.filter_sel)
                .and_then(|(idx, _)| self.node_at(idx))
                .map(|n| n.info.clone())
                .unwrap_or_default()
        } else {
            let depth = self.path.len() - 1;
            let has_preview = self.current().is_some_and(|n| !n.children.is_empty());
            let levels = depth + 1 + usize::from(has_preview);
            let fit = (area.w / MIN_COLUMN).max(1) as usize;
            let n = fit.min(levels).min(3);
            // Always show the active column; the preview only if there is room for both.
            let end = if has_preview && n == 1 {
                levels - 1
            } else {
                levels
            };
            let first = end - n;
            let col_w = area.w / n as u16;
            for (k, level) in (first..end).enumerate() {
                let x = area.x + k as u16 * col_w;
                let w = if k + 1 == n {
                    area.w - k as u16 * col_w
                } else {
                    col_w - 1
                };
                let col = Rect::new(x, cols_y, w, cols_h);
                let selected = (level <= depth).then(|| self.path[level]);
                let details = n == 1 || w >= 40;
                self.draw_column(
                    s,
                    col,
                    self.level(level),
                    selected,
                    level == depth,
                    details,
                    width,
                );
                if k + 1 < n {
                    for y in 0..cols_h {
                        s.put_str(x + w, cols_y + y, "│", Style::DIM, width);
                    }
                }
            }
            self.current().map(|n| n.info.clone()).unwrap_or_default()
        };

        if detail_h > 0 {
            let rule_y = foot_y - detail_h - 1;
            s.put_in(
                Rect::new(area.x, rule_y, area.w, 1),
                0,
                &"─".repeat(area.w as usize),
                Style::DIM,
                width,
            );
            let rows: Vec<String> = info
                .iter()
                .flat_map(|l| wrap(l, area.w as usize, 2, width))
                .take(detail_h as usize)
                .collect();
            for (i, r) in rows.iter().enumerate() {
                let line = Rect::new(area.x, rule_y + 1 + i as u16, area.w, 1);
                s.put_in(line, 0, r, Style::default(), width);
            }
        }
        let foot = Rect::new(area.x, foot_y, area.w, 1);
        s.put_in(foot, 0, self.hint(), accent, width);
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        if self.filter.is_some() {
            return self.filter_key(key);
        }
        let depth = self.path.len() - 1;
        let last = self.level(depth).len().saturating_sub(1);
        let page = (self.rows.get() as usize).max(1);
        let sel = &mut self.path[depth];
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => *sel = (*sel + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => *sel = sel.saturating_sub(1),
            KeyCode::Char('d') if key.mods.ctrl => *sel = (*sel + page / 2).min(last),
            KeyCode::Char('u') if key.mods.ctrl => *sel = sel.saturating_sub(page / 2),
            KeyCode::Char('g') | KeyCode::Home => *sel = 0,
            KeyCode::Char('G') | KeyCode::End => *sel = last,
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                let Some(node) = self.current() else {
                    return Outcome::Handled;
                };
                if !node.children.is_empty() {
                    self.path.push(0);
                } else if key.code == KeyCode::Enter
                    && let Some(line) = node.run.clone()
                {
                    return self.run(&line);
                }
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                if depth > 0 {
                    self.path.pop();
                }
            }
            KeyCode::Char('?') => {
                if let Some(line) = self.current().and_then(|n| n.help.clone()) {
                    return self.run(&line);
                }
            }
            KeyCode::Char('/') => {
                self.filter = Some(String::new());
                self.filter_sel = 0;
            }
            KeyCode::Char('q') | KeyCode::Esc => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }

    fn cursor(&self, area: Rect) -> Option<(u16, u16)> {
        let q = self.filter.as_ref()?;
        let col = (q.chars().count() + 1) as u16;
        Some((area.x + col.min(area.w.saturating_sub(1)), area.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::parse_keys;

    fn leaf(label: &str) -> TreeNode {
        TreeNode {
            run: Some(format!("run {label}")),
            help: Some(format!("help {label}")),
            ..TreeNode::new(label)
        }
    }

    fn tree() -> TreeView {
        let plugin = TreeNode {
            children: vec![leaf("add"), leaf("remove")],
            ..TreeNode::new("plugin")
        };
        let plugins = TreeNode {
            children: vec![plugin, leaf("plugins")],
            ..TreeNode::new("Plugins")
        };
        let file = TreeNode {
            children: vec![leaf("write")],
            ..TreeNode::new("File")
        };
        TreeView::new("Commands", "Commands", vec![file, plugins])
    }

    fn keys(v: &mut TreeView, k: &str) -> Outcome {
        let mut last = Outcome::Handled;
        for key in parse_keys(k) {
            last = v.handle_key(key);
        }
        last
    }

    #[test]
    fn drill_down_and_back() {
        let mut v = tree();
        assert_eq!(v.breadcrumb(), "Commands › File");
        keys(&mut v, "jll");
        assert_eq!(v.breadcrumb(), "Commands › Plugins › plugin › add");
        assert_eq!(
            keys(&mut v, "j<CR>"),
            Outcome::RunAndClose("run remove".into())
        );
        assert_eq!(
            keys(&mut v, "?"),
            Outcome::RunAndClose("help remove".into())
        );
        keys(&mut v, "h");
        assert_eq!(v.breadcrumb(), "Commands › Plugins › plugin");
        keys(&mut v, "hhh");
        assert_eq!(v.selected_path(), ["Plugins"]);
        assert_eq!(keys(&mut v, "<Esc>"), Outcome::Close);
    }

    #[test]
    fn panel_keeps_view_open() {
        let mut v = tree().in_panel();
        assert_eq!(keys(&mut v, "l<CR>"), Outcome::Run("run write".into()));
    }

    #[test]
    fn filter_jumps_into_the_tree() {
        let mut v = tree();
        keys(&mut v, "/rem");
        let m = v.matches();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].1, "Plugins › plugin › remove");
        keys(&mut v, "<CR>");
        assert_eq!(v.breadcrumb(), "Commands › Plugins › plugin › remove");
        // Esc leaves the filter, not the view; Backspace on empty text too.
        keys(&mut v, "/x<Esc>");
        assert_eq!(v.breadcrumb(), "Commands › Plugins › plugin › remove");
        keys(&mut v, "/<BS>");
        assert_eq!(keys(&mut v, "<Esc>"), Outcome::Close);
    }
}
