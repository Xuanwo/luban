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
            let handle = app.handle();
            let web_dist = resolve_web_dist(handle);
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

            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Luban")
                .inner_size(1280.0, 800.0)
                .build()
                .context("failed to build window")?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .context("tauri runtime failed")?;

    Ok(())
}
