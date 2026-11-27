//! AI backend for vox: implements [`vox_editor::assist::Assistant`] by running Claude Code
//! (`claude -p`) headless. It uses the login of the installed `claude` CLI (e.g. a Claude
//! Pro or Max plan), so no API key is needed.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use vox_editor::assist::{AssistReply, AssistRequest, Assistant};

/// Longest wait for one answer; the `claude` process is stopped after it.
const TIMEOUT: Duration = Duration::from_secs(600);

/// What the model is told about its job.
const SYSTEM: &str = "You are the editing backend of vox, a code editor. The user marks a \
range of lines in a file and says what to change. Reply with the complete new text for \
exactly that range: it replaces those lines. Keep the file's indentation and style. Do not \
repeat lines outside the range. If the instruction does not apply to the range, return the \
range unchanged. Put only the new text in `text`: no explanation, no Markdown fences.";

/// The Claude backend: runs the `claude` CLI.
pub struct Claude {
    /// Program to run, normally `claude` (found on `PATH`).
    program: String,
}

impl Default for Claude {
    /// Runs `claude` from `PATH`.
    fn default() -> Self {
        Self::new("claude")
    }
}

impl Claude {
    /// Run `program` as the Claude CLI.
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
        }
    }

    /// Run the CLI with `args`, `input` on stdin; returns exit success, stdout, stderr.
    fn run(&self, args: &[String], input: &str) -> Result<(bool, String, String), String> {
        let mut child = Command::new(&self.program)
            .args(args)
            // Outside any project, so no project CLAUDE.md or settings are loaded.
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => format!(
                    "`{}` not found: install Claude Code (claude.com/claude-code) and run `claude` once to log in",
                    self.program
                ),
                _ => format!("could not start `{}`: {e}", self.program),
            })?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let input = input.to_string();
        let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
        let read = |mut r: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut s = String::new();
                r.read_to_string(&mut s).map(|_| s)
            })
        };
        let out = read(Box::new(child.stdout.take().expect("piped stdout")));
        let err = read(Box::new(child.stderr.take().expect("piped stderr")));
        let start = Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
                break s;
            }
            if start.elapsed() > TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("no answer after {} s", TIMEOUT.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let _ = writer.join();
        let out = out.join().ok().and_then(|r| r.ok()).unwrap_or_default();
        let err = err.join().ok().and_then(|r| r.ok()).unwrap_or_default();
        Ok((status.success(), out, err))
    }
}

impl Assistant for Claude {
    /// Ask Claude for the new text of the request's range.
    fn ask(&self, req: &AssistRequest) -> Result<AssistReply, String> {
        let (ok, out, err) = self.run(&args(req), &prompt(req))?;
        parse_output(ok, &out, &err)
    }
}

/// Command-line arguments: headless, JSON result with a `{"text"}` schema, no tools, no
/// saved session, and the configured model and effort.
fn args(req: &AssistRequest) -> Vec<String> {
    let schema = json!({
        "type": "object",
        "properties": {"text": {"type": "string"}},
        "required": ["text"],
        "additionalProperties": false
    });
    let mut a: Vec<String> = [
        "-p",
        "--output-format",
        "json",
        "--json-schema",
        &schema.to_string(),
        "--system-prompt",
        SYSTEM,
        "--tools",
        "",
        "--no-session-persistence",
        "--strict-mcp-config",
        "--effort",
        &req.effort,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if !req.model.is_empty() {
        a.extend(["--model".to_string(), req.model.clone()]);
    }
    a
}

/// The prompt sent on stdin: the whole file, then the range and the instruction.
fn prompt(req: &AssistRequest) -> String {
    let path = req.path.as_deref().unwrap_or("(unnamed buffer)");
    format!(
        "File: {path}\n<file>\n{}\n</file>\n\nLines {}-{} ({}):\n<range>\n{}\n</range>\n\nInstruction: {}",
        req.text,
        req.start + 1,
        req.end + 1,
        req.kind.name(),
        req.target(),
        req.instruction
    )
}

/// The new text from the CLI's JSON result, or why there is none.
fn parse_output(ok: bool, out: &str, err: &str) -> Result<AssistReply, String> {
    let Ok(v) = serde_json::from_str::<Value>(out) else {
        let detail = err
            .lines()
            .chain(out.lines())
            .find(|l| !l.trim().is_empty());
        return Err(match detail {
            Some(d) => format!("claude failed: {}", d.trim()),
            None if ok => "claude gave no answer".into(),
            None => "claude failed".into(),
        });
    };
    if v["is_error"].as_bool().unwrap_or(!ok) {
        let msg = v["result"]
            .as_str()
            .filter(|m| !m.is_empty())
            .or(v["subtype"].as_str())
            .unwrap_or("unknown error");
        return Err(format!("claude: {msg}"));
    }
    if v["stop_reason"] == "refusal" {
        return Err("Claude declined this request".into());
    }
    let text = v["structured_output"]["text"].as_str().ok_or_else(|| {
        let r = v["result"].as_str().unwrap_or_default();
        format!(
            "unreadable answer from claude: {}",
            r.lines().next().unwrap_or("(empty)")
        )
    })?;
    Ok(AssistReply {
        text: text.strip_suffix('\n').unwrap_or(text).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_core::Position;
    use vox_editor::structure::UnitKind;

    fn request(model: &str) -> AssistRequest {
        AssistRequest {
            instruction: "rename x to y".into(),
            path: Some("src/a.rs".into()),
            text: "fn f() {\n    let x = 1;\n}\nfn g() {}".into(),
            start: 0,
            end: 2,
            kind: UnitKind::Function,
            cursor: Position::new(1, 4),
            model: model.into(),
            effort: "medium".into(),
        }
    }

    /// The CLI's JSON result with `structured_output.text = answer`.
    fn result(answer: &str) -> String {
        json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "stop_reason": "tool_use",
            "result": json!({"text": answer}).to_string(),
            "structured_output": {"text": answer}
        })
        .to_string()
    }

    #[test]
    fn args_are_headless_with_schema_and_no_tools() {
        let a = args(&request(""));
        let has = |a: &[String], flag: &str, value: &str| {
            a.windows(2).any(|w| w[0] == flag && w[1] == value)
        };
        assert_eq!(a[0], "-p");
        assert!(has(&a, "--output-format", "json"));
        assert!(has(&a, "--tools", ""));
        assert!(has(&a, "--effort", "medium"));
        assert!(a.contains(&"--no-session-persistence".to_string()));
        let i = a.iter().position(|x| x == "--json-schema").unwrap();
        let schema: Value = serde_json::from_str(&a[i + 1]).unwrap();
        assert_eq!(schema["required"][0], "text");
        assert!(
            !a.contains(&"--model".to_string()),
            "no model: the CLI's default"
        );
        assert!(has(&args(&request("sonnet")), "--model", "sonnet"));
    }

    #[test]
    fn prompt_has_file_range_and_instruction() {
        let p = prompt(&request(""));
        assert!(p.starts_with("File: src/a.rs\n<file>\nfn f() {"));
        assert!(
            p.contains("Lines 1-3 (function):\n<range>\nfn f() {\n    let x = 1;\n}\n</range>")
        );
        assert!(p.ends_with("Instruction: rename x to y"));
    }

    #[test]
    fn reply_is_the_structured_text() {
        let r = parse_output(true, &result("fn f() {\n    let y = 1;\n}\n"), "");
        assert_eq!(r.unwrap().text, "fn f() {\n    let y = 1;\n}");
    }

    #[test]
    fn errors() {
        let e = json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "result": "Not logged in · Please run /login"});
        assert_eq!(
            parse_output(false, &e.to_string(), "").unwrap_err(),
            "claude: Not logged in · Please run /login"
        );
        let e = json!({"is_error": true, "subtype": "error_max_turns"});
        assert_eq!(
            parse_output(false, &e.to_string(), "").unwrap_err(),
            "claude: error_max_turns"
        );
        let refusal = json!({"is_error": false, "stop_reason": "refusal", "result": ""});
        assert!(
            parse_output(true, &refusal.to_string(), "")
                .unwrap_err()
                .contains("declined")
        );
        let plain = json!({"is_error": false, "result": "sure, here you go"});
        assert_eq!(
            parse_output(true, &plain.to_string(), "").unwrap_err(),
            "unreadable answer from claude: sure, here you go"
        );
        assert_eq!(
            parse_output(false, "", "error: unknown option '--json-schema'\n").unwrap_err(),
            "claude failed: error: unknown option '--json-schema'"
        );
    }

    #[test]
    fn missing_program_says_how_to_install() {
        let c = Claude::new("vox-test-no-such-claude");
        let e = c.ask(&request("")).unwrap_err();
        assert!(e.contains("not found: install Claude Code"), "{e}");
    }

    /// A fake `claude`: saves its arguments and stdin next to itself, prints `stdout`.
    #[cfg(unix)]
    fn fake_cli(name: &str, stdout: &str, code: i32) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vox-assistant-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("out.json"), stdout).unwrap();
        let script = dir.join("claude");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args'\ncat > '{d}/stdin'\ncat '{d}/out.json'\nexit {code}\n",
                d = dir.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        (script, dir)
    }

    #[cfg(unix)]
    #[test]
    fn runs_the_cli_with_the_prompt_on_stdin() {
        let (script, dir) = fake_cli("ok", &result("fn f() {}"), 0);
        let c = Claude::new(script.to_str().unwrap());
        assert_eq!(c.ask(&request("opus")).unwrap().text, "fn f() {}");
        let args = std::fs::read_to_string(dir.join("args")).unwrap();
        assert!(args.starts_with("-p\n--output-format\njson\n"), "{args}");
        assert!(args.ends_with("--model\nopus\n"), "{args}");
        let stdin = std::fs::read_to_string(dir.join("stdin")).unwrap();
        assert!(stdin.ends_with("Instruction: rename x to y"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cli_errors_are_reported() {
        let out = json!({"is_error": true, "subtype": "error_during_execution", "result": "Usage limit reached"});
        let (script, dir) = fake_cli("err", &out.to_string(), 1);
        let c = Claude::new(script.to_str().unwrap());
        assert_eq!(
            c.ask(&request("")).unwrap_err(),
            "claude: Usage limit reached"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Asks the real `claude` CLI (your login; uses plan usage). Run with
    /// `cargo test -p vox-assistant -- --ignored`.
    #[test]
    #[ignore]
    fn live_cli() {
        let mut req = request("haiku");
        req.effort = "low".into();
        let reply = Claude::default().ask(&req).unwrap();
        assert!(reply.text.contains("let y = 1;"), "{}", reply.text);
        assert!(
            !reply.text.contains("fn g()"),
            "only the range: {}",
            reply.text
        );
    }
}
