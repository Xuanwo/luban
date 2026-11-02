/// Reads login-shell variables (PATH + proxy vars) and applies them to the
/// current process on macOS. This keeps Tauri startup behavior aligned with
/// CLI/server entrypoints that share the same bootstrap logic.
pub fn fix_path_env() -> anyhow::Result<()> {
    luban_server::shell_env::apply_runtime_shell_env_defaults()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_path_is_noop_off_macos() {
        if cfg!(target_os = "macos") {
            return;
        }
        fix_path_env().expect("should not fail off macOS");
    }
}
