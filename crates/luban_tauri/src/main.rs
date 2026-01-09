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
                let addr: SocketAddr = "127.0.0.1:0".parse().context("invalid bind addr")?;
                luban_server::start_server(addr).await
            })
            .context("failed to start luban_server")?;

            let url: tauri::Url = format!("http://{}/", server.addr)
                .parse()
                .context("invalid server url")?;

            app.manage(server);

            let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Luban")
                .inner_size(1280.0, 800.0);

            #[cfg(target_os = "macos")]
            {
                builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
            }

            #[cfg(not(target_os = "macos"))]
            {
                builder = builder.decorations(false);
            }

            builder.build().context("failed to build window")?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .context("tauri runtime failed")?;

    Ok(())
}
