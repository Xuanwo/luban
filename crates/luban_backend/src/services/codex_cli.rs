use super::thread_io::spawn_read_to_string;
use anyhow::{Context as _, anyhow};
use luban_domain::CodexThreadEvent;
use std::{
    ffi::OsString,
    io::{BufRead as _, BufReader, Write as _},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

use super::cancel_killer::spawn_cancel_killer;

fn should_skip_git_repo_check(worktree_path: &Path) -> bool {
    !worktree_path.join(".git").exists()
}

const CODEX_APPROVAL_POLICY_NEVER: &str = "never";
const CODEX_SANDBOX_MODE_DANGER_FULL_ACCESS: &str = "danger-full-access";

fn build_codex_exec_args(
    _sandbox_mode: &str,
    worktree_path: &Path,
    thread_id: Option<&str>,
    image_paths: &[PathBuf],
    model: Option<&str>,
    model_reasoning_effort: Option<&str>,
    skip_git_repo_check: bool,
) -> Vec<OsString> {
    let sandbox_mode = CODEX_SANDBOX_MODE_DANGER_FULL_ACCESS;
    // Important: `--ask-for-approval` and `--search` are interactive-mode flags and are NOT
    // accepted by `codex exec`. If we pass them before `exec`, Codex will silently fall back to
    // user config defaults, which makes Luban appear "stuck" in read-only mode.
    //
    // To ensure deterministic behavior regardless of `~/.codex/config.toml`, explicitly override
    // the relevant config values on `codex exec`.
    let mut args: Vec<OsString> = vec![
        "exec".into(),
        "--dangerously-bypass-approvals-and-sandbox".into(),
        "--sandbox".into(),
        sandbox_mode.into(),
        "-c".into(),
        format!("approval_policy=\"{CODEX_APPROVAL_POLICY_NEVER}\"").into(),
        "-c".into(),
        format!("sandbox_mode=\"{sandbox_mode}\"").into(),
    ];

    if skip_git_repo_check {
        args.push("--skip-git-repo-check".into());
    }

    args.push("--json".into());
    args.push("-C".into());
    args.push(worktree_path.as_os_str().to_owned());

    if !image_paths.is_empty() {
        args.push("--image".into());
        for path in image_paths {
            args.push(path.as_os_str().to_owned());
        }
    }

    if let Some(model) = model {
        args.push("--model".into());
        args.push(model.into());
    }
    if let Some(effort) = model_reasoning_effort {
        args.push("-c".into());
        args.push(format!("model_reasoning_effort=\"{effort}\"").into());
    }

    if let Some(thread_id) = thread_id {
        args.push("resume".into());
        args.push(thread_id.into());
        args.push("-".into());
    } else {
        args.push("-".into());
    }

    args
}

pub(super) struct CodexTurnParams {
    pub(super) thread_id: Option<String>,
    pub(super) worktree_path: PathBuf,
    pub(super) prompt: String,
    pub(super) image_paths: Vec<PathBuf>,
    pub(super) model: Option<String>,
    pub(super) model_reasoning_effort: Option<String>,
    pub(super) sandbox_mode: Option<String>,
}

enum CodexStdoutLine {
    Event(Box<CodexThreadEvent>),
    Ignored { message: String },
    Noise { message: String },
}

fn parse_codex_stdout_line(line: &str) -> anyhow::Result<CodexStdoutLine> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(CodexStdoutLine::Noise {
            message: String::new(),
        });
    }

    let payload = trimmed;
    let looks_like_json = payload.starts_with('{') || payload.starts_with('[');
    if !looks_like_json {
        return Ok(CodexStdoutLine::Noise {
            message: payload.to_owned(),
        });
    }

    match serde_json::from_str::<CodexThreadEvent>(payload) {
        Ok(event) => Ok(CodexStdoutLine::Event(Box::new(event))),
        Err(_err) => {
            let value = match serde_json::from_str::<serde_json::Value>(payload) {
                Ok(value) => value,
                Err(_) => {
                    return Ok(CodexStdoutLine::Noise {
                        message: payload.to_owned(),
                    });
                }
            };

            let type_name = value
                .as_object()
                .and_then(|obj| obj.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("<missing type>");

            Ok(CodexStdoutLine::Ignored {
                message: format!("ignored codex event: {type_name}"),
            })
        }
    }
}

pub(super) fn run_codex_turn_streamed_via_cli(
    codex: &Path,
    params: CodexTurnParams,
    cancel: Arc<AtomicBool>,
    mut on_event: impl FnMut(CodexThreadEvent) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let CodexTurnParams {
        thread_id,
        worktree_path,
        prompt,
        image_paths,
        model,
        model_reasoning_effort,
        sandbox_mode,
    } = params;

    let _ = sandbox_mode.as_deref();
    let sandbox_mode = CODEX_SANDBOX_MODE_DANGER_FULL_ACCESS;

    let mut command = Command::new(codex);
    command.args(build_codex_exec_args(
        sandbox_mode,
        &worktree_path,
        thread_id.as_deref(),
        &image_paths,
        model.as_deref(),
        model_reasoning_effort.as_deref(),
        should_skip_git_repo_check(&worktree_path),
    ));

    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "missing codex executable ({}): install Codex CLI and ensure it is available on PATH (note: macOS apps launched from Finder/Dock may not inherit your shell PATH; set LUBAN_CODEX_BIN to an absolute path if needed)",
                    codex.display()
                )
            } else {
                anyhow!(err).context("failed to spawn codex")
            }
        })?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("missing stdin"))?
        .write_all(prompt.as_bytes())
        .context("failed to write stdin")?;
    drop(child.stdin.take());

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("missing stderr"))?;

    let finished = Arc::new(AtomicBool::new(false));
    let child = Arc::new(std::sync::Mutex::new(child));
    let killer = spawn_cancel_killer(child.clone(), cancel.clone(), finished.clone());

    let stderr_handle = spawn_read_to_string(stderr);

    let stdout_reader = BufReader::new(stdout);
    let mut stdout_noise: Vec<String> = Vec::new();
    for line in stdout_reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                return Err(err).context("failed to read stdout line");
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if cancel.load(Ordering::SeqCst) {
            break;
        }

        match parse_codex_stdout_line(trimmed) {
            Ok(CodexStdoutLine::Event(event)) => on_event(*event)?,
            Ok(CodexStdoutLine::Ignored { message } | CodexStdoutLine::Noise { message }) => {
                if message.is_empty() {
                    continue;
                }
                if stdout_noise.len() < 64 {
                    stdout_noise.push(message);
                }
            }
            Err(err) => {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                return Err(err).context("failed to parse codex stdout");
            }
        }
    }

    let status = child
        .lock()
        .map_err(|_| anyhow!("failed to lock codex child"))?
        .wait()
        .context("failed to wait for codex")?;
    finished.store(true, Ordering::SeqCst);
    let _ = killer.join();
    let stderr_text = stderr_handle.join().unwrap_or_default();

    if cancel.load(Ordering::SeqCst) {
        return Ok(());
    }

    if !status.success() {
        let codex_noise = if stdout_noise.is_empty() {
            String::new()
        } else {
            format!("\nstdout (non-protocol):\n{}\n", stdout_noise.join("\n"))
        };
        return Err(anyhow!(
            "codex failed ({}):\nstderr:\n{}{}",
            status,
            stderr_text.trim(),
            codex_noise
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_git_repo_check_flag_is_after_exec() {
        let args = build_codex_exec_args(
            "danger-full-access",
            Path::new("/tmp/non-git"),
            None,
            &[],
            None,
            None,
            true,
        );
        let args = args
            .iter()
            .map(|v| v.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let exec_pos = args.iter().position(|v| v == "exec").expect("missing exec");
        let skip_pos = args
            .iter()
            .position(|v| v == "--skip-git-repo-check")
            .expect("missing skip flag");
        let json_pos = args
            .iter()
            .position(|v| v == "--json")
            .expect("missing --json");
        assert!(exec_pos < skip_pos);
        assert!(skip_pos < json_pos);
    }

    #[test]
    fn codex_exec_args_override_approval_policy_and_sandbox_mode() {
        let args = build_codex_exec_args(
            "read-only",
            Path::new("/tmp/non-git"),
            None,
            &[],
            None,
            None,
            true,
        );
        let args = args
            .iter()
            .map(|v| v.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args.iter().any(|v| v == "exec"));
        assert!(
            args.iter()
                .any(|v| v == "--dangerously-bypass-approvals-and-sandbox")
        );
        assert!(args.iter().any(|v| v == "approval_policy=\"never\""));
        assert!(
            args.iter()
                .any(|v| v == "sandbox_mode=\"danger-full-access\"")
        );
    }

    #[test]
    fn codex_exec_args_do_not_include_interactive_only_flags() {
        let args = build_codex_exec_args(
            "danger-full-access",
            Path::new("/tmp/non-git"),
            None,
            &[],
            None,
            None,
            true,
        );
        let args = args
            .iter()
            .map(|v| v.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!args.iter().any(|v| v == "--ask-for-approval"));
        assert!(!args.iter().any(|v| v == "--search"));
    }

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn skip_git_repo_check_is_true_for_non_git_dirs() {
        let root = temp_dir("luban-codex-nongit");
        assert!(should_skip_git_repo_check(&root));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skip_git_repo_check_is_false_when_dot_git_exists() {
        let root = temp_dir("luban-codex-gitfile");
        std::fs::write(root.join(".git"), "gitdir: /tmp/example\n").expect("write .git");
        assert!(!should_skip_git_repo_check(&root));
        let _ = std::fs::remove_dir_all(root);

        let root2 = temp_dir("luban-codex-gitdir");
        std::fs::create_dir(root2.join(".git")).expect("mkdir .git");
        assert!(!should_skip_git_repo_check(&root2));
        let _ = std::fs::remove_dir_all(root2);
    }

    #[test]
    fn codex_stdout_parsing_accepts_events() {
        let parsed =
            parse_codex_stdout_line("{\"type\":\"turn.started\"}").expect("parse should succeed");
        assert!(matches!(
            parsed,
            CodexStdoutLine::Event(event) if matches!(*event, CodexThreadEvent::TurnStarted)
        ));
    }

    #[test]
    fn codex_stdout_parsing_accepts_legacy_json_events() {
        let parsed =
            parse_codex_stdout_line("{\"type\":\"turn.started\"}").expect("parse should succeed");
        assert!(matches!(
            parsed,
            CodexStdoutLine::Event(event) if matches!(*event, CodexThreadEvent::TurnStarted)
        ));
    }

    #[test]
    fn codex_stdout_parsing_ignores_unknown_events() {
        let parsed = parse_codex_stdout_line("{\"type\":\"turn.reconnect\",\"detail\":\"x\"}")
            .expect("parse should succeed");
        assert!(matches!(parsed, CodexStdoutLine::Ignored { .. }));
    }

    #[test]
    fn codex_stdout_parsing_treats_plain_text_as_noise() {
        let parsed = parse_codex_stdout_line("retry/reconnect").expect("parse should succeed");
        assert!(matches!(parsed, CodexStdoutLine::Noise { .. }));
    }
}
