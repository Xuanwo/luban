fn main() {
    if std::env::var_os("CI").is_some() {
        let manifest_dir = std::path::PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"),
        );
        let web_out = manifest_dir.join("../../web/out");

        std::fs::create_dir_all(&web_out).expect("create web/out for CI builds");
        std::fs::write(web_out.join(".gitkeep"), "\n").ok();
    }

    tauri_build::build()
}
