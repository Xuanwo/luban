#[cfg(target_os = "linux")]
use anyhow::anyhow;
#[cfg(target_os = "linux")]
use luban_domain::OpenTarget;
#[cfg(target_os = "linux")]
use std::path::Path;

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCommand {
    pub(crate) program: &'static str,
    pub(crate) args: Vec<std::ffi::OsString>,
    pub(crate) label: &'static str,
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_open_command(
    target: OpenTarget,
    worktree_path: &Path,
) -> anyhow::Result<OpenCommand> {
    let mut args = Vec::new();
    let (program, label) = match target {
        OpenTarget::Vscode => {
            args.push(worktree_path.as_os_str().to_os_string());
            ("code", "code")
        }
        OpenTarget::Zed => {
            args.push(worktree_path.as_os_str().to_os_string());
            ("zed", "zed")
        }
        OpenTarget::Finder => {
            args.push(worktree_path.as_os_str().to_os_string());
            ("xdg-open", "xdg-open")
        }
        OpenTarget::Cursor => {
            return Err(anyhow!("opening Cursor is not supported on Linux"));
        }
        OpenTarget::Ghostty => {
            return Err(anyhow!("opening Ghostty is not supported on Linux"));
        }
    };

    Ok(OpenCommand {
        program,
        args,
        label,
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::linux_open_command;
    use luban_domain::OpenTarget;
    use std::path::Path;

    #[test]
    fn linux_open_command_uses_code_for_vscode() {
        let worktree_path = Path::new("/tmp/luban-worktree");
        let command =
            linux_open_command(OpenTarget::Vscode, worktree_path).expect("vscode open command");
        assert_eq!(command.program, "code");
        assert_eq!(command.label, "code");
        assert_eq!(
            command.args,
            vec![std::ffi::OsString::from("/tmp/luban-worktree")]
        );
    }

    #[test]
    fn linux_open_command_uses_zed_for_zed() {
        let worktree_path = Path::new("/tmp/luban-worktree");
        let command = linux_open_command(OpenTarget::Zed, worktree_path).expect("zed open command");
        assert_eq!(command.program, "zed");
        assert_eq!(command.label, "zed");
        assert_eq!(
            command.args,
            vec![std::ffi::OsString::from("/tmp/luban-worktree")]
        );
    }
}
