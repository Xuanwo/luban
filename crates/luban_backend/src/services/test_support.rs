use std::{
    path::Path,
    process::{Command, Output},
    sync::MutexGuard,
};

pub(super) fn lock_env() -> MutexGuard<'static, ()> {
    crate::env::lock_env_for_tests()
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
