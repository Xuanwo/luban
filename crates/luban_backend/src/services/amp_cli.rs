use anyhow::{Context as _, anyhow};
use luban_domain::{
    AgentCommandExecutionStatus, AgentErrorMessage, AgentFileUpdateChange, AgentMcpToolCallStatus,
    AgentPatchApplyStatus, AgentPatchChangeKind, AgentThreadError, AgentThreadEvent,
    AgentThreadItem, AgentUsage,
};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::ansi::strip_ansi_control_sequences;
use super::stream_json::{
    extract_content_array, extract_string_field, parse_tool_result_content, tool_name_key,
    value_as_string,
};

pub(super) struct AmpTurnParams {
    pub(super) thread_id: Option<String>,
    pub(super) worktree_path: PathBuf,
    pub(super) prompt: String,
    pub(super) mode: Option<String>,
}

fn resolve_amp_exec() -> PathBuf {
    std::env::var_os("LUBAN_AMP_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("amp"))
}

fn run_amp_threads_new(
    amp: &Path,
    worktree_path: &Path,
    mode: Option<&str>,
) -> anyhow::Result<String> {
    let mut command = Command::new(amp);
    command.current_dir(worktree_path);
    command.args(["--no-notifications", "--no-ide", "--no-jetbrains"]);
    if let Some(mode) = mode {
        command.args(["--mode", mode]);
    }
    command.args(["threads", "new"]);

    let output = command
        .output()
        .context("failed to spawn amp threads new")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("amp threads new failed: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.lines().last().unwrap_or("").trim().to_owned();
    if id.is_empty() {
        return Err(anyhow!("amp threads new returned empty thread id"));
    }
    Ok(id)
}

#[derive(Default)]
struct AmpStreamState {
    agent_message_id: String,
    reasoning_id: String,
    agent_message: String,
    reasoning: String,
    tools: HashMap<String, AmpToolUse>,
}

impl AmpStreamState {
    fn new() -> Self {
        Self {
            agent_message_id: "agent_message".to_owned(),
            reasoning_id: "reasoning".to_owned(),
            agent_message: String::new(),
            reasoning: String::new(),
            tools: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct AmpToolUse {
    name: String,
    input: Value,
    kind: AmpToolKind,
    summary: AmpToolSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AmpToolKind {
    CommandExecution,
    FileChange,
    WebSearch,
    McpToolCall,
}

#[derive(Clone, Debug)]
enum AmpToolSummary {
    None,
    Command { command: String },
    FileChange { changes: Vec<(String, String)> },
    WebSearch { query: String },
}

fn summarize_tool_use(name: &str, input: &Value) -> (AmpToolKind, AmpToolSummary) {
    let key = tool_name_key(name);

    if key == "bash" {
        let command =
            extract_string_field(input, &["command", "cmd"]).unwrap_or_else(|| "bash".to_owned());
        return (
            AmpToolKind::CommandExecution,
            AmpToolSummary::Command { command },
        );
    }

    if key == "web_search" {
        let query =
            extract_string_field(input, &["query", "q"]).unwrap_or_else(|| "web_search".to_owned());
        return (AmpToolKind::WebSearch, AmpToolSummary::WebSearch { query });
    }

    if key == "edit_file" || key == "create_file" || key == "undo_edit" {
        let path =
            extract_string_field(input, &["path", "file_path", "filename"]).unwrap_or_default();
        let kind = if key == "create_file" {
            "add"
        } else {
            "update"
        };
        let changes = if path.is_empty() {
            Vec::new()
        } else {
            vec![(path, kind.to_owned())]
        };
        return (
            AmpToolKind::FileChange,
            AmpToolSummary::FileChange { changes },
        );
    }

    (AmpToolKind::McpToolCall, AmpToolSummary::None)
}

fn parse_amp_stream_json_line(
    state: &mut AmpStreamState,
    line: &str,
) -> anyhow::Result<Vec<AgentThreadEvent>> {
    let stripped = strip_ansi_control_sequences(line);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let payload: Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(_) => return Ok(Vec::new()),
    };

    let type_name = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let mut out = Vec::new();

    if type_name == "system" {
        let subtype = payload
            .get("subtype")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if subtype == "init" {
            let thread_id = payload
                .get("session_id")
                .and_then(|v| v.as_str())
                .or_else(|| payload.get("thread_id").and_then(|v| v.as_str()))
                .map(ToOwned::to_owned);
            if let Some(thread_id) = thread_id {
                out.push(AgentThreadEvent::ThreadStarted { thread_id });
            }
        }
        return Ok(out);
    }

    if type_name == "assistant" {
        if let Some(content) = extract_content_array(&payload) {
            for item in content {
                let item_type = item
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();

                match item_type.as_str() {
                    "text" => {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !state.agent_message.is_empty() {
                                state.agent_message.push('\n');
                            }
                            state.agent_message.push_str(text);
                            out.push(AgentThreadEvent::ItemUpdated {
                                item: AgentThreadItem::AgentMessage {
                                    id: state.agent_message_id.clone(),
                                    text: state.agent_message.clone(),
                                },
                            });
                        }
                    }
                    "thinking" => {
                        let text = item
                            .get("thinking")
                            .and_then(|v| v.as_str())
                            .or_else(|| item.get("text").and_then(|v| v.as_str()));
                        if let Some(text) = text {
                            if !state.reasoning.is_empty() {
                                state.reasoning.push('\n');
                            }
                            state.reasoning.push_str(text);
                            out.push(AgentThreadEvent::ItemUpdated {
                                item: AgentThreadItem::Reasoning {
                                    id: state.reasoning_id.clone(),
                                    text: state.reasoning.clone(),
                                },
                            });
                        }
                    }
                    "tool_use" => {
                        let id = item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool_use")
                            .to_owned();
                        let name = item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool")
                            .to_owned();
                        let input = item.get("input").cloned().unwrap_or(Value::Null);
                        let (kind, summary) = summarize_tool_use(&name, &input);
                        state.tools.insert(
                            id.clone(),
                            AmpToolUse {
                                name: name.clone(),
                                input: input.clone(),
                                kind,
                                summary,
                            },
                        );

                        match kind {
                            AmpToolKind::CommandExecution => {
                                let command = match state.tools.get(&id).map(|t| &t.summary) {
                                    Some(AmpToolSummary::Command { command }) => command.clone(),
                                    _ => "bash".to_owned(),
                                };
                                out.push(AgentThreadEvent::ItemStarted {
                                    item: AgentThreadItem::CommandExecution {
                                        id,
                                        command,
                                        aggregated_output: String::new(),
                                        exit_code: None,
                                        status: AgentCommandExecutionStatus::InProgress,
                                    },
                                });
                            }
                            AmpToolKind::WebSearch => {
                                let query = match state.tools.get(&id).map(|t| &t.summary) {
                                    Some(AmpToolSummary::WebSearch { query }) => query.clone(),
                                    _ => String::new(),
                                };
                                out.push(AgentThreadEvent::ItemStarted {
                                    item: AgentThreadItem::WebSearch { id, query },
                                });
                            }
                            AmpToolKind::FileChange => {
                                let changes = match state.tools.get(&id).map(|t| &t.summary) {
                                    Some(AmpToolSummary::FileChange { changes }) => changes
                                        .iter()
                                        .map(|(path, kind)| AgentFileUpdateChange {
                                            path: path.clone(),
                                            kind: match kind.as_str() {
                                                "add" => AgentPatchChangeKind::Add,
                                                "delete" => AgentPatchChangeKind::Delete,
                                                _ => AgentPatchChangeKind::Update,
                                            },
                                        })
                                        .collect(),
                                    _ => Vec::new(),
                                };
                                out.push(AgentThreadEvent::ItemStarted {
                                    item: AgentThreadItem::FileChange {
                                        id,
                                        changes,
                                        status: AgentPatchApplyStatus::InProgress,
                                    },
                                });
                            }
                            AmpToolKind::McpToolCall => {
                                out.push(AgentThreadEvent::ItemStarted {
                                    item: AgentThreadItem::McpToolCall {
                                        id,
                                        server: "amp".to_owned(),
                                        tool: name,
                                        arguments: input,
                                        result: None,
                                        error: None,
                                        status: AgentMcpToolCallStatus::InProgress,
                                    },
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        return Ok(out);
    }

    if type_name == "user" {
        if let Some(content) = extract_content_array(&payload) {
            for item in content {
                let item_type = item
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if item_type != "tool_result" {
                    continue;
                }

                let tool_use_id = item
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool_use")
                    .to_owned();
                let is_error = item
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let content_value = item.get("content").cloned().unwrap_or(Value::Null);

                let result = parse_tool_result_content(&content_value);

                let tool = state.tools.get(&tool_use_id).cloned().unwrap_or_else(|| {
                    let (kind, summary) = summarize_tool_use("tool", &Value::Null);
                    AmpToolUse {
                        name: "tool".to_owned(),
                        input: Value::Null,
                        kind,
                        summary,
                    }
                });

                match tool.kind {
                    AmpToolKind::CommandExecution => {
                        let (aggregated_output, exit_code) = match result.as_object() {
                            Some(obj) => {
                                let stdout =
                                    obj.get("stdout").and_then(Value::as_str).unwrap_or("");
                                let stderr =
                                    obj.get("stderr").and_then(Value::as_str).unwrap_or("");
                                let output = if stdout.is_empty() && stderr.is_empty() {
                                    value_as_string(&result).unwrap_or_default()
                                } else if stderr.is_empty() {
                                    stdout.to_owned()
                                } else if stdout.is_empty() {
                                    stderr.to_owned()
                                } else {
                                    format!("{stdout}\n{stderr}")
                                };

                                let exit_code = obj
                                    .get("exitCode")
                                    .or_else(|| obj.get("exit_code"))
                                    .and_then(Value::as_i64)
                                    .and_then(|v| i32::try_from(v).ok());

                                (output, exit_code)
                            }
                            None => (
                                value_as_string(&result).unwrap_or_else(|| result.to_string()),
                                None,
                            ),
                        };

                        let command = match tool.summary {
                            AmpToolSummary::Command { command } => command,
                            _ => "bash".to_owned(),
                        };

                        out.push(AgentThreadEvent::ItemCompleted {
                            item: AgentThreadItem::CommandExecution {
                                id: tool_use_id,
                                command,
                                aggregated_output,
                                exit_code,
                                status: if is_error {
                                    AgentCommandExecutionStatus::Failed
                                } else {
                                    AgentCommandExecutionStatus::Completed
                                },
                            },
                        });
                    }
                    AmpToolKind::WebSearch => {
                        let query = match tool.summary {
                            AmpToolSummary::WebSearch { query } => query,
                            _ => String::new(),
                        };
                        out.push(AgentThreadEvent::ItemCompleted {
                            item: AgentThreadItem::WebSearch {
                                id: tool_use_id,
                                query,
                            },
                        });
                    }
                    AmpToolKind::FileChange => {
                        let changes = match tool.summary {
                            AmpToolSummary::FileChange { changes } => changes
                                .into_iter()
                                .map(|(path, kind)| AgentFileUpdateChange {
                                    path,
                                    kind: match kind.as_str() {
                                        "add" => AgentPatchChangeKind::Add,
                                        "delete" => AgentPatchChangeKind::Delete,
                                        _ => AgentPatchChangeKind::Update,
                                    },
                                })
                                .collect(),
                            _ => Vec::new(),
                        };

                        out.push(AgentThreadEvent::ItemCompleted {
                            item: AgentThreadItem::FileChange {
                                id: tool_use_id,
                                changes,
                                status: if is_error {
                                    AgentPatchApplyStatus::Failed
                                } else {
                                    AgentPatchApplyStatus::Completed
                                },
                            },
                        });
                    }
                    AmpToolKind::McpToolCall => {
                        out.push(AgentThreadEvent::ItemCompleted {
                            item: AgentThreadItem::McpToolCall {
                                id: tool_use_id,
                                server: "amp".to_owned(),
                                tool: tool.name,
                                arguments: tool.input,
                                result: if is_error { None } else { Some(result.clone()) },
                                error: if is_error {
                                    Some(AgentErrorMessage {
                                        message: value_as_string(&result)
                                            .unwrap_or_else(|| result.to_string()),
                                    })
                                } else {
                                    None
                                },
                                status: if is_error {
                                    AgentMcpToolCallStatus::Failed
                                } else {
                                    AgentMcpToolCallStatus::Completed
                                },
                            },
                        });
                    }
                }
            }
        }
        return Ok(out);
    }

    if type_name == "result" {
        let subtype = payload
            .get("subtype")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if subtype == "success" {
            let result_text = payload
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();

            let final_text = if !result_text.is_empty() {
                result_text
            } else {
                state.agent_message.trim().to_owned()
            };

            if !final_text.is_empty() {
                out.push(AgentThreadEvent::ItemCompleted {
                    item: AgentThreadItem::AgentMessage {
                        id: state.agent_message_id.clone(),
                        text: final_text,
                    },
                });
            }

            out.push(AgentThreadEvent::TurnCompleted {
                usage: AgentUsage {
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                },
            });
            return Ok(out);
        }

        let message = payload
            .get("error")
            .or_else(|| payload.get("result"))
            .and_then(|v| v.as_str())
            .unwrap_or("amp result error")
            .to_owned();

        out.push(AgentThreadEvent::TurnFailed {
            error: AgentThreadError { message },
        });
        return Ok(out);
    }

    if type_name == "error" {
        let message = payload
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("amp error")
            .to_owned();
        out.push(AgentThreadEvent::Error { message });
        return Ok(out);
    }

    Ok(Vec::new())
}

pub(super) fn run_amp_turn_streamed_via_cli(
    params: AmpTurnParams,
    cancel: Arc<AtomicBool>,
    mut on_event: impl FnMut(AgentThreadEvent) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let AmpTurnParams {
        thread_id,
        worktree_path,
        prompt,
        mode,
    } = params;

    let amp = resolve_amp_exec();
    let mode = mode.as_deref();

    let thread_id = match thread_id {
        Some(id) => id,
        None => run_amp_threads_new(&amp, &worktree_path, mode)?,
    };

    on_event(AgentThreadEvent::ThreadStarted {
        thread_id: thread_id.clone(),
    })?;
    on_event(AgentThreadEvent::TurnStarted)?;

    let mut command = Command::new(&amp);
    command.current_dir(&worktree_path);
    command.args([
        "--no-notifications",
        "--no-ide",
        "--no-jetbrains",
        "--dangerously-allow-all",
    ]);
    if let Some(mode) = mode {
        command.args(["--mode", mode]);
    }
    command.args([
        "threads",
        "continue",
        &thread_id,
        "--execute",
        &prompt,
        "--stream-json",
    ]);

    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "missing amp executable ({}): install Amp CLI and ensure it is available on PATH (or set LUBAN_AMP_BIN to an absolute path)",
                    amp.display()
                )
            } else {
                anyhow!(err).context("failed to spawn amp")
            }
        })?;

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"\n");
    }
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

    let mut state = AmpStreamState::new();
    let stdout_reader = BufReader::new(stdout);
    for line in stdout_reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                return Err(err).context("failed to read amp stdout line");
            }
        };
        if cancel.load(Ordering::SeqCst) {
            break;
        }

        for event in parse_amp_stream_json_line(&mut state, &line)? {
            on_event(event)?;
        }
    }

    let status = child
        .lock()
        .map_err(|_| anyhow!("failed to lock amp child"))?
        .wait()
        .context("failed to wait for amp")?;
    finished.store(true, Ordering::SeqCst);
    let _ = killer.join();
    let stderr_text = stderr_handle.join().unwrap_or_default();

    if cancel.load(Ordering::SeqCst) {
        return Ok(());
    }

    if status.success() {
        return Ok(());
    }

    let message = stderr_text.trim();
    if !message.is_empty() {
        return Err(anyhow!(message.to_owned()));
    }

    Err(anyhow!("amp exited with status {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_init_as_thread_started() {
        let mut state = AmpStreamState::new();
        let events = parse_amp_stream_json_line(
            &mut state,
            r#"{"type":"system","subtype":"init","session_id":"thread_123"}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ThreadStarted { thread_id }] if thread_id == "thread_123"
        ));
    }

    #[test]
    fn parses_tool_use_and_tool_result() {
        let mut state = AmpStreamState::new();
        let events = parse_amp_stream_json_line(
            &mut state,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"bash","input":{"command":"echo hi"}}]}}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemStarted { item: AgentThreadItem::CommandExecution { id, .. } }] if id == "t1"
        ));

        let events = parse_amp_stream_json_line(
            &mut state,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"ok","is_error":false}]}}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemCompleted { item: AgentThreadItem::CommandExecution { id, status: AgentCommandExecutionStatus::Completed, .. } }] if id == "t1"
        ));
    }

    #[test]
    fn parses_result_success_as_turn_completed() {
        let mut state = AmpStreamState::new();
        let _ = parse_amp_stream_json_line(
            &mut state,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#,
        )
        .expect("parse ok");
        let events = parse_amp_stream_json_line(
            &mut state,
            r#"{"type":"result","subtype":"success","result":"Done"}"#,
        )
        .expect("parse ok");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentThreadEvent::TurnCompleted { .. }))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            AgentThreadEvent::ItemCompleted {
                item: AgentThreadItem::AgentMessage { .. }
            }
        )));
    }
}
