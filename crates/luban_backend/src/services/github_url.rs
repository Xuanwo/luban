pub(super) fn extract_first_github_url(input: &str) -> Option<String> {
    let needle = "https://github.com/";
    let start = input.find(needle)?;
    let rest = &input[start..];
    let end = rest
        .find(|c: char| {
            c.is_whitespace() || c == '"' || c == '\'' || c == ')' || c == ']' || c == '>'
        })
        .unwrap_or(rest.len());
    let url = rest[..end].trim_end_matches('/').to_owned();
    Some(url)
}
