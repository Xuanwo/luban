use anyhow::{Context as _, anyhow};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShallowEntryKind {
    Folder,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShallowEntry {
    pub path: String,
    pub name: String,
    pub kind: ShallowEntryKind,
}

pub fn read_optional_root_shallow_entries(
    root: &Path,
    stat_context: &'static str,
    not_a_directory_label: &'static str,
) -> anyhow::Result<Vec<ShallowEntry>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let meta = std::fs::metadata(root).context(stat_context)?;
    if !meta.is_dir() {
        return Err(anyhow!(
            "{not_a_directory_label} is not a directory: {}",
            root.display()
        ));
    }

    read_shallow_entries(root)
}

fn rel_to_string(rel: &Path) -> String {
    rel.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn read_shallow_entries(root: &Path) -> anyhow::Result<Vec<ShallowEntry>> {
    let mut entries = std::fs::read_dir(root)
        .with_context(|| format!("failed to read directory {}", root.display()))?
        .filter_map(|entry| entry.ok())
        .collect::<Vec<_>>();

    entries.sort_by_key(|entry| entry.file_name());

    let mut out = Vec::new();
    for entry in entries {
        let file_type = entry.file_type().context("failed to stat entry")?;

        let name = entry.file_name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }

        let rel_child = Path::new("").join(&name);
        let path = rel_to_string(&rel_child);

        let (is_dir, is_file) = if file_type.is_symlink() {
            match std::fs::metadata(entry.path()) {
                Ok(meta) => (meta.is_dir(), meta.is_file()),
                Err(_) => (false, true),
            }
        } else {
            (file_type.is_dir(), file_type.is_file())
        };

        if is_dir {
            out.push(ShallowEntry {
                path,
                name,
                kind: ShallowEntryKind::Folder,
            });
        } else if is_file {
            out.push(ShallowEntry {
                path,
                name,
                kind: ShallowEntryKind::File,
            });
        }
    }

    out.sort_by(|a, b| match (a.kind, b.kind) {
        (ShallowEntryKind::Folder, ShallowEntryKind::File) => std::cmp::Ordering::Less,
        (ShallowEntryKind::File, ShallowEntryKind::Folder) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(out)
}
