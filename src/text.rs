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

/// Strip common markdown formatting for terminal display.
/// Converts headers, bold, italic, links, images, code blocks, etc. to plain text.
pub fn strip_markdown(input: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static PATTERNS: OnceLock<Vec<(Regex, &str)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // Images: ![alt](url) -> alt
            (Regex::new(r"!\[([^\]]*)\]\([^)]+\)").unwrap(), "$1"),
            // Links: [text](url) -> text
            (Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap(), "$1"),
            // Bold/italic: ***text*** or ___text___
            (Regex::new(r"\*{3}([^*]+)\*{3}").unwrap(), "$1"),
            (Regex::new(r"_{3}([^_]+)_{3}").unwrap(), "$1"),
            // Bold: **text** or __text__
            (Regex::new(r"\*{2}([^*]+)\*{2}").unwrap(), "$1"),
            (Regex::new(r"_{2}([^_]+)_{2}").unwrap(), "$1"),
            // Italic: *text* or _text_
            (Regex::new(r"\*([^*]+)\*").unwrap(), "$1"),
            (
                Regex::new(r"(?:^|[\s(])_([^_]+)_(?:[\s).,;:!?]|$)").unwrap(),
                "$1",
            ),
            // Strikethrough: ~~text~~
            (Regex::new(r"~~([^~]+)~~").unwrap(), "$1"),
            // Inline code: `code`
            (Regex::new(r"`([^`]+)`").unwrap(), "$1"),
            // Headers: # text -> text
            (Regex::new(r"(?m)^#{1,6}\s+").unwrap(), ""),
            // Horizontal rules
            (Regex::new(r"(?m)^[-*_]{3,}\s*$").unwrap(), ""),
            // Blockquotes: > text -> text
            (Regex::new(r"(?m)^>\s?").unwrap(), ""),
            // Unordered list markers: - item or * item
            (Regex::new(r"(?m)^[\s]*[-*+]\s").unwrap(), "  "),
            // Ordered list markers: 1. item
            (Regex::new(r"(?m)^[\s]*\d+\.\s").unwrap(), "  "),
        ]
    });

    let mut result = input.to_string();

    // Remove fenced code block markers (```lang ... ```)
    // but keep the content
    static CODE_FENCE: OnceLock<Regex> = OnceLock::new();
    let code_fence = CODE_FENCE.get_or_init(|| Regex::new(r"(?m)^```\w*\s*$").unwrap());
    result = code_fence.replace_all(&result, "").to_string();

    for (pattern, replacement) in patterns {
        result = pattern.replace_all(&result, *replacement).to_string();
    }

    // Collapse multiple blank lines into one
    static MULTI_BLANK: OnceLock<Regex> = OnceLock::new();
    let multi_blank = MULTI_BLANK.get_or_init(|| Regex::new(r"\n{3,}").unwrap());
    result = multi_blank.replace_all(&result, "\n\n").to_string();

    result.trim().to_string()
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

    #[test]
    fn test_strip_markdown_headers() {
        assert_eq!(strip_markdown("# Title"), "Title");
        assert_eq!(strip_markdown("## Subtitle"), "Subtitle");
        assert_eq!(strip_markdown("### Deep"), "Deep");
    }

    #[test]
    fn test_strip_markdown_bold_italic() {
        assert_eq!(strip_markdown("**bold**"), "bold");
        assert_eq!(strip_markdown("__bold__"), "bold");
        assert_eq!(strip_markdown("*italic*"), "italic");
        assert_eq!(strip_markdown("***both***"), "both");
    }

    #[test]
    fn test_strip_markdown_links() {
        assert_eq!(
            strip_markdown("[click here](https://example.com)"),
            "click here"
        );
        assert_eq!(strip_markdown("![alt text](image.png)"), "alt text");
    }

    #[test]
    fn test_strip_markdown_code() {
        assert_eq!(strip_markdown("`inline code`"), "inline code");
        assert_eq!(strip_markdown("```rust\nlet x = 1;\n```"), "let x = 1;");
    }

    #[test]
    fn test_strip_markdown_strikethrough() {
        assert_eq!(strip_markdown("~~deleted~~"), "deleted");
    }

    #[test]
    fn test_strip_markdown_blockquote() {
        assert_eq!(strip_markdown("> quoted text"), "quoted text");
    }

    #[test]
    fn test_strip_markdown_lists() {
        // Single-line lists get leading spaces trimmed by final trim()
        assert_eq!(strip_markdown("- item one"), "item one");
        assert_eq!(strip_markdown("* item two"), "item two");
        assert_eq!(strip_markdown("1. numbered"), "numbered");
        // Multi-line preserves indentation for non-first items
        assert_eq!(strip_markdown("text\n- item"), "text\n  item");
    }

    #[test]
    fn test_strip_markdown_horizontal_rule() {
        assert_eq!(strip_markdown("above\n---\nbelow"), "above\n\nbelow");
    }

    #[test]
    fn test_strip_markdown_plain_text() {
        assert_eq!(strip_markdown("just plain text"), "just plain text");
    }

    #[test]
    fn test_strip_markdown_collapses_blank_lines() {
        assert_eq!(strip_markdown("a\n\n\n\nb"), "a\n\nb");
    }
}
