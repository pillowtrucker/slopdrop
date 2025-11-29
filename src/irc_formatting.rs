/// IRC formatting and message utilities
///
/// Handles IRC color codes, formatting, and smart message splitting

/// Strip IRC color codes and formatting from a message
///
/// Removes:
/// - Color codes (\x03)
/// - Bold (\x02)
/// - Underline (\x1F)
/// - Reverse (\x16)
/// - Reset (\x0F)
/// - Italics (\x1D)
/// - Monospace (\x11)
pub fn strip_irc_formatting(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            // Color code - may be followed by digits
            '\x03' => {
                // Skip foreground color (0-2 digits)
                let mut digit_count = 0;
                while digit_count < 2 {
                    if let Some(&next_ch) = chars.peek() {
                        if next_ch.is_ascii_digit() {
                            chars.next();
                            digit_count += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // Check for comma + background color
                if let Some(&',') = chars.peek() {
                    chars.next(); // consume comma
                    digit_count = 0;
                    while digit_count < 2 {
                        if let Some(&next_ch) = chars.peek() {
                            if next_ch.is_ascii_digit() {
                                chars.next();
                                digit_count += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
            // Other formatting codes - just skip them
            '\x02' | '\x1F' | '\x16' | '\x0F' | '\x1D' | '\x11' => {
                // Skip
            }
            // Normal character - keep it
            _ => result.push(ch),
        }
    }

    result
}

/// Smart message splitting on word boundaries
///
/// Splits a message into chunks of max_len, trying to break on word boundaries.
/// Handles multiple lines and preserves them.
/// Calculates length based on VISIBLE characters (excluding IRC formatting codes)
/// to avoid breaking color sequences.
pub fn split_message_smart(text: &str, max_len: usize) -> Vec<String> {
    let mut result = Vec::new();

    for line in text.lines() {
        // Calculate visible length (excluding formatting codes)
        let visible_len = strip_irc_formatting(line).len();

        if visible_len <= max_len {
            result.push(line.to_string());
        } else {
            // Split long lines on word boundaries based on VISIBLE length
            let mut current = String::new();
            let mut visible_current_len = 0;

            for word in line.split_whitespace() {
                // Calculate visible length of this word
                let visible_word_len = strip_irc_formatting(word).len();

                // If a single word is longer than max_len (visibly), we have to split it
                if visible_word_len > max_len {
                    // Flush current buffer if not empty
                    if !current.is_empty() {
                        result.push(current.trim_end().to_string());
                        current.clear();
                        visible_current_len = 0;
                    }

                    // For words without formatting codes, split character by character
                    // For words with formatting codes, we push as-is to avoid complexity
                    if word == strip_irc_formatting(word) {
                        // No formatting codes - safe to split by character
                        let chars: Vec<char> = word.chars().collect();
                        let mut start = 0;
                        while start < chars.len() {
                            let end = (start + max_len).min(chars.len());
                            result.push(chars[start..end].iter().collect());
                            start = end;
                        }
                    } else {
                        // Has formatting codes - push as-is to avoid breaking them
                        result.push(word.to_string());
                    }
                    continue;
                }

                // Check if adding this word would exceed the limit (based on visible length)
                let space_needed = if visible_current_len == 0 { 0 } else { 1 }; // space before word
                if visible_current_len + space_needed + visible_word_len > max_len {
                    // Flush current buffer
                    if !current.is_empty() {
                        result.push(current.trim_end().to_string());
                        current.clear();
                        visible_current_len = 0;
                    }
                }

                // Add word to current buffer
                if !current.is_empty() {
                    current.push(' ');
                    visible_current_len += 1;
                }
                current.push_str(word);
                visible_current_len += visible_word_len;
            }

            // Flush remaining buffer
            if !current.is_empty() {
                result.push(current.trim_end().to_string());
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_color_codes() {
        // Color codes
        assert_eq!(strip_irc_formatting("\x0304red text"), "red text");
        assert_eq!(strip_irc_formatting("\x0304,08red on yellow"), "red on yellow");
        assert_eq!(strip_irc_formatting("normal \x0304red\x03 normal"), "normal red normal");

        // Bold
        assert_eq!(strip_irc_formatting("\x02bold text\x02"), "bold text");

        // Multiple formatting
        assert_eq!(
            strip_irc_formatting("\x02\x0304bold red\x0F normal"),
            "bold red normal"
        );
    }

    #[test]
    fn test_split_message_smart() {
        // Short message - no split
        assert_eq!(
            split_message_smart("hello world", 50),
            vec!["hello world"]
        );

        // Split on word boundary
        assert_eq!(
            split_message_smart("hello world this is a test", 15),
            vec!["hello world", "this is a test"]
        );

        // Very long word - must split
        assert_eq!(
            split_message_smart("aaaaaaaaaaaaaaaaaaaaaaaaa", 10),
            vec!["aaaaaaaaaa", "aaaaaaaaaa", "aaaaa"]
        );

        // Multiple lines preserved
        assert_eq!(
            split_message_smart("line1\nline2", 50),
            vec!["line1", "line2"]
        );
    }

    #[test]
    fn test_split_message_with_color_codes() {
        // Short message with colors - no split
        let colored = "\x0304red\x03 \x0302blue\x03";
        assert_eq!(
            split_message_smart(colored, 50),
            vec![colored]
        );

        // Long message with colors - should split on word boundaries, not in color codes
        // Visual: "red blue green yellow orange purple" (37 visible chars)
        // With codes it's longer, but we split based on visible length
        let long_colored = "\x0304red\x03 \x0302blue\x03 \x0303green\x03 \x0308yellow\x03 \x0307orange\x03 \x0306purple\x03";
        let result = split_message_smart(long_colored, 20);

        // Should split but keep color codes intact
        assert!(result.len() > 1);

        // Each chunk should have complete color codes (no split codes)
        for chunk in &result {
            // Verify no orphaned color code starts
            // If we have \x03, it should be followed by valid color digits or reset
            let mut chars = chunk.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\x03' {
                    // Either end of string (reset) or digits
                    if let Some(&next) = chars.peek() {
                        // Should be digit or end of color code
                        assert!(
                            next.is_ascii_digit() || next == ' ' || next.is_alphabetic(),
                            "Color code should be complete in chunk: {:?}", chunk
                        );
                    }
                }
            }
        }

        // Verify visible length of each chunk is within limit
        for chunk in &result {
            let visible = strip_irc_formatting(chunk);
            assert!(
                visible.len() <= 20,
                "Chunk visible length {} exceeds max 20: {:?}",
                visible.len(),
                chunk
            );
        }
    }

    #[test]
    fn test_split_message_preserves_formatting() {
        // Message with color codes that needs splitting
        // Visual length: "word1 word2 word3 word4 word5" (29 chars with spaces)
        let msg = "\x0304word1\x03 \x0302word2\x03 \x0303word3\x03 \x0308word4\x03 \x0307word5\x03";
        let result = split_message_smart(msg, 15); // Split at ~15 visible chars

        // Should split into at least 2 chunks
        assert!(result.len() >= 2);

        // When we strip and recombine, we should get the original visible text
        let recombined_visible = result.iter()
            .map(|s| strip_irc_formatting(s))
            .collect::<Vec<_>>()
            .join(" ");

        // Should have all the same words (order might have spaces added)
        assert!(recombined_visible.contains("word1"));
        assert!(recombined_visible.contains("word2"));
        assert!(recombined_visible.contains("word3"));
        assert!(recombined_visible.contains("word4"));
        assert!(recombined_visible.contains("word5"));
    }
}
