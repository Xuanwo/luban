fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let web_out_dir = manifest_dir.join("../../web/out");
    let web_out_index = web_out_dir.join("index.html");
    if !web_out_index.exists() {
        let stub = manifest_dir.join("tauri.stub.conf.json");
        println!("cargo:warning=web/out is missing; falling back to {stub:?}");
        unsafe {
            std::env::set_var("TAURI_CONFIG", stub);
        }
    }
    tauri_build::build()
}
