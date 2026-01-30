use std::fs;
use std::path::PathBuf;

fn read_repo_file(path: &str) -> String {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // dev/
    root.push(path);
    fs::read_to_string(&root).expect("read repo file")
}

fn slice_between<'a>(haystack: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = haystack.find(start).expect("find start marker");
    let rest = &haystack[start_idx..];
    let end_idx = rest.find(end).expect("find end marker");
    &rest[..end_idx]
}

#[test]
fn mock_task_preview_uses_local_path_project_spec() {
    let content = read_repo_file("web/lib/mock/mock-runtime.ts");
    assert!(
        content.contains("project: { type: \"local_path\""),
        "task_preview should produce a local_path project spec in mock mode"
    );
}

#[test]
fn mock_task_execute_requires_workdir_id_and_creates_task() {
    let content = read_repo_file("web/lib/mock/mock-runtime.ts");
    let block = slice_between(
        &content,
        "if (action.type === \"task_execute\") {",
        "if (action.type === \"feedback_submit\") {",
    );
    assert!(
        block.contains("task_execute requires workdir_id"),
        "task_execute should require workdir_id in mock mode"
    );
    assert!(
        block.contains("createTaskInWorkdir"),
        "task_execute should create a task in the selected workdir in mock mode"
    );
}
