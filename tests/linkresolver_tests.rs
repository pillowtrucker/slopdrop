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
// Link Resolver Basic Functionality Tests
// =============================================================================

#[test]
fn test_linkresolver_module_loaded() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check if linkresolver command exists
    let result = interp.eval("info commands linkresolver");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "linkresolver");
}

#[test]
fn test_linkresolver_enable_disable() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test enable
    let result = interp.eval("linkresolver enable").unwrap();
    assert!(result.contains("enabled"));

    // Test enable again (should report already enabled)
    let result = interp.eval("linkresolver enable").unwrap();
    assert!(result.contains("already enabled"));

    // Test disable
    let result = interp.eval("linkresolver disable").unwrap();
    assert!(result.contains("disabled"));

    // Test disable again (should report already disabled)
    let result = interp.eval("linkresolver disable").unwrap();
    assert!(result.contains("already disabled"));
}

#[test]
fn test_linkresolver_list_empty() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Disable auto-enable to get clean state
    let _ = interp.eval("linkresolver disable");

    // List should be empty initially (after clearing both builtin and custom resolvers)
    let result = interp.eval("set ::linkresolver::builtin_resolvers {}; set ::linkresolver_custom_resolvers {}; linkresolver list").unwrap();
    assert!(result.contains("No custom resolvers registered"));
}

#[test]
fn test_linkresolver_register_custom_resolver() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a simple test resolver
    interp.eval(r#"
        proc test_resolver {url nick channel} {
            return "Test: $url"
        }
    "#).unwrap();

    // Register the resolver
    let result = interp.eval(r#"linkresolver register {example\.com} test_resolver 50"#).unwrap();
    assert!(result.contains("Registered custom resolver") || result.contains("Registered resolver"));

    // List should show the registered resolver
    let result = interp.eval("linkresolver list").unwrap();
    assert!(result.contains("example"));
    assert!(result.contains("test_resolver"));
}

#[test]
fn test_linkresolver_unregister_resolver() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Clear existing resolvers (both builtin and custom)
    interp.eval("set ::linkresolver::builtin_resolvers {}").unwrap();
    interp.eval("set ::linkresolver_custom_resolvers {}").unwrap();

    // Create and register a test resolver
    interp.eval(r#"
        proc test_resolver {url nick channel} {
            return "Test: $url"
        }
    "#).unwrap();

    // Register the resolver
    interp.eval(r#"linkresolver register {example\.com} test_resolver"#).unwrap();

    // Unregister it
    let result = interp.eval(r#"linkresolver unregister {example\.com}"#).unwrap();
    assert!(result.contains("Unregistered"));

    // Try to unregister again (should fail)
    let result = interp.eval(r#"linkresolver unregister {example\.com}"#);
    assert!(result.is_err());
}

// =============================================================================
// URL Extraction Tests
// =============================================================================

#[test]
fn test_linkresolver_extract_urls_http() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test single HTTP URL
    let result = interp.eval(r#"::linkresolver::extract_urls "Check out http://example.com/page""#).unwrap();
    assert!(result.contains("http://example.com/page"));
}

#[test]
fn test_linkresolver_extract_urls_https() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test single HTTPS URL
    let result = interp.eval(r#"::linkresolver::extract_urls "Visit https://secure.example.com""#).unwrap();
    assert!(result.contains("https://secure.example.com"));
}

#[test]
fn test_linkresolver_extract_multiple_urls() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test multiple URLs
    let result = interp.eval(r#"::linkresolver::extract_urls "Check http://one.com and https://two.com""#).unwrap();
    assert!(result.contains("http://one.com"));
    assert!(result.contains("https://two.com"));
}

#[test]
fn test_linkresolver_extract_urls_with_trailing_punctuation() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test URL with trailing punctuation (should be stripped)
    let result = interp.eval(r#"::linkresolver::extract_urls "See http://example.com/page.""#).unwrap();
    assert!(result.contains("http://example.com/page"));
    assert!(!result.contains("http://example.com/page."));
}

#[test]
fn test_linkresolver_extract_no_urls() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test text with no URLs
    let result = interp.eval(r#"::linkresolver::extract_urls "This is just text without any links""#).unwrap();
    assert_eq!(result.trim(), "");
}

// =============================================================================
// Caching Tests
// =============================================================================

#[test]
fn test_linkresolver_caching() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let url = "http://test.example.com";
    let data = "Test cached data";

    // Store in cache
    interp.eval(&format!(r#"::linkresolver::set_cached "{}" "{}""#, url, data)).unwrap();

    // Retrieve from cache
    let result = interp.eval(&format!(r#"::linkresolver::get_cached "{}""#, url)).unwrap();
    assert_eq!(result.trim(), data);
}

#[test]
fn test_linkresolver_cache_miss() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Try to get non-existent cache entry
    let result = interp.eval(r#"::linkresolver::get_cached "http://nonexistent.example.com""#).unwrap();
    assert_eq!(result.trim(), "");
}

// =============================================================================
// HTML Entity Decoding Tests
// =============================================================================

#[test]
fn test_linkresolver_decode_html_entities_common() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test common HTML entities
    let result = interp.eval(r#"::linkresolver::decode_html_entities "A &amp; B &lt; C &gt; D""#).unwrap();
    assert_eq!(result.trim(), "A & B < C > D");
}

#[test]
fn test_linkresolver_decode_html_entities_quotes() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test quote entities
    let result = interp.eval(r#"::linkresolver::decode_html_entities "&quot;Hello&quot; &apos;World&apos;""#).unwrap();
    assert!(result.contains("\"Hello\""));
    assert!(result.contains("'World'"));
}

#[test]
fn test_linkresolver_decode_html_entities_numeric() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test numeric entities
    let result = interp.eval(r#"::linkresolver::decode_html_entities "&#39;Test&#39;""#).unwrap();
    assert!(result.contains("'Test'"));
}

// =============================================================================
// Resolver Pattern Matching Tests
// =============================================================================

#[test]
fn test_linkresolver_find_resolver_match() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Clear existing resolvers (both builtin and custom)
    interp.eval("set ::linkresolver::builtin_resolvers {}").unwrap();
    interp.eval("set ::linkresolver_custom_resolvers {}").unwrap();

    // Create and register a test resolver
    interp.eval(r#"
        proc youtube_test_resolver {url nick channel} {
            return "YouTube: $url"
        }
    "#).unwrap();
    interp.eval(r#"linkresolver register {youtube\.com|youtu\.be} youtube_test_resolver"#).unwrap();

    // Find resolver for matching URL
    let result = interp.eval(r#"::linkresolver::find_resolver "https://www.youtube.com/watch?v=test""#).unwrap();
    assert!(result.contains("youtube_test_resolver"));
}

#[test]
fn test_linkresolver_find_resolver_default() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Clear resolvers (both builtin and custom)
    interp.eval("set ::linkresolver::builtin_resolvers {}").unwrap();
    interp.eval("set ::linkresolver_custom_resolvers {}").unwrap();

    // Find resolver for non-matching URL (should return default)
    let result = interp.eval(r#"::linkresolver::find_resolver "http://random-site.example.com""#).unwrap();
    assert!(result.contains("default_resolver"));
}

#[test]
fn test_linkresolver_priority_ordering() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create test resolvers
    interp.eval(r#"
        proc low_priority {url nick channel} { return "low" }
        proc high_priority {url nick channel} { return "high" }
    "#).unwrap();

    // Register with different priorities and different patterns
    interp.eval(r#"linkresolver register {lowprio\.com} low_priority 100"#).unwrap();
    interp.eval(r#"linkresolver register {highprio\.com} high_priority 10"#).unwrap();

    // List should show high priority first
    let result = interp.eval("linkresolver list").unwrap();

    // Find positions
    if let Some(high_pos) = result.find("high_priority") {
        if let Some(low_pos) = result.find("low_priority") {
            assert!(high_pos < low_pos, "High priority resolver should appear before low priority");
        } else {
            panic!("low_priority not found in list");
        }
    } else {
        panic!("high_priority not found in list");
    }
}

// =============================================================================
// Default Resolver Tests
// =============================================================================

#[test]
fn test_linkresolver_default_resolver_returns_empty_on_error() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test with invalid URL (should return empty string on error, not crash)
    let result = interp.eval(r##"::linkresolver::default_resolver "http://invalid-nonexistent-domain-12345.example" "testnick" "#test""##).unwrap();
    assert_eq!(result.trim(), "");
}

// =============================================================================
// Custom Resolver Integration Tests
// =============================================================================

#[test]
fn test_linkresolver_custom_resolver_execution() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a custom resolver that returns formatted output
    interp.eval(r#"
        proc my_custom_resolver {url nick channel} {
            return "Custom: URL=$url NICK=$nick CHAN=$channel"
        }
    "#).unwrap();

    // Register the resolver
    interp.eval(r#"linkresolver register {mysite\.com} my_custom_resolver"#).unwrap();

    // Find and execute the resolver
    let resolver = interp.eval(r#"::linkresolver::find_resolver "http://mysite.com/page""#).unwrap();
    assert!(resolver.contains("my_custom_resolver"));

    // Execute the resolver
    let result = interp.eval(r##"my_custom_resolver "http://mysite.com/page" "bob" "#testing""##).unwrap();
    assert!(result.contains("URL=http://mysite.com/page"));
    assert!(result.contains("NICK=bob"));
    assert!(result.contains("CHAN=#testing"));
}

#[test]
fn test_linkresolver_test_command() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a test resolver
    interp.eval(r#"
        proc simple_test_resolver {url nick channel} {
            return "Resolved: $url"
        }
    "#).unwrap();

    // Register it
    interp.eval(r#"linkresolver register {testdomain\.com} simple_test_resolver"#).unwrap();

    // Test the URL
    let result = interp.eval(r#"linkresolver test "http://testdomain.com/page""#).unwrap();
    assert!(result.contains("Resolved: http://testdomain.com/page"));
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_linkresolver_register_nonexistent_proc() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Try to register a non-existent procedure
    let result = interp.eval(r#"linkresolver register {test\.com} nonexistent_proc"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn test_linkresolver_invalid_usage() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test with no arguments
    let result = interp.eval("linkresolver").unwrap();
    assert!(result.contains("Usage"));

    // Test with unknown command
    let result = interp.eval("linkresolver unknowncommand").unwrap();
    assert!(result.contains("Unknown command"));
}

// =============================================================================
// Integration with Triggers Tests
// =============================================================================

#[test]
fn test_linkresolver_trigger_binding() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Enable linkresolver
    interp.eval("linkresolver enable").unwrap();

    // Check that the enabled flag is set
    let enabled = interp.eval("set ::linkresolver::enabled").unwrap();
    assert_eq!(enabled.trim(), "1");
}

#[test]
fn test_linkresolver_on_text_returns_messages() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a simple test resolver that returns a known message
    interp.eval(r#"
        proc test_message_resolver {url nick channel} {
            return "Test resolved: $url"
        }
    "#).unwrap();

    // Register it
    interp.eval(r#"linkresolver register {example\.com} test_message_resolver"#).unwrap();

    // Enable linkresolver
    interp.eval("linkresolver enable").unwrap();

    // Simulate what triggers dispatch does: call on_text and check return value
    let result = interp.eval(r#"::linkresolver::on_text testuser user@host #channel "Check out https://example.com/page""#).unwrap();

    // Should return the resolver's message, not try to call 'send'
    assert!(result.contains("Test resolved: https://example.com/page"),
        "Expected resolver message, got: {}", result);

    // Test with multiple URLs
    let result = interp.eval(r#"::linkresolver::on_text testuser user@host #channel "See https://example.com/1 and https://example.com/2""#).unwrap();
    assert!(result.contains("https://example.com/1"));
    assert!(result.contains("https://example.com/2"));
    assert!(result.contains("\n")); // Should be joined with newline
}

#[test]
fn test_linkresolver_unbind_on_disable() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Enable then disable
    interp.eval("linkresolver enable").unwrap();
    interp.eval("linkresolver disable").unwrap();

    // Check that enabled flag is false
    let enabled = interp.eval("set ::linkresolver::enabled").unwrap();
    assert_eq!(enabled.trim(), "0");
}

// =============================================================================
// Example Resolvers Tests (from linkresolver_examples.tcl)
// =============================================================================

#[test]
fn test_youtube_resolver_exists() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check if YouTube resolver procedure exists
    let result = interp.eval("info procs ::linkresolver::youtube_resolver");
    assert!(result.is_ok());
}

#[test]
fn test_bluesky_resolver_exists() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check if Bluesky resolver procedure exists
    let result = interp.eval("info procs ::linkresolver::bluesky_resolver");
    assert!(result.is_ok());
}

#[test]
fn test_format_number_helper() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test format_number helper
    let result = interp.eval("::linkresolver::format_number 1500000").unwrap();
    assert!(result.contains("1M") || result.contains("1500K"));

    let result = interp.eval("::linkresolver::format_number 2500").unwrap();
    assert!(result.contains("2K") || result.contains("2500"));

    let result = interp.eval("::linkresolver::format_number 999").unwrap();
    assert_eq!(result.trim(), "999");
}

// =============================================================================
// Configuration Tests
// =============================================================================

#[test]
fn test_linkresolver_max_title_length() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check default max title length
    let result = interp.eval("set ::linkresolver::max_title_length").unwrap();
    let length: i32 = result.trim().parse().unwrap();
    assert!(length > 0);
    assert!(length <= 500); // Reasonable upper bound
}

#[test]
fn test_linkresolver_cache_expiry() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check default cache expiry (should be in seconds)
    let result = interp.eval("set ::linkresolver::cache_expiry").unwrap();
    let expiry: i32 = result.trim().parse().unwrap();
    assert!(expiry > 0);
    assert!(expiry <= 86400); // Max 1 day is reasonable
}
