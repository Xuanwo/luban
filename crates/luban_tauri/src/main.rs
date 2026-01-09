use anyhow::Context as _;
use std::net::SocketAddr;
use std::path::PathBuf;
use tauri::{Manager as _, WebviewUrl, WebviewWindowBuilder};

fn resolve_web_dist(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidates = [
            resource_dir.join("web").join("out"),
            resource_dir.join("out"),
            resource_dir.join("web_out"),
        ];
        for c in candidates {
            if c.join("index.html").exists() {
                return c;
            }
        }
    }

    PathBuf::from("web/out")
}

fn main() -> anyhow::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            let web_dist = resolve_web_dist(&app.handle());
            unsafe {
                std::env::set_var("LUBAN_WEB_DIST_DIR", &web_dist);
            }

            let server = tauri::async_runtime::block_on(async {
                let addr: SocketAddr = std::env::var("LUBAN_SERVER_ADDR")
                    .unwrap_or_else(|_| "127.0.0.1:8421".to_owned())
                    .parse()
                    .context("invalid LUBAN_SERVER_ADDR")?;
                luban_server::start_server(addr).await
            })
            .context("failed to start luban_server")?;

            let url: tauri::Url = format!("http://{}/", server.addr)
                .parse()
                .context("invalid server url")?;

            app.manage(server);

            let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("")
                .inner_size(1280.0, 800.0);

            #[cfg(target_os = "macos")]
            {
                builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
            }

            #[cfg(not(target_os = "macos"))]
            {
                builder = builder.decorations(false);
            }

            let window = builder.build().context("failed to build window")?;

            #[cfg(target_os = "macos")]
            {
                let w = window.clone();
                // Best-effort re-apply at runtime to avoid platform defaults winning during creation.
                tauri::async_runtime::spawn(async move {
                    let _ = w.set_title_bar_style(tauri::TitleBarStyle::Overlay);
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .context("tauri runtime failed")?;

    Ok(())
}
