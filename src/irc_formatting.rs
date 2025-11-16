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
pub fn split_message_smart(text: &str, max_len: usize) -> Vec<String> {
    let mut result = Vec::new();

    for line in text.lines() {
        if line.len() <= max_len {
            result.push(line.to_string());
        } else {
            // Split long lines on word boundaries
            let mut current = String::new();

            for word in line.split_whitespace() {
                // If a single word is longer than max_len, we have to split it
                if word.len() > max_len {
                    // Flush current buffer if not empty
                    if !current.is_empty() {
                        result.push(current.trim_end().to_string());
                        current.clear();
                    }

                    // Split the long word character by character
                    let mut start = 0;
                    while start < word.len() {
                        let end = (start + max_len).min(word.len());
                        result.push(word[start..end].to_string());
                        start = end;
                    }
                    continue;
                }

                // Check if adding this word would exceed the limit
                let space_needed = if current.is_empty() { 0 } else { 1 }; // space before word
                if current.len() + space_needed + word.len() > max_len {
                    // Flush current buffer
                    if !current.is_empty() {
                        result.push(current.trim_end().to_string());
                        current.clear();
                    }
                }

                // Add word to current buffer
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
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
}
