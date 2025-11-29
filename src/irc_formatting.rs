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

/// Tracks the active IRC formatting state
#[derive(Debug, Clone, Default, PartialEq)]
struct FormattingState {
    bold: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
    monospace: bool,
    color: Option<String>, // Stores the complete color code (e.g., "04" or "04,08")
}

impl FormattingState {
    /// Parse text and return the formatting state at the end
    fn from_text(text: &str) -> Self {
        let mut state = Self::default();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '\x0F' => {
                    // Full reset - clear everything
                    state = Self::default();
                }
                '\x02' => {
                    // Bold - toggle
                    state.bold = !state.bold;
                }
                '\x1D' => {
                    // Italic - toggle
                    state.italic = !state.italic;
                }
                '\x1F' => {
                    // Underline - toggle
                    state.underline = !state.underline;
                }
                '\x16' => {
                    // Reverse - toggle
                    state.reverse = !state.reverse;
                }
                '\x11' => {
                    // Monospace - toggle
                    state.monospace = !state.monospace;
                }
                '\x03' => {
                    // Color code
                    let mut color_code = String::new();
                    // Read up to 2 digits for foreground
                    for _ in 0..2 {
                        if let Some(&next_ch) = chars.peek() {
                            if next_ch.is_ascii_digit() {
                                color_code.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    // Check for comma + background
                    if chars.peek() == Some(&',') {
                        color_code.push(chars.next().unwrap()); // consume comma
                        for _ in 0..2 {
                            if let Some(&next_ch) = chars.peek() {
                                if next_ch.is_ascii_digit() {
                                    color_code.push(chars.next().unwrap());
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    // If no digits, it's a color reset
                    if color_code.is_empty() {
                        state.color = None;
                    } else {
                        state.color = Some(color_code);
                    }
                }
                _ => {
                    // Regular character, continue
                }
            }
        }

        state
    }

    /// Generate the IRC codes needed to apply this formatting state
    fn to_codes(&self) -> String {
        let mut codes = String::new();

        if self.bold {
            codes.push('\x02');
        }
        if self.italic {
            codes.push('\x1D');
        }
        if self.underline {
            codes.push('\x1F');
        }
        if self.reverse {
            codes.push('\x16');
        }
        if self.monospace {
            codes.push('\x11');
        }
        if let Some(ref color) = self.color {
            codes.push('\x03');
            codes.push_str(color);
        }

        codes
    }

    /// Generate codes to close/reset this formatting
    fn to_close_codes(&self) -> String {
        // In IRC, most formatting codes are toggles, so we send them again to close
        // For color, we use \x03 without digits to reset
        let mut codes = String::new();

        if self.color.is_some() {
            codes.push('\x03'); // Color reset
        }
        if self.monospace {
            codes.push('\x11');
        }
        if self.reverse {
            codes.push('\x16');
        }
        if self.underline {
            codes.push('\x1F');
        }
        if self.italic {
            codes.push('\x1D');
        }
        if self.bold {
            codes.push('\x02');
        }

        codes
    }
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

/// Check if text ends with an incomplete color code sequence
/// Returns (complete_text, incomplete_suffix) where incomplete_suffix should be moved to next chunk
fn split_incomplete_color_code(text: &str) -> (&str, &str) {
    let bytes = text.as_bytes();
    let len = bytes.len();

    if len == 0 {
        return (text, "");
    }

    // Check if ends with \x03 alone (could be start of color code)
    // We treat this as incomplete to avoid orphaning color codes at boundaries
    if len >= 1 && bytes[len - 1] == 0x03 {
        // Check if it's NOT part of a complete color code
        // Look back to see if there are digits before it
        if len == 1 {
            return ("", text); // Just \x03
        }
        // If preceded by non-digit, it's a standalone \x03 (incomplete/reset)
        if !bytes[len - 2].is_ascii_digit() && bytes[len - 2] != b',' {
            return (&text[..len - 1], &text[len - 1..]);
        }
    }

    // Check if ends with \x03N (one digit - might be a two-digit color)
    if len >= 2 && bytes[len - 2] == 0x03 && bytes[len - 1].is_ascii_digit() {
        return (&text[..len - 2], &text[len - 2..]);
    }

    // Check if ends with \x03NN, (comma after color - background color incomplete)
    if len >= 4
        && bytes[len - 4] == 0x03
        && bytes[len - 3].is_ascii_digit()
        && bytes[len - 2].is_ascii_digit()
        && bytes[len - 1] == b',' {
        return (&text[..len - 4], &text[len - 4..]);
    }

    // Check if ends with \x03NN,N (background color with one digit)
    if len >= 5
        && bytes[len - 5] == 0x03
        && bytes[len - 4].is_ascii_digit()
        && bytes[len - 3].is_ascii_digit()
        && bytes[len - 2] == b','
        && bytes[len - 1].is_ascii_digit() {
        return (&text[..len - 5], &text[len - 5..]);
    }

    (text, "")
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

            // Split on spaces (not all whitespace) to preserve formatting codes
            // that might be adjacent to other whitespace characters
            let words: Vec<&str> = if line.contains(' ') {
                line.split(' ').collect()
            } else {
                vec![line]
            };

            for word in words {
                // Skip empty words (from consecutive spaces)
                if word.is_empty() {
                    continue;
                }

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

                    // Use state machine to find the active formatting during the visible text
                    // We need to parse just the LEADING codes to get the active state,
                    // not the whole word (which includes trailing close codes)
                    let (leading_codes, visible, _trailing_codes) = extract_formatting(word);
                    let active_state = FormattingState::from_text(&leading_codes);

                    // Split the visible text into chunks
                    let visible_chars: Vec<char> = visible.chars().collect();
                    let mut start = 0;

                    // Calculate formatting overhead (codes to open + close state)
                    let state_overhead = active_state.to_codes().len() + active_state.to_close_codes().len();
                    let chunk_visible_size = max_len.saturating_sub(state_overhead).max(1);

                    while start < visible_chars.len() {
                        let end = (start + chunk_visible_size).min(visible_chars.len());
                        let chunk: String = visible_chars[start..end].iter().collect();

                        // Wrap each chunk with the active formatting state
                        let formatted_chunk = format!(
                            "{}{}{}",
                            active_state.to_codes(),
                            chunk,
                            active_state.to_close_codes()
                        );
                        result.push(formatted_chunk);
                        start = end;
                    }
                    current.clear();
                    visible_current_len = 0;
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
                        // When flushing mid-processing, trailing \x03 is a complete RESET code
                        // not an incomplete color code, so don't split it off
                        result.push(current.trim_end().to_string());
                        current.clear();
                        visible_current_len = 0;
                    }
                }

                // Add word to current buffer
                // Don't add space if current is only an incomplete color code (has no visible chars)
                // When we carry forward incomplete codes, we set visible_current_len = 0
                if !current.is_empty() && visible_current_len > 0 {
                    current.push(' ');
                    visible_current_len += 1;
                }
                current.push_str(word);
                visible_current_len += visible_word_len;
            }

            // Flush remaining buffer
            if !current.is_empty() {
                // On final flush, a trailing \x03 alone is a RESET code, not incomplete
                // Only OTHER incomplete patterns (\x03N, \x03NN,, etc.) are actually incomplete
                // Since this is the end of input, we just push everything as-is
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
    fn test_incomplete_color_code_detection() {
        // Test detection of various incomplete color code patterns
        assert_eq!(
            split_incomplete_color_code("text\x03"),
            ("text", "\x03")
        );

        assert_eq!(
            split_incomplete_color_code("text\x031"),
            ("text", "\x031")
        );

        assert_eq!(
            split_incomplete_color_code("text\x0313,"),
            ("text", "\x0313,")
        );

        assert_eq!(
            split_incomplete_color_code("text\x0313,5"),
            ("text", "\x0313,5")
        );

        // Complete codes should not be split
        assert_eq!(
            split_incomplete_color_code("text\x0313,05"),
            ("text\x0313,05", "")
        );

        assert_eq!(
            split_incomplete_color_code("text\x0313"),
            ("text\x0313", "")
        );
    }

    #[test]
    fn test_no_orphaned_color_codes_at_boundaries() {
        // This test catches the exact bug we had: \x03 at end of one chunk,
        // color number at start of next
        let text = "\x0304ab\x03 \x0312cd\x03 \x0308ef\x03 \x0301gh\x03";
        let chunks = split_message_smart(text, 20);

        for (i, chunk) in chunks.iter().enumerate() {
            // Check that no chunk starts with orphaned digits
            // (digits that should have been part of a color code in the previous chunk)
            if let Some(first_char) = chunk.chars().next() {
                if first_char.is_ascii_digit() {
                    panic!(
                        "Chunk {} starts with orphaned color digit: {:?}",
                        i, chunk
                    );
                }
            }

            // Check for incomplete color code patterns at the end
            // These are actually incomplete, unlike a standalone \x03 which is a complete reset
            let bytes = chunk.as_bytes();
            let len = bytes.len();
            if len >= 2 && bytes[len - 2] == 0x03 && bytes[len - 1].is_ascii_digit() {
                panic!("Chunk {} ends with incomplete color code \\x03N: {:?}", i, chunk);
            }
            if len >= 4 && bytes[len - 4] == 0x03 && bytes[len - 3].is_ascii_digit()
                && bytes[len - 2].is_ascii_digit() && bytes[len - 1] == b',' {
                panic!("Chunk {} ends with incomplete color code \\x03NN,: {:?}", i, chunk);
            }
            if len >= 5 && bytes[len - 5] == 0x03 && bytes[len - 4].is_ascii_digit()
                && bytes[len - 3].is_ascii_digit() && bytes[len - 2] == b','
                && bytes[len - 1].is_ascii_digit() {
                panic!("Chunk {} ends with incomplete color code \\x03NN,N: {:?}", i, chunk);
            }
        }
    }

    #[test]
    fn test_spew_like_output_with_random_spaces() {
        // Simulate spew output: color codes around text with random spaces
        // This is the pattern that was breaking: \x03NN,MMtext1 text2\x03\x03PP,QQtext3
        let text = "\x0304,08abc def\x03\x0312,05ghi jkl\x03\x0308,01mno pqr\x03\x0313,02stu vwx\x03";
        let chunks = split_message_smart(text, 30);

        // Verify each chunk has valid color codes
        for chunk in &chunks {
            // Check that every \x03 is either:
            // 1. Followed by digits (color code)
            // 2. At end of string (reset)
            // 3. NOT orphaned
            let bytes = chunk.as_bytes();
            for i in 0..bytes.len() {
                if bytes[i] == 0x03 {
                    if i < bytes.len() - 1 {
                        // Not at end, should be followed by digit or another \x03
                        let next = bytes[i + 1];
                        assert!(
                            next.is_ascii_digit() || next == 0x03,
                            "Color code at position {} not followed by digit or \\x03 in chunk: {:?}",
                            i, chunk
                        );
                    }
                }
            }

            // No orphaned digits at the start
            if let Some(first) = bytes.first() {
                if first.is_ascii_digit() {
                    panic!("Chunk starts with orphaned digit: {:?}", chunk);
                }
            }

            // Verify actual byte length doesn't exceed limit
            assert!(
                chunk.len() <= 30,
                "Chunk exceeds max length: {} bytes in {:?}",
                chunk.len(), chunk
            );
        }

        // Verify content is preserved
        let recombined = chunks.join(" ");
        assert!(strip_irc_formatting(&recombined).contains("abc"));
        assert!(strip_irc_formatting(&recombined).contains("def"));
        assert!(strip_irc_formatting(&recombined).contains("vwx"));
    }

    #[test]
    fn test_exactly_at_limit_with_color_codes() {
        // Test the spew 400 bug: when visible length exactly equals limit
        // but actual length is much higher due to color codes
        let mut text = String::new();
        for i in 0..50 {
            text.push_str(&format!("\x0304,08{:02}\x03 ", i));
        }

        // 50 words * 2 visible chars + spaces = ~150 visible chars
        // But actual length is much more due to color codes
        let visible = strip_irc_formatting(&text);
        assert!(visible.len() >= 100 && visible.len() <= 200); // Should be around 150

        let chunks = split_message_smart(&text, 100);

        // Should split because actual length exceeds 100, even though visible doesn't
        assert!(
            chunks.len() > 1,
            "Should split when actual length exceeds limit even if visible doesn't"
        );

        // Each chunk should respect BOTH limits
        for chunk in &chunks {
            assert!(chunk.len() <= 100, "Chunk actual length {} exceeds 100", chunk.len());
            assert!(strip_irc_formatting(chunk).len() <= 100,
                    "Chunk visible length {} exceeds 100", strip_irc_formatting(chunk).len());
        }
    }

    #[test]
    fn test_word_with_heavy_formatting_exceeds_limit() {
        // Test a single word where visible length is OK but actual length exceeds limit
        // This was the bug where we only checked visible_word_len
        let word = format!("\x0304,08{}\x03", "a".repeat(50)); // 50 visible, but 57+ actual
        let text = format!("{} more text", word);

        let chunks = split_message_smart(&text, 55);

        // The heavy word should be split
        assert!(chunks.len() > 1);

        // Each chunk should respect byte limit
        for chunk in &chunks {
            assert!(chunk.len() <= 55, "Chunk {} bytes exceeds limit", chunk.len());
        }
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

    #[test]
    fn test_combined_formatting_across_boundaries() {
        // Test that combined formatting (bold+underline+color) is properly preserved
        // when splitting across boundaries

        // Create a very long word with combined formatting
        // Bold + Underline + Color red
        let long_text = "a".repeat(100);
        let formatted = format!("\x02\x1F\x0304{}\x03\x1F\x02", long_text);

        let chunks = split_message_smart(&formatted, 30);

        // Should have multiple chunks
        assert!(chunks.len() > 1, "Should split into multiple chunks");

        // Each chunk should have the same formatting applied
        for (i, chunk) in chunks.iter().enumerate() {
            // Parse just the leading codes to get the active state during visible text
            let (leading, visible, _trailing) = extract_formatting(chunk);
            let state = FormattingState::from_text(&leading);

            // All chunks should have bold, underline, and color red active
            assert!(state.bold, "Chunk {} missing bold formatting", i);
            assert!(state.underline, "Chunk {} missing underline formatting", i);
            assert_eq!(
                state.color,
                Some("04".to_string()),
                "Chunk {} missing or wrong color",
                i
            );

            // Verify visible content is present
            assert!(!visible.is_empty(), "Chunk {} has no visible content", i);
            assert!(visible.chars().all(|c| c == 'a'), "Chunk {} has wrong content", i);
        }

        // Verify total content is preserved
        let total_visible: String = chunks
            .iter()
            .map(|c| strip_irc_formatting(c))
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(total_visible.len(), 100, "Content length mismatch");
        assert!(total_visible.chars().all(|c| c == 'a'), "Content corrupted");
    }

    #[test]
    fn test_formatting_state_tracking() {
        // Test that FormattingState correctly tracks nested/combined formatting

        // Bold + color
        let state = FormattingState::from_text("\x02\x0304text");
        assert!(state.bold);
        assert_eq!(state.color, Some("04".to_string()));
        assert!(!state.underline);

        // Bold + underline + italic + color
        let state = FormattingState::from_text("\x02\x1F\x1D\x0312,05text");
        assert!(state.bold);
        assert!(state.underline);
        assert!(state.italic);
        assert_eq!(state.color, Some("12,05".to_string()));

        // Color reset in middle
        let state = FormattingState::from_text("\x0304red\x03 normal");
        assert_eq!(state.color, None); // Color was reset

        // Full reset
        let state = FormattingState::from_text("\x02\x1F\x0304text\x0F");
        assert!(!state.bold);
        assert!(!state.underline);
        assert_eq!(state.color, None);

        // Toggle bold twice (should end up off)
        let state = FormattingState::from_text("\x02bold\x02");
        assert!(!state.bold);
    }
}
