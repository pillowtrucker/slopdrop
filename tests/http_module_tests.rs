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

// =============================================================================
// Where requests leave from ([tcl] http_proxy)
// =============================================================================

use slopdrop::http_tcl_commands::{http_context_script, requester_tag};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

/// A SOCKS5 proxy that serves one small page itself, and records the
/// user name and the destination of every connection it is asked for.
fn fake_socks() -> (String, Arc<Mutex<Vec<(String, String)>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let log = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let log = log.clone();
            std::thread::spawn(move || -> std::io::Result<()> {
                let mut b = [0u8; 2];
                s.read_exact(&mut b)?;
                let mut methods = vec![0u8; b[1] as usize];
                s.read_exact(&mut methods)?;
                let mut user = String::new();
                if methods.contains(&2) {
                    s.write_all(&[5, 2])?;
                    s.read_exact(&mut b)?;
                    let mut u = vec![0u8; b[1] as usize];
                    s.read_exact(&mut u)?;
                    let mut pl = [0u8; 1];
                    s.read_exact(&mut pl)?;
                    let mut pw = vec![0u8; pl[0] as usize];
                    s.read_exact(&mut pw)?;
                    user = String::from_utf8_lossy(&u).into_owned();
                    s.write_all(&[1, 0])?;
                } else {
                    s.write_all(&[5, 0])?;
                }
                let mut head = [0u8; 4];
                s.read_exact(&mut head)?;
                let host = match head[3] {
                    1 => {
                        let mut a = [0u8; 4];
                        s.read_exact(&mut a)?;
                        std::net::Ipv4Addr::from(a).to_string()
                    }
                    3 => {
                        let mut n = [0u8; 1];
                        s.read_exact(&mut n)?;
                        let mut name = vec![0u8; n[0] as usize];
                        s.read_exact(&mut name)?;
                        String::from_utf8_lossy(&name).into_owned()
                    }
                    _ => return Ok(()),
                };
                let mut port = [0u8; 2];
                s.read_exact(&mut port)?;
                log.lock()
                    .unwrap()
                    .push((user, format!("{host}:{}", u16::from_be_bytes(port))));
                s.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])?;
                let mut req = [0u8; 4096];
                let _ = s.read(&mut req)?;
                let page = "<html><body>fetched through the proxy</body></html>";
                s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                         Connection: close\r\n\r\n{page}",
                        page.len()
                    )
                    .as_bytes(),
                )?;
                Ok(())
            });
        }
    });
    (format!("socks5h://{addr}"), seen)
}

/// Someone's `http get` (and so a link preview of theirs) goes through
/// the proxy, which resolves the name itself, under a user name of that
/// person's own: a Tor port with IsolateSOCKSAuth gives them a circuit
/// of their own.
#[test]
fn other_peoples_requests_leave_through_the_proxy_under_their_own_name() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();
    let (proxy, seen) = fake_socks();
    interp
        .eval(&http_context_script(Some(&proxy), false, "synirc", "alice"))
        .unwrap();
    let body = interp.eval("lindex [http get http://example.org/page] 2").unwrap();
    assert!(body.contains("fetched through the proxy"), "{body}");
    assert_eq!(
        seen.lock().unwrap().clone(),
        vec![(
            format!("slopdrop-{}", requester_tag("synirc", "alice")),
            "example.org:80".to_string()
        )]
    );
}

/// The owner's own requests, and every request where no proxy is
/// configured, leave from this host with no proxy at all.
#[test]
fn the_owners_requests_and_an_unset_proxy_leave_directly() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();
    let proxy = "socks5h://127.0.0.1:9050";
    interp
        .eval(&http_context_script(Some(proxy), false, "synirc", "alice"))
        .unwrap();
    let theirs = interp.eval("::httpx::proxy_options").unwrap();
    let tag = requester_tag("synirc", "alice");
    assert_eq!(
        theirs.trim(),
        format!("-proxy {proxy} -proxyuserpwd slopdrop-{tag}:x")
    );
    interp
        .eval(&http_context_script(Some(proxy), true, "synirc", "wrath"))
        .unwrap();
    assert_eq!(interp.eval("::httpx::proxy_options").unwrap().trim(), "");
    for unset in [None, Some(""), Some("  ")] {
        interp
            .eval(&http_context_script(unset, false, "synirc", "alice"))
            .unwrap();
        assert_eq!(
            interp.eval("::httpx::proxy_options").unwrap().trim(),
            "",
            "{unset:?}"
        );
    }
}

/// Each person on each network has a tag of their own, the same for
/// every request of theirs; a nick's case does not make a new person.
#[test]
fn each_requester_has_a_tag_of_their_own() {
    let a = requester_tag("synirc", "alice");
    assert_eq!(a, requester_tag("synirc", "Alice"));
    assert_ne!(a, requester_tag("synirc", "bob"));
    assert_ne!(a, requester_tag("irc4fun", "alice"));
    assert_eq!(a.len(), 16);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
}
