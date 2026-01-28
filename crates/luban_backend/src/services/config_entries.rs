use super::config_tree;
use luban_domain::{
    ClaudeConfigEntry, ClaudeConfigEntryKind, CodexConfigEntry, CodexConfigEntryKind,
};

pub(super) fn codex_entries_from_shallow(
    entries: Vec<config_tree::ShallowEntry>,
) -> Vec<CodexConfigEntry> {
    entries
        .into_iter()
        .map(|entry| CodexConfigEntry {
            path: entry.path,
            name: entry.name,
            kind: match entry.kind {
                config_tree::ShallowEntryKind::Folder => CodexConfigEntryKind::Folder,
                config_tree::ShallowEntryKind::File => CodexConfigEntryKind::File,
            },
            children: Vec::new(),
        })
        .collect()
}

pub(super) fn amp_entries_from_shallow(
    entries: Vec<config_tree::ShallowEntry>,
) -> Vec<luban_domain::AmpConfigEntry> {
    entries
        .into_iter()
        .map(|entry| luban_domain::AmpConfigEntry {
            path: entry.path,
            name: entry.name,
            kind: match entry.kind {
                config_tree::ShallowEntryKind::Folder => luban_domain::AmpConfigEntryKind::Folder,
                config_tree::ShallowEntryKind::File => luban_domain::AmpConfigEntryKind::File,
            },
            children: Vec::new(),
        })
        .collect()
}

pub(super) fn claude_entries_from_shallow(
    entries: Vec<config_tree::ShallowEntry>,
) -> Vec<ClaudeConfigEntry> {
    entries
        .into_iter()
        .map(|entry| ClaudeConfigEntry {
            path: entry.path,
            name: entry.name,
            kind: match entry.kind {
                config_tree::ShallowEntryKind::Folder => ClaudeConfigEntryKind::Folder,
                config_tree::ShallowEntryKind::File => ClaudeConfigEntryKind::File,
            },
            children: Vec::new(),
        })
        .collect()
}
