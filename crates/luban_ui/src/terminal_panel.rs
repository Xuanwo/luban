use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use gpui::{Entity, SharedString, Window, prelude::*};
use gpui_component::ActiveTheme as _;
use gpui_ghostty_terminal::view::{TerminalInput, TerminalView};
use gpui_ghostty_terminal::{TerminalConfig, TerminalSession, default_terminal_font};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

pub struct WorkspaceTerminal {
    view: Entity<TerminalView>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    closed: std::sync::Arc<AtomicBool>,
}

impl WorkspaceTerminal {
    pub fn view(&self) -> Entity<TerminalView> {
        self.view.clone()
    }

    pub fn resize<T>(&self, cols: u16, rows: u16, cx: &mut gpui::Context<T>) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.view
            .update(cx, |this, cx| this.resize_terminal(cols, rows, cx));
    }

    pub fn kill(&mut self) {
        self.closed.store(true, Ordering::Relaxed);
        let _ = self.killer.kill();
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }
}

pub fn spawn_workspace_terminal<T>(
    cx: &mut gpui::Context<T>,
    window: &Window,
    cwd: PathBuf,
) -> Result<WorkspaceTerminal, String>
where
    T: 'static,
{
    let theme = cx.theme();
    let default_fg = hsla_to_vt_rgb(theme.foreground);
    let default_bg = hsla_to_vt_rgb(theme.secondary);

    let config = TerminalConfig {
        default_fg,
        default_bg,
        update_window_title: false,
        ..Default::default()
    };

    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {e}"))?;

    let master: Box<dyn MasterPty + Send> = pty_pair.master;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-l");
    cmd.cwd(cwd.as_os_str());
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "luban");

    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn pty command failed: {e}"))?;

    let killer = child.clone_killer();
    thread::spawn(move || {
        let _ = child.wait();
    });

    let mut pty_reader = master
        .try_clone_reader()
        .map_err(|e| format!("pty reader failed: {e}"))?;
    let mut pty_writer = master
        .take_writer()
        .map_err(|e| format!("pty writer failed: {e}"))?;

    let (stdin_tx, stdin_rx) = mpsc::channel::<Vec<u8>>();
    let (stdout_tx, stdout_rx) = async_channel::unbounded::<Vec<u8>>();

    thread::spawn(move || {
        while let Ok(bytes) = stdin_rx.recv() {
            if pty_writer.write_all(&bytes).is_err() {
                break;
            }
            let _ = pty_writer.flush();
        }
    });

    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let n = match pty_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if stdout_tx.send_blocking(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let session = TerminalSession::new(config).map_err(|e| format!("terminal init failed: {e}"))?;
    let view = cx.new(|cx| {
        let focus_handle = cx.focus_handle();
        let input = TerminalInput::new(move |bytes| {
            let _ = stdin_tx.send(bytes.to_vec());
        });
        TerminalView::new_with_input(session, focus_handle, input)
    });

    let view_for_task = view.clone();
    window
        .spawn(cx, async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;

                let mut batch = Vec::new();
                while let Ok(chunk) = stdout_rx.try_recv() {
                    batch.extend_from_slice(&chunk);
                }
                if batch.is_empty() {
                    if stdout_rx.is_closed() {
                        break;
                    }
                    continue;
                }

                cx.update(|_, cx| {
                    view_for_task.update(cx, |this, cx| {
                        this.queue_output_bytes(&batch, cx);
                    });
                })
                .ok();
            }
        })
        .detach();

    Ok(WorkspaceTerminal {
        view,
        master,
        killer,
        closed: std::sync::Arc::new(AtomicBool::new(false)),
    })
}

pub fn terminal_cell_metrics(window: &mut Window) -> Option<(f32, f32)> {
    let mut style = window.text_style();
    let font = default_terminal_font();
    style.font_family = font.family.clone();
    style.font_features = gpui_ghostty_terminal::default_terminal_font_features();
    style.font_fallbacks = font.fallbacks.clone();

    let rem_size = window.rem_size();
    let font_size = style.font_size.to_pixels(rem_size);
    let line_height = style.line_height.to_pixels(style.font_size, rem_size);

    let run = style.to_run(1);
    let lines = window
        .text_system()
        .shape_text(SharedString::from("M"), font_size, &[run], None, Some(1))
        .ok()?;
    let line = lines.first()?;

    let cell_width = f32::from(line.width()).max(1.0);
    let cell_height = f32::from(line_height).max(1.0);
    Some((cell_width, cell_height))
}

fn hsla_to_vt_rgb(color: gpui::Hsla) -> ghostty_vt::Rgb {
    let rgba = color.to_rgb();
    let packed = u32::from(rgba);
    ghostty_vt::Rgb {
        r: ((packed >> 24) & 0xff) as u8,
        g: ((packed >> 16) & 0xff) as u8,
        b: ((packed >> 8) & 0xff) as u8,
    }
}
