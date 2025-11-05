/// Validates that brackets are balanced in TCL code
/// Returns Ok(()) if balanced, Err with error message if not
pub fn validate_brackets(code: &str) -> Result<(), String> {
    check_balanced(code, '{', '}')
}

fn check_balanced(s: &str, open: char, close: char) -> Result<(), String> {
    let mut depth = 0;
    let mut pos = 0;
    let chars: Vec<char> = s.chars().collect();

    while pos < chars.len() {
        let c = chars[pos];

        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth < 0 {
                return Err(format!("Unmatched closing bracket at position {}", pos));
            }
        } else if c == '\\' && pos + 1 < chars.len() {
            // Skip escaped characters
            let next = chars[pos + 1];
            if next == open || next == close {
                pos += 1; // Skip the next character
            }
        }

        pos += 1;
    }

    if depth > 0 {
        Err("Opening bracket unmatched until end of command".to_string())
    } else if depth < 0 {
        Err("Unmatched closing bracket".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balanced() {
        assert!(validate_brackets("{ hello }").is_ok());
        assert!(validate_brackets("{ { nested } }").is_ok());
        assert!(validate_brackets("no brackets").is_ok());
    }

    #[test]
    fn test_unbalanced_open() {
        assert!(validate_brackets("{ hello").is_err());
        assert!(validate_brackets("{ { }").is_err());
    }

    #[test]
    fn test_unbalanced_close() {
        assert!(validate_brackets("hello }").is_err());
        assert!(validate_brackets("{ } }").is_err());
    }

    #[test]
    fn test_escaped() {
        assert!(validate_brackets(r"{ \{ \} }").is_ok());
        assert!(validate_brackets(r"{ test \{ }").is_ok());
    }
}
