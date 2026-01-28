use anyhow::anyhow;
use std::path::PathBuf;

pub(super) fn parse_strict_relative_list_dir_path(path: &str) -> anyhow::Result<PathBuf> {
    let rel = path.trim();
    if rel.starts_with('/') {
        return Err(anyhow!("path must be relative"));
    }
    if rel.contains('\\') {
        return Err(anyhow!("invalid path separator"));
    }

    let mut rel_path = PathBuf::new();
    if !rel.is_empty() {
        for segment in rel.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(anyhow!("invalid path segment"));
            }
            rel_path.push(segment);
        }
    }
    Ok(rel_path)
}

pub(super) fn parse_lenient_relative_list_dir_path(path: &str) -> anyhow::Result<PathBuf> {
    let rel = path.trim();
    if rel.starts_with('/') {
        return Err(anyhow!("path must be relative"));
    }
    if rel.contains('\\') {
        return Err(anyhow!("invalid path separator"));
    }

    let mut rel_path = PathBuf::new();
    for segment in rel.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." {
            return Err(anyhow!("invalid path segment"));
        }
        rel_path.push(segment);
    }
    Ok(rel_path)
}

pub(super) fn parse_strict_relative_file_path(path: &str) -> anyhow::Result<PathBuf> {
    let rel = path.trim();
    if rel.is_empty() {
        return Err(anyhow!("path is empty"));
    }
    parse_strict_relative_list_dir_path(rel)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_lenient_relative_list_dir_path, parse_strict_relative_file_path,
        parse_strict_relative_list_dir_path,
    };

    #[test]
    fn strict_accepts_empty_as_root() {
        assert_eq!(
            parse_strict_relative_list_dir_path("").unwrap().as_os_str(),
            ""
        );
        assert_eq!(
            parse_strict_relative_list_dir_path("   ")
                .unwrap()
                .as_os_str(),
            ""
        );
    }

    #[test]
    fn strict_rejects_empty_segments() {
        assert_eq!(
            parse_strict_relative_list_dir_path("a/")
                .unwrap_err()
                .to_string(),
            "invalid path segment"
        );
        assert_eq!(
            parse_strict_relative_list_dir_path("a//b")
                .unwrap_err()
                .to_string(),
            "invalid path segment"
        );
    }

    #[test]
    fn strict_rejects_dot_and_dotdot_segments() {
        assert_eq!(
            parse_strict_relative_list_dir_path(".")
                .unwrap_err()
                .to_string(),
            "invalid path segment"
        );
        assert_eq!(
            parse_strict_relative_list_dir_path("..")
                .unwrap_err()
                .to_string(),
            "invalid path segment"
        );
        assert_eq!(
            parse_strict_relative_list_dir_path("a/../b")
                .unwrap_err()
                .to_string(),
            "invalid path segment"
        );
    }

    #[test]
    fn lenient_skips_empty_segments() {
        assert_eq!(
            parse_lenient_relative_list_dir_path("a/").unwrap(),
            std::path::PathBuf::from("a")
        );
        assert_eq!(
            parse_lenient_relative_list_dir_path("a//b").unwrap(),
            std::path::PathBuf::from("a/b")
        );
    }

    #[test]
    fn both_reject_absolute_paths_and_backslashes() {
        assert_eq!(
            parse_strict_relative_list_dir_path("/a")
                .unwrap_err()
                .to_string(),
            "path must be relative"
        );
        assert_eq!(
            parse_lenient_relative_list_dir_path("/a")
                .unwrap_err()
                .to_string(),
            "path must be relative"
        );
        assert_eq!(
            parse_strict_relative_list_dir_path("a\\\\b")
                .unwrap_err()
                .to_string(),
            "invalid path separator"
        );
        assert_eq!(
            parse_lenient_relative_list_dir_path("a\\\\b")
                .unwrap_err()
                .to_string(),
            "invalid path separator"
        );
    }

    #[test]
    fn file_path_requires_non_empty() {
        assert_eq!(
            parse_strict_relative_file_path("").unwrap_err().to_string(),
            "path is empty"
        );
        assert_eq!(
            parse_strict_relative_file_path("   ")
                .unwrap_err()
                .to_string(),
            "path is empty"
        );
    }
}
