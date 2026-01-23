use anyhow::{Context as _, anyhow};
use luban_domain::paths;
use luban_domain::{
    AgentCommandExecutionStatus, AgentErrorMessage, AgentFileUpdateChange, AgentMcpToolCallStatus,
    AgentPatchApplyStatus, AgentPatchChangeKind, AgentThreadError, AgentThreadEvent,
    AgentThreadItem, AgentUsage,
};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(super) struct ClaudeTurnParams {
    pub(super) thread_id: Option<String>,
    pub(super) worktree_path: PathBuf,
    pub(super) prompt: String,
    pub(super) add_dirs: Vec<PathBuf>,
}

fn strip_ansi_control_sequences(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }

        if matches!(chars.peek(), Some('[')) {
            let _ = chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }

        let _ = chars.next();
    }
    out
}

fn resolve_claude_exec() -> PathBuf {
    std::env::var_os(paths::LUBAN_CLAUDE_BIN_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("claude"))
}

#[derive(Default)]
struct ClaudeStreamState {
    agent_message_id: String,
    reasoning_id: String,
    agent_message: String,
    reasoning: String,
    tools: HashMap<String, ClaudeToolUse>,
    saw_turn_completed: bool,
}

impl ClaudeStreamState {
    fn new() -> Self {
        Self {
            agent_message_id: "agent_message".to_owned(),
            reasoning_id: "reasoning".to_owned(),
            agent_message: String::new(),
            reasoning: String::new(),
            tools: HashMap::new(),
            saw_turn_completed: false,
        }
    }
}

#[derive(Clone, Debug)]
struct ClaudeToolUse {
    name: String,
    input: Value,
    kind: ClaudeToolKind,
    summary: ClaudeToolSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClaudeToolKind {
    CommandExecution,
    FileChange,
    WebSearch,
    McpToolCall,
}

#[derive(Clone, Debug)]
enum ClaudeToolSummary {
    None,
    Command { command: String },
    FileChange { changes: Vec<(String, String)> },
    WebSearch { query: String },
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn extract_content_array(value: &Value) -> Option<&Vec<Value>> {
    value
        .pointer("/message/content")
        .and_then(|v| v.as_array())
        .or_else(|| value.get("content").and_then(|v| v.as_array()))
}

fn parse_tool_result_content(content: &Value) -> Value {
    if let Some(s) = content.as_str() {
        return Value::String(s.to_owned());
    }
    content.clone()
}

fn tool_name_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn extract_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = value.get(*key)
            && let Some(s) = v.as_str()
        {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

fn summarize_tool_use(name: &str, input: &Value) -> (ClaudeToolKind, ClaudeToolSummary) {
    let key = tool_name_key(name);

    if key == "bash" {
        let command =
            extract_string_field(input, &["command", "cmd"]).unwrap_or_else(|| "bash".to_owned());
        return (
            ClaudeToolKind::CommandExecution,
            ClaudeToolSummary::Command { command },
        );
    }

    if key == "web_search" || key == "websearch" {
        let query =
            extract_string_field(input, &["query", "q"]).unwrap_or_else(|| "web_search".to_owned());
        return (
            ClaudeToolKind::WebSearch,
            ClaudeToolSummary::WebSearch { query },
        );
    }

    if key == "edit_file"
        || key == "create_file"
        || key == "undo_edit"
        || key == "edit"
        || key == "write"
        || key == "writefile"
        || key == "write_file"
    {
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
            ClaudeToolKind::FileChange,
            ClaudeToolSummary::FileChange { changes },
        );
    }

    (ClaudeToolKind::McpToolCall, ClaudeToolSummary::None)
}

fn parse_claude_stream_json_line(
    state: &mut ClaudeStreamState,
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

                if item_type == "thinking" {
                    let thinking = item
                        .get("thinking")
                        .or_else(|| item.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if thinking.is_empty() {
                        continue;
                    }
                    let was_empty = state.reasoning.is_empty();
                    state.reasoning.push_str(thinking);
                    if was_empty {
                        out.push(AgentThreadEvent::ItemStarted {
                            item: AgentThreadItem::Reasoning {
                                id: state.reasoning_id.clone(),
                                text: state.reasoning.clone(),
                            },
                        });
                    } else {
                        out.push(AgentThreadEvent::ItemUpdated {
                            item: AgentThreadItem::Reasoning {
                                id: state.reasoning_id.clone(),
                                text: state.reasoning.clone(),
                            },
                        });
                    }
                    continue;
                }

                if item_type == "text" {
                    let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    if text.is_empty() {
                        continue;
                    }
                    let was_empty = state.agent_message.is_empty();
                    state.agent_message.push_str(text);
                    if was_empty {
                        out.push(AgentThreadEvent::ItemStarted {
                            item: AgentThreadItem::AgentMessage {
                                id: state.agent_message_id.clone(),
                                text: state.agent_message.clone(),
                            },
                        });
                    } else {
                        out.push(AgentThreadEvent::ItemUpdated {
                            item: AgentThreadItem::AgentMessage {
                                id: state.agent_message_id.clone(),
                                text: state.agent_message.clone(),
                            },
                        });
                    }
                    continue;
                }

                if item_type == "tool_use" {
                    let id = item
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned();
                    if id.is_empty() {
                        continue;
                    }
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool")
                        .to_owned();
                    let input = item.get("input").cloned().unwrap_or(Value::Null);

                    let (kind, summary) = summarize_tool_use(&name, &input);
                    state.tools.insert(
                        id.clone(),
                        ClaudeToolUse {
                            name: name.clone(),
                            input: input.clone(),
                            kind,
                            summary,
                        },
                    );

                    match kind {
                        ClaudeToolKind::CommandExecution => {
                            let command = match state.tools.get(&id).map(|t| &t.summary) {
                                Some(ClaudeToolSummary::Command { command }) => command.clone(),
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
                        ClaudeToolKind::WebSearch => {
                            let query = match state.tools.get(&id).map(|t| &t.summary) {
                                Some(ClaudeToolSummary::WebSearch { query }) => query.clone(),
                                _ => String::new(),
                            };
                            out.push(AgentThreadEvent::ItemStarted {
                                item: AgentThreadItem::WebSearch { id, query },
                            });
                        }
                        ClaudeToolKind::FileChange => {
                            let changes = match state.tools.get(&id).map(|t| &t.summary) {
                                Some(ClaudeToolSummary::FileChange { changes }) => changes
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
                        ClaudeToolKind::McpToolCall => {
                            out.push(AgentThreadEvent::ItemStarted {
                                item: AgentThreadItem::McpToolCall {
                                    id,
                                    server: "claude".to_owned(),
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
                    .unwrap_or("")
                    .to_owned();
                if tool_use_id.is_empty() {
                    continue;
                }

                let content_value = item.get("content").cloned().unwrap_or(Value::Null);
                let result = parse_tool_result_content(&content_value);
                let is_error = item
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let tool = state.tools.remove(&tool_use_id);
                if let Some(tool) = tool {
                    match tool.kind {
                        ClaudeToolKind::CommandExecution => {
                            let (aggregated_output, exit_code) = match result.as_object() {
                                Some(obj) => {
                                    let stdout = obj
                                        .get("stdout")
                                        .or_else(|| obj.get("output"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_owned();
                                    let stderr = obj
                                        .get("stderr")
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_owned();
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
                            out.push(AgentThreadEvent::ItemCompleted {
                                item: AgentThreadItem::CommandExecution {
                                    id: tool_use_id,
                                    command: match tool.summary {
                                        ClaudeToolSummary::Command { command } => command,
                                        _ => tool.name,
                                    },
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
                        ClaudeToolKind::WebSearch => {
                            out.push(AgentThreadEvent::ItemCompleted {
                                item: AgentThreadItem::WebSearch {
                                    id: tool_use_id,
                                    query: match tool.summary {
                                        ClaudeToolSummary::WebSearch { query } => query,
                                        _ => tool.name,
                                    },
                                },
                            });
                        }
                        ClaudeToolKind::FileChange => {
                            let changes = match tool.summary {
                                ClaudeToolSummary::FileChange { changes } => changes,
                                _ => Vec::new(),
                            };
                            out.push(AgentThreadEvent::ItemCompleted {
                                item: AgentThreadItem::FileChange {
                                    id: tool_use_id,
                                    changes: changes
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
                                    status: if is_error {
                                        AgentPatchApplyStatus::Failed
                                    } else {
                                        AgentPatchApplyStatus::Completed
                                    },
                                },
                            });
                        }
                        ClaudeToolKind::McpToolCall => {
                            out.push(AgentThreadEvent::ItemCompleted {
                                item: AgentThreadItem::McpToolCall {
                                    id: tool_use_id,
                                    server: "claude".to_owned(),
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

            state.saw_turn_completed = true;
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
            .unwrap_or("claude result error")
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
            .unwrap_or("claude error")
            .to_owned();
        out.push(AgentThreadEvent::Error { message });
        return Ok(out);
    }

    Ok(Vec::new())
}

pub(super) fn run_claude_turn_streamed_via_cli(
    params: ClaudeTurnParams,
    cancel: Arc<AtomicBool>,
    mut on_event: impl FnMut(AgentThreadEvent) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let ClaudeTurnParams {
        thread_id,
        worktree_path,
        prompt,
        add_dirs,
    } = params;

    let claude = resolve_claude_exec();

    on_event(AgentThreadEvent::TurnStarted)?;

    let mut command = Command::new(&claude);
    command.current_dir(&worktree_path);
    command.args([
        "--print",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-mode",
        "bypassPermissions",
    ]);
    for dir in add_dirs {
        command.arg("--add-dir").arg(dir);
    }
    if let Some(thread_id) = thread_id.as_deref() {
        command.arg("--resume").arg(thread_id);
    }
    command.arg(prompt);

    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "missing claude executable ({}): install Claude Code and ensure it is available on PATH (or set LUBAN_CLAUDE_BIN to an absolute path)",
                    claude.display()
                )
            } else {
                anyhow!(err).context("failed to spawn claude")
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

    let mut state = ClaudeStreamState::new();
    let stdout_reader = BufReader::new(stdout);
    for line in stdout_reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                return Err(err).context("failed to read claude stdout line");
            }
        };
        if cancel.load(Ordering::SeqCst) {
            break;
        }

        for event in parse_claude_stream_json_line(&mut state, &line)? {
            on_event(event)?;
        }
    }

    let status = child
        .lock()
        .map_err(|_| anyhow!("failed to lock claude child"))?
        .wait()
        .context("failed to wait for claude")?;
    finished.store(true, Ordering::SeqCst);
    let _ = killer.join();
    let stderr_text = stderr_handle.join().unwrap_or_default();

    if cancel.load(Ordering::SeqCst) {
        return Ok(());
    }

    if status.success() {
        if !state.saw_turn_completed {
            let final_text = state.agent_message.trim().to_owned();
            if !final_text.is_empty() {
                on_event(AgentThreadEvent::ItemCompleted {
                    item: AgentThreadItem::AgentMessage {
                        id: state.agent_message_id,
                        text: final_text,
                    },
                })?;
            }
            on_event(AgentThreadEvent::TurnCompleted {
                usage: AgentUsage {
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                },
            })?;
        }
        return Ok(());
    }

    let message = stderr_text.trim();
    if !message.is_empty() {
        return Err(anyhow!(message.to_owned()));
    }

    Err(anyhow!("claude exited with status {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_init_as_thread_started() {
        let mut state = ClaudeStreamState::new();
        let events = parse_claude_stream_json_line(
            &mut state,
            r#"{"type":"system","subtype":"init","session_id":"session_123"}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ThreadStarted { thread_id }] if thread_id == "session_123"
        ));
    }

    #[test]
    fn parses_bash_tool_use_and_tool_result() {
        let mut state = ClaudeStreamState::new();
        let events = parse_claude_stream_json_line(
            &mut state,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"bash","input":{"command":"echo hi"}}]}}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemStarted { item: AgentThreadItem::CommandExecution { id, .. } }] if id == "t1"
        ));

        let events = parse_claude_stream_json_line(
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
        let mut state = ClaudeStreamState::new();
        let _ = parse_claude_stream_json_line(
            &mut state,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#,
        )
        .expect("parse ok");
        let events = parse_claude_stream_json_line(
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
