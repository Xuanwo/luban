use std::path::Path;

fn strip_prefix_ascii_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    s.get(prefix.len()..)
}

fn parse_amp_mode_from_config_text(contents: &str) -> Option<String> {
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        let value = if let Some(rest) = strip_prefix_ascii_case(line, "mode") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix('='));
            rest.map(str::trim)
        } else if let Some(rest) = strip_prefix_ascii_case(line, "\"mode\"") {
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

        if value.eq_ignore_ascii_case("smart") {
            return Some("smart".to_owned());
        }
        if value.eq_ignore_ascii_case("rush") {
            return Some("rush".to_owned());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_prefix_ascii_case_handles_short_strings() {
        assert_eq!(strip_prefix_ascii_case("", "mode"), None);
        assert_eq!(strip_prefix_ascii_case("mo", "mode"), None);
        assert_eq!(strip_prefix_ascii_case("MODE", "mode"), Some(""));
    }

    #[test]
    fn parse_amp_mode_accepts_yaml_like_syntax() {
        assert_eq!(
            parse_amp_mode_from_config_text("mode: smart\n"),
            Some("smart".to_owned())
        );
        assert_eq!(
            parse_amp_mode_from_config_text("Mode: SMART\n"),
            Some("smart".to_owned())
        );
        assert_eq!(
            parse_amp_mode_from_config_text("mode = rush\n"),
            Some("rush".to_owned())
        );
    }

    #[test]
    fn parse_amp_mode_accepts_json_like_syntax() {
        assert_eq!(
            parse_amp_mode_from_config_text("\"mode\": \"rush\""),
            Some("rush".to_owned())
        );
        assert_eq!(
            parse_amp_mode_from_config_text("\"Mode\": \"RUSH\""),
            Some("rush".to_owned())
        );
    }

    #[test]
    fn parse_amp_mode_ignores_comments_and_noise() {
        let contents = r#"
            # mode: smart
            // mode: rush
            something_else: smart
            mode:
            mode: ""
        "#;
        assert_eq!(parse_amp_mode_from_config_text(contents), None);
    }
}
