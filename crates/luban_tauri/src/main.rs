use anyhow::Context as _;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager as _, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_updater::UpdaterExt as _;

#[cfg(target_os = "macos")]
mod macos_process_name;
mod path_env;

static UPDATE_CHECK_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

const MENU_ID_CHECK_FOR_UPDATES: &str = "check_for_updates";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildChannel {
    Dev,
    Release,
}

fn parse_build_channel(raw: &str) -> BuildChannel {
    match raw.trim().to_ascii_lowercase().as_str() {
        "release" => BuildChannel::Release,
        _ => BuildChannel::Dev,
    }
}

fn build_channel() -> BuildChannel {
    if let Ok(raw) = std::env::var("LUBAN_BUILD_CHANNEL")
        && !raw.trim().is_empty()
    {
        return parse_build_channel(&raw);
    }
    if let Some(raw) = option_env!("LUBAN_BUILD_CHANNEL")
        && !raw.trim().is_empty()
    {
        return parse_build_channel(raw);
    }
    BuildChannel::Dev
}

fn auto_update_enabled() -> bool {
    matches!(build_channel(), BuildChannel::Release)
}

#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("unsupported url scheme".to_owned());
    }
    open::that(url).map_err(|e| e.to_string())?;
    Ok(())
}

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

fn resolve_server_addr() -> anyhow::Result<SocketAddr> {
    // Use a random available port by default.
    //
    // This avoids launch failures when another Luban instance is already running
    // (common during development and when testing release bundles).
    let default_addr = "127.0.0.1:0";
    let addr: SocketAddr = std::env::var("LUBAN_SERVER_ADDR")
        .unwrap_or_else(|_| default_addr.to_owned())
        .parse()
        .context("invalid LUBAN_SERVER_ADDR")?;
    Ok(addr)
}

fn webview_devtools_enabled() -> bool {
    // Disable the built-in WebView devtools context menu in Luban's desktop app.
    //
    // This removes the "Reload" item from the right-click menu, avoiding accidental
    // reloads that can disrupt long-running tasks.
    false
}

fn build_app_menu<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
) -> anyhow::Result<tauri::menu::Menu<R>> {
    let menu_builder = tauri::menu::MenuBuilder::new(manager).item(&{
        let app_submenu_builder = tauri::menu::SubmenuBuilder::new(manager, "Luban")
            .text(MENU_ID_CHECK_FOR_UPDATES, "Check for Updates...")
            .separator()
            .about(None);

        #[cfg(target_os = "macos")]
        let app_submenu_builder = app_submenu_builder
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others();

        let app_submenu_builder = app_submenu_builder.separator().quit();

        app_submenu_builder.build().context("build app submenu")?
    });

    let menu_builder = menu_builder.item(&{
        let edit_submenu_builder = tauri::menu::SubmenuBuilder::new(manager, "Edit");

        #[cfg(target_os = "macos")]
        let edit_submenu_builder = edit_submenu_builder.undo().redo().separator();

        edit_submenu_builder
            .cut()
            .copy()
            .paste()
            .separator()
            .select_all()
            .build()
            .context("build edit submenu")?
    });

    menu_builder.build().context("build app menu")
}

fn install_app_menu(app: &tauri::App) -> anyhow::Result<()> {
    let handle = app.handle();
    let menu = build_app_menu(handle)?;
    app.set_menu(menu).context("set app menu")?;
    Ok(())
}

fn dialog_ok(
    app: &tauri::AppHandle,
    title: &str,
    description: &str,
    level: rfd::MessageLevel,
) -> anyhow::Result<()> {
    use std::sync::mpsc::channel;
    let (tx, rx) = channel();
    let title = title.to_owned();
    let description = description.to_owned();
    app.run_on_main_thread(move || {
        let _ = rfd::MessageDialog::new()
            .set_level(level)
            .set_title(title)
            .set_description(description)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        let _ = tx.send(());
    })
    .context("run dialog on main thread")?;
    let _ = rx.recv();
    Ok(())
}

fn dialog_yes_no(
    app: &tauri::AppHandle,
    title: &str,
    description: &str,
    level: rfd::MessageLevel,
) -> anyhow::Result<bool> {
    use std::sync::mpsc::channel;
    let (tx, rx) = channel();
    let title = title.to_owned();
    let description = description.to_owned();
    app.run_on_main_thread(move || {
        let result = rfd::MessageDialog::new()
            .set_level(level)
            .set_title(title)
            .set_description(description)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        let _ = tx.send(matches!(result, rfd::MessageDialogResult::Yes));
    })
    .context("run dialog on main thread")?;
    Ok(rx.recv().unwrap_or(false))
}

async fn check_for_updates(app: tauri::AppHandle, user_initiated: bool) -> anyhow::Result<()> {
    let updater = app.updater_builder().build().context("build updater")?;
    let update = updater.check().await.context("check update")?;

    let Some(update) = update else {
        if user_initiated {
            dialog_ok(
                &app,
                "Luban",
                "No updates available.",
                rfd::MessageLevel::Info,
            )?;
        }
        return Ok(());
    };

    let version = update.version.clone();
    let should_install = dialog_yes_no(
        &app,
        "Luban",
        &format!("A new version is available: {version}\n\nInstall now?"),
        rfd::MessageLevel::Info,
    )?;
    if !should_install {
        return Ok(());
    }

    update
        .download_and_install(
            |_downloaded, _total| {},
            || {
                eprintln!("updater: download finished");
            },
        )
        .await
        .context("download and install update")?;

    let should_restart = dialog_yes_no(
        &app,
        "Luban",
        "Update installed.\n\nRestart Luban now?",
        rfd::MessageLevel::Info,
    )?;
    if should_restart {
        app.request_restart();
    }

    Ok(())
}

async fn run_update_flow(app: tauri::AppHandle, user_initiated: bool) {
    if !user_initiated && !auto_update_enabled() {
        return;
    }
    if UPDATE_CHECK_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        if user_initiated {
            let _ = dialog_ok(
                &app,
                "Luban",
                "Update check is already in progress.",
                rfd::MessageLevel::Info,
            );
        }
        return;
    }

    let result = check_for_updates(app.clone(), user_initiated).await;
    UPDATE_CHECK_IN_PROGRESS.store(false, Ordering::SeqCst);

    if let Err(err) = result {
        if user_initiated {
            let _ = dialog_ok(
                &app,
                "Luban",
                &format!("Failed to check for updates:\n{err:#}"),
                rfd::MessageLevel::Error,
            );
        } else {
            eprintln!("updater: failed to check for updates: {err:#}");
        }
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    macos_process_name::set_process_name("Luban");

    tauri::Builder::default()
        .on_menu_event(|app, event| {
            if event.id() == MENU_ID_CHECK_FOR_UPDATES {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    run_update_flow(handle, true).await;
                });
            }
        })
        .invoke_handler(tauri::generate_handler![open_external])
        .setup(|app| {
            let _ = path_env::fix_path_env();
            let handle = app.handle();
            handle
                .plugin(tauri_plugin_updater::Builder::new().build())
                .context("register updater plugin")?;
            install_app_menu(app)?;

            let web_dist = resolve_web_dist(handle);
            unsafe {
                std::env::set_var("LUBAN_WEB_DIST_DIR", &web_dist);
            }

            let server = tauri::async_runtime::block_on(async {
                let addr = resolve_server_addr()?;
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
                .devtools(webview_devtools_enabled())
                .build()
                .context("failed to build window")?;

            if auto_update_enabled() {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    run_update_flow(handle, false).await;
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .context("tauri runtime failed")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `muda` requires creating menu items on the macOS main thread.
    // Rust's test harness runs `#[test]` functions on worker threads, so these
    // menu-structure assertions are only enabled off macOS.
    #[cfg(not(target_os = "macos"))]
    fn find_submenu<R: tauri::Runtime>(
        menu: &tauri::menu::Menu<R>,
        name: &str,
    ) -> tauri::menu::Submenu<R> {
        menu.items()
            .expect("menu items must be available")
            .into_iter()
            .filter_map(|item| match item {
                tauri::menu::MenuItemKind::Submenu(submenu) => Some(submenu),
                _ => None,
            })
            .find(|submenu| match submenu.text() {
                Ok(text) => text == name,
                Err(_) => false,
            })
            .unwrap_or_else(|| panic!("submenu {name} must exist"))
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn app_menu_includes_edit_submenu_with_clipboard_items() {
        fn normalize_menu_label(text: &str) -> String {
            let text = text.split('\t').next().unwrap_or(text).trim();
            text.chars()
                .filter(|c| *c != '_' && *c != '&')
                .collect::<String>()
        }

        let app = tauri::test::mock_app();
        let menu = build_app_menu(app.handle()).expect("app menu must build");

        let edit = find_submenu(&menu, "Edit");
        let item_texts: Vec<String> = edit
            .items()
            .expect("edit submenu items must be available")
            .into_iter()
            .filter_map(|item| match item {
                // Depending on platform/runtime, `cut/copy/paste/select_all` may be surfaced as
                // predefined menu items or regular menu items. Normalize the label to keep this
                // assertion stable across targets.
                tauri::menu::MenuItemKind::Predefined(predefined) => predefined.text().ok(),
                tauri::menu::MenuItemKind::MenuItem(menu_item) => menu_item.text().ok(),
                _ => None,
            })
            .map(|text| normalize_menu_label(&text))
            .collect();

        for text in ["Cut", "Copy", "Paste", "Select All"] {
            assert!(
                item_texts.iter().any(|t| t == text),
                "edit submenu must include {text}; got: {item_texts:?}"
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn app_menu_includes_update_check_item() {
        let app = tauri::test::mock_app();
        let menu = build_app_menu(app.handle()).expect("app menu must build");

        let luban = find_submenu(&menu, "Luban");
        let menu_item_texts: Vec<String> = luban
            .items()
            .expect("app submenu items must be available")
            .into_iter()
            .filter_map(|item| match item {
                tauri::menu::MenuItemKind::MenuItem(menu_item) => menu_item.text().ok(),
                _ => None,
            })
            .collect();

        assert!(
            menu_item_texts.iter().any(|t| t == "Check for Updates..."),
            "app submenu must include the update check item"
        );
    }

    #[test]
    fn server_addr_defaults_to_random_port() {
        let prev = std::env::var_os("LUBAN_SERVER_ADDR");
        unsafe {
            std::env::remove_var("LUBAN_SERVER_ADDR");
        }
        let addr = resolve_server_addr().expect("default addr must parse");
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 0);
        if let Some(value) = prev {
            unsafe {
                std::env::set_var("LUBAN_SERVER_ADDR", value);
            }
        }
    }

    #[test]
    fn tauri_capability_allows_any_localhost_port() {
        let contents = include_str!("../capabilities/main.json");
        assert!(
            contents.contains("http://127.0.0.1:*/*"),
            "capability must allow 127.0.0.1 on any port"
        );
        assert!(
            contents.contains("http://localhost:*/*"),
            "capability must allow localhost on any port"
        );
    }

    #[test]
    fn desktop_webview_devtools_are_disabled() {
        assert!(!webview_devtools_enabled());
    }

    #[test]
    fn updater_plugin_is_configured() {
        let contents = include_str!("../tauri.conf.json");
        assert!(
            contents.contains("\"updater\""),
            "tauri.conf.json must configure the updater plugin"
        );
        assert!(
            contents.contains("latest.json"),
            "updater endpoints must point to latest.json"
        );
        assert!(
            contents.contains("\"pubkey\""),
            "updater must include a public key"
        );
    }

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_var<T>(key: &str, value: &str, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        let out = f();
        match prev {
            Some(prev) => unsafe {
                std::env::set_var(key, prev);
            },
            None => unsafe {
                std::env::remove_var(key);
            },
        }
        out
    }

    #[test]
    fn auto_update_is_disabled_on_dev_channel() {
        with_env_var("LUBAN_BUILD_CHANNEL", "dev", || {
            assert!(!auto_update_enabled());
        });
    }

    #[test]
    fn auto_update_is_enabled_on_release_channel() {
        with_env_var("LUBAN_BUILD_CHANNEL", "release", || {
            assert!(auto_update_enabled());
        });
    }
}
