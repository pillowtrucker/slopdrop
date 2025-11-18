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
// HTTP Module URL Validation Tests
// =============================================================================

#[test]
fn test_http_url_normalization() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test URL normalization - should add http:// if missing
    let result = interp.eval("::httpx::normalize_url example.com").unwrap();
    assert_eq!(result.trim(), "http://example.com");

    let result = interp.eval("::httpx::normalize_url http://example.com").unwrap();
    assert_eq!(result.trim(), "http://example.com");

    let result = interp.eval("::httpx::normalize_url https://example.com").unwrap();
    assert_eq!(result.trim(), "https://example.com");
}

#[test]
fn test_http_url_validation_blocks_localhost() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Should block localhost
    let result = interp.eval("::httpx::validate_url http://localhost/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("localhost"));

    // Should block 127.0.0.1
    let result = interp.eval("::httpx::validate_url http://127.0.0.1/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("localhost"));
}

#[test]
fn test_http_url_validation_blocks_private_ips() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Should block 10.x.x.x
    let result = interp.eval("::httpx::validate_url http://10.0.0.1/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("private"));

    // Should block 192.168.x.x
    let result = interp.eval("::httpx::validate_url http://192.168.1.1/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("private"));

    // Should block 172.16-31.x.x
    let result = interp.eval("::httpx::validate_url http://172.16.0.1/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("private"));
}

#[test]
fn test_http_url_validation_blocks_link_local() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Should block 169.254.x.x
    let result = interp.eval("::httpx::validate_url http://169.254.1.1/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("link-local"));
}

#[test]
fn test_http_url_validation_allows_public_urls() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Should allow public URLs
    let result = interp.eval("::httpx::validate_url http://example.com/test").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("::httpx::validate_url https://google.com").unwrap();
    assert_eq!(result.trim(), "1");
}

#[test]
fn test_http_transfer_limit() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Set up context
    interp.eval("set ::nick testuser").unwrap();
    interp.eval("set ::nick_channel #test").unwrap();

    // Try to exceed transfer limit in single request
    let result = interp.eval("::httpx::check_limits 1000000");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("transfer limit"));
}

// =============================================================================
// HTTP Namespace Tests
// =============================================================================

#[test]
fn test_httpx_namespace_exists() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check that httpx namespace exists
    let result = interp.eval("namespace exists ::httpx").unwrap();
    assert_eq!(result.trim(), "1");
}

// =============================================================================
// HTTP Security Tests
// =============================================================================

#[test]
fn test_http_post_body_limit() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Set up context
    interp.eval("set ::nick testuser").unwrap();
    interp.eval("set ::nick_channel #test").unwrap();

    // Check the post limit variable
    let result = interp.eval("set ::httpx::post_limit").unwrap();
    assert_eq!(result.trim(), "150000");
}

#[test]
fn test_http_get_channel_and_user() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test without context set
    let result = interp.eval("::httpx::get_channel").unwrap();
    assert_eq!(result.trim(), "");

    let result = interp.eval("::httpx::get_user").unwrap();
    assert_eq!(result.trim(), "unknown");

    // Set context
    interp.eval("set ::nick testuser").unwrap();
    interp.eval("set ::nick_channel #test").unwrap();

    let result = interp.eval("::httpx::get_channel").unwrap();
    assert_eq!(result.trim(), "#test");

    let result = interp.eval("::httpx::get_user").unwrap();
    assert_eq!(result.trim(), "testuser");
}

#[test]
fn test_http_time_limit_config() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check the time limit variable
    let result = interp.eval("set ::httpx::time_limit").unwrap();
    assert_eq!(result.trim(), "5000");
}

#[test]
fn test_http_max_redirects_config() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check the max redirects variable
    let result = interp.eval("set ::httpx::max_redirects").unwrap();
    assert_eq!(result.trim(), "5");
}
