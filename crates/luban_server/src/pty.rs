use anyhow::Context as _;
use axum::extract::ws::{Message, WebSocket};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::{HashMap, VecDeque};
use std::io::{Read as _, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

type PtyKey = (u64, u64);
type PtySessions = HashMap<PtyKey, Arc<PtySession>>;

const MAX_OUTPUT_HISTORY_BYTES: usize = 512 * 1024;

#[derive(Clone)]
pub struct PtyManager {
    inner: Arc<Mutex<PtySessions>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_create(
        &self,
        workspace_id: u64,
        thread_id: u64,
        cwd: PathBuf,
    ) -> anyhow::Result<Arc<PtySession>> {
        let mut guard = self.inner.lock().expect("pty manager lock poisoned");
        if let Some(existing) = guard.get(&(workspace_id, thread_id)) {
            return Ok(existing.clone());
        }

        let session = Arc::new(PtySession::spawn(cwd)?);
        guard.insert((workspace_id, thread_id), session.clone());
        Ok(session)
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PtySession {
    output: broadcast::Sender<Vec<u8>>,
    history: Arc<Mutex<OutputHistory>>,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    _child: Mutex<Box<dyn portable_pty::Child + Send>>,
}

#[derive(Default)]
struct OutputHistory {
    chunks: VecDeque<Vec<u8>>,
    total_bytes: usize,
}

impl OutputHistory {
    fn push(&mut self, chunk: Vec<u8>) {
        self.total_bytes = self.total_bytes.saturating_add(chunk.len());
        self.chunks.push_back(chunk);
        while self.total_bytes > MAX_OUTPUT_HISTORY_BYTES {
            let Some(front) = self.chunks.pop_front() else {
                self.total_bytes = 0;
                break;
            };
            self.total_bytes = self.total_bytes.saturating_sub(front.len());
        }
    }

    fn snapshot_chunks(&self) -> Vec<Vec<u8>> {
        self.chunks.iter().cloned().collect()
    }
}

impl PtySession {
    fn spawn(cwd: PathBuf) -> anyhow::Result<Self> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("openpty failed")?;

        let shell = std::env::var_os("SHELL")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/bin/zsh"));
        let mut cmd = CommandBuilder::new(shell);
        cmd.cwd(cwd);

        let child = pair.slave.spawn_command(cmd).context("spawn pty command")?;
        let reader = pair.master.try_clone_reader().context("clone pty reader")?;
        let writer = pair.master.take_writer().context("take pty writer")?;

        let (output, _) = broadcast::channel::<Vec<u8>>(256);
        let output_for_thread = output.clone();
        let history = Arc::new(Mutex::new(OutputHistory::default()));
        let history_for_thread = history.clone();

        std::thread::Builder::new()
            .name("luban-pty-read".to_owned())
            .spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let chunk = buf[..n].to_vec();
                            if let Ok(mut guard) = history_for_thread.lock() {
                                guard.push(chunk.clone());
                            }
                            let _ = output_for_thread.send(chunk);
                        }
                        Err(_) => break,
                    }
                }
            })
            .context("spawn pty reader thread")?;

        Ok(Self {
            output,
            history,
            writer: Mutex::new(writer),
            master: Mutex::new(pair.master),
            _child: Mutex::new(child),
        })
    }

    pub fn subscribe_output(&self) -> broadcast::Receiver<Vec<u8>> {
        self.output.subscribe()
    }

    pub fn snapshot_output_history(&self) -> Vec<Vec<u8>> {
        self.history
            .lock()
            .map(|h| h.snapshot_chunks())
            .unwrap_or_default()
    }

    pub fn write_input(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().expect("pty writer lock poisoned");
        writer.write_all(bytes).context("pty write")?;
        writer.flush().ok();
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        let master = self.master.lock().expect("pty master lock poisoned");
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
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PtyClientMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

pub async fn pty_ws_task(mut socket: WebSocket, session: Arc<PtySession>) {
    let mut output = session.subscribe_output();

    for chunk in session.snapshot_output_history() {
        if socket.send(Message::Binary(chunk.into())).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(msg)) = incoming else { break };
                if handle_incoming(&session, msg).is_err() {
                    break;
                }
            }
            outgoing = output.recv() => {
                let Ok(bytes) = outgoing else { break };
                if socket.send(Message::Binary(bytes.into())).await.is_err() {
                    break;
                }
            }
        }
    }
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
        Message::Binary(bytes) => session.write_input(&bytes),
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

        let chunk = vec![0u8; 128 * 1024];
        for _ in 0..16 {
            history.push(chunk.clone());
        }

        assert!(history.total_bytes <= MAX_OUTPUT_HISTORY_BYTES);
        assert!(!history.chunks.is_empty());
    }
}
