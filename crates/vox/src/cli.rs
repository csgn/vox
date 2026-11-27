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
    /// File to open (shown first).
    pub file: Option<PathBuf>,
    /// More files, opened as buffers behind the first.
    pub more: Vec<PathBuf>,
    /// `vox run`: run commands without a terminal instead of starting the editor.
    pub run: Option<Run>,
}

/// What `vox run` should do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Run {
    /// Command lines, in order.
    pub commands: Vec<String>,
    /// Also read command lines from stdin (`-`), after `commands`.
    pub stdin: bool,
    /// Print replies as JSON instead of text.
    pub json: bool,
}

/// What to do instead of starting the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    /// Print help.
    Help,
    /// Print `vox run` help.
    RunHelp,
    /// Print the version.
    Version,
    /// Bad arguments; the message says why.
    Error(String),
}

pub const HELP: &str = "\
vox — a Vim-like terminal editor

Usage: vox [OPTIONS] [FILE]...

Options:
  --local-config [DIR]  Use the project's .vox/ config and plugins only (not the global ones).
                        DIR is the project folder; without it, the nearest folder upward
                        containing .vox/ is used, else the current folder.
  --vanilla             Start with no config and no plugins.
  -h, --help            Print this help.
  -V, --version         Print the version.

More than one FILE opens each in a buffer (:ls, :bn); the first is shown.
Without options, vox uses ~/.config/vox/config.toml and ~/.config/vox/plugins/.
Run commands without the editor: vox run --help";

pub const RUN_HELP: &str = "\
vox run — run editor commands without a terminal

Usage: vox run [OPTIONS] COMMAND... | -

Each COMMAND is a command line as typed after `:`, e.g. \"plugin list\" or
\"settings set ui.side_width 60\". `-` reads more command lines from stdin (blank lines
are skipped). Commands run in order; vox waits for background work (installs) to finish.
The first failing command stops the run.

Options:
  --local-config [DIR]  Use the project's .vox/ config and plugins (as for the editor).
  --vanilla             No config, no plugins.
  --json                Print replies as JSON, one line per command that returns data.
  --file FILE           Open FILE first (for commands like `write`).
  -h, --help            Print this help.

Replies go to stdout; messages and notifications go to stderr.
Exit status: 0 success, 1 a command failed, 2 bad arguments.";

/// Parse arguments (without the program name). `is_dir` tells whether a path is a
/// folder, so `--local-config DIR FILE` and `--local-config FILE` both work.
pub fn parse(args: Vec<OsString>, is_dir: impl Fn(&Path) -> bool) -> Result<Cli, Exit> {
    let mut mode = Mode::Global;
    let mut file = None;
    let mut more = Vec::new();
    let mut it = args.into_iter().peekable();
    let mut run = None;
    if it.peek().and_then(|a| a.to_str()) == Some("run") {
        it.next();
        run = Some(Run::default());
    }
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
            Some("-h" | "--help") if run.is_some() => return Err(Exit::RunHelp),
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
            _ if run.is_some() => {
                let r = run.as_mut().expect("checked above");
                match arg.to_str() {
                    Some("--json") => r.json = true,
                    Some("--file") => {
                        let f = it.next().ok_or(Exit::Error("--file needs a FILE".into()))?;
                        file = Some(PathBuf::from(f));
                    }
                    Some("-") => r.stdin = true,
                    Some(s) if s.starts_with('-') => {
                        return Err(Exit::Error(format!(
                            "unknown option {s} (see vox run --help)"
                        )));
                    }
                    Some(s) => r.commands.push(s.to_string()),
                    None => return Err(Exit::Error("commands must be UTF-8".into())),
                }
            }
            Some(s) if s.starts_with('-') && s.len() > 1 => {
                return Err(Exit::Error(format!("unknown option {s} (see --help)")));
            }
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ => more.push(PathBuf::from(arg)),
        }
    }
    if run
        .as_ref()
        .is_some_and(|r| r.commands.is_empty() && !r.stdin)
    {
        return Err(Exit::Error(
            "vox run needs a COMMAND or - (see vox run --help)".into(),
        ));
    }
    Ok(Cli {
        mode,
        file,
        more,
        run,
    })
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
            more: Vec::new(),
            run: None,
        })
    }

    fn run(
        mode: Mode,
        file: Option<&str>,
        commands: &[&str],
        stdin: bool,
        json: bool,
    ) -> Result<Cli, Exit> {
        Ok(Cli {
            mode,
            file: file.map(PathBuf::from),
            more: Vec::new(),
            run: Some(Run {
                commands: commands.iter().map(|c| c.to_string()).collect(),
                stdin,
                json,
            }),
        })
    }

    #[test]
    fn run_commands() {
        assert_eq!(
            p(&["run", "plugin list"]),
            run(Mode::Global, None, &["plugin list"], false, false)
        );
        assert_eq!(
            p(&["run", "--vanilla", "--json", "a", "-", "b c"]),
            run(Mode::Vanilla, None, &["a", "b c"], true, true)
        );
        assert_eq!(
            p(&["run", "--local-config", "proj/", "--file", "f.txt", "w"]),
            run(
                Mode::Local(Some("proj/".into())),
                Some("f.txt"),
                &["w"],
                false,
                false
            )
        );
        assert_eq!(
            p(&["run", "--local-config", "scope show"]),
            run(Mode::Local(None), None, &["scope show"], false, false),
            "a command is not taken as DIR"
        );
        assert_eq!(p(&["run", "--help"]), Err(Exit::RunHelp));
        assert!(matches!(p(&["run"]), Err(Exit::Error(e)) if e.contains("needs a COMMAND")));
        assert!(matches!(p(&["run", "--json"]), Err(Exit::Error(_))));
        assert!(matches!(p(&["run", "--file"]), Err(Exit::Error(e)) if e.contains("FILE")));
        assert!(
            matches!(p(&["run", "--nope", "x"]), Err(Exit::Error(e)) if e.contains("vox run --help"))
        );
        assert!(
            p(&["a.txt", "run"]).is_ok_and(|c| c.run.is_none() && c.more == [PathBuf::from("run")]),
            "run only counts as the first argument"
        );
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
        let several = p(&["a.txt", "--vanilla", "b.txt", "c.txt"]).unwrap();
        assert_eq!(several.file, Some("a.txt".into()));
        assert_eq!(
            several.more,
            [PathBuf::from("b.txt"), PathBuf::from("c.txt")]
        );
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
    }
}
