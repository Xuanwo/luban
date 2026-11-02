use anyhow::Context as _;
use std::path::PathBuf;
use std::process::Command;

const SHELL_ENV_DELIMITER: &str = "_SHELL_ENV_DELIMITER_";

/// Environment keys needed by runtime subprocesses that are commonly missing
/// when macOS apps are launched outside an interactive shell.
pub const RUNTIME_SHELL_ENV_VARS: &[&str] = &[
    "PATH",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
];

fn default_shell() -> &'static str {
    if cfg!(target_os = "macos") {
        "/bin/zsh"
    } else {
        "/bin/sh"
    }
}

fn shell_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Applies runtime shell environment defaults (PATH + proxy vars).
pub fn apply_runtime_shell_env_defaults() -> anyhow::Result<()> {
    apply_user_shell_env_vars(RUNTIME_SHELL_ENV_VARS)
}

/// Reads a login shell environment and applies selected variables.
///
/// This is a macOS-specific compatibility path: apps launched from Finder/Dock
/// can miss shell environment customizations required by CLI subprocesses.
pub fn apply_user_shell_env_vars(vars: &[&str]) -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| default_shell().to_owned());
    let mut cmd = Command::new(shell);
    cmd.arg("-ilc")
        .arg(format!(
            "echo -n \"{SHELL_ENV_DELIMITER}\"; env; echo -n \"{SHELL_ENV_DELIMITER}\"; exit"
        ))
        // Oh My Zsh auto-update can block startup in interactive shells.
        .env("DISABLE_AUTO_UPDATE", "true");

    if let Some(home) = shell_home_dir() {
        cmd.current_dir(home);
    }

    let output = cmd.output().context("failed to run login shell")?;
    if !output.status.success() {
        anyhow::bail!(
            "login shell exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let env_block = stdout
        .split(SHELL_ENV_DELIMITER)
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("invalid shell env output"))?;

    for line in env_block.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.splitn(2, '=');
        let Some(key) = parts.next() else { continue };
        let Some(value) = parts.next() else { continue };
        if vars.is_empty() || vars.contains(&key) {
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn apply_shell_env_is_noop_off_macos() {
        if cfg!(target_os = "macos") {
            return;
        }
        apply_runtime_shell_env_defaults().expect("should not fail off macOS");
    }

    #[test]
    fn apply_shell_env_reads_path_and_proxy_vars() {
        if !cfg!(target_os = "macos") {
            return;
        }

        let _guard = ENV_LOCK.lock().expect("env lock poisoned");

        let dir = std::env::temp_dir().join(format!(
            "luban-shell-env-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let shell = dir.join("fake-shell");
        let expected_path = "/opt/test/bin:/usr/bin";
        let expected_proxy = "http://127.0.0.1:7890";
        let script = format!(
            "#!/bin/sh\n\
echo -n \"{d}\"\n\
echo \"PATH={p}\"\n\
echo \"HTTPS_PROXY={proxy}\"\n\
echo \"NO_PROXY=localhost,127.0.0.1\"\n\
echo -n \"{d}\"\n",
            d = SHELL_ENV_DELIMITER,
            p = expected_path,
            proxy = expected_proxy,
        );
        std::fs::write(&shell, script).expect("write fake shell");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&shell).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&shell, perms).unwrap();
        }

        let prev_shell = std::env::var_os("SHELL");
        let prev_path = std::env::var_os("PATH");
        let prev_https_proxy = std::env::var_os("HTTPS_PROXY");

        unsafe {
            std::env::set_var("SHELL", shell.as_os_str());
            std::env::set_var("PATH", "/usr/bin");
            std::env::set_var("HTTPS_PROXY", "");
        }

        apply_runtime_shell_env_defaults().expect("apply env must succeed");

        assert_eq!(std::env::var("PATH").unwrap(), expected_path);
        assert_eq!(std::env::var("HTTPS_PROXY").unwrap(), expected_proxy);

        unsafe {
            if let Some(value) = prev_shell {
                std::env::set_var("SHELL", value);
            } else {
                std::env::remove_var("SHELL");
            }
            if let Some(value) = prev_path {
                std::env::set_var("PATH", value);
            } else {
                std::env::remove_var("PATH");
            }
            if let Some(value) = prev_https_proxy {
                std::env::set_var("HTTPS_PROXY", value);
            } else {
                std::env::remove_var("HTTPS_PROXY");
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
