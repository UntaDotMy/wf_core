use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

/// Lazily-initialised BPE tokenizer (cl100k_base — GPT-4 / GPT-3.5-turbo).
///
/// Falls back to `chars / 4` when the tokenizer data cannot be loaded (e.g. on
/// the very first run before the cache is populated).  Once loaded the result
/// is cached for the lifetime of the process.
static TOKENIZER: OnceLock<Option<CoreBPE>> = OnceLock::new();

fn get_tokenizer() -> Option<&'static CoreBPE> {
    TOKENIZER
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
}

/// Count tokens in `text` using a real BPE tokenizer, falling back to a
/// `ceil(chars / 4)` approximation when the tokenizer data isn't available.
pub struct TokenMeter;

impl TokenMeter {
    /// Estimate the number of tokens in a text string.
    pub fn estimate_text(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        match get_tokenizer() {
            Some(bpe) => bpe.encode_with_special_tokens(text).len(),
            // Fallback: chars/4 approximation (same as the legacy heuristic).
            None => (text.chars().count() + 3) / 4,
        }
    }

    /// Estimate the number of tokens in a byte slice (treated as lossy UTF-8).
    pub fn estimate_bytes(bytes: &[u8]) -> usize {
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
    fn non_empty_text_produces_tokens() {
        // Real BPE should give a non-zero, reasonable count.
        let count = TokenMeter::estimate_text("Hello, world!");
        assert!(count > 0, "expected at least 1 token");
        // Typical BPE: "Hello" (1) + "," (1) + " world" (1) + "!" (1) ≈ 3-5.
        assert!(count < 10, "expected <10 tokens, got {count}");
    }

    #[test]
    fn estimated_bytes_matches_text() {
        let text = "hello world this is a longer piece of text that should have more tokens";
        assert_eq!(
            TokenMeter::estimate_bytes(text.as_bytes()),
            TokenMeter::estimate_text(text)
        );
    }

    #[test]
    fn english_token_ratio_is_reasonable() {
        let text = "The quick brown fox jumps over the lazy dog. ";
        let repeated = text.repeat(100); // 4500 chars
        let count = TokenMeter::estimate_text(&repeated);
        // English BPE compresses at roughly 0.25–0.5 tokens per character
        // for common words.  4500 chars should be ≈ 1000–2000 tokens,
        // certainly more than 100 and less than 4500.
        assert!(
            count > 100,
            "expected >100 tokens for 4500 chars, got {count}"
        );
        assert!(
            count < 4500,
            "expected <4500 tokens for 4500 chars, got {count}"
        );
    }

    #[test]
    fn estimate_bytes_zero_for_empty_slice() {
        assert_eq!(TokenMeter::estimate_bytes(b""), 0);
    }
}
