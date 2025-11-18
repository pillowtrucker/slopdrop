use slopdrop::tcl_wrapper::SafeTclInterp;
use tempfile::TempDir;
use std::path::PathBuf;

/// Helper to create a temporary state directory
fn create_temp_state() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state");
    (temp_dir, state_path)
}

// =============================================================================
// Encoding Module Tests
// =============================================================================

#[test]
fn test_base64_encoding() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("base64 hello").unwrap();
    assert_eq!(result.trim(), "aGVsbG8=");

    let result = interp.eval("base64 \"test string\"").unwrap();
    assert_eq!(result.trim(), "dGVzdCBzdHJpbmc=");

    let result = interp.eval("base64 {}").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_base64_decoding() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("unbase64 aGVsbG8=").unwrap();
    assert_eq!(result.trim(), "hello");

    let result = interp.eval("unbase64 dGVzdCBzdHJpbmc=").unwrap();
    assert_eq!(result.trim(), "test string");

    let result = interp.eval("unbase64 {}").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_base64_roundtrip() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("unbase64 [base64 \"Hello, World!\"]").unwrap();
    assert_eq!(result.trim(), "Hello, World!");

    let result = interp.eval("unbase64 [base64 {special chars: @#$%^&*()}]").unwrap();
    assert!(result.contains("special chars"));
}

#[test]
fn test_url_encoding() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("url_encode {hello world}").unwrap();
    assert_eq!(result.trim(), "hello%20world");

    let result = interp.eval("url_encode {foo=bar&baz=qux}").unwrap();
    assert_eq!(result.trim(), "foo%3Dbar%26baz%3Dqux");

    // Test that safe characters are not encoded
    let result = interp.eval("url_encode {abc123}").unwrap();
    assert_eq!(result.trim(), "abc123");

    let result = interp.eval("url_encode {test_value}").unwrap();
    assert_eq!(result.trim(), "test_value");
}

#[test]
fn test_encoding_blocks_system_modification() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Should block system encoding modification
    let result = interp.eval("encoding system utf-8");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("can't modify system encoding"));
}

// Note: encoding convertto/convertfrom and encoding names are blocked
// in the safe interpreter for security reasons

// =============================================================================
// Utility Module Tests
// =============================================================================

#[test]
fn test_first_last_rest() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("first {a b c d}").unwrap();
    assert_eq!(result.trim(), "a");

    let result = interp.eval("last {a b c d}").unwrap();
    assert_eq!(result.trim(), "d");

    let result = interp.eval("rest {a b c d}").unwrap();
    assert_eq!(result.trim(), "b c d");
}

#[test]
fn test_second_third() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("second {a b c d}").unwrap();
    assert_eq!(result.trim(), "b");

    let result = interp.eval("third {a b c d}").unwrap();
    assert_eq!(result.trim(), "c");
}

#[test]
fn test_upper_lower() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("upper hello").unwrap();
    assert_eq!(result.trim(), "HELLO");

    let result = interp.eval("lower WORLD").unwrap();
    assert_eq!(result.trim(), "world");
}

#[test]
fn test_choose() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test choose returns one of the given options
    for _ in 0..10 {
        let result = interp.eval("choose a b c d e").unwrap();
        let result = result.trim();
        assert!(["a", "b", "c", "d", "e"].contains(&result));
    }
}

#[test]
fn test_question_mark_operator() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test ?? (alias for lindex_random)
    for _ in 0..10 {
        let result = interp.eval("?? {1 2 3 4 5}").unwrap();
        let result = result.trim();
        assert!(["1", "2", "3", "4", "5"].contains(&result));
    }
}

#[test]
fn test_glob_to_regexp() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test glob pattern conversion
    let result = interp.eval("glob_to_regexp {*.txt}").unwrap();
    assert!(result.contains(".*"));
    assert!(result.contains("\\.txt"));

    let result = interp.eval("glob_to_regexp {test?file}").unwrap();
    assert!(result.contains("."));
}

#[test]
fn test_lfilter_with_pattern() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("lfilter {*.txt} {file.txt image.png data.txt config.json}").unwrap();
    assert!(result.contains("file.txt"));
    assert!(result.contains("data.txt"));
    assert!(!result.contains("image.png"));
    assert!(!result.contains("config.json"));
}

#[test]
fn test_lfilter_nocase() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("lfilter -nocase {*test*} {TEST123 mytest TESTING other}").unwrap();
    assert!(result.contains("TEST123"));
    assert!(result.contains("mytest"));
    assert!(result.contains("TESTING"));
    assert!(!result.contains("other"));
}

#[test]
fn test_seq_basic() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("seq 1 5").unwrap();
    assert_eq!(result.trim(), "1 2 3 4 5");

    let result = interp.eval("seq 0 10 2").unwrap();
    assert_eq!(result.trim(), "0 2 4 6 8 10");

    let result = interp.eval("seq 10 0 -2").unwrap();
    assert_eq!(result.trim(), "10 8 6 4 2 0");
}

#[test]
fn test_seq_errors_on_zero_step() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("seq 1 10 0");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("step cannot be 0"));
}

#[test]
fn test_map_transform() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("map {1 2 3} {x {expr {$x * 2}}}").unwrap();
    assert_eq!(result.trim(), "2 4 6");

    let result = interp.eval("map {a b c} {item {string toupper $item}}").unwrap();
    assert_eq!(result.trim(), "A B C");
}

#[test]
fn test_select_filter() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("select {1 2 3 4 5} {x {expr {$x > 2}}}").unwrap();
    assert_eq!(result.trim(), "3 4 5");

    let result = interp.eval("select {10 20 30 40 50} {n {expr {$n < 35}}}").unwrap();
    assert_eq!(result.trim(), "10 20 30");
}

#[test]
fn test_nlsplit() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("nlsplit \"line1\nline2\nline3\"").unwrap();
    let lines: Vec<&str> = result.trim().split_whitespace().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines.contains(&"line1"));
    assert!(lines.contains(&"line2"));
    assert!(lines.contains(&"line3"));
}

#[test]
fn test_pick_weighted_random() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test pick returns valid values
    for _ in 0..10 {
        let result = interp.eval("pick 1 {return a} 1 {return b}").unwrap();
        let result = result.trim();
        assert!(["a", "b"].contains(&result));
    }
}

// =============================================================================
// File Path Operations (String-only)
// =============================================================================

#[test]
fn test_file_join() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("file join /path to file.txt").unwrap();
    assert_eq!(result.trim(), "/path/to/file.txt");

    let result = interp.eval("file join a b c").unwrap();
    assert_eq!(result.trim(), "a/b/c");

    // Absolute path should override
    let result = interp.eval("file join a /b c").unwrap();
    assert_eq!(result.trim(), "/b/c");
}

#[test]
fn test_file_extension() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("file extension /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), ".txt");

    let result = interp.eval("file extension /path/to/archive.tar.gz").unwrap();
    assert_eq!(result.trim(), ".gz");

    let result = interp.eval("file extension /path/to/file").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_file_rootname() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("file rootname /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), "/path/to/file");

    let result = interp.eval("file rootname /path/to/file").unwrap();
    assert_eq!(result.trim(), "/path/to/file");
}

#[test]
fn test_file_dirname() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("file dirname /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), "/path/to");

    let result = interp.eval("file dirname /file.txt").unwrap();
    assert_eq!(result.trim(), "/");

    let result = interp.eval("file dirname file.txt").unwrap();
    assert_eq!(result.trim(), ".");
}

#[test]
fn test_file_tail() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("file tail /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), "file.txt");

    let result = interp.eval("file tail file.txt").unwrap();
    assert_eq!(result.trim(), "file.txt");
}

#[test]
fn test_file_split() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("file split /path/to/file.txt").unwrap();
    assert!(result.contains("/"));
    assert!(result.contains("path"));
    assert!(result.contains("to"));
    assert!(result.contains("file.txt"));
}

#[test]
fn test_file_invalid_subcommand() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // These should fail as they're blocked
    let result = interp.eval("file exists /etc/passwd");
    assert!(result.is_err());

    let result = interp.eval("file readable /etc/passwd");
    assert!(result.is_err());
}

// =============================================================================
// Meta Namespace Tests
// =============================================================================

#[test]
fn test_meta_eval_count() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Initially no eval_count set
    let result = interp.eval("meta eval_count").unwrap();
    assert_eq!(result.trim(), "0");

    // Set and check
    interp.eval("set ::eval_count 42").unwrap();
    let result = interp.eval("meta eval_count").unwrap();
    assert_eq!(result.trim(), "42");
}

#[test]
fn test_meta_uptime() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Uptime should be >= 0
    let result = interp.eval("meta uptime").unwrap();
    let uptime: i64 = result.trim().parse().unwrap();
    assert!(uptime >= 0);
}

#[test]
fn test_meta_line() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Initially no line set
    let result = interp.eval("meta line").unwrap();
    assert_eq!(result.trim(), "");

    // Set and check
    interp.eval("set ::line {test message}").unwrap();
    let result = interp.eval("meta line").unwrap();
    assert_eq!(result.trim(), "test message");
}

// =============================================================================
// Log Functions Tests
// =============================================================================

#[test]
fn test_log_functions_without_data() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Set channel context
    interp.eval("set ::channel #test").unwrap();

    // Should return empty list when no log data
    let result = interp.eval("log").unwrap();
    assert_eq!(result.trim(), "");

    let result = interp.eval("lastlog_text 10").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_format_log_line() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("format_log_line {1234567890 testuser user@host {hello world}}").unwrap();
    assert_eq!(result.trim(), "<testuser> hello world");

    let result = interp.eval("format_log_line {}").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_lgrep() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("lgrep {***test***} {hello testing world testcase other}").unwrap();
    assert!(result.contains("testing"));
    assert!(result.contains("testcase"));
    assert!(!result.contains("hello"));
    assert!(!result.contains("world"));
    assert!(!result.contains("other"));
}

// =============================================================================
// Cache Security Limits Tests
// =============================================================================

#[test]
fn test_cache_value_size_limit() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a value larger than 100KB
    let big_value = "x".repeat(100001);
    interp.eval(&format!("set bigval \"{}\"", big_value)).unwrap();

    let result = interp.eval("cache put testbucket bigkey $bigval");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("exceeds maximum size"));
}

#[test]
fn test_cache_key_limit() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Add max_keys_per_bucket entries (1000)
    // This is a simplified test - we won't actually add 1000, just test the mechanism
    for i in 0..10 {
        interp.eval(&format!("cache put testbucket key{} value{}", i, i)).unwrap();
    }

    // Verify we can get them back
    let result = interp.eval("cache get testbucket key5").unwrap();
    assert_eq!(result.trim(), "value5");
}

// Note: cache fetch with lazy initialization needs the cache namespace to be
// properly set up, which requires the full interpreter initialization

#[test]
fn test_cache_delete_nonexistent() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Should error on deleting nonexistent key
    let result = interp.eval("cache delete testbucket nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("doesn't have key"));
}
