use std::path::{Path, PathBuf};

pub const LUBAN_CODEX_BIN_ENV: &str = "LUBAN_CODEX_BIN";
pub const LUBAN_CODEX_ROOT_ENV: &str = "LUBAN_CODEX_ROOT";
pub const LUBAN_AMP_ROOT_ENV: &str = "LUBAN_AMP_ROOT";
pub const LUBAN_CLAUDE_BIN_ENV: &str = "LUBAN_CLAUDE_BIN";
pub const LUBAN_CLAUDE_ROOT_ENV: &str = "LUBAN_CLAUDE_ROOT";
pub const LUBAN_ROOT_ENV: &str = "LUBAN_ROOT";

pub fn worktrees_root(luban_root: &Path) -> PathBuf {
    luban_root.join("worktrees")
}

pub fn projects_root(luban_root: &Path) -> PathBuf {
    luban_root.join("projects")
}

pub fn conversations_root(luban_root: &Path) -> PathBuf {
    luban_root.join("conversations")
}

pub fn sqlite_path(luban_root: &Path) -> PathBuf {
    luban_root.join("luban.db")
}

pub fn task_prompts_root(luban_root: &Path) -> PathBuf {
    luban_root.join("task")
}

pub fn workspace_conversation_dir(
    conversations_root: &Path,
    project_slug: &str,
    workspace_name: &str,
) -> PathBuf {
    conversations_root.join(project_slug).join(workspace_name)
}

pub fn workspace_context_dir(
    conversations_root: &Path,
    project_slug: &str,
    workspace_name: &str,
) -> PathBuf {
    workspace_conversation_dir(conversations_root, project_slug, workspace_name).join("context")
}

pub(crate) fn normalize_project_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = out.pop();
                if !popped {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_join_to_expected_subdirs() {
        let base = PathBuf::from("luban-root");
        assert_eq!(worktrees_root(&base), base.join("worktrees"));
        assert_eq!(projects_root(&base), base.join("projects"));
        assert_eq!(conversations_root(&base), base.join("conversations"));
        assert_eq!(sqlite_path(&base), base.join("luban.db"));
        assert_eq!(task_prompts_root(&base), base.join("task"));
        assert_eq!(LUBAN_CODEX_BIN_ENV, "LUBAN_CODEX_BIN");
        assert_eq!(LUBAN_CODEX_ROOT_ENV, "LUBAN_CODEX_ROOT");
        assert_eq!(LUBAN_AMP_ROOT_ENV, "LUBAN_AMP_ROOT");
        assert_eq!(LUBAN_CLAUDE_BIN_ENV, "LUBAN_CLAUDE_BIN");
        assert_eq!(LUBAN_CLAUDE_ROOT_ENV, "LUBAN_CLAUDE_ROOT");
        assert_eq!(LUBAN_ROOT_ENV, "LUBAN_ROOT");
    }

    #[test]
    fn workspace_context_dir_is_nested_under_conversations() {
        let conversations = PathBuf::from("luban-root").join("conversations");
        assert_eq!(
            workspace_conversation_dir(&conversations, "proj", "ws"),
            conversations.join("proj").join("ws")
        );
        assert_eq!(
            workspace_context_dir(&conversations, "proj", "ws"),
            conversations.join("proj").join("ws").join("context")
        );
    }

    #[test]
    fn normalize_project_path_removes_curdir_components() {
        let path = PathBuf::from("a").join(".").join("b");
        assert_eq!(normalize_project_path(&path), PathBuf::from("a").join("b"));
    }

    #[test]
    fn normalize_project_path_simplifies_parentdirs_when_possible() {
        let path = PathBuf::from("a").join("b").join("..").join("c");
        assert_eq!(normalize_project_path(&path), PathBuf::from("a").join("c"));
    }

    #[test]
    fn normalize_project_path_preserves_leading_parentdirs() {
        let path = PathBuf::from("..").join("a");
        assert_eq!(normalize_project_path(&path), path);
    }
}
