fn contains_attempt_fraction(message: &str) -> bool {
    let bytes = message.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }

        if i >= bytes.len() || bytes[i] != b'/' {
            continue;
        }
        i += 1;

        if i < bytes.len() && bytes[i].is_ascii_digit() {
            return true;
        }
    }

    false
}

fn contains_reconnecting_case_insensitive(message: &str) -> bool {
    const NEEDLE: &[u8] = b"reconnecting";

    let haystack = message.as_bytes();
    if haystack.len() < NEEDLE.len() {
        return false;
    }

    for start in 0..=haystack.len() - NEEDLE.len() {
        let mut matches = true;
        for (offset, expected) in NEEDLE.iter().copied().enumerate() {
            if haystack[start + offset].to_ascii_lowercase() != expected {
                matches = false;
                break;
            }
        }
        if matches {
            return true;
        }
    }

    false
}

pub(super) fn is_transient_reconnect_notice(message: &str) -> bool {
    let message = message.trim();
    if message.is_empty() {
        return false;
    }

    if !contains_reconnecting_case_insensitive(message) {
        return false;
    }

    contains_attempt_fraction(message)
}

#[cfg(test)]
mod tests {
    use super::{contains_attempt_fraction, contains_reconnecting_case_insensitive};

    #[test]
    fn attempt_fraction_requires_digits_slash_digits() {
        assert!(!contains_attempt_fraction(""));
        assert!(!contains_attempt_fraction("no digits here"));
        assert!(!contains_attempt_fraction("1/"));
        assert!(!contains_attempt_fraction("/1"));
        assert!(contains_attempt_fraction("1/2"));
        assert!(contains_attempt_fraction("12/345"));
        assert!(contains_attempt_fraction("prefix 12/345 suffix"));
    }

    #[test]
    fn reconnecting_detection_is_ascii_case_insensitive() {
        assert!(contains_reconnecting_case_insensitive("reconnecting"));
        assert!(contains_reconnecting_case_insensitive("Reconnecting"));
        assert!(contains_reconnecting_case_insensitive("RECONNECTING"));
        assert!(contains_reconnecting_case_insensitive("x reconnecting y"));
        assert!(!contains_reconnecting_case_insensitive("reconnect"));
        assert!(!contains_reconnecting_case_insensitive("connecting"));
    }
}
