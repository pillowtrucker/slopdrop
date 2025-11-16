use slopdrop::config::{SecurityConfig, TclConfig};
use slopdrop::tcl_service::{EvalContext, TclService};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tempfile::TempDir;

/// Helper function to create a temporary state directory
fn create_temp_state() -> (TempDir, PathBuf) {
    let temp = TempDir::new().unwrap();
    let state_path = temp.path().join("state");
    (temp, state_path)
}

/// Helper function to generate TCL code that produces N lines of output
fn generate_multiline_output(prefix: &str, count: usize) -> String {
    let lines: Vec<String> = (0..count).map(|i| format!("{}{}", prefix, i)).collect();
    format!("join [list {}] \\n", lines.join(" "))
}

/// Helper function to create a test TclService
fn create_test_service(state_path: PathBuf) -> TclService {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec!["admin!*@*".to_string(), "alice!*@*.example.com".to_string()],
    };

    let tcl_config = TclConfig {
        state_path,
        state_repo: None,
        ssh_key: None,
        max_output_lines: 5,  // Small for testing pagination
    };

    let channel_members = Arc::new(RwLock::new(HashMap::new()));

    TclService::new(security_config, tcl_config, channel_members).unwrap()
}

#[tokio::test]
async fn test_basic_eval() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string());

    let response = service.eval("expr {1 + 1}", ctx).await.unwrap();

    assert!(!response.is_error);
    assert_eq!(response.output.len(), 1);
    assert_eq!(response.output[0], "2");
    assert!(!response.more_available);

    service.shutdown();
}

#[tokio::test]
async fn test_eval_with_channel() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string())
        .with_channel("#test".to_string());

    let response = service.eval("expr {2 * 3}", ctx).await.unwrap();

    assert!(!response.is_error);
    assert_eq!(response.output.len(), 1);
    assert_eq!(response.output[0], "6");

    service.shutdown();
}

#[tokio::test]
async fn test_eval_error() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string());

    let response = service.eval("invalid tcl syntax {{{", ctx).await.unwrap();

    assert!(response.is_error);
    assert!(!response.output.is_empty());

    service.shutdown();
}

#[tokio::test]
async fn test_eval_with_admin() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    // Admin can define procedures
    let ctx_admin = EvalContext::new("admin".to_string(), "user@localhost".to_string())
        .with_admin(true);
    let response = service.eval("proc test {} { return 42 }", ctx_admin.clone()).await.unwrap();
    assert!(!response.is_error);

    // Regular users can also call defined procedures
    let ctx_user = EvalContext::new("testuser".to_string(), "testhost".to_string());
    let response = service.eval("test", ctx_user.clone()).await.unwrap();
    assert!(!response.is_error);
    assert_eq!(response.output[0], "42");

    // In slopdrop, both admin and non-admin can define procs (no restriction)
    let response = service.eval("proc userproc {x} { expr {$x * 2} }", ctx_user).await.unwrap();
    assert!(!response.is_error);

    service.shutdown();
}

#[tokio::test]
async fn test_pagination_basic() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string())
        .with_channel("#test".to_string());

    // Generate output with newlines (TCL join creates multi-line output)
    let code = "join [list Line0 Line1 Line2 Line3 Line4 Line5 Line6 Line7 Line8 Line9] \\n";
    let response = service.eval(code, ctx.clone()).await.unwrap();

    assert!(!response.is_error);
    assert_eq!(response.output.len(), 5);
    assert_eq!(response.output[0], "Line0");
    assert_eq!(response.output[4], "Line4");
    assert!(response.more_available);

    // Get more output
    let more_response = service.more(ctx.clone()).await.unwrap();
    assert_eq!(more_response.output.len(), 5);
    assert_eq!(more_response.output[0], "Line5");
    assert_eq!(more_response.output[4], "Line9");
    assert!(!more_response.more_available);

    // Try to get more when cache is empty (should indicate no cached output)
    let no_more = service.more(ctx).await.unwrap();
    assert_eq!(no_more.output.len(), 1);
    assert_eq!(no_more.output[0], "No cached output. Run a command first.");
    assert!(!no_more.more_available);

    service.shutdown();
}

#[tokio::test]
async fn test_pagination_multiple_pages() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string())
        .with_channel("#test".to_string());

    // Generate 20 lines of output (max_output_lines is 5)
    let code = generate_multiline_output("Line", 20);
    let response = service.eval(&code, ctx.clone()).await.unwrap();

    assert_eq!(response.output.len(), 5);
    assert!(response.more_available);

    // Page 2
    let page2 = service.more(ctx.clone()).await.unwrap();
    assert_eq!(page2.output.len(), 5);
    assert_eq!(page2.output[0], "Line5");
    assert!(page2.more_available);

    // Page 3
    let page3 = service.more(ctx.clone()).await.unwrap();
    assert_eq!(page3.output.len(), 5);
    assert_eq!(page3.output[0], "Line10");
    assert!(page3.more_available);

    // Page 4 (last page)
    let page4 = service.more(ctx.clone()).await.unwrap();
    assert_eq!(page4.output.len(), 5);
    assert_eq!(page4.output[0], "Line15");
    assert!(!page4.more_available);

    service.shutdown();
}

#[tokio::test]
async fn test_pagination_per_user() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx1 = EvalContext::new("user1".to_string(), "host1".to_string())
        .with_channel("#test".to_string());
    let ctx2 = EvalContext::new("user2".to_string(), "host2".to_string())
        .with_channel("#test".to_string());

    // User1 generates output
    let code = generate_multiline_output("User1Line", 10);
    let response1 = service.eval(&code, ctx1.clone()).await.unwrap();
    assert_eq!(response1.output.len(), 5);
    assert!(response1.more_available);

    // User2 generates output
    let code = generate_multiline_output("User2Line", 10);
    let response2 = service.eval(&code, ctx2.clone()).await.unwrap();
    assert_eq!(response2.output.len(), 5);
    assert!(response2.more_available);

    // User1 gets their more
    let more1 = service.more(ctx1.clone()).await.unwrap();
    assert_eq!(more1.output[0], "User1Line5");

    // User2 gets their more
    let more2 = service.more(ctx2.clone()).await.unwrap();
    assert_eq!(more2.output[0], "User2Line5");

    service.shutdown();
}

#[tokio::test]
async fn test_pagination_per_channel() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx1 = EvalContext::new("user1".to_string(), "host1".to_string())
        .with_channel("#channel1".to_string());
    let ctx2 = EvalContext::new("user1".to_string(), "host1".to_string())
        .with_channel("#channel2".to_string());

    // Same user, different channels
    let code = generate_multiline_output("Channel1Line", 10);
    let response1 = service.eval(&code, ctx1.clone()).await.unwrap();
    assert!(response1.more_available);

    let code = generate_multiline_output("Channel2Line", 10);
    let response2 = service.eval(&code, ctx2.clone()).await.unwrap();
    assert!(response2.more_available);

    // Get more from each channel
    let more1 = service.more(ctx1.clone()).await.unwrap();
    assert!(more1.output[0].starts_with("Channel1"));

    let more2 = service.more(ctx2.clone()).await.unwrap();
    assert!(more2.output[0].starts_with("Channel2"));

    service.shutdown();
}

#[tokio::test]
async fn test_more_without_eval() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string());

    // Try to get more without running anything first
    let response = service.more(ctx).await.unwrap();
    assert_eq!(response.output.len(), 1);
    assert_eq!(response.output[0], "No cached output. Run a command first.");
    assert!(!response.more_available);

    service.shutdown();
}

#[tokio::test]
async fn test_is_admin() {
    let (_temp, state_path) = create_temp_state();
    let service = create_test_service(state_path);

    // Check admin patterns
    assert!(service.is_admin("admin!user@localhost"));
    assert!(service.is_admin("admin!~admin@192.168.1.1"));
    assert!(service.is_admin("alice!user@www.example.com"));
    assert!(service.is_admin("alice!~alice@subdomain.example.com"));

    // Check non-admin
    assert!(!service.is_admin("bob!user@localhost"));
    assert!(!service.is_admin("alice!user@other.com"));
    assert!(!service.is_admin("user!admin@localhost"));
    assert!(!service.is_admin("alice!user@example.com")); // Doesn't match *.example.com
}

#[tokio::test]
async fn test_history() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("admin".to_string(), "user@localhost".to_string())
        .with_admin(true);

    // Make some state changes to create history
    service.eval("set x 1", ctx.clone()).await.unwrap();
    service.eval("set y 2", ctx.clone()).await.unwrap();
    service.eval("set z 3", ctx.clone()).await.unwrap();

    // Get history
    let history = service.history(10).await.unwrap();

    // Should have at least 3 commits
    assert!(history.len() >= 3);

    // Each commit should have required fields
    for commit in &history {
        assert!(!commit.commit_id.is_empty());
        assert!(!commit.author.is_empty());
        assert!(!commit.message.is_empty());
    }

    service.shutdown();
}

#[tokio::test]
async fn test_history_limit() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("admin".to_string(), "user@localhost".to_string())
        .with_admin(true);

    // Create 10 commits
    for i in 0..10 {
        service.eval(&format!("set var{} {}", i, i), ctx.clone()).await.unwrap();
    }

    // Get limited history
    let history = service.history(5).await.unwrap();

    // Should have exactly 5 commits
    assert_eq!(history.len(), 5);

    service.shutdown();
}

#[tokio::test]
async fn test_rollback() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("admin".to_string(), "user@localhost".to_string())
        .with_admin(true);

    // Set initial value
    service.eval("set x 100", ctx.clone()).await.unwrap();

    // Get the commit hash
    let history = service.history(1).await.unwrap();
    let commit_hash = history[0].commit_id.clone();

    // Change the value
    service.eval("set x 200", ctx.clone()).await.unwrap();

    // Verify new value
    let response = service.eval("set x", ctx.clone()).await.unwrap();
    assert_eq!(response.output[0], "200");

    // Rollback to previous commit
    let rollback_msg = service.rollback(&commit_hash).await.unwrap();
    assert!(rollback_msg.contains("Rolled back"));

    // Verify old value is restored
    let response = service.eval("set x", ctx.clone()).await.unwrap();
    assert_eq!(response.output[0], "100");

    service.shutdown();
}

#[tokio::test]
async fn test_state_persistence_across_evals() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("admin".to_string(), "user@localhost".to_string())
        .with_admin(true);

    // Set a variable
    service.eval("set myvar \"test value\"", ctx.clone()).await.unwrap();

    // Read it back in a different eval
    let response = service.eval("set myvar", ctx.clone()).await.unwrap();
    assert_eq!(response.output[0], "test value");

    // Define a procedure
    service.eval("proc double {x} { expr {$x * 2} }", ctx.clone()).await.unwrap();

    // Call the procedure
    let response = service.eval("double 21", ctx.clone()).await.unwrap();
    assert_eq!(response.output[0], "42");

    service.shutdown();
}

#[tokio::test]
async fn test_commit_info_on_state_change() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("admin".to_string(), "user@localhost".to_string())
        .with_admin(true);

    // State-changing eval should return commit info
    let response = service.eval("set newvar 123", ctx.clone()).await.unwrap();
    assert!(response.commit_info.is_some());

    let commit_info = response.commit_info.unwrap();
    assert!(!commit_info.commit_id.is_empty());
    assert!(!commit_info.author.is_empty());

    service.shutdown();
}

#[tokio::test]
async fn test_no_pagination_for_small_output() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string());

    // Generate only 3 lines (max_output_lines is 5)
    let code = generate_multiline_output("Line", 3);
    let response = service.eval(&code, ctx).await.unwrap();

    assert_eq!(response.output.len(), 3);
    assert!(!response.more_available);

    service.shutdown();
}

#[tokio::test]
async fn test_empty_output() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx = EvalContext::new("admin".to_string(), "user@localhost".to_string())
        .with_admin(true);

    // Command with no output
    let response = service.eval("set x 1", ctx).await.unwrap();

    assert!(!response.is_error);
    // set command returns the value
    assert_eq!(response.output.len(), 1);
    assert_eq!(response.output[0], "1");

    service.shutdown();
}

#[tokio::test]
async fn test_concurrent_users() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);

    let ctx1 = EvalContext::new("user1".to_string(), "host1".to_string())
        .with_channel("#test".to_string());
    let ctx2 = EvalContext::new("user2".to_string(), "host2".to_string())
        .with_channel("#test".to_string());

    // Both users evaluate at the same time
    let code1 = "expr {10 + 10}";
    let code2 = "expr {20 + 20}";

    let response1 = service.eval(code1, ctx1).await.unwrap();
    let response2 = service.eval(code2, ctx2).await.unwrap();

    assert_eq!(response1.output[0], "20");
    assert_eq!(response2.output[0], "40");

    service.shutdown();
}
