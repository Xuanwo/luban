fn contains_attempt_fraction(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if !ch.is_ascii_digit() {
            continue;
        }

        while matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
            let _ = chars.next();
        }

        if !matches!(chars.peek(), Some('/')) {
            continue;
        }
        let _ = chars.next();

        if !matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
            continue;
        }
        return true;
    }

    false
}

pub(super) fn is_transient_reconnect_notice(message: &str) -> bool {
    let message = message.trim();
    if message.is_empty() {
        return false;
    }

    let lower = message.to_ascii_lowercase();
    if !lower.contains("reconnecting") {
        return false;
    }

    contains_attempt_fraction(&lower)
}
