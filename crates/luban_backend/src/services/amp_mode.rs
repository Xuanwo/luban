use std::path::Path;

fn parse_amp_mode_from_config_text(contents: &str) -> Option<String> {
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        let lowered = line.to_ascii_lowercase();

        let value = if let Some(rest) = lowered.strip_prefix("mode") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix('='));
            rest.map(str::trim)
        } else if let Some(rest) = lowered.strip_prefix("\"mode\"") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix(':');
            rest.map(str::trim)
        } else {
            None
        };

        let Some(value) = value else {
            continue;
        };

        let value = value
            .trim_matches('"')
            .trim_matches('\'')
            .split(|c: char| c.is_whitespace() || c == ',' || c == '#')
            .next()
            .unwrap_or("")
            .trim();
        if value.is_empty() {
            continue;
        }

        if value == "smart" || value == "rush" {
            return Some(value.to_owned());
        }
    }
    None
}

pub(super) fn detect_amp_mode_from_config_root(root: &Path) -> Option<String> {
    let candidates = [
        "config.toml",
        "config.yaml",
        "config.yml",
        "config.json",
        "amp.toml",
        "amp.yaml",
        "amp.yml",
        "amp.json",
        "settings.toml",
        "settings.yaml",
        "settings.yml",
        "settings.json",
    ];

    for rel in candidates {
        let path = root.join(rel);
        let meta = match std::fs::metadata(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !meta.is_file() {
            continue;
        }
        if meta.len() > 2 * 1024 * 1024 {
            continue;
        }
        let contents = match std::fs::read_to_string(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(mode) = parse_amp_mode_from_config_text(&contents) {
            return Some(mode);
        }
    }

    None
}
