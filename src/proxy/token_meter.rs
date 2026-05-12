/// Estimate the number of context tokens for a piece of text.
///
/// We do not bundle a real tokenizer to keep the binary tiny and the build
/// portable. The widely-used `chars / 4` approximation is good enough for
/// reporting savings deltas. Always label values as "estimated tokens" in
/// human-facing output.
pub struct TokenMeter;

impl TokenMeter {
    pub fn estimate_text(text: &str) -> usize {
        let char_count = text.chars().count();
        if char_count == 0 {
            return 0;
        }
        // ceil(char_count / 4)
        (char_count + 3) / 4
    }

    pub fn estimate_bytes(bytes: &[u8]) -> usize {
        // Use lossy UTF-8 so non-text bytes still produce a stable estimate.
        let text = String::from_utf8_lossy(bytes);
        Self::estimate_text(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_zero_tokens() {
        assert_eq!(TokenMeter::estimate_text(""), 0);
    }

    #[test]
    fn rounds_up_partial_token() {
        // 5 characters -> ceil(5/4) = 2
        assert_eq!(TokenMeter::estimate_text("abcde"), 2);
    }

    #[test]
    fn longer_text_scales() {
        let text = "abcd".repeat(100); // 400 chars
        assert_eq!(TokenMeter::estimate_text(&text), 100);
    }

    #[test]
    fn estimate_bytes_matches_text() {
        let text = "hello world";
        assert_eq!(
            TokenMeter::estimate_bytes(text.as_bytes()),
            TokenMeter::estimate_text(text)
        );
    }
}
