//! Test plugin that breaks the rules on purpose. Used by the plugin host tests to check
//! that limits work and that the editor survives. Not an example to copy.

use vox_plugin_sdk::{Event, Plugin, editor, export_plugin};

struct Misbehave;

impl Plugin for Misbehave {
    fn init() -> Result<(), String> {
        for name in ["spin", "grow", "crash", "reenter", "fail", "ok"] {
            editor::register_command(name, "test command");
        }
        Ok(())
    }

    fn run_command(name: String, _arg: Option<String>, _force: bool) -> Result<(), String> {
        match name.as_str() {
            // Never returns: must hit the time limit.
            "spin" => loop {
                std::hint::black_box(());
            },
            // Allocates until the memory limit.
            "grow" => {
                let mut v: Vec<Vec<u8>> = Vec::new();
                loop {
                    v.push(vec![1; 1 << 20]);
                    std::hint::black_box(&v);
                }
            }
            "crash" => panic!("crash on purpose"),
            // Calls its own command again through the editor.
            "reenter" => {
                editor::run_command("reenter");
                editor::message("reenter finished");
                Ok(())
            }
            "fail" => Err("E900: failed on purpose".into()),
            _ => {
                editor::message("ok");
                Ok(())
            }
        }
    }

    fn on_event(_event: Event) {}
}

export_plugin!(Misbehave);
