use slopdrop::config::{SecurityConfig, TclConfig};
use slopdrop::tcl_service::{EvalContext, TclService};
use std::collections::{HashMap, HashSet};
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
    let channel_members = Arc::new(RwLock::new(HashMap::new()));
    create_test_service_with_members(state_path, channel_members)
}

/// Helper function to create a test TclService with custom channel members
fn create_test_service_with_members(state_path: PathBuf, channel_members: Arc<RwLock<HashMap<String, HashSet<String>>>>) -> TclService {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec!["admin!*@*".to_string(), "alice!*@*.example.com".to_string()],
        blacklisted_users: vec![],
        memory_limit_mb: 0, // Disabled for tests - RLIMIT_AS affects entire process
        max_recursion_depth: 1000,
        notify_self: false,
    };

    let tcl_config = TclConfig {
        state_path,
        state_repo: None,
        ssh_key: None,
        max_output_lines: 5,  // Small for testing pagination
        show_error_traces: false,
        http_proxy: None,
        stock_proxy: None,
    };

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

#[tokio::test]
async fn test_chanlist_command() {
    let (_temp, state_path) = create_temp_state();

    // Channel members are tracked per-network using a composite "network:#channel" key.
    let channel_members = Arc::new(RwLock::new(HashMap::new()));
    {
        let mut members = channel_members.write().unwrap();
        members.insert("libera:#test".to_string(), HashSet::from([
            "user1".to_string(),
            "user2".to_string(),
            "user3".to_string(),
        ]));
        members.insert("libera:#other".to_string(), HashSet::from([
            "alice".to_string(),
            "bob".to_string(),
        ]));
        // Same channel name on a different network with different members,
        // to make sure we never mix lists across networks.
        members.insert("oftc:#test".to_string(), HashSet::from([
            "ofuser".to_string(),
        ]));
    }

    let mut service = create_test_service_with_members(state_path, channel_members);

    let ctx = EvalContext::new("testuser".to_string(), "testhost".to_string())
        .with_channel("#test".to_string())
        .with_network("libera".to_string());

    // Top-level "chanlist <chan>" invocation (intercepted in tcl_thread)
    let response = service.eval("chanlist #test", ctx.clone()).await.unwrap();
    assert!(!response.is_error);
    assert_eq!(response.output.len(), 1);
    let output = &response.output[0];
    assert!(output.contains("user1"), "Should contain user1: {}", output);
    assert!(output.contains("user2"), "Should contain user2: {}", output);
    assert!(output.contains("user3"), "Should contain user3: {}", output);
    assert!(!output.contains("ofuser"), "Must not leak from oftc:#test: {}", output);

    let response = service.eval("chanlist #other", ctx.clone()).await.unwrap();
    assert!(!response.is_error);
    let output = &response.output[0];
    assert!(output.contains("alice"), "Should contain alice: {}", output);
    assert!(output.contains("bob"), "Should contain bob: {}", output);

    // chanlist for a non-existent channel should produce empty output.
    let response = service.eval("chanlist #nonexistent", ctx.clone()).await.unwrap();
    assert!(!response.is_error);
    if !response.output.is_empty() {
        assert!(response.output[0].is_empty() || response.output[0].contains("No members"));
    }

    // Calling chanlist from inside a TCL proc has to go through the registered
    // proc (not the top-level intercept), so this exercises the codepath that
    // `names` / `name` / user-defined helpers actually use.
    let response = service
        .eval("set r [chanlist #test]; lsort $r", ctx.clone())
        .await
        .unwrap();
    assert!(!response.is_error);
    let output = &response.output[0];
    assert!(output.contains("user1") && output.contains("user2") && output.contains("user3"),
        "chanlist proc should resolve current network: {}", output);

    // names returns the membership of the current channel
    let response = service.eval("lsort [names]", ctx.clone()).await.unwrap();
    assert!(!response.is_error);
    let output = &response.output[0];
    assert!(output.contains("user1") && output.contains("user2") && output.contains("user3"),
        "names should return current channel members: {}", output);

    // Switching networks must give the other network's members
    let oftc_ctx = EvalContext::new("testuser".to_string(), "testhost".to_string())
        .with_channel("#test".to_string())
        .with_network("oftc".to_string());
    let response = service.eval("lsort [names]", oftc_ctx).await.unwrap();
    assert!(!response.is_error);
    let output = &response.output[0];
    assert!(output.contains("ofuser"), "Should resolve oftc network: {}", output);
    assert!(!output.contains("user1"), "Must not leak from libera: {}", output);

    service.shutdown();
}

/// Headless, the reported room is the ONLY roster there is.
///
/// `chanlist <chan>` typed at top level is intercepted in Rust and
/// never reaches the Tcl proc of the same name, so it reads the map
/// that slopdrop's OWN IRC connection fills. With no such connection
/// that map is permanently empty — and the failure is silent: the
/// command returns an empty list, exactly as it does for a channel
/// nobody is in. Meanwhile `[names]` and `chanlist $::channel` from
/// inside a proc answered correctly the whole time, because those read
/// the Tcl array the bridge does write. One name, two sources, and
/// only one of them was bridged.
#[tokio::test]
async fn test_chanlist_top_level_answers_from_the_reported_room() {
    let (_temp, state_path) = create_temp_state();
    // EMPTY on purpose: this is what headless looks like.
    let channel_members = Arc::new(RwLock::new(HashMap::new()));
    let mut service = create_test_service_with_members(state_path, channel_members);

    let mut ctx = EvalContext::new("user:wrath".to_string(), "web".to_string())
        .with_channel("#coven".to_string())
        .with_network("irc.IRC4Fun.net".to_string());
    ctx.room = slopdrop::tcl_service::RoomContext {
        nick: Some("lewrker".to_string()),
        mask: Some("~hastur@users/lewrker".to_string()),
        channel: Some("#coven".to_string()),
        topic: Some("the coven".to_string()),
        members: vec![
            "lewrker".to_string(),
            "Psy-Q".to_string(),
            "gid".to_string(),
            "jackma".to_string(),
        ],
        bsky: None,
        owner: false,
    };

    let response = service.eval("chanlist #coven", ctx.clone()).await.unwrap();
    assert!(!response.is_error);
    assert_eq!(response.output.len(), 1);
    let output = &response.output[0];
    assert!(
        output.contains("Psy-Q") && output.contains("jackma") && output.contains("gid"),
        "the intercepted chanlist must read the reported room: {}",
        output
    );

    // A channel the room does NOT name falls through to our own map,
    // which here is empty. It must not borrow this room's roster —
    // "who is in #coven" is not an answer to "who is in #elsewhere".
    let response = service.eval("chanlist #elsewhere", ctx.clone()).await.unwrap();
    assert!(!response.is_error);
    assert!(
        response.output.is_empty() || response.output[0].is_empty(),
        "a different channel must not inherit this room's roster: {:?}",
        response.output
    );

    service.shutdown();
}

/// A Bluesky line: the post reaches Tcl, the reply spec comes back on the
/// response, and nothing about it is committed to the state repo — the
/// ::bsky namespace is per-eval input, not channel state. The next line
/// from anywhere else sees none of it.
#[tokio::test]
async fn test_a_bluesky_eval_carries_its_post_in_and_its_spec_out_and_commits_nothing() {
    let (_temp, state_path) = create_temp_state();
    let channel_members = Arc::new(RwLock::new(HashMap::new()));
    let mut service = create_test_service_with_members(state_path, channel_members);
    // Warm-up: a fresh service's FIRST eval commits the `chanlist` proc
    // the worker defines at start, whatever the line was. Spend it here
    // so the assertion below is about the Bluesky line alone.
    let _ = service
        .eval("expr 1", EvalContext::new("wrath".to_string(), "irc".to_string()))
        .await
        .unwrap();

    let mut ctx = EvalContext::new("atproto:did:plc:a".to_string(), "web".to_string())
        .with_channel("bluesky".to_string())
        .with_network("atproto".to_string());
    ctx.room = slopdrop::tcl_service::RoomContext {
        nick: Some("a.test".to_string()),
        channel: Some("bluesky".to_string()),
        bsky: Some(serde_json::json!({
            "post": {"uri": "at://did:plc:a/app.bsky.feed.post/1", "text": "{hello} #tcl",
                     "did": "did:plc:a", "handle": "a.test",
                     "facets": [{"start": 8, "end": 12, "type": "tag", "value": "tcl"}]},
        })),
        ..Default::default()
    };
    let r = service
        .eval(
            "return \"[bsky::text] [bsky::tags] [bsky::link docs https://d.test]\"",
            ctx.clone(),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    assert_eq!(r.output, vec!["{hello} #tcl tcl docs".to_string()]);
    let spec = r.bsky.expect("the spec rides the response");
    assert_eq!(spec[0]["kind"], "link");
    assert_eq!(spec[0]["uri"], "https://d.test");
    assert!(r.commit_info.is_none(), "per-eval Bluesky input is not state: {:?}", r.commit_info);

    // The next line, from IRC: no post, no spec, the IRC rendering.
    let irc = EvalContext::new("wrath".to_string(), "irc".to_string())
        .with_channel("#coven".to_string());
    let r = service
        .eval("return \"[bsky::text]|[bsky::link docs https://d.test]\"", irc)
        .await
        .unwrap();
    assert_eq!(r.output, vec!["|docs <https://d.test>".to_string()]);
    assert!(r.bsky.is_none());

    service.shutdown();
}

/// A Bluesky LIKE reaches a bound proc with its arguments and its whole
/// event in $::bsky::event, and the event is gone after the dispatch.
#[tokio::test]
async fn test_a_bluesky_like_dispatches_to_its_binding_with_the_event() {
    let (_temp, state_path) = create_temp_state();
    let channel_members = Arc::new(RwLock::new(HashMap::new()));
    let mut service = create_test_service_with_members(state_path, channel_members);
    let ctx = EvalContext::new("wrath".to_string(), "web".to_string());
    let r = service
        .eval(
            "proc on_like {h d c u} { return \"thanks $h for $u ([dict get $::bsky::event kind])\" }; \
             bind LIKE bluesky on_like",
            ctx.clone(),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);

    let ev = serde_json::json!({"kind": "LIKE", "did": "did:plc:a", "handle": "a.test",
                                "subject": "at://bot/post/1"});
    let out = service
        .dispatch_event(
            "LIKE",
            "atproto",
            &["a.test".into(), "did:plc:a".into(), "bluesky".into(), "at://bot/post/1".into()],
            None,
            &[],
            Some(&ev),
            false,
        )
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].channel, "bluesky");
    assert_eq!(out[0].message, "thanks a.test for at://bot/post/1 (LIKE)");
    assert_eq!(
        out[0].proc_name.as_deref(),
        Some("on_like"),
        "the response names the proc that wrote it"
    );
    assert!(
        out[0].invoked_hash.is_some(),
        "a stored proc's hash rides with its response: {:?}",
        out[0]
    );
    let r = service.eval("set ::bsky::event", ctx).await.unwrap();
    assert_eq!(r.output.concat(), "", "the event is cleared after its dispatch");

    service.shutdown();
}

/// A Bluesky evaluation context: the post that asked (a reply) and the
/// thread above it.
fn bsky_ctx() -> EvalContext {
    let mut ctx = EvalContext::new("atproto:did:plc:c".to_string(), "web".to_string())
        .with_channel("bluesky".to_string())
        .with_network("atproto".to_string());
    ctx.room = slopdrop::tcl_service::RoomContext {
        nick: Some("c.test".to_string()),
        channel: Some("bluesky".to_string()),
        bsky: Some(serde_json::json!({
            "post": {"uri": "at://did:plc:c/app.bsky.feed.post/ask", "text": "/tcl ...",
                     "did": "did:plc:c", "handle": "c.test",
                     "reply": {"root": "at://did:plc:r/app.bsky.feed.post/root",
                               "parent": "at://did:plc:p/app.bsky.feed.post/par"}},
            "thread": [
                {"uri": "at://did:plc:r/app.bsky.feed.post/root", "text": "the root",
                 "did": "did:plc:r", "handle": "r.test", "created": "2026-10-12T01:00:00Z"},
                {"uri": "at://did:plc:p/app.bsky.feed.post/par", "text": "hello {there}",
                 "did": "did:plc:p", "handle": "p.test", "created": "2026-10-12T01:05:00Z",
                 "reply": {"root": "at://did:plc:r/app.bsky.feed.post/root",
                           "parent": "at://did:plc:r/app.bsky.feed.post/root"}},
            ],
        })),
        ..Default::default()
    };
    ctx
}

/// `bsky_quote [bsky_get_parent] {lambda}` (USER 2026-10-12): the parent
/// is found in the thread, quoted, and the lambda's multi-line answer
/// rides as a picture (`kind art`) with no text of its own; a one-line
/// answer is the post's text. Nothing of it is committed to the state.
#[tokio::test]
async fn test_bsky_quote_quotes_the_parent_and_draws_a_multiline_answer() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);
    let _ = service
        .eval("expr 1", EvalContext::new("wrath".to_string(), "irc".to_string()))
        .await
        .unwrap();
    let r = service
        .eval(
            "bsky_quote [bsky_get_parent] {{t} {return \"  __\\n (oo) $t\"}}",
            bsky_ctx(),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    assert_eq!(r.output.concat(), "", "the drawing is the picture, not the text");
    let spec = r.bsky.expect("a spec");
    assert_eq!(spec[0]["kind"], "quote");
    assert_eq!(spec[0]["uri"], "at://did:plc:p/app.bsky.feed.post/par");
    assert_eq!(spec[1]["kind"], "art");
    assert_eq!(spec[1]["text"], "  __\n (oo) hello {there}");
    assert!(r.commit_info.is_none(), "{:?}", r.commit_info);

    let r = service
        .eval("bsky_quote [bsky_get_root] {{t} {string toupper $t}}", bsky_ctx())
        .await
        .unwrap();
    assert_eq!(r.output, vec!["THE ROOT".to_string()]);
    let spec = r.bsky.expect("a spec");
    assert_eq!(spec.as_array().unwrap().len(), 1, "a quote, no picture: {spec}");
    assert_eq!(spec[0]["uri"], "at://did:plc:r/app.bsky.feed.post/root");

    // The USER's spelling: the lambda's body unbraced. To `apply` a third
    // word is a namespace; the body is the rest as typed, groups kept.
    // And `apply` is not Tcl's: the live base library redefines it as the
    // old bot's `apply cmd args`, which this test service does too.
    let r = service
        .eval(
            "proc apply {cmd arg} { uplevel [concat $cmd $arg] }",
            EvalContext::new("wrath".to_string(), "irc".to_string()),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    let r = service
        .eval(
            "bsky_quote [bsky_get_parent] {{parentbody} string map {o 0} $parentbody}",
            bsky_ctx(),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    assert_eq!(r.output, vec!["hell0 {there}".to_string()]);
    let r = service
        .eval(
            "bsky_quote [bsky_get_parent] {{t} return \"  __\\n (oo) $t\"}",
            bsky_ctx(),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    assert_eq!(r.bsky.expect("a spec")[1]["text"], "  __\n (oo) hello {there}");
    assert!(r.commit_info.is_none(), "the lambda's proc is never committed: {:?}", r.commit_info);
    let r = service
        .eval("info commands ::_bsky_lambda_*", bsky_ctx())
        .await
        .unwrap();
    assert_eq!(r.output.concat(), "", "and is gone after the evaluation");

    // Off Bluesky the same procs answer the IRC way: the art is the text.
    let irc = EvalContext::new("wrath".to_string(), "irc".to_string())
        .with_channel("#coven".to_string());
    let r = service.eval("bsky_art \"a\\nb\"", irc).await.unwrap();
    assert_eq!(r.output, vec!["a".to_string(), "b".to_string()]);
    assert!(r.bsky.is_none());
    service.shutdown();
}

/// `bsky_caption` and `bsky_magick` (PLAN-slopdrop-art.md P6) record an
/// image macro: the URL of the picture the post shows and the op chain,
/// each op's words joined by U+001F and the ops by U+001E, for veles to
/// fetch and draw. Unknown ops and wrong arities are errors before
/// anything is recorded; off Bluesky the procs answer the picture's URL.
#[tokio::test]
async fn test_an_image_macro_records_the_threads_picture_and_its_ops() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);
    // The first evaluation defines the stock helpers; it is not ours.
    let _ = service
        .eval("expr 1", EvalContext::new("wrath".to_string(), "irc".to_string()))
        .await
        .unwrap();
    let pic = "https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:p/bafkcat@jpeg";
    let mut ctx = bsky_ctx();
    if let Some(b) = ctx.room.bsky.as_mut() {
        b["thread"][1]["embed"] = serde_json::json!({"type": "images",
            "images": [[pic, "a cat"]]});
    }
    let r = service
        .eval("bsky_caption [bsky_get_parent] {when you} {see it}", ctx.clone())
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    assert_eq!(r.output.concat(), "");
    let spec = r.bsky.expect("a spec");
    assert_eq!(spec[0]["kind"], "magick");
    assert_eq!(spec[0]["src"], pic);
    assert_eq!(spec[0]["ops"], "caption\u{1f}when you\u{1f}see it");
    assert!(r.commit_info.is_none(), "{:?}", r.commit_info);

    let r = service
        .eval(
            "bsky_magick [bsky_get_parent] -alt {a cat, upside down} {rotate 180} grayscale {resize 400 0}",
            ctx.clone(),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    let spec = r.bsky.expect("a spec");
    assert_eq!(spec[0]["ops"], "rotate\u{1f}180\u{1e}grayscale\u{1e}resize\u{1f}400\u{1f}0");
    assert_eq!(spec[0]["alt"], "a cat, upside down");

    for (code, why) in [
        ("bsky_magick [bsky_get_parent] {explode}", "unknown op"),
        ("bsky_magick [bsky_get_parent] {resize 400}", "wrong number of arguments"),
        ("bsky_magick [bsky_get_parent]", "no ops"),
        ("bsky_caption [bsky_get_root] hi", "has no picture"),
    ] {
        let r = service.eval(code, ctx.clone()).await.unwrap();
        assert!(r.is_error, "{code}: {:?}", r.output);
        assert!(r.output.concat().contains(why), "{code}: {:?}", r.output);
        assert!(r.bsky.is_none(), "{code}: nothing recorded");
    }

    let irc = EvalContext::new("wrath".to_string(), "irc".to_string())
        .with_channel("#coven".to_string());
    let r = service
        .eval(&format!("bsky_caption {pic} hi"), irc)
        .await
        .unwrap();
    assert_eq!(r.output, vec![pic.to_string()], "off Bluesky: the picture's URL");
    service.shutdown();
}

/// The base library's image procs (veles-agent PLAN-slopdrop-art.md,
/// wrap-up): once the state is loaded, `magick_scheme`, `magick` and
/// `magick_id` bound to the dead magick.buttes.org service are rebound
/// (`::magick_dsl::revive`) to record programs for veles to draw. Each
/// call returns its handle, a later program names it, and the old
/// `magick CMD SOURCE …` API becomes a program too; somebody's own
/// `magick_id` stays, and off Bluesky there is nothing to draw with.
#[tokio::test]
async fn test_magick_scheme_records_programs_after_the_base_library() {
    let (_temp, state_path) = create_temp_state();
    // A state repo made by a first start, then a base library with the
    // dead service's versions, as stolen-treasure.tcl defines them; the
    // second start loads it after the stock procs.
    create_test_service(state_path.clone()).shutdown();
    std::fs::write(
        state_path.join("stolen-treasure.tcl"),
        [
            "proc magick_scheme args { ~http://magick.buttes.org/abez/magick.rb?[? cmd scheme id [join $args]] }",
            "proc magick args {~apply magick_uncached $args}",
            "proc magick_id url {if ![regexp ^http://magick.buttes.org $url] {magick_id [magick noop $url]} else {lindex [regexp -inline {/([0-9a-f]*)\\.} $url] 1}}",
            "proc mirror_left url {magick_scheme (let ((cbimg (image $url)) (x (width cbimg)) (y (height cbimg)) (half (crop cbimg 0 0 (/ x 2) y)) (flop (flop half))) (composite cbimg flop (/ x 2) 0))}",
            "proc magick_rotate {img r} { magick_scheme (rotate [scheme_image_or_sexpr $img] $r ) }",
        ]
        .join("\n"),
    )
    .unwrap();
    let mut service = create_test_service(state_path);
    let irc = EvalContext::new("wrath".to_string(), "irc".to_string())
        .with_channel("#coven".to_string());
    for name in ["magick_scheme", "magick", "magick_id"] {
        let r = service
            .eval(&format!("info body {name}"), irc.clone())
            .await
            .unwrap();
        assert!(
            r.output.concat().contains("magick_dsl"),
            "{name} is drawn again after the base library: {:?}",
            r.output
        );
    }

    let pic = "https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:p/bafkcat@jpeg";
    let mut ctx = bsky_ctx();
    if let Some(b) = ctx.room.bsky.as_mut() {
        b["thread"][1]["embed"] = serde_json::json!({"type": "images",
            "images": [[pic, "a cat"]]});
    }
    let r = service.eval("bsky_image", ctx.clone()).await.unwrap();
    assert_eq!(r.output.concat(), pic, "the post above's picture");

    let r = service
        .eval("magick_rotate [mirror_left [bsky_image]] 45", ctx.clone())
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    assert_eq!(r.output.concat(), "magick:2", "the outer program's handle");
    let spec = r.bsky.expect("a spec");
    assert_eq!(spec[0]["kind"], "magick");
    assert_eq!(spec[0]["id"], "1");
    let first = spec[0]["scheme"].as_str().unwrap();
    assert!(
        first.starts_with("(let ((cbimg (image ") && first.contains(pic),
        "{first}"
    );
    assert_eq!(spec[1]["id"], "2");
    assert_eq!(spec[1]["scheme"], "(rotate (image \"magick:1\") 45 )");
    assert!(r.commit_info.is_none(), "{:?}", r.commit_info);

    // The old API: `magick CMD SOURCE ARG …`.
    let r = service
        .eval("magick annotate [magick resize [bsky_image] 100 50] 16 5 5 white {say \"hi\"}", ctx.clone())
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    let spec = r.bsky.expect("a spec");
    assert_eq!(spec[0]["scheme"], format!("(resize (image \"{pic}\") 100 50)"));
    assert_eq!(
        spec[1]["scheme"],
        "(annotate (image \"magick:1\") 16 5 5 \"white\" \"say \\\"hi\\\"\")"
    );
    for (code, why) in [
        ("magick solarize [bsky_image] 5", "not drawn here"),
        ("magick resize [bsky_image] big 5", "not a number"),
    ] {
        let r = service.eval(code, ctx.clone()).await.unwrap();
        assert!(r.is_error, "{code}: {:?}", r.output);
        assert!(r.output.concat().contains(why), "{code}: {:?}", r.output);
    }

    // Somebody's own magick_id is theirs.
    let r = service
        .eval("proc magick_id url { return mine:$url }", irc.clone())
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    service.eval("::magick_dsl::revive", irc.clone()).await.unwrap();
    let r = service.eval("magick_id x", irc.clone()).await.unwrap();
    assert_eq!(r.output.concat(), "mine:x");

    let r = service.eval("mirror_left https://x.test/a.png", irc).await.unwrap();
    assert!(r.is_error, "{:?}", r.output);
    assert!(r.output.concat().contains("on Bluesky only"), "{:?}", r.output);
    service.shutdown();
}

/// The thread is the channel's log while a Bluesky line runs, so the
/// procs that read IRC history read the posts above; the next evaluation
/// in that channel sees none of it.
#[tokio::test]
async fn test_a_bluesky_thread_is_the_channel_log_for_its_evaluation_only() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);
    let r = service
        .eval("join [lastlog_text 10] |", bsky_ctx())
        .await
        .unwrap();
    assert_eq!(r.output, vec!["the root|hello {there}".to_string()]);
    let r = service
        .eval("lindex [log] end 1", bsky_ctx())
        .await
        .unwrap();
    assert_eq!(r.output, vec!["p.test".to_string()], "the nick is the handle");
    let plain = EvalContext::new("wrath".to_string(), "web".to_string())
        .with_channel("bluesky".to_string());
    let r = service.eval("llength [log]", plain).await.unwrap();
    assert_eq!(r.output, vec!["0".to_string()], "cleared after the evaluation");
    service.shutdown();
}

/// What one evaluation records is capped (anybody's code writes it), and
/// a Bluesky evaluation is not paged at the IRC line cap: a drawing must
/// reach veles whole.
#[tokio::test]
async fn test_a_bluesky_evaluation_records_within_caps_and_returns_tall_output_whole() {
    let (_temp, state_path) = create_temp_state();
    let mut service = create_test_service(state_path);
    let r = service
        .eval(
            "lappend ::bsky::out [dict create kind art text [string repeat x 200000]]; \
             for {set i 0} {$i < 100} {incr i} {lappend ::bsky::out [dict create kind tag tag t$i]}; \
             return ok",
            bsky_ctx(),
        )
        .await
        .unwrap();
    let spec = r.bsky.expect("a spec");
    let recs = spec.as_array().unwrap();
    assert_eq!(recs[0]["text"].as_str().unwrap().len(), 64 * 1024);
    assert!(recs.len() <= 64, "{}", recs.len());
    let r = service
        .eval(&generate_multiline_output("line", 50), bsky_ctx())
        .await
        .unwrap();
    assert_eq!(r.output.len(), 50, "the test config pages IRC at 5");
    assert!(!r.more_available);
    service.shutdown();
}

/// An event's handlers fetch as the person whose event it is: a link
/// someone pasted is previewed through the proxy under their tag, and
/// one the owner pasted leaves directly (the bridge says `owner`).
#[tokio::test]
async fn an_events_requests_leave_as_its_senders() {
    let (_temp, state_path) = create_temp_state();
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec![],
        blacklisted_users: vec![],
        memory_limit_mb: 0,
        max_recursion_depth: 1000,
        notify_self: false,
    };
    let proxy = "socks5h://127.0.0.1:9050";
    let tcl_config = TclConfig {
        state_path,
        state_repo: None,
        ssh_key: None,
        max_output_lines: 5,
        show_error_traces: false,
        http_proxy: Some(proxy.to_string()),
        stock_proxy: None,
    };
    let mut service =
        TclService::new(security_config, tcl_config, Arc::new(RwLock::new(HashMap::new()))).unwrap();
    let ctx = EvalContext::new("wrath".to_string(), "web".to_string());
    let r = service
        .eval(
            "proc where_from {n m c t} { return \"[::httpx::proxy_options]|$::httpx::direct\" }; \
             bind TEXT * where_from",
            ctx,
        )
        .await
        .unwrap();
    assert!(!r.is_error, "{:?}", r.output);
    let args: Vec<String> = ["alice", "a@h", "#room", "hello there"]
        .map(String::from)
        .to_vec();
    let theirs = service
        .dispatch_event("TEXT", "synirc", &args, None, &[], None, false)
        .await
        .unwrap();
    let tag = slopdrop::http_tcl_commands::requester_tag("synirc", "alice");
    assert_eq!(
        theirs[0].message,
        format!("-proxy {proxy} -proxyuserpwd slopdrop-{tag}:x|0")
    );
    let owners = service
        .dispatch_event("TEXT", "synirc", &args, None, &[], None, true)
        .await
        .unwrap();
    assert_eq!(owners[0].message, "|1");
    service.shutdown();
}
