//! vox: a Vim-like terminal editor.

use anyhow::{Context, Result, bail};
use vox_core::Document;
use vox_tui::App;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let doc = match (args.next(), args.next()) {
        (None, _) => Document::default(),
        (Some(path), None) => {
            Document::open(&path).with_context(|| format!("opening {}", path.display()))?
        }
        _ => bail!("usage: vox [FILE]"),
    };
    let mut app = App::new(doc);
    let mut terminal = ratatui::init();
    let result = vox_tui::run(&mut app, &mut terminal);
    ratatui::restore();
    Ok(result?)
}
