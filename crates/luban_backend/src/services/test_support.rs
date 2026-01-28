use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::MutexGuard,
};

use luban_domain::AttachmentRef;

pub(super) fn lock_env() -> MutexGuard<'static, ()> {
    crate::env::lock_env_for_tests()
}

pub(super) fn temp_services_dir(unique: u128) -> PathBuf {
    std::env::temp_dir().join(format!("luban-services-{}-{}", std::process::id(), unique))
}

pub(super) fn run_git(repo_path: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .expect("git should spawn")
}

pub(super) fn assert_git_success(repo_path: &Path, args: &[&str]) {
    let output = run_git(repo_path, args);
    if !output.status.success() {
        panic!(
            "git failed ({:?}):\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

pub(super) fn git_rev_parse(repo_path: &Path, rev: &str) -> String {
    let out = run_git(repo_path, &["rev-parse", "--verify", rev]);
    assert!(
        out.status.success(),
        "git rev-parse {rev} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

pub(super) fn stored_blob_path(
    service: &super::GitWorkspaceService,
    project_slug: &str,
    workspace_name: &str,
    attachment: &AttachmentRef,
) -> PathBuf {
    service
        .context_blobs_dir(project_slug, workspace_name)
        .join(format!("{}.{}", attachment.id, attachment.extension))
}
