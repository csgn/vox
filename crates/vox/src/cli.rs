//! Command-line arguments.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Which setup to start with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// `~/.config/vox` (default).
    Global,
    /// A project's `.vox/`; `None` = find the project from the current directory.
    Local(Option<PathBuf>),
    /// No config, no plugins.
    Vanilla,
}

/// Parsed arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    /// Setup to use.
    pub mode: Mode,
    /// File to open.
    pub file: Option<PathBuf>,
}

/// What to do instead of starting the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    /// Print help.
    Help,
    /// Print the version.
    Version,
    /// Bad arguments; the message says why.
    Error(String),
}

pub const HELP: &str = "\
vox — a Vim-like terminal editor

Usage: vox [OPTIONS] [FILE]

Options:
  --local-config [DIR]  Use the project's .vox/ config and plugins only (not the global ones).
                        DIR is the project folder; without it, the nearest folder upward
                        containing .vox/ is used, else the current folder.
  --vanilla             Start with no config and no plugins.
  -h, --help            Print this help.
  -V, --version         Print the version.

Without options, vox uses ~/.config/vox/config.toml and ~/.config/vox/plugins/.";

/// Parse arguments (without the program name). `is_dir` tells whether a path is a
/// folder, so `--local-config DIR FILE` and `--local-config FILE` both work.
pub fn parse(args: Vec<OsString>, is_dir: impl Fn(&Path) -> bool) -> Result<Cli, Exit> {
    let mut mode = Mode::Global;
    let mut file = None;
    let mut it = args.into_iter().peekable();
    let set_mode = |mode: &mut Mode, new: Mode| -> Result<(), Exit> {
        if *mode != Mode::Global {
            return Err(Exit::Error(
                "use only one of --local-config and --vanilla".into(),
            ));
        }
        *mode = new;
        Ok(())
    };
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Err(Exit::Help),
            Some("-V" | "--version") => return Err(Exit::Version),
            Some("--vanilla") => set_mode(&mut mode, Mode::Vanilla)?,
            Some("--local-config") => {
                let dir = match it.peek() {
                    Some(next) if is_dir(Path::new(next)) => it.next().map(PathBuf::from),
                    _ => None,
                };
                set_mode(&mut mode, Mode::Local(dir))?;
            }
            Some(s) if s.starts_with("--local-config=") => {
                let dir = PathBuf::from(&s["--local-config=".len()..]);
                set_mode(&mut mode, Mode::Local(Some(dir)))?;
            }
            Some(s) if s.starts_with('-') && s.len() > 1 => {
                return Err(Exit::Error(format!("unknown option {s} (see --help)")));
            }
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ => return Err(Exit::Error("only one FILE can be opened".into())),
        }
    }
    Ok(Cli { mode, file })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Cli, Exit> {
        parse(args.iter().map(OsString::from).collect(), |p| {
            p.to_str().is_some_and(|s| s.ends_with('/'))
        })
    }

    fn cli(mode: Mode, file: Option<&str>) -> Result<Cli, Exit> {
        Ok(Cli {
            mode,
            file: file.map(PathBuf::from),
        })
    }

    #[test]
    fn modes_and_file() {
        assert_eq!(p(&[]), cli(Mode::Global, None));
        assert_eq!(p(&["a.txt"]), cli(Mode::Global, Some("a.txt")));
        assert_eq!(
            p(&["--vanilla", "a.txt"]),
            cli(Mode::Vanilla, Some("a.txt"))
        );
        assert_eq!(p(&["--local-config"]), cli(Mode::Local(None), None));
        assert_eq!(
            p(&["--local-config", "a.txt"]),
            cli(Mode::Local(None), Some("a.txt")),
            "a file is not taken as DIR"
        );
        assert_eq!(
            p(&["--local-config", "proj/", "a.txt"]),
            cli(Mode::Local(Some("proj/".into())), Some("a.txt"))
        );
        assert_eq!(
            p(&["a.txt", "--local-config=proj"]),
            cli(Mode::Local(Some("proj".into())), Some("a.txt"))
        );
        assert_eq!(p(&["-"]), cli(Mode::Global, Some("-")));
    }

    #[test]
    fn errors_and_info() {
        assert_eq!(p(&["--help"]), Err(Exit::Help));
        assert_eq!(p(&["a", "-V"]), Err(Exit::Version));
        assert!(matches!(
            p(&["--vanilla", "--local-config"]),
            Err(Exit::Error(_))
        ));
        assert!(matches!(p(&["--nope"]), Err(Exit::Error(e)) if e.contains("--nope")));
        assert!(matches!(p(&["a", "b"]), Err(Exit::Error(_))));
    }
}
