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

/// Extract leading and trailing IRC formatting codes from text
///
/// Returns (leading_codes, visible_text, trailing_codes)
fn extract_formatting(text: &str) -> (String, String, String) {
    let mut leading = String::new();
    let mut trailing = String::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut start = 0;
    let mut end = chars.len();

    // Extract leading formatting codes
    while start < end {
        match chars[start] {
            '\x03' => {
                leading.push('\x03');
                start += 1;
                // Skip color digits
                while start < end && chars[start].is_ascii_digit() && leading.len() - leading.rfind('\x03').unwrap_or(0) <= 3 {
                    leading.push(chars[start]);
                    start += 1;
                }
                // Check for comma + background color
                if start < end && chars[start] == ',' {
                    leading.push(',');
                    start += 1;
                    while start < end && chars[start].is_ascii_digit() && leading.chars().rev().take_while(|&c| c != ',').count() <= 2 {
                        leading.push(chars[start]);
                        start += 1;
                    }
                }
            }
            '\x02' | '\x1F' | '\x16' | '\x0F' | '\x1D' | '\x11' => {
                leading.push(chars[start]);
                start += 1;
            }
            _ => break,
        }
    }

    // Extract trailing formatting codes
    while end > start {
        match chars[end - 1] {
            '\x03' | '\x02' | '\x1F' | '\x16' | '\x0F' | '\x1D' | '\x11' => {
                trailing.insert(0, chars[end - 1]);
                end -= 1;
            }
            c if c.is_ascii_digit() && end > start + 1 && matches!(chars[end - 2], '\x03' | ',') => {
                trailing.insert(0, c);
                end -= 1;
            }
            ',' if end > start + 1 && chars[end - 2] == '\x03' => {
                trailing.insert(0, ',');
                end -= 1;
            }
            _ => break,
        }
    }

    let visible: String = chars[start..end].iter().collect();
    (leading, visible, trailing)
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
        let actual_len = line.len();

        // Only skip splitting if BOTH visible and actual lengths are within limit
        if visible_len <= max_len && actual_len <= max_len {
            result.push(line.to_string());
        } else {
            // Split long lines on word boundaries based on VISIBLE length
            // but also respect ACTUAL byte length to stay within IRC protocol limits
            let mut current = String::new();
            let mut visible_current_len = 0;

            for word in line.split_whitespace() {
                // Calculate visible length of this word
                let visible_word_len = strip_irc_formatting(word).len();
                let actual_word_len = word.len();

                // If a single word is longer than max_len (either visibly or in bytes), we have to split it
                if visible_word_len > max_len || actual_word_len > max_len {
                    // Flush current buffer if not empty
                    if !current.is_empty() {
                        result.push(current.trim_end().to_string());
                        current.clear();
                        visible_current_len = 0;
                    }

                    // Extract formatting codes and split the visible text
                    let (leading, visible, trailing) = extract_formatting(word);
                    let visible_chars: Vec<char> = visible.chars().collect();
                    let mut start = 0;

                    while start < visible_chars.len() {
                        let end = (start + max_len).min(visible_chars.len());
                        let chunk: String = visible_chars[start..end].iter().collect();
                        // Reapply formatting codes to each chunk
                        result.push(format!("{}{}{}", leading, chunk, trailing));
                        start = end;
                    }
                    continue;
                }

                // Calculate the space needed and resulting lengths
                let space_needed = if current.is_empty() { 0 } else { 1 };
                let new_visible_len = visible_current_len + space_needed + visible_word_len;
                let new_actual_len = current.len() + space_needed + word.len();

                // Check if adding this word would exceed EITHER limit
                // We check both visible length (for readability) and actual byte length (for protocol)
                if new_visible_len > max_len || new_actual_len > max_len {
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

    #[test]
    fn test_split_respects_actual_byte_length() {
        // Create a message where visible length is small but actual byte length is large
        // Each word: \x0304XX\x03 = 7 bytes but 2 visible chars
        let words: Vec<String> = (0..100)
            .map(|i| format!("\x0304{:02}\x03", i))
            .collect();
        let msg = words.join(" ");

        // Visible length: 100 words * 2 chars = 200 chars
        // Actual length: 100 words * 7 bytes + 99 spaces = 799 bytes
        let result = split_message_smart(&msg, 400);

        // Should split into multiple chunks
        assert!(result.len() >= 2, "Should split into at least 2 chunks");

        // Each chunk should not exceed 400 bytes (actual length)
        for chunk in &result {
            assert!(
                chunk.len() <= 400,
                "Chunk actual length {} exceeds max 400 bytes",
                chunk.len()
            );
        }

        // Each chunk should not exceed 400 visible characters
        for chunk in &result {
            let visible = strip_irc_formatting(chunk);
            assert!(
                visible.len() <= 400,
                "Chunk visible length {} exceeds max 400 chars",
                visible.len()
            );
        }

        // All original content should be preserved across chunks
        let recombined = result.join(" ");
        let recombined_visible = strip_irc_formatting(&recombined);
        for i in 0..100 {
            let expected = format!("{:02}", i);
            assert!(
                recombined_visible.contains(&expected),
                "Missing content: {}",
                expected
            );
        }
    }
}
