use anyhow::{Context as _, anyhow};
use std::path::Path;

const MAX_EDITABLE_FILE_SIZE_BYTES: u64 = 2 * 1024 * 1024;

pub fn read_small_utf8_file(abs: &Path) -> anyhow::Result<String> {
    let meta =
        std::fs::metadata(abs).with_context(|| format!("failed to stat {}", abs.display()))?;
    if !meta.is_file() {
        return Err(anyhow!("not a file: {}", abs.display()));
    }
    if meta.len() > MAX_EDITABLE_FILE_SIZE_BYTES {
        return Err(anyhow!("file is too large to edit"));
    }

    let bytes = std::fs::read(abs).with_context(|| format!("failed to read {}", abs.display()))?;
    let text = String::from_utf8(bytes).context("file is not valid UTF-8")?;
    Ok(text)
}

pub fn write_file_creating_parent_dirs(abs: &Path, contents: &str) -> anyhow::Result<()> {
    let parent = abs
        .parent()
        .ok_or_else(|| anyhow!("invalid path"))?
        .to_path_buf();
    std::fs::create_dir_all(&parent)
        .with_context(|| format!("failed to create directory {}", parent.display()))?;

    std::fs::write(abs, contents.as_bytes())
        .with_context(|| format!("failed to write {}", abs.display()))?;
    Ok(())
}
