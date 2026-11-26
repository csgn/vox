//! Log: what vox and plugins did, for developers. Kept in memory (newest last), shown by
//! `:log` in a live view.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use vox_view::CharWidth;

use crate::keys::{Key, KeyCode};
use crate::surface::{Rect, Style, Surface};
use crate::ui::{Level, Outcome, View};

/// Most entries kept; older ones are dropped.
pub const LOG_CAPACITY: usize = 2000;

/// One log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// Time since the editor started.
    pub time: Duration,
    /// Importance.
    pub level: Level,
    /// Where it came from: `editor`, `config`, `plugin:NAME`, ...
    pub source: String,
    /// What happened.
    pub message: String,
}

/// The log buffer.
#[derive(Debug, Clone)]
pub struct Log {
    entries: VecDeque<LogEntry>,
    started: Instant,
}

impl Log {
    /// An empty log; times are measured from `started`.
    pub fn new(started: Instant) -> Self {
        Self {
            entries: VecDeque::new(),
            started,
        }
    }

    /// Add an entry at time `now`.
    pub fn push(&mut self, now: Instant, level: Level, source: &str, message: &str) {
        if self.entries.len() == LOG_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry {
            time: now.saturating_duration_since(self.started),
            level,
            source: source.to_string(),
            message: message.to_string(),
        });
    }

    /// All entries, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Live log view. Keys: `j`/`k` scroll, `g`/`G` top/bottom (bottom follows new lines),
/// `f` cycles the minimum level, `s` cycles the source filter, `c` clears, `q` closes.
#[derive(Debug, Clone)]
pub struct LogView {
    lines: Vec<LogEntry>,
    /// Lowest level shown.
    pub min_level: Level,
    /// Only this source, if set.
    pub source: Option<String>,
    /// All sources seen, for cycling the source filter.
    sources: Vec<String>,
    /// First visible line; `usize::MAX` follows the end.
    top: usize,
}

impl LogView {
    /// A view showing entries at `min_level` or above, optionally from one source.
    pub fn new(min_level: Level, source: Option<String>) -> Self {
        Self {
            lines: Vec::new(),
            min_level,
            source,
            sources: Vec::new(),
            top: usize::MAX,
        }
    }

    /// Reload the shown lines from `log` (called by the editor when the log changes).
    pub fn refresh(&mut self, log: &Log) {
        self.sources = log.entries().map(|e| e.source.clone()).collect();
        self.sources.sort();
        self.sources.dedup();
        self.lines = log
            .entries()
            .filter(|e| e.level >= self.min_level)
            .filter(|e| self.source.as_ref().is_none_or(|s| *s == e.source))
            .cloned()
            .collect();
    }

    /// Lines currently shown.
    pub fn lines(&self) -> &[LogEntry] {
        &self.lines
    }
}

impl View for LogView {
    fn title(&self) -> String {
        let src = self.source.as_deref().unwrap_or("all sources");
        format!(
            "Log — {}+ — {src} — f level, s source, c clear",
            self.min_level.name()
        )
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        let max_top = self.lines.len().saturating_sub(area.h as usize);
        let top = self.top.min(max_top);
        for (i, e) in self
            .lines
            .iter()
            .skip(top)
            .take(area.h as usize)
            .enumerate()
        {
            let t = e.time.as_millis();
            let text = format!(
                "{:>4}.{:03} {:<7} {:<16} {}",
                t / 1000,
                t % 1000,
                e.level.name(),
                e.source,
                e.message
            );
            let style = match e.level {
                Level::Info => Style::default(),
                l => Style::default().fg(l.color()),
            };
            s.put_in(area, i as u16, &text, style, width);
        }
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        let max_top = self.lines.len().saturating_sub(1);
        let top = self.top.min(max_top);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.top = (top + 1).min(max_top),
            KeyCode::Char('k') | KeyCode::Up => self.top = top.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => self.top = 0,
            KeyCode::Char('G') | KeyCode::End => self.top = usize::MAX,
            KeyCode::Char('f') => {
                self.min_level = match self.min_level {
                    Level::Debug => Level::Info,
                    Level::Info => Level::Warn,
                    Level::Warn => Level::Error,
                    Level::Error => Level::Debug,
                };
                return Outcome::Run("log refresh".into());
            }
            KeyCode::Char('s') => {
                let next = match &self.source {
                    None => self.sources.first().cloned(),
                    Some(cur) => {
                        let i = self.sources.iter().position(|s| s == cur);
                        i.and_then(|i| self.sources.get(i + 1)).cloned()
                    }
                };
                self.source = next;
                return Outcome::Run("log refresh".into());
            }
            KeyCode::Char('c') => return Outcome::Run("log clear".into()),
            KeyCode::Char('q') | KeyCode::Esc => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }

    /// This is the log view.
    fn as_log_view(&mut self) -> Option<&mut LogView> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::parse_keys;

    fn log() -> Log {
        let t0 = Instant::now();
        let mut log = Log::new(t0);
        log.push(
            t0 + Duration::from_millis(1500),
            Level::Debug,
            "editor",
            "a",
        );
        log.push(t0, Level::Info, "config", "b");
        log.push(t0, Level::Error, "plugin:x", "c");
        log
    }

    #[test]
    fn ring_buffer_drops_oldest() {
        let t0 = Instant::now();
        let mut log = Log::new(t0);
        for i in 0..LOG_CAPACITY + 5 {
            log.push(t0, Level::Info, "editor", &i.to_string());
        }
        assert_eq!(log.len(), LOG_CAPACITY);
        assert_eq!(log.entries().next().unwrap().message, "5");
    }

    #[test]
    fn filters_by_level_and_source() {
        let log = log();
        let mut v = LogView::new(Level::Debug, None);
        v.refresh(&log);
        assert_eq!(v.lines().len(), 3);
        assert_eq!(v.lines()[0].time, Duration::from_millis(1500));
        for k in parse_keys("f") {
            assert_eq!(v.handle_key(k), Outcome::Run("log refresh".into()));
        }
        v.refresh(&log);
        assert_eq!(
            v.lines()
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            ["b", "c"]
        );
        for k in parse_keys("s") {
            v.handle_key(k);
        }
        assert_eq!(v.source.as_deref(), Some("config"));
        v.refresh(&log);
        assert_eq!(v.lines().len(), 1);
        // Sources are config, editor, plugin:x: three more presses cycle back to all.
        for k in parse_keys("sss") {
            v.handle_key(k);
        }
        assert_eq!(v.source, None, "cycles back to all sources");
    }
}
