use anyhow::{Context as _, anyhow};
use luban_domain::paths;
use luban_domain::{
    AgentCommandExecutionStatus, AgentErrorMessage, AgentFileUpdateChange, AgentMcpToolCallStatus,
    AgentPatchApplyStatus, AgentPatchChangeKind, AgentThreadEvent, AgentThreadItem, AgentUsage,
};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::ansi::strip_ansi_control_sequences;
use super::cancel_killer::spawn_cancel_killer;
use super::stream_json::{extract_string_field, tool_name_key, value_as_string};
use super::thread_io::spawn_read_to_string;

pub(super) struct DroidTurnParams {
    pub(super) session_id: Option<String>,
    pub(super) worktree_path: PathBuf,
    pub(super) prompt: String,
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<String>,
    pub(super) auto_level: Option<String>,
}

fn resolve_droid_exec() -> PathBuf {
    std::env::var_os(paths::LUBAN_DROID_BIN_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("droid"))
}

/// State for parsing Droid's stream-json JSONL output.
///
/// Droid emits JSONL with event types: system, message, tool_call, tool_result,
/// completion. This mirrors ClaudeStreamState but with droid-specific event
/// mapping.
#[derive(Default)]
pub struct DroidStreamState {
    agent_message_id: String,
    reasoning_id: String,
    agent_message: String,
    reasoning: String,
    tools: HashMap<String, DroidToolUse>,
    saw_turn_completed: bool,
}

impl DroidStreamState {
    pub fn new() -> Self {
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
struct DroidToolUse {
    name: String,
    input: Value,
    kind: DroidToolKind,
    summary: DroidToolSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DroidToolKind {
    CommandExecution,
    FileChange,
    McpToolCall,
}

#[derive(Clone, Debug)]
enum DroidToolSummary {
    None,
    Command { command: String },
    FileChange { changes: Vec<(String, String)> },
}

fn summarize_droid_tool(name: &str, input: &Value) -> (DroidToolKind, DroidToolSummary) {
    let key = tool_name_key(name);

    if key == "bash" || key == "shell" || key == "exec" || key == "execute" || key == "run_command"
    {
        let command =
            extract_string_field(input, &["command", "cmd"]).unwrap_or_else(|| "bash".to_owned());
        return (
            DroidToolKind::CommandExecution,
            DroidToolSummary::Command { command },
        );
    }

    if key == "edit_file"
        || key == "create_file"
        || key == "write_file"
        || key == "edit"
        || key == "write"
        || key == "patch"
        || key == "create"
        || key == "applypatch"
    {
        let path =
            extract_string_field(input, &["path", "file_path", "filename"]).unwrap_or_default();
        let kind = if key == "create_file" || key == "create" {
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
            DroidToolKind::FileChange,
            DroidToolSummary::FileChange { changes },
        );
    }

    (DroidToolKind::McpToolCall, DroidToolSummary::None)
}

/// Parse a single JSONL line from Droid's `--output-format stream-json` output.
///
/// Droid emits these event types:
/// - `system`: session info (may contain session_id)
/// - `message`: text content from the agent
/// - `tool_call`: tool invocation with name + input
/// - `tool_result`: result for a previous tool_call
/// - `completion`: turn finished with optional usage stats
pub fn parse_droid_stream_json_line(
    state: &mut DroidStreamState,
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
        let session_id = payload
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        if let Some(id) = session_id {
            out.push(AgentThreadEvent::ThreadStarted { thread_id: id });
        }
        return Ok(out);
    }

    if type_name == "message" {
        // Reason: Droid CLI emits message events for both user and assistant.
        // We only process assistant messages; user messages are echoes of the prompt.
        let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role == "user" {
            return Ok(Vec::new());
        }

        let thinking = payload
            .get("thinking")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !thinking.is_empty() {
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
        }

        let text = payload.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if !text.is_empty() {
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
        }
        return Ok(out);
    }

    if type_name == "tool_call" {
        let id = payload
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        if id.is_empty() {
            return Ok(Vec::new());
        }
        let name = payload
            .get("toolName")
            .and_then(|v| v.as_str())
            .unwrap_or("tool")
            .to_owned();
        let input = payload.get("parameters").cloned().unwrap_or(Value::Null);

        let (kind, summary) = summarize_droid_tool(&name, &input);
        state.tools.insert(
            id.clone(),
            DroidToolUse {
                name: name.clone(),
                input: input.clone(),
                kind,
                summary,
            },
        );

        match kind {
            DroidToolKind::CommandExecution => {
                let command = match state.tools.get(&id).map(|t| &t.summary) {
                    Some(DroidToolSummary::Command { command }) => command.clone(),
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
            DroidToolKind::FileChange => {
                let changes = match state.tools.get(&id).map(|t| &t.summary) {
                    Some(DroidToolSummary::FileChange { changes }) => changes
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
            DroidToolKind::McpToolCall => {
                out.push(AgentThreadEvent::ItemStarted {
                    item: AgentThreadItem::McpToolCall {
                        id,
                        server: "droid".to_owned(),
                        tool: name,
                        arguments: input,
                        result: None,
                        error: None,
                        status: AgentMcpToolCallStatus::InProgress,
                    },
                });
            }
        }
        return Ok(out);
    }

    if type_name == "tool_result" {
        let tool_call_id = payload
            .get("tool_call_id")
            .or_else(|| payload.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        if tool_call_id.is_empty() {
            return Ok(Vec::new());
        }

        let content = payload.get("value").cloned().unwrap_or(Value::Null);
        let result_text = value_as_string(&content).unwrap_or_else(|| content.to_string());
        let is_error = payload
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let tool = state.tools.remove(&tool_call_id);
        if let Some(tool) = tool {
            match tool.kind {
                DroidToolKind::CommandExecution => {
                    out.push(AgentThreadEvent::ItemCompleted {
                        item: AgentThreadItem::CommandExecution {
                            id: tool_call_id,
                            command: match tool.summary {
                                DroidToolSummary::Command { command } => command,
                                _ => tool.name,
                            },
                            aggregated_output: result_text,
                            exit_code: None,
                            status: if is_error {
                                AgentCommandExecutionStatus::Failed
                            } else {
                                AgentCommandExecutionStatus::Completed
                            },
                        },
                    });
                }
                DroidToolKind::FileChange => {
                    let changes = match tool.summary {
                        DroidToolSummary::FileChange { changes } => changes,
                        _ => Vec::new(),
                    };
                    out.push(AgentThreadEvent::ItemCompleted {
                        item: AgentThreadItem::FileChange {
                            id: tool_call_id,
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
                DroidToolKind::McpToolCall => {
                    out.push(AgentThreadEvent::ItemCompleted {
                        item: AgentThreadItem::McpToolCall {
                            id: tool_call_id,
                            server: "droid".to_owned(),
                            tool: tool.name,
                            arguments: tool.input,
                            result: if is_error {
                                None
                            } else {
                                Some(content.clone())
                            },
                            error: if is_error {
                                Some(AgentErrorMessage {
                                    message: result_text,
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
        return Ok(out);
    }

    if type_name == "completion" {
        // Reason: Droid "completion" events signal end-of-turn. Usage stats may
        // be present under "usage" with input_tokens/output_tokens fields.
        let usage = payload.get("usage");
        let input_tokens = usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let final_text = state.agent_message.trim().to_owned();
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
                input_tokens,
                cached_input_tokens: 0,
                output_tokens,
            },
        });
        return Ok(out);
    }

    if type_name == "error" {
        let message = payload
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("droid error")
            .to_owned();
        out.push(AgentThreadEvent::Error { message });
        return Ok(out);
    }

    // Reason: Unknown event types are silently skipped to allow forward
    // compatibility with future droid CLI versions.
    Ok(Vec::new())
}

pub(super) fn run_droid_turn_streamed_via_cli(
    params: DroidTurnParams,
    cancel: Arc<AtomicBool>,
    mut on_event: impl FnMut(AgentThreadEvent) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let DroidTurnParams {
        session_id,
        worktree_path,
        prompt,
        model,
        reasoning_effort,
        auto_level,
    } = params;

    let droid = resolve_droid_exec();

    on_event(AgentThreadEvent::TurnStarted)?;

    let mut command = Command::new(&droid);
    command.args(["exec", "--output-format", "stream-json"]);
    command.arg("--cwd").arg(&worktree_path);

    if let Some(m) = model.as_deref()
        && !m.is_empty()
    {
        command.arg("-m").arg(m);
    }
    if let Some(r) = reasoning_effort.as_deref()
        && !r.is_empty()
    {
        command.arg("-r").arg(r);
    }
    if let Some(a) = auto_level.as_deref()
        && !a.is_empty()
    {
        command.arg("--auto").arg(a);
    }
    if let Some(sid) = session_id.as_deref()
        && !sid.is_empty()
    {
        command.arg("-s").arg(sid);
    }

    // Reason: Droid reads the prompt from piped stdin when no positional
    // argument is given. The deprecated `-` flag was removed in v0.57+.

    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "missing droid executable ({}): install the Droid CLI and ensure \
                     it is available on PATH (or set LUBAN_DROID_BIN to an absolute path)",
                    droid.display()
                )
            } else {
                anyhow!(err).context("failed to spawn droid")
            }
        })?;

    // Pipe the prompt into stdin, then close it.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
        let _ = stdin.write_all(b"\n");
    }

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

    let mut stream_state = DroidStreamState::new();
    let stdout_reader = BufReader::new(stdout);
    for line in stdout_reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                return Err(err).context("failed to read droid stdout line");
            }
        };
        if cancel.load(Ordering::SeqCst) {
            break;
        }

        for event in parse_droid_stream_json_line(&mut stream_state, &line)? {
            on_event(event)?;
        }
    }

    let status = child
        .lock()
        .map_err(|_| anyhow!("failed to lock droid child"))?
        .wait()
        .context("failed to wait for droid")?;
    finished.store(true, Ordering::SeqCst);
    let _ = killer.join();
    let stderr_text = stderr_handle.join().unwrap_or_default();

    if cancel.load(Ordering::SeqCst) {
        return Ok(());
    }

    if status.success() {
        if !stream_state.saw_turn_completed {
            let final_text = stream_state.agent_message.trim().to_owned();
            if !final_text.is_empty() {
                on_event(AgentThreadEvent::ItemCompleted {
                    item: AgentThreadItem::AgentMessage {
                        id: stream_state.agent_message_id,
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

    Err(anyhow!("droid exited with status {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_event_as_thread_started() {
        let mut state = DroidStreamState::new();
        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"system","session_id":"ses_abc123"}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ThreadStarted { thread_id }] if thread_id == "ses_abc123"
        ));
    }

    #[test]
    fn parses_message_text_as_agent_message() {
        let mut state = DroidStreamState::new();
        let events =
            parse_droid_stream_json_line(&mut state, r#"{"type":"message","text":"Hello world"}"#)
                .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemStarted { item: AgentThreadItem::AgentMessage { text, .. } }]
                if text == "Hello world"
        ));

        // Second message chunk should be an update
        let events =
            parse_droid_stream_json_line(&mut state, r#"{"type":"message","text":" more text"}"#)
                .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemUpdated { item: AgentThreadItem::AgentMessage { text, .. } }]
                if text == "Hello world more text"
        ));
    }

    #[test]
    fn parses_message_thinking_as_reasoning() {
        let mut state = DroidStreamState::new();
        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"message","thinking":"Let me think..."}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemStarted { item: AgentThreadItem::Reasoning { text, .. } }]
                if text == "Let me think..."
        ));
    }

    #[test]
    fn skips_user_role_messages() {
        let mut state = DroidStreamState::new();
        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"message","role":"user","id":"u1","text":"read README.md"}"#,
        )
        .expect("parse ok");
        assert!(events.is_empty(), "user messages should be skipped");
        assert!(
            state.agent_message.is_empty(),
            "user text should not accumulate"
        );
    }

    // -- Tests matching the actual Droid CLI stream-json format --

    #[test]
    fn parses_droid_tool_call_and_tool_result_real_format() {
        // Reason: Droid CLI uses "toolName" / "parameters" / "value" / "isError"
        let mut state = DroidStreamState::new();
        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"tool_call","id":"tc1","toolName":"Execute","parameters":{"command":"echo hi"}}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemStarted {
                item: AgentThreadItem::CommandExecution { id, command, .. }
            }] if id == "tc1" && command == "echo hi"
        ));

        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"tool_result","id":"tc1","value":"hi\n","isError":false}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemCompleted {
                item: AgentThreadItem::CommandExecution {
                    id, status: AgentCommandExecutionStatus::Completed, ..
                }
            }] if id == "tc1"
        ));
    }

    #[test]
    fn parses_droid_mcp_tool_call_real_format() {
        // Actual Droid CLI format: toolName + parameters
        let mut state = DroidStreamState::new();
        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"tool_call","id":"tc2","toolName":"Read","parameters":{"file_path":"README.md","limit":5}}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemStarted {
                item: AgentThreadItem::McpToolCall { id, server, tool, .. }
            }] if id == "tc2" && server == "droid" && tool == "Read"
        ));
    }

    #[test]
    fn parses_droid_mcp_tool_result_real_format() {
        let mut state = DroidStreamState::new();
        let _ = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"tool_call","id":"tc3","toolName":"Read","parameters":{"file_path":"README.md"}}"#,
        )
        .expect("parse ok");

        let events = parse_droid_stream_json_line(
            &mut state,
            r##"{"type":"tool_result","id":"tc3","value":"# Hello","isError":false}"##,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemCompleted {
                item: AgentThreadItem::McpToolCall {
                    id, status: AgentMcpToolCallStatus::Completed, ..
                }
            }] if id == "tc3"
        ));
    }

    #[test]
    fn parses_droid_mcp_tool_error_result_real_format() {
        let mut state = DroidStreamState::new();
        let _ = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"tool_call","id":"tc4","toolName":"Read","parameters":{"file_path":"missing.txt"}}"#,
        )
        .expect("parse ok");

        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"tool_result","id":"tc4","value":"file not found","isError":true}"#,
        )
        .expect("parse ok");
        assert!(matches!(
            events.as_slice(),
            [AgentThreadEvent::ItemCompleted {
                item: AgentThreadItem::McpToolCall {
                    id, status: AgentMcpToolCallStatus::Failed, ..
                }
            }] if id == "tc4"
        ));
    }

    #[test]
    fn parses_completion_as_turn_completed() {
        let mut state = DroidStreamState::new();
        // Emit a message first so completion has something to finalize
        let _ = parse_droid_stream_json_line(&mut state, r#"{"type":"message","text":"Done."}"#);
        let events = parse_droid_stream_json_line(
            &mut state,
            r#"{"type":"completion","usage":{"input_tokens":100,"output_tokens":50}}"#,
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

    #[test]
    fn skips_empty_and_invalid_lines() {
        let mut state = DroidStreamState::new();
        assert!(
            parse_droid_stream_json_line(&mut state, "")
                .expect("parse ok")
                .is_empty()
        );
        assert!(
            parse_droid_stream_json_line(&mut state, "not json at all")
                .expect("parse ok")
                .is_empty()
        );
        assert!(
            parse_droid_stream_json_line(&mut state, r#"{"type":"unknown_future_event"}"#)
                .expect("parse ok")
                .is_empty()
        );
    }
}
