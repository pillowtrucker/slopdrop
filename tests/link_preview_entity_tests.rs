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
// HTML Entity Decoding Tests
// =============================================================================

#[test]
fn test_decode_basic_html_entities() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "Hello &amp; World""#).unwrap();
    assert_eq!(result.trim(), "Hello & World");
}

#[test]
fn test_decode_quotes() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "&quot;Hello&quot;""#).unwrap();
    assert!(result.trim().contains("Hello"));
}

#[test]
fn test_decode_lt_gt() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "&lt;tag&gt;""#).unwrap();
    assert_eq!(result.trim(), "<tag>");
}

#[test]
fn test_decode_nbsp_to_space() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "Hello&nbsp;World""#).unwrap();
    assert_eq!(result.trim(), "Hello World");
}

#[test]
fn test_decode_numeric_decimal_entity() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // &#65; = 'A'
    let result = interp.eval(r#"::linkresolver::decode_html_entities "&#65;BC""#).unwrap();
    assert_eq!(result.trim(), "ABC");
}

#[test]
fn test_decode_numeric_hex_entity() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // &#x41; = 'A'
    let result = interp.eval(r#"::linkresolver::decode_html_entities "&#x41;BC""#).unwrap();
    assert_eq!(result.trim(), "ABC");
}

#[test]
fn test_decode_hex_entity_uppercase() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // &#X41; = 'A' (uppercase X)
    let result = interp.eval(r#"::linkresolver::decode_html_entities "&#X41;BC""#).unwrap();
    assert_eq!(result.trim(), "ABC");
}

#[test]
fn test_decode_numeric_160_nbsp() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // &#160; is non-breaking space, should become regular space
    let result = interp.eval(r#"::linkresolver::decode_html_entities "Hello&#160;World""#).unwrap();
    assert_eq!(result.trim(), "Hello World");
}

#[test]
fn test_decode_hex_a0_nbsp() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // &#xa0; is non-breaking space in hex, should become regular space after normalization
    let result = interp.eval(r#"::linkresolver::decode_html_entities "Hello&#xa0;World""#).unwrap();
    // The hex entity decodes to U+00A0, then the unicode normalization maps it to space
    assert_eq!(result.trim(), "Hello World");
}

#[test]
fn test_decode_hellip_entity() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "Wait&hellip;""#).unwrap();
    assert_eq!(result.trim(), "Wait...");
}

#[test]
fn test_decode_mdash_ndash() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "A&mdash;B&ndash;C""#).unwrap();
    assert_eq!(result.trim(), "A--B-C");
}

#[test]
fn test_decode_smart_quotes() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "&lsquo;Hello&rsquo;""#).unwrap();
    assert_eq!(result.trim(), "'Hello'");
}

#[test]
fn test_decode_double_smart_quotes() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "&ldquo;Hello&rdquo;""#).unwrap();
    // Both ldquo and rdquo decode to regular double quote
    assert!(result.trim().contains("Hello"));
    assert!(result.trim().starts_with('"') || result.trim().starts_with("\""));
}

#[test]
fn test_decode_shy_removed() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // &shy; (soft hyphen) should be removed entirely
    let result = interp.eval(r#"::linkresolver::decode_html_entities "inter&shy;national""#).unwrap();
    assert_eq!(result.trim(), "international");
}

#[test]
fn test_decode_guillemets() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "&laquo;Hello&raquo;""#).unwrap();
    assert_eq!(result.trim(), "<<Hello>>");
}

#[test]
fn test_decode_numeric_39_apostrophe() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "it&#39;s""#).unwrap();
    assert_eq!(result.trim(), "it's");
}

#[test]
fn test_decode_multiple_entities_in_title() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::decode_html_entities "Tom &amp; Jerry &mdash; The &lsquo;Classic&rsquo; Show""#).unwrap();
    assert_eq!(result.trim(), "Tom & Jerry -- The 'Classic' Show");
}

#[test]
fn test_backslash_space_removal() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test that backslash-space artifacts are cleaned up
    let result = interp.eval(r#"::linkresolver::decode_html_entities "Hello\\ World""#).unwrap();
    assert_eq!(result.trim(), "Hello World");
}

// =============================================================================
// URL Extraction Tests
// =============================================================================

#[test]
fn test_extract_urls_basic() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::extract_urls "check out https://example.com cool""#).unwrap();
    assert!(result.trim().contains("https://example.com"));
}

#[test]
fn test_extract_urls_strips_trailing_punctuation() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::extract_urls "see https://example.com, or this""#).unwrap();
    // Should strip trailing comma
    assert!(result.trim().contains("https://example.com"));
    assert!(!result.trim().contains("https://example.com,"));
}

#[test]
fn test_extract_urls_multiple() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::extract_urls "https://a.com and http://b.com""#).unwrap();
    assert!(result.contains("https://a.com"));
    assert!(result.contains("http://b.com"));
}

#[test]
fn test_extract_urls_no_urls() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval(r#"::linkresolver::extract_urls "no links here""#).unwrap();
    assert!(result.trim().is_empty());
}
