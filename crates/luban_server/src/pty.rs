use anyhow::Context as _;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt as _, StreamExt as _};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::{HashMap, VecDeque};
use std::io::{Read as _, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::Duration;

type PtyKey = (u64, String);
type PtySessions = HashMap<PtyKey, Arc<PtySession>>;

const MAX_OUTPUT_HISTORY_BYTES: usize = 512 * 1024;
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const LIVE_BUFFER_CAPACITY: usize = 64;

#[derive(Clone, Debug)]
enum PtyProgram {
    Shell,
    ShellCommand { command: String },
}

fn trace_bytes(label: &str, bytes: &[u8]) {
    if std::env::var_os("LUBAN_PTY_TRACE").is_none() {
        return;
    }
    const MAX: usize = 64;
    let mut out = String::new();
    for (idx, b) in bytes.iter().take(MAX).enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
    if bytes.len() > MAX {
        out.push_str(" …");
    }
    tracing::info!(label = %label, len = bytes.len(), hex = %out);
}

#[derive(Clone)]
pub struct PtyManager {
    inner: Arc<Mutex<PtySessions>>,
    idle_timeout: Duration,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }

    pub fn get_or_create(
        &self,
        workspace_id: u64,
        reconnect: String,
        cwd: PathBuf,
    ) -> anyhow::Result<Arc<PtySession>> {
        self.get_or_create_with_program(workspace_id, reconnect, cwd, PtyProgram::Shell)
    }

    pub fn spawn_command(
        &self,
        workspace_id: u64,
        reconnect: String,
        cwd: PathBuf,
        command: String,
    ) -> anyhow::Result<Arc<PtySession>> {
        self.get_or_create_with_program(
            workspace_id,
            reconnect,
            cwd,
            PtyProgram::ShellCommand { command },
        )
    }

    fn get_or_create_with_program(
        &self,
        workspace_id: u64,
        reconnect: String,
        cwd: PathBuf,
        program: PtyProgram,
    ) -> anyhow::Result<Arc<PtySession>> {
        let mut guard = self.inner.lock().expect("pty manager lock poisoned");
        if let Some(existing) = guard.get(&(workspace_id, reconnect.clone())) {
            if !existing.is_terminated() {
                return Ok(existing.clone());
            }
            guard.remove(&(workspace_id, reconnect.clone()));
        }

        let session = Arc::new(PtySession::spawn(
            cwd,
            program,
            self.idle_timeout,
            Arc::downgrade(&self.inner),
            (workspace_id, reconnect.clone()),
        )?);
        guard.insert((workspace_id, reconnect), session.clone());
        Ok(session)
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PtySession {
    terminated: Arc<std::sync::atomic::AtomicBool>,
    terminated_tx: broadcast::Sender<()>,
    connection_count_tx: watch::Sender<usize>,
    state: Arc<Mutex<PtySessionState>>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    child: Arc<Mutex<Option<Box<dyn portable_pty::Child + Send>>>>,
}

#[derive(Default)]
struct OutputHistory {
    chunks: VecDeque<HistoryChunk>,
    total_bytes: usize,
}

impl OutputHistory {
    fn push(&mut self, chunk: Bytes) {
        self.total_bytes = self.total_bytes.saturating_add(chunk.len());
        self.chunks.push_back(HistoryChunk { bytes: chunk });
        while self.total_bytes > MAX_OUTPUT_HISTORY_BYTES {
            let Some(front) = self.chunks.pop_front() else {
                self.total_bytes = 0;
                break;
            };
            self.total_bytes = self.total_bytes.saturating_sub(front.bytes.len());
        }
    }

    fn snapshot_chunks(&self) -> Vec<Bytes> {
        self.chunks.iter().map(|c| c.bytes.clone()).collect()
    }

    fn snapshot_bytes(&self) -> (Vec<u8>, usize) {
        if self.total_bytes == 0 {
            return (Vec::new(), 0);
        }
        let mut out = Vec::with_capacity(self.total_bytes);
        for chunk in &self.chunks {
            out.extend_from_slice(&chunk.bytes);
        }
        (out, self.total_bytes)
    }
}

#[derive(Clone)]
struct HistoryChunk {
    bytes: Bytes,
}

#[derive(Clone)]
struct LiveChunk {
    seq: u64,
    bytes: Bytes,
}

struct PtySessionState {
    history: OutputHistory,
    next_seq: u64,
    active: Option<ActiveConnection>,
    next_connection_id: u64,
}

#[derive(Clone)]
struct ActiveConnection {
    id: u64,
    tx: mpsc::Sender<LiveChunk>,
}

impl PtySession {
    fn spawn(
        cwd: PathBuf,
        program: PtyProgram,
        idle_timeout: Duration,
        manager: std::sync::Weak<Mutex<PtySessions>>,
        key: PtyKey,
    ) -> anyhow::Result<Self> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("openpty failed")?;

        let shell = default_shell_path();
        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(cwd);
        if std::env::var_os("TERM").is_none() {
            cmd.env("TERM", "xterm-256color");
        }
        if std::env::var_os("COLORTERM").is_none() {
            cmd.env("COLORTERM", "truecolor");
        }

        if let PtyProgram::ShellCommand { command } = program {
            let args = shell_command_args(shell.as_path(), &command);
            cmd.args(args);
        }

        let child = pair.slave.spawn_command(cmd).context("spawn pty command")?;
        let reader = pair.master.try_clone_reader().context("clone pty reader")?;
        let writer = pair.master.take_writer().context("take pty writer")?;

        let (terminated_tx, _) = broadcast::channel::<()>(8);
        let (connection_count_tx, _) = watch::channel::<usize>(0);
        let terminated = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let terminated_for_thread = terminated.clone();
        let terminated_tx_for_thread = terminated_tx.clone();
        let state = Arc::new(Mutex::new(PtySessionState {
            history: OutputHistory::default(),
            next_seq: 1,
            active: None,
            next_connection_id: 1,
        }));
        let state_for_thread = state.clone();
        let connection_count_for_thread = connection_count_tx.clone();
        let manager_for_thread = manager.clone();
        let key_for_thread = key.clone();

        std::thread::Builder::new()
            .name("luban-pty-read".to_owned())
            .spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let chunk = Bytes::copy_from_slice(&buf[..n]);
                            let (seq, active) = match state_for_thread.lock() {
                                Ok(mut guard) => {
                                    let seq = guard.next_seq;
                                    guard.next_seq = guard.next_seq.saturating_add(1);
                                    guard.history.push(chunk.clone());
                                    (seq, guard.active.clone())
                                }
                                Err(_) => break,
                            };

                            let Some(active) = active else {
                                continue;
                            };

                            if active
                                .tx
                                .blocking_send(LiveChunk { seq, bytes: chunk })
                                .is_err()
                                && let Ok(mut guard) = state_for_thread.lock()
                                && guard.active.as_ref().is_some_and(|c| c.id == active.id)
                            {
                                guard.active = None;
                                let _ = connection_count_for_thread.send(0);
                            }
                        }
                        Err(_) => break,
                    }
                }
                terminated_for_thread.store(true, Ordering::SeqCst);
                if let Ok(mut guard) = state_for_thread.lock() {
                    guard.active = None;
                }
                let _ = connection_count_for_thread.send(0);
                if let Some(manager) = manager_for_thread.upgrade()
                    && let Ok(mut guard) = manager.lock()
                {
                    guard.remove(&key_for_thread);
                }
                let _ = terminated_tx_for_thread.send(());
            })
            .context("spawn pty reader thread")?;

        let session = Self {
            terminated,
            terminated_tx,
            connection_count_tx,
            state,
            writer: Arc::new(Mutex::new(Some(writer))),
            master: Arc::new(Mutex::new(Some(pair.master))),
            child: Arc::new(Mutex::new(Some(child))),
        };

        session.spawn_idle_reaper(idle_timeout, manager, key);

        Ok(session)
    }

    fn spawn_idle_reaper(
        &self,
        idle_timeout: Duration,
        manager: std::sync::Weak<Mutex<PtySessions>>,
        key: PtyKey,
    ) {
        let mut rx = self.connection_count_tx.subscribe();
        let terminated = self.terminated.clone();
        let terminated_tx = self.terminated_tx.clone();
        let connection_count_tx = self.connection_count_tx.clone();
        let state = self.state.clone();
        let child = self.child.clone();
        let writer = self.writer.clone();
        let master = self.master.clone();

        tokio::spawn(async move {
            loop {
                if terminated.load(Ordering::SeqCst) {
                    break;
                }

                if *rx.borrow() > 0 {
                    if rx.changed().await.is_err() {
                        break;
                    }
                    continue;
                }

                tokio::select! {
                    _ = tokio::time::sleep(idle_timeout) => {
                        if terminated.load(Ordering::SeqCst) {
                            break;
                        }
                        if *rx.borrow() > 0 {
                            continue;
                        }
                        terminated.store(true, Ordering::SeqCst);
                        if let Ok(mut guard) = state.lock() {
                            guard.active = None;
                        }
                        let _ = connection_count_tx.send(0);

                        if let Ok(mut guard) = child.lock()
                            && let Some(mut child) = guard.take()
                        {
                            let _ = child.kill();
                        }
                        if let Ok(mut guard) = writer.lock() {
                            guard.take();
                        }
                        if let Ok(mut guard) = master.lock() {
                            guard.take();
                        }
                        if let Some(manager) = manager.upgrade()
                            && let Ok(mut guard) = manager.lock()
                        {
                            guard.remove(&key);
                        }
                        let _ = terminated_tx.send(());
                        break;
                    }
                    changed = rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::SeqCst)
    }

    pub fn subscribe_terminated(&self) -> broadcast::Receiver<()> {
        self.terminated_tx.subscribe()
    }

    fn attach(&self) -> (u64, Vec<Bytes>, u64, mpsc::Receiver<LiveChunk>) {
        let mut guard = self.state.lock().expect("pty session lock poisoned");
        let history = guard.history.snapshot_chunks();
        let connection_id = guard.next_connection_id;
        guard.next_connection_id = guard.next_connection_id.saturating_add(1);
        let (tx, rx) = mpsc::channel::<LiveChunk>(LIVE_BUFFER_CAPACITY);
        guard.active = Some(ActiveConnection {
            id: connection_id,
            tx,
        });
        let last_seq = guard.next_seq.saturating_sub(1);
        let _ = self.connection_count_tx.send(1);
        (connection_id, history, last_seq, rx)
    }

    fn detach(&self, connection_id: u64) {
        let mut guard = self.state.lock().expect("pty session lock poisoned");
        if guard.active.as_ref().is_some_and(|c| c.id == connection_id) {
            guard.active = None;
            let _ = self.connection_count_tx.send(0);
        }
    }

    pub fn write_input(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().expect("pty writer lock poisoned");
        let Some(writer) = writer.as_mut() else {
            return Ok(());
        };
        if let Err(err) = writer.write_all(bytes) {
            self.terminated.store(true, Ordering::SeqCst);
            let _ = self.terminated_tx.send(());
            return Err(err).context("pty write");
        }
        writer.flush().ok();
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        let master = self.master.lock().expect("pty master lock poisoned");
        let Some(master) = master.as_ref() else {
            return Ok(());
        };
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("pty resize")?;
        Ok(())
    }

    pub fn output_snapshot(&self) -> (Vec<u8>, u64) {
        let guard = self.state.lock().expect("pty session lock poisoned");
        let (bytes, len) = guard.history.snapshot_bytes();
        (bytes, len as u64)
    }
}

fn default_shell_path() -> PathBuf {
    if let Some(shell) = std::env::var_os("SHELL")
        && !shell.to_string_lossy().trim().is_empty()
    {
        return PathBuf::from(shell);
    }

    let candidates = ["/bin/zsh", "/bin/bash", "/bin/sh"];
    for cand in candidates {
        let path = PathBuf::from(cand);
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("/bin/sh")
}

fn shell_command_args(shell_path: &std::path::Path, command: &str) -> Vec<String> {
    let name = shell_path
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if name.contains("zsh") || name.contains("bash") {
        return vec![
            "-l".to_owned(),
            "-i".to_owned(),
            "-c".to_owned(),
            command.to_owned(),
        ];
    }
    vec!["-c".to_owned(), command.to_owned()]
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PtyClientMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

pub async fn pty_ws_task(socket: WebSocket, session: Arc<PtySession>) {
    let (connection_id, history, mut last_seq, mut output) = session.attach();
    let (mut sender, mut receiver) = socket.split();

    for chunk in history {
        if sender.send(Message::Binary(chunk)).await.is_err() {
            session.detach(connection_id);
            return;
        }
    }

    let session_for_incoming = session.clone();
    let mut incoming_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if handle_incoming(&session_for_incoming, msg).is_err() {
                break;
            }
        }
    });

    let mut terminated_for_outgoing = session.subscribe_terminated();
    let mut outgoing_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = terminated_for_outgoing.recv() => {
                    let _ = sender
                        .send(Message::Text("{\"type\":\"exited\"}".to_owned().into()))
                        .await;
                    let _ = sender.send(Message::Close(None)).await;
                    break;
                }
                chunk = output.recv() => {
                    let Some(chunk) = chunk else { break };
                    if chunk.seq <= last_seq {
                        continue;
                    }
                    last_seq = chunk.seq;
                    if sender.send(Message::Binary(chunk.bytes)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = &mut incoming_task => {
            outgoing_task.abort();
        }
        _ = &mut outgoing_task => {
            incoming_task.abort();
        }
    }

    session.detach(connection_id);
}

fn handle_incoming(session: &PtySession, msg: Message) -> anyhow::Result<()> {
    match msg {
        Message::Text(text) => {
            let parsed: PtyClientMessage =
                serde_json::from_str(&text).context("parse pty message")?;
            match parsed {
                PtyClientMessage::Input { data } => session.write_input(data.as_bytes()),
                PtyClientMessage::Resize { cols, rows } => session.resize(cols, rows),
            }
        }
        Message::Binary(bytes) => {
            trace_bytes("pty_in_binary", &bytes);
            session.write_input(&bytes)
        }
        Message::Close(_) => Ok(()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_history_is_bounded() {
        let mut history = OutputHistory::default();

        let chunk = Bytes::from(vec![0u8; 128 * 1024]);
        for _ in 0..16 {
            history.push(chunk.clone());
        }

        assert!(history.total_bytes <= MAX_OUTPUT_HISTORY_BYTES);
        assert!(!history.chunks.is_empty());
    }
}
