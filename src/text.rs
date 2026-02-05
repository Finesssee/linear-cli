pub fn truncate(value: &str, max_len: Option<usize>) -> String {
    let Some(max_len) = max_len else {
        return value.to_string();
    };
    if max_len == 0 {
        return String::new();
    }

    let char_count = value.chars().count();
    if char_count <= max_len {
        return value.to_string();
    }

    if max_len <= 3 {
        return value.chars().take(max_len).collect();
    }

    // Take (max_len - 3) chars and add ellipsis
    let truncated: String = value.chars().take(max_len - 3).collect();
    format!("{}...", truncated)
}

pub fn is_uuid(value: &str) -> bool {
    value.len() == 36 && value.matches("-").count() == 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_none() {
        assert_eq!(truncate("hello world", None), "hello world");
    }

    #[test]
    fn test_truncate_zero() {
        assert_eq!(truncate("hello", Some(0)), "");
    }

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hi", Some(10)), "hi");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", Some(5)), "hello");
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        assert_eq!(truncate("hello world", Some(8)), "hello...");
    }

    #[test]
    fn test_truncate_unicode() {
        // Unicode chars are counted correctly
        assert_eq!(truncate("こんにちは世界", Some(5)), "こん...");
        // "hello世界" is 7 chars, so max_len=8 doesn't truncate
        assert_eq!(truncate("hello世界", Some(8)), "hello世界");
        // But max_len=6 does truncate
        assert_eq!(truncate("hello世界", Some(6)), "hel...");
    }

    #[test]
    fn test_is_uuid_valid() {
        assert!(is_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn test_is_uuid_invalid() {
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("550e8400e29b41d4a716446655440000")); // no dashes
        assert!(!is_uuid("550e8400-e29b-41d4-a716")); // too short
        assert!(!is_uuid("")); // empty
    }
}
