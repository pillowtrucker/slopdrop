/// Hostmask matching with wildcard support
///
/// Supports IRC-style wildcards:
/// - `*` matches any sequence of characters (including empty)
/// - `?` matches exactly one character
///
/// Examples:
/// - `*!*@*.example.com` matches anyone from example.com
/// - `alice!*@*` matches alice with any ident/host
/// - `*!~user@host.com` matches anyone with ident ~user from host.com

/// Match a hostmask against a pattern with wildcard support
pub fn matches_hostmask(hostmask: &str, pattern: &str) -> bool {
    // Convert IRC wildcard pattern to regex
    // Escape regex special chars except * and ?
    let escaped = regex::escape(pattern);

    // Replace escaped wildcards back to regex equivalents
    let regex_pattern = escaped
        .replace("\\*", ".*")  // * matches any sequence
        .replace("\\?", ".");   // ? matches one character

    // Anchor the pattern to match the entire string
    let full_pattern = format!("^{}$", regex_pattern);

    // Match the hostmask
    if let Ok(re) = regex::Regex::new(&full_pattern) {
        re.is_match(hostmask)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(matches_hostmask("alice!user@example.com", "alice!user@example.com"));
        assert!(!matches_hostmask("bob!user@example.com", "alice!user@example.com"));
    }

    #[test]
    fn test_wildcard_star() {
        // Match all from a domain
        assert!(matches_hostmask("alice!user@host.example.com", "*!*@*.example.com"));
        assert!(matches_hostmask("bob!admin@db.example.com", "*!*@*.example.com"));
        assert!(!matches_hostmask("eve!user@other.org", "*!*@*.example.com"));

        // Match specific nick with any ident/host
        assert!(matches_hostmask("alice!user@host.com", "alice!*@*"));
        assert!(matches_hostmask("alice!admin@other.net", "alice!*@*"));
        assert!(!matches_hostmask("bob!user@host.com", "alice!*@*"));

        // Match any nick/ident with specific host
        assert!(matches_hostmask("alice!user@host.example.com", "*!*@host.example.com"));
        assert!(matches_hostmask("bob!admin@host.example.com", "*!*@host.example.com"));
        assert!(!matches_hostmask("eve!user@other.com", "*!*@host.example.com"));
    }

    #[test]
    fn test_wildcard_question() {
        // Match single character
        assert!(matches_hostmask("alice!user@host.com", "alice!use?@host.com"));
        assert!(!matches_hostmask("alice!username@host.com", "alice!use?@host.com"));
        assert!(!matches_hostmask("alice!use@host.com", "alice!use?@host.com"));
    }

    #[test]
    fn test_combined_wildcards() {
        assert!(matches_hostmask("alice!~user@192.168.1.100", "alice!~*@192.168.?.???"));
        assert!(matches_hostmask("bob!~admin@192.168.5.200", "bob!~*@192.168.?.???"));
    }

    #[test]
    fn test_special_chars() {
        // Ensure regex special characters are properly escaped
        assert!(matches_hostmask("user!ident@host.example.com", "user!ident@host.example.com"));
        assert!(matches_hostmask("user!id[123]@host.com", "user!id[123]@host.com"));
    }
}
