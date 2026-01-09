use anyhow::{Context as _, anyhow};
use luban_domain::CodexThreadEvent;
use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub(super) struct CodexTurnParams {
    pub(super) thread_id: Option<String>,
    pub(super) worktree_path: PathBuf,
    pub(super) prompt: String,
    pub(super) image_paths: Vec<PathBuf>,
    pub(super) model: Option<String>,
    pub(super) model_reasoning_effort: Option<String>,
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
    } = params;

    let mut command = Command::new(codex);
    command
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg("--ask-for-approval")
        .arg("never")
        .arg("--search")
        .arg("exec")
        .arg("--json")
        .arg("-C")
        .arg(&worktree_path);

    if !image_paths.is_empty() {
        command.arg("--image");
        for path in &image_paths {
            command.arg(path);
        }
    }

    if let Some(model) = model {
        command.arg("--model").arg(model);
    }
    if let Some(effort) = model_reasoning_effort {
        command
            .arg("-c")
            .arg(format!("model_reasoning_effort=\"{effort}\""));
    }

    if let Some(thread_id) = thread_id {
        command.arg("resume").arg(thread_id).arg("-");
    } else {
        command.arg("-");
    }

    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "missing codex executable ({}): install Codex CLI and ensure it is available on PATH",
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
    let killer = {
        let child = child.clone();
        let cancel = cancel.clone();
        let finished = finished.clone();
        std::thread::spawn(move || {
            while !finished.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(25));
            }
            if cancel.load(Ordering::SeqCst)
                && let Ok(mut child) = child.lock()
            {
                let _ = child.kill();
            }
        })
    };

    let stderr_handle = std::thread::spawn(move || -> String {
        let mut buf = Vec::new();
        let mut reader = BufReader::new(stderr);
        let _ = reader.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).to_string()
    });

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
