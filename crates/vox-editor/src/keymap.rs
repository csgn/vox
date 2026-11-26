//! User key mappings: a key sequence in a mode maps to other keys or to a command.
//! Mappings are checked before the built-in Vim grammar. Mapped keys are not mapped again
//! (like Vim's `noremap`).

use crate::keys::{Key, parse_keys};

/// Mode a mapping applies in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapMode {
    /// Normal mode (including operator-pending, e.g. after `d`).
    Normal,
    /// Insert mode.
    Insert,
    /// Visual mode (charwise and linewise).
    Visual,
}

/// What a mapping does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// Feed these keys instead (empty = disable the key).
    Keys(Vec<Key>),
    /// Run this command line, e.g. `"w"` for `:w`.
    Command(String),
}

/// Result of matching typed keys against the mappings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lookup<'a> {
    /// No mapping starts with these keys.
    None,
    /// Some mapping starts with these keys; wait for more.
    Prefix,
    /// A mapping matches exactly and no longer one starts with these keys.
    Exact(&'a Binding),
    /// A mapping matches exactly, and longer ones start with these keys. Wait for more
    /// keys; on timeout use this one.
    ExactAndPrefix(&'a Binding),
}

/// All user key mappings.
#[derive(Debug, Clone)]
pub struct Keymap {
    /// (mode, keys, binding, owner), oldest first. For the same keys the newest wins.
    maps: Vec<(MapMode, Vec<Key>, Binding, String)>,
    /// Owner given to new mappings.
    pub(crate) owner: String,
}

impl Default for Keymap {
    fn default() -> Self {
        Self {
            maps: Vec::new(),
            owner: crate::owner::USER.into(),
        }
    }
}

impl Keymap {
    /// Map `lhs` to `binding` in `mode`. Replaces this owner's mapping of `lhs`; hides
    /// other owners' mappings of `lhs` until this one is removed.
    pub fn set(&mut self, mode: MapMode, lhs: Vec<Key>, binding: Binding) {
        let owner = self.owner.clone();
        self.maps
            .retain(|(m, l, _, o)| !(*m == mode && *l == lhs && *o == owner));
        self.maps.push((mode, lhs, binding, owner));
    }

    /// Remove all mappings of `lhs` in `mode`.
    pub fn remove(&mut self, mode: MapMode, lhs: &[Key]) {
        self.maps.retain(|(m, l, _, _)| !(*m == mode && l == lhs));
    }

    /// Remove every mapping added by `owner`.
    pub fn remove_owned(&mut self, owner: &str) {
        self.maps.retain(|(_, _, _, o)| o != owner);
    }

    /// All mappings with their owner, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = (MapMode, &[Key], &Binding, &str)> {
        self.maps
            .iter()
            .map(|(m, l, b, o)| (*m, l.as_slice(), b, o.as_str()))
    }

    /// Number of mappings.
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Whether there are no mappings.
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }

    /// Match typed `keys` against the mappings of `mode`.
    pub(crate) fn lookup(&self, mode: MapMode, keys: &[Key]) -> Lookup<'_> {
        let mut exact = None;
        let mut longer = false;
        for (m, lhs, b, _) in &self.maps {
            if *m != mode || !lhs.starts_with(keys) {
                continue;
            }
            if lhs.len() == keys.len() {
                exact = Some(b);
            } else {
                longer = true;
            }
        }
        match (exact, longer) {
            (None, false) => Lookup::None,
            (None, true) => Lookup::Prefix,
            (Some(b), false) => Lookup::Exact(b),
            (Some(b), true) => Lookup::ExactAndPrefix(b),
        }
    }
}

/// Replace `<leader>` (any case) in `s` with `leader`.
fn expand_leader(s: &str, leader: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.to_ascii_lowercase().find("<leader>") {
        out.push_str(&rest[..i]);
        out.push_str(leader);
        rest = &rest[i + "<leader>".len()..];
    }
    out.push_str(rest);
    out
}

/// Parse the left side of a mapping (Vim notation, `<leader>` allowed).
pub fn parse_lhs(s: &str, leader: &str) -> Vec<Key> {
    parse_keys(&expand_leader(s, leader))
}

/// Parse the right side of a mapping: `":cmd args"` runs a command, anything else is keys
/// in Vim notation (`<leader>` allowed). An empty string disables the key.
pub fn parse_binding(s: &str, leader: &str) -> Binding {
    match s.strip_prefix(':') {
        Some(cmd) => Binding::Command(cmd.trim().to_string()),
        None => Binding::Keys(parse_lhs(s, leader)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::KeyCode;

    fn k(s: &str) -> Vec<Key> {
        parse_keys(s)
    }

    #[test]
    fn lookup_prefix_and_exact() {
        let mut m = Keymap::default();
        m.set(MapMode::Insert, k("jk"), Binding::Keys(k("<Esc>")));
        m.set(MapMode::Normal, k(" w"), Binding::Command("w".into()));
        m.set(MapMode::Normal, k(" "), Binding::Keys(k("l")));
        assert_eq!(m.lookup(MapMode::Insert, &k("j")), Lookup::Prefix);
        assert_eq!(
            m.lookup(MapMode::Insert, &k("jk")),
            Lookup::Exact(&Binding::Keys(vec![Key::new(KeyCode::Esc)]))
        );
        assert_eq!(m.lookup(MapMode::Insert, &k("jj")), Lookup::None);
        assert_eq!(m.lookup(MapMode::Normal, &k("j")), Lookup::None);
        assert_eq!(
            m.lookup(MapMode::Normal, &k(" ")),
            Lookup::ExactAndPrefix(&Binding::Keys(k("l")))
        );
        assert_eq!(
            m.lookup(MapMode::Normal, &k(" w")),
            Lookup::Exact(&Binding::Command("w".into()))
        );
    }

    #[test]
    fn set_replaces_and_remove() {
        let mut m = Keymap::default();
        m.set(MapMode::Normal, k("H"), Binding::Keys(k("0")));
        m.set(MapMode::Normal, k("H"), Binding::Keys(k("^")));
        assert_eq!(m.len(), 1);
        assert_eq!(
            m.lookup(MapMode::Normal, &k("H")),
            Lookup::Exact(&Binding::Keys(k("^")))
        );
        m.remove(MapMode::Normal, &k("H"));
        assert!(m.is_empty());
    }

    #[test]
    fn owners_hide_and_restore() {
        let mut m = Keymap {
            owner: "config".into(),
            ..Keymap::default()
        };
        m.set(MapMode::Normal, k("H"), Binding::Keys(k("0")));
        m.owner = "plugin:x".into();
        m.set(MapMode::Normal, k("H"), Binding::Keys(k("^")));
        assert_eq!(
            m.lookup(MapMode::Normal, &k("H")),
            Lookup::Exact(&Binding::Keys(k("^")))
        );
        m.remove_owned("plugin:x");
        assert_eq!(
            m.lookup(MapMode::Normal, &k("H")),
            Lookup::Exact(&Binding::Keys(k("0")))
        );
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn leader_and_bindings() {
        assert_eq!(parse_lhs("<leader>w", " "), k(" w"));
        assert_eq!(parse_lhs("<Leader>a<LEADER>", ","), k(",a,"));
        assert_eq!(parse_binding(":w", "\\"), Binding::Command("w".into()));
        assert_eq!(
            parse_binding(": e! x ", "\\"),
            Binding::Command("e! x".into())
        );
        assert_eq!(parse_binding("<Esc>", "\\"), Binding::Keys(k("<Esc>")));
        assert_eq!(parse_binding("", "\\"), Binding::Keys(vec![]));
    }
}
