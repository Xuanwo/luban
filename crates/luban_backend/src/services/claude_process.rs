use anyhow::anyhow;
use luban_domain::AgentThreadEvent;
use luban_domain::paths;
use serde_json;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::claude_cli::{ClaudeStreamState, parse_claude_stream_json_line_public};

/// A persistent Claude process that maintains MCP connections across multiple turns.
///
/// Each thread/tab gets its own ClaudeThreadProcess. The process stays alive
/// as long as the tab is open, and MCP connections are established once during warmup.
///
/// Note: Many fields and methods are currently unused as this is infrastructure
/// for when Claude CLI supports persistent/interactive mode.
#[allow(dead_code)]
pub struct ClaudeThreadProcess {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<BufWriter<ChildStdin>>>,
    session_id: Option<String>,
    worktree_path: PathBuf,
    ready: AtomicBool,
    shutdown: AtomicBool,

    /// Events received from stdout, ready to be consumed
    event_queue: Arc<Mutex<VecDeque<AgentThreadEvent>>>,

    /// Handle to the stdout reader thread
    reader_handle: Option<JoinHandle<()>>,

    /// Signal that a turn has completed
    turn_completed: Arc<AtomicBool>,
}

#[allow(dead_code)]
fn resolve_claude_exec() -> PathBuf {
    std::env::var_os(paths::LUBAN_CLAUDE_BIN_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("claude"))
}

#[allow(dead_code)]
impl ClaudeThreadProcess {
    /// Spawn a new Claude process in persistent/interactive mode.
    ///
    /// The process uses `--input-format stream-json` to accept prompts from stdin,
    /// allowing multiple turns without reconnecting MCP each time.
    pub fn spawn_and_warmup(
        worktree_path: &Path,
        thread_id: Option<&str>,
        add_dirs: &[PathBuf],
    ) -> anyhow::Result<Self> {
        let claude = resolve_claude_exec();

        let mut command = Command::new(&claude);
        command.current_dir(worktree_path);

        // Use stream-json for both input and output to enable persistent mode
        // where we can send multiple prompts via stdin without restarting
        command.args([
            "--print",
            "--output-format",
            "stream-json",
            "--input-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-mode",
            "bypassPermissions",
        ]);

        // Add extra directories for context
        for dir in add_dirs {
            command.arg("--add-dir").arg(dir);
        }

        // Resume from existing thread if available
        if let Some(tid) = thread_id {
            command.arg("--resume").arg(tid);
        }

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to get stdin handle"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to get stdout handle"))?;

        let event_queue = Arc::new(Mutex::new(VecDeque::new()));
        let turn_completed = Arc::new(AtomicBool::new(false));

        // Spawn stdout reader thread
        let reader_handle =
            Self::spawn_stdout_reader(stdout, event_queue.clone(), turn_completed.clone());

        let process = Self {
            child: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(BufWriter::new(stdin))),
            session_id: None,
            worktree_path: worktree_path.to_path_buf(),
            ready: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            event_queue,
            reader_handle: Some(reader_handle),
            turn_completed,
        };

        Ok(process)
    }

    fn spawn_stdout_reader(
        stdout: ChildStdout,
        event_queue: Arc<Mutex<VecDeque<AgentThreadEvent>>>,
        turn_completed: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            let mut state = ClaudeStreamState::new();

            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };

                if let Ok(events) = parse_claude_stream_json_line_public(&mut state, &line) {
                    let mut queue = event_queue.lock().unwrap();
                    for event in events {
                        // Check if this is a turn completion event
                        if matches!(
                            &event,
                            AgentThreadEvent::TurnCompleted { .. }
                                | AgentThreadEvent::TurnFailed { .. }
                        ) {
                            turn_completed.store(true, Ordering::SeqCst);
                        }
                        queue.push_back(event);
                    }
                }
            }
        })
    }

    /// Check if the process is still alive
    pub fn is_alive(&self) -> bool {
        if self.shutdown.load(Ordering::SeqCst) {
            return false;
        }

        if let Ok(mut child) = self.child.lock() {
            match child.try_wait() {
                Ok(Some(_)) => false, // Process has exited
                Ok(None) => true,     // Process is still running
                Err(_) => false,      // Error checking status
            }
        } else {
            false
        }
    }

    /// Check if MCP warmup has completed
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Mark the process as ready (MCP connected)
    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    /// Get the session ID if available
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Set the session ID (called when ThreadStarted event is received)
    pub fn set_session_id(&mut self, session_id: String) {
        self.session_id = Some(session_id);
    }

    /// Send a prompt to the process via stdin in stream-json format
    ///
    /// Returns immediately after writing. Use poll_events() to get responses.
    pub fn send_prompt(&self, prompt: &str) -> anyhow::Result<()> {
        self.turn_completed.store(false, Ordering::SeqCst);

        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| anyhow!("failed to lock stdin"))?;

        // Format the prompt as stream-json input
        // The correct format for Claude CLI --input-format stream-json is:
        // {"type":"user","message":{"role":"user","content":"text"}}
        let input = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": prompt
            }
        });

        // Write the JSON followed by newline
        writeln!(stdin, "{}", input)?;
        stdin.flush()?;

        Ok(())
    }

    /// Poll for events from the process
    ///
    /// Returns all queued events since last poll.
    pub fn poll_events(&self) -> Vec<AgentThreadEvent> {
        let mut queue = self.event_queue.lock().unwrap();
        queue.drain(..).collect()
    }

    /// Check if the current turn has completed
    pub fn is_turn_completed(&self) -> bool {
        self.turn_completed.load(Ordering::SeqCst)
    }

    /// Wait for the current turn to complete with timeout
    pub fn wait_for_turn_completion(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if self.is_turn_completed() || !self.is_alive() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Shutdown the process gracefully
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);

        // Try to kill the process
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Wait for reader thread to finish
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ClaudeThreadProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Key for identifying a Claude process (project_slug, workspace_name, thread_local_id)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClaudeProcessKey {
    pub project_slug: String,
    pub workspace_name: String,
    pub thread_local_id: u64,
}

impl ClaudeProcessKey {
    pub fn new(
        project_slug: impl Into<String>,
        workspace_name: impl Into<String>,
        thread_local_id: u64,
    ) -> Self {
        Self {
            project_slug: project_slug.into(),
            workspace_name: workspace_name.into(),
            thread_local_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_key_equality() {
        let key1 = ClaudeProcessKey::new("proj", "ws", 1);
        let key2 = ClaudeProcessKey::new("proj", "ws", 1);
        let key3 = ClaudeProcessKey::new("proj", "ws", 2);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }
}
