use std::{ops::Range, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextTokenKind {
    Image,
    Text,
    File,
}

impl ContextTokenKind {
    fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("image") {
            return Some(Self::Image);
        }
        if raw.eq_ignore_ascii_case("text") {
            return Some(Self::Text);
        }
        if raw.eq_ignore_ascii_case("file") {
            return Some(Self::File);
        }
        None
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Text => "text",
            Self::File => "file",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextToken {
    pub kind: ContextTokenKind,
    pub path: PathBuf,
    pub range: Range<usize>,
}

const PREFIX: &str = "<<context:";
const SUFFIX: &str = ">>>";

fn scan_context_tokens(text: &str, mut on_token: impl FnMut(ContextTokenKind, &str, Range<usize>)) {
    let mut cursor = 0usize;

    while let Some(rel_start) = text[cursor..].find(PREFIX) {
        let start = cursor + rel_start;
        let after_prefix = start + PREFIX.len();

        let Some(kind_sep_rel) = text[after_prefix..].find(':') else {
            cursor = after_prefix;
            continue;
        };
        let kind_end = after_prefix + kind_sep_rel;
        let kind = &text[after_prefix..kind_end];
        let Some(kind) = ContextTokenKind::parse(kind) else {
            cursor = kind_end + 1;
            continue;
        };

        let path_start = kind_end + 1;
        let Some(suffix_rel) = text[path_start..].find(SUFFIX) else {
            cursor = path_start;
            continue;
        };
        let end = path_start + suffix_rel + SUFFIX.len();
        let path = text[path_start..(path_start + suffix_rel)].trim();
        if path.is_empty() {
            cursor = end;
            continue;
        }

        on_token(kind, path, start..end);
        cursor = end;
    }
}

pub fn find_context_tokens(text: &str) -> Vec<ContextToken> {
    let mut out = Vec::new();
    scan_context_tokens(text, |kind, path, range| {
        out.push(ContextToken {
            kind,
            path: PathBuf::from(path),
            range,
        });
    });

    out
}

pub fn extract_context_image_paths_in_order(text: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    scan_context_tokens(text, |kind, path, _| {
        if kind == ContextTokenKind::Image {
            out.push(PathBuf::from(path));
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_token_parsing_is_tolerant_and_ordered() {
        let text = concat!(
            "hello\n",
            "<<context:image:/a.png>>>\n",
            "world\n",
            "<<context:text:/b.txt>>>\n",
            "<<context:image:/c.jpg>>>\n"
        );
        let tokens = find_context_tokens(text);
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, ContextTokenKind::Image);
        assert_eq!(tokens[0].path, PathBuf::from("/a.png"));
        assert_eq!(tokens[1].kind, ContextTokenKind::Text);
        assert_eq!(tokens[2].path, PathBuf::from("/c.jpg"));

        let images = extract_context_image_paths_in_order(text);
        assert_eq!(
            images,
            vec![PathBuf::from("/a.png"), PathBuf::from("/c.jpg")]
        );
    }

    #[test]
    fn unknown_kinds_are_ignored() {
        let text = "<<context:unknown:/x>>>";
        assert!(find_context_tokens(text).is_empty());
    }

    #[test]
    fn context_token_parsing_trims_kind_and_path() {
        let text = "<<context:  ImAgE  :  /a.png  >>>";
        let tokens = find_context_tokens(text);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, ContextTokenKind::Image);
        assert_eq!(tokens[0].path, PathBuf::from("/a.png"));
    }

    #[test]
    fn context_token_ranges_cover_full_token() {
        let text = "x <<context:image:/a.png>>> y";
        let tokens = find_context_tokens(text);
        assert_eq!(tokens.len(), 1);
        let start = text.find("<<context:").expect("missing token start");
        let expected = "<<context:image:/a.png>>>";
        let end = start + expected.len();
        assert_eq!(tokens[0].range, start..end);
        assert_eq!(&text[tokens[0].range.clone()], expected);
    }
}
