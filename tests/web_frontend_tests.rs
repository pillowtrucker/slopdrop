#[cfg(feature = "frontend-web")]
use axum::body::Body;
#[cfg(feature = "frontend-web")]
use axum::http::{Request, StatusCode};
#[cfg(feature = "frontend-web")]
use slopdrop::config::{SecurityConfig, TclConfig};
#[cfg(feature = "frontend-web")]
use slopdrop::tcl_service::TclService;
#[cfg(feature = "frontend-web")]
use std::collections::HashMap;
#[cfg(feature = "frontend-web")]
use std::sync::{Arc, RwLock};
#[cfg(feature = "frontend-web")]
use tempfile::TempDir;
#[cfg(feature = "frontend-web")]
use tokio::sync::Mutex;
#[cfg(feature = "frontend-web")]
use tower::util::ServiceExt; // for oneshot

#[cfg(feature = "frontend-web")]
use slopdrop::frontends::web::{create_router, AppState};

/// Helper function to create a temporary state directory
#[cfg(feature = "frontend-web")]
fn create_temp_state() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let state_path = temp.path().join("state");
    (temp, state_path)
}

/// Helper function to create test AppState
#[cfg(feature = "frontend-web")]
async fn create_test_app_state(state_path: std::path::PathBuf) -> AppState {
    use slopdrop::frontends::web::WebConfig;

    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec!["admin!*@*".to_string(), "web!*".to_string()],
        blacklisted_users: vec![],
        memory_limit_mb: 0, // Disabled for tests - RLIMIT_AS affects entire process
        max_recursion_depth: 1000,
        // The commit-notification knob, added to SecurityConfig after this
        // file was last built. Its absence is why the whole suite stopped
        // compiling — and why none of these twelve tests had run since.
        notify_self: false,
    };

    let tcl_config = TclConfig {
        state_path,
        state_repo: None,
        ssh_key: None,
        max_output_lines: 10,
        show_error_traces: false,
    };

    let channel_members = Arc::new(RwLock::new(HashMap::new()));
    let service = TclService::new(security_config, tcl_config, channel_members).unwrap();

    AppState {
        tcl_service: Arc::new(Mutex::new(service)),
        config: WebConfig::default(),
    }
}

/// The same state, but with bearer tokens configured — the posture an
/// operator actually deploys, and the only one in which anybody is an
/// admin. Privilege belongs to the token, so testing admin at all
/// requires issuing one.
#[cfg(feature = "frontend-web")]
async fn create_test_app_state_with_tokens(state_path: std::path::PathBuf) -> AppState {
    use slopdrop::config::WebToken;
    let mut state = create_test_app_state(state_path).await;
    state.config.tokens = vec![
        WebToken {
            token: "plain-token".to_string(),
            admin: false,
            name: Some("plain".to_string()),
        },
        WebToken {
            token: "admin-token".to_string(),
            admin: true,
            name: Some("boss".to_string()),
        },
    ];
    state
}

/// What `is_admin` actually does, since it is easy to assume otherwise:
/// there is ONE interpreter (`TclThread::interp`), and both branches of
/// `handle_eval` evaluate in it, so admin does NOT hand out a second,
/// unrestricted `tclsh` — `exec` and friends are removed for everybody.
/// Admin means (a) the code runs without the caller-context injection,
/// (b) `rollback` / `blacklist` / `history` become reachable, and (c)
/// the request is first checked against `security.privileged_users` by
/// hostmask.
///
/// (c) is the observable one here, and it is what makes a clean probe:
/// an admin request from a hostmask that is NOT privileged is refused by
/// name, while a non-admin request with the same code just runs. So
/// "did the body's `is_admin` get honoured" has a yes/no answer that
/// does not depend on the test machine.
#[cfg(feature = "frontend-web")]
const PROBE_CODE: &str = "expr {6 * 7}";
#[cfg(feature = "frontend-web")]
const UNPRIVILEGED_USER: &str = "nobody-in-particular";

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_health_endpoint() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["message"], "OK");
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_eval_endpoint_basic() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    let request_body = serde_json::json!({
        "code": "expr {1 + 1}",
        "is_admin": false
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["is_error"], false);
    assert_eq!(json["output"][0], "2");
    assert_eq!(json["more_available"], false);
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_eval_endpoint_error() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    let request_body = serde_json::json!({
        "code": "invalid syntax {{{",
        "is_admin": false
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["is_error"], true);
    assert!(json["output"].as_array().unwrap().len() > 0);
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_eval_endpoint_admin() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    // Test that a caller can define and call a procedure in a single
    // request. This used to pass `"is_admin": true` and assert 42 — but
    // the SAFE interp defines procs and returns 42 perfectly well, so
    // the test proved nothing about admin. It is the ordinary-eval test
    // now, and admin has a real one below.
    let request_body = serde_json::json!({
        "code": "proc test {} { return 42 }; test"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["is_error"], false, "Error: {:?}", json["output"]);
    assert_eq!(json["output"][0], "42");
}

/// Privilege comes from the TOKEN, never from the request body.
///
/// `is_admin` used to be a field in the JSON, which made the
/// unrestricted interpreter — exec, file, socket, on this host —
/// self-service for anyone who could reach the port. With auth off by
/// default and CORS wide open, "anyone" included any page in any browser
/// on the machine.
#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_admin_comes_from_the_token_not_the_body() {
    let (_temp, state_path) = create_temp_state();
    let state = create_test_app_state_with_tokens(state_path).await;

    let probe = |token: Option<&str>, body: serde_json::Value, state: AppState| {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/eval")
            .header("content-type", "application/json");
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let req = req
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        async move { create_router(state).oneshot(req).await.unwrap() }
    };
    let read = |response: axum::response::Response| async move {
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice::<serde_json::Value>(&body).unwrap_or(serde_json::Value::Null),
        )
    };

    // No token at all, while tokens ARE configured: refused outright.
    let (status, _) = read(
        probe(
            None,
            serde_json::json!({"code": PROBE_CODE}),
            state.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a bearer is required");

    // A wrong token is refused too.
    let (status, _) = read(
        probe(
            Some("not-a-token"),
            serde_json::json!({"code": PROBE_CODE}),
            state.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A plain token runs as a NON-admin, and shouting `is_admin` in the
    // body changes nothing — which is the whole point. An unprivileged
    // hostmask plus a genuine admin request is refused (see below), so
    // the code simply running is the proof the claim was dropped.
    let (status, json) = read(
        probe(
            Some("plain-token"),
            serde_json::json!({
                "code": PROBE_CODE, "user": UNPRIVILEGED_USER, "is_admin": true
            }),
            state.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["is_error"], false, "{json:?}");
    assert_eq!(
        json["output"][0].as_str().unwrap_or("?"),
        "42",
        "the body's is_admin must be ignored, not honoured: {json:?}"
    );

    // The same request with an ADMIN token IS an admin request, and an
    // admin request from an unprivileged hostmask is refused by name.
    // That refusal is the evidence the token was read.
    let (status, json) = read(
        probe(
            Some("admin-token"),
            serde_json::json!({"code": PROBE_CODE, "user": UNPRIVILEGED_USER}),
            state.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["is_error"], true,
        "an admin token makes this an admin request: {json:?}"
    );
    assert!(
        json["output"][0]
            .as_str()
            .unwrap_or("")
            .contains("requires privileges"),
        "and slopdrop's own hostmask check is what refuses it: {json:?}"
    );

    // An admin token whose caller IS privileged runs normally — the
    // test config privileges `web!*`, and `user` defaults to `web`.
    let (status, json) = read(
        probe(
            Some("admin-token"),
            serde_json::json!({"code": PROBE_CODE}),
            state.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["is_error"], false, "{json:?}");
    assert_eq!(json["output"][0].as_str().unwrap_or("?"), "42");
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_eval_endpoint_pagination() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;

    // Generate output with 20 lines (using join for reliable output)
    let request_body = serde_json::json!({
        "code": "join [list Line0 Line1 Line2 Line3 Line4 Line5 Line6 Line7 Line8 Line9 Line10 Line11 Line12 Line13 Line14 Line15 Line16 Line17 Line18 Line19] \\n",
        "is_admin": false
    });

    let app = create_router(app_state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["is_error"], false);
    assert_eq!(json["output"].as_array().unwrap().len(), 10);
    assert_eq!(json["more_available"], true);
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_more_endpoint() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;

    // First, generate output that will be paginated
    let request_body = serde_json::json!({
        "code": "join [list Line0 Line1 Line2 Line3 Line4 Line5 Line6 Line7 Line8 Line9 Line10 Line11 Line12 Line13 Line14 Line15 Line16 Line17 Line18 Line19] \\n",
        "is_admin": false
    });

    let app1 = create_router(app_state.clone());
    let _ = app1
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Now get more output with a new router
    let app2 = create_router(app_state);
    let response = app2
        .oneshot(
            Request::builder()
                .uri("/api/more")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["is_error"], false);
    assert!(json["output"].as_array().unwrap().len() > 0);
}

/// A bridged eval's remainder has to be reachable by the room that
/// asked for it.
///
/// `eval` caches the overflow under `"{channel}:{user}"`. `more` looked
/// it up under the literal "default", because `MoreRequest` had no
/// channel field to pass one — so every caller that named a room (which
/// is every bridged call) filled a cache slot nothing could read. The
/// existing `test_more_endpoint` names no channel on EITHER call, so
/// both sides agreed on "default" and the bug was invisible to it.
#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_more_finds_the_cache_the_room_filled() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;

    let request_body = serde_json::json!({
        "code": "join [list L0 L1 L2 L3 L4 L5 L6 L7 L8 L9 L10 L11 L12 L13 L14 L15 L16 L17 L18 L19] \\n",
        "user": "user:wrath",
        "channel": "#coven",
        "network": "irc.IRC4Fun.net"
    });
    let response = create_router(app_state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["more_available"], true,
        "the fixture must actually paginate, or this test proves nothing: {json}"
    );

    // The SAME room asks for the rest.
    let response = create_router(app_state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/more?user=user:wrath&channel=%23coven")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let out = json["output"].as_array().unwrap();
    assert!(
        !out.is_empty() && out[0] != "No cached output. Run a command first.",
        "the room that filled the cache must be able to read it: {json}"
    );

    // A DIFFERENT room must not, or the key is not really the room.
    let response = create_router(app_state)
        .oneshot(
            Request::builder()
                .uri("/api/more?user=user:wrath&channel=%23elsewhere")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["output"].as_array().unwrap()[0],
        "No cached output. Run a command first.",
        "another channel must not read this room's remainder: {json}"
    );
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_history_endpoint() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    // History endpoint should return an array (may be empty for new state)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/history?limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should return a history array directly (may be empty)
    assert!(json.is_array(), "History should be an array, got: {:?}", json);
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_rollback_endpoint() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;

    // Create initial state
    let request_body = serde_json::json!({
        "code": "set x 100",
        "is_admin": true
    });

    let app1 = create_router(app_state.clone());
    let _ = app1
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Get the commit hash
    let app2 = create_router(app_state.clone());
    let history_response = app2
        .oneshot(
            Request::builder()
                .uri("/api/history?limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let history_body = axum::body::to_bytes(history_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let history_json: serde_json::Value = serde_json::from_slice(&history_body).unwrap();

    // History endpoint returns array directly, not wrapped in object
    assert!(
        history_json.is_array(),
        "History should be an array, got: {:?}",
        history_json
    );
    let history_array = history_json.as_array().unwrap();
    assert!(
        !history_array.is_empty(),
        "History should not be empty after eval. History: {:?}",
        history_json
    );

    let commit_hash = history_json[0]["commit_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Change state
    let request_body = serde_json::json!({
        "code": "set x 200",
        "is_admin": true
    });

    let app3 = create_router(app_state.clone());
    let _ = app3
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Rollback. It rewrites everyone's procs and vars — on IRC that is
    // `tclAdmin rollback`, privileged-users only — and here it had NO
    // check at all, so it was reachable by anything that could reach the
    // port. It needs an admin token now, which means this test has to
    // issue one.
    let rollback_body = serde_json::json!({
        "commit_hash": commit_hash
    });

    let rollback = |token: Option<&str>, state: AppState| {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/rollback")
            .header("content-type", "application/json");
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let req = req
            .body(Body::from(serde_json::to_vec(&rollback_body).unwrap()))
            .unwrap();
        async move { create_router(state).oneshot(req).await.unwrap() }
    };

    // Without tokens configured, nobody is an admin — the unauthenticated
    // loopback posture is deliberately not a privileged one.
    let response = rollback(None, app_state.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], false);
    assert!(
        json["message"].as_str().unwrap().contains("admin"),
        "the refusal names what is missing: {json:?}"
    );

    // A plain token is still not an admin.
    let mut with_tokens = app_state.clone();
    with_tokens.config.tokens = vec![
        slopdrop::config::WebToken {
            token: "plain-token".to_string(),
            admin: false,
            name: None,
        },
        slopdrop::config::WebToken {
            token: "admin-token".to_string(),
            admin: true,
            name: None,
        },
    ];
    let response = rollback(Some("plain-token"), with_tokens.clone()).await;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], false, "a plain token cannot roll back");

    // An admin token can.
    let response = rollback(Some("admin-token"), with_tokens).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        json["message"].as_str().unwrap().contains("Rolled back"),
        "{json:?}"
    );
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_root_endpoint_returns_html() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("text/html"));

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Slopdrop"));
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_invalid_json_returns_error() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_missing_fields_returns_error() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    // Missing 'code' field
    let request_body = serde_json::json!({
        "is_admin": false
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[cfg(not(feature = "frontend-web"))]
#[test]
fn test_web_frontend_not_enabled() {
    // This test just ensures the test file compiles when web frontend is disabled
    assert!(true);
}

/// The room a bridge reports must reach the interpreter globals a
/// channel's procs actually read.
///
/// This is the whole point of going headless: `tcl/utils.tcl` publishes
/// `[nick]`, `[names]`, `[name]` and `[hostmask]` on top of `::nick`,
/// `::mask` and `chanlist`, and `tcl/timtom.tcl` builds `channel_nicks`
/// and `random_other_nick` on top of those. Measured against the real
/// #coven state on 2026-09-11: 18 stored procs call `[nick]`, 9 call
/// `[name]`, 5 call `[names]`. Without this they all answer nothing,
/// silently, the moment the bot leaves IRC.
#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn a_bridges_room_reaches_the_procs_that_read_it() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    // One throwaway evaluation first: a FRESH state repo commits the
    // built-in procs the first time it sees them (`chanlist` among
    // them), and that is not what this test is about.
    let warmup = serde_json::json!({"code": "expr 1"});
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&warmup).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let request_body = serde_json::json!({
        "code": "list [nick] [channel] [hostmask] [topic] [lsort [names]]",
        "user": "irc:irc4fun/Demotion!~d@bouncer",
        "nick": "Demotion",
        "mask": "~d@bouncer",
        "channel": "#coven",
        "network": "irc4fun",
        "topic": "the topic",
        "members": ["Psy-Q", "gid", "cromega"],
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["is_error"], false, "{json}");
    assert_eq!(
        json["output"][0], "Demotion #coven ~d@bouncer {the topic} {Psy-Q cromega gid}",
        "every global the procs read comes from the request: {json}"
    );
    // …and reporting the room is not a state CHANGE. `::topic` is new,
    // so it was not on the internal-variable list the state diff filters
    // by, and every bridged evaluation committed "+var: topic" to the
    // channel's git repo — a commit per `tcl` line, and veles announcing
    // "(state: +var: topic)" on every reply.
    assert!(
        json["commit_info"].is_null(),
        "telling the interpreter where it is must not be a commit: {json}"
    );
}

/// …and a request that reports no room does not invent one.
///
/// The interpreter keeps its globals between evaluations, so a blank
/// `::channel` is worse than an absent one: it makes `[names]` answer
/// "nobody is here" with confidence. An API caller who never heard of
/// the room fields must leave the last real room alone.
#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn a_request_with_no_room_leaves_the_last_one_alone() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state(state_path).await);

    let with_room = serde_json::json!({
        "code": "expr 1",
        "user": "bridge",
        "channel": "#coven",
        "network": "irc4fun",
        "members": ["Psy-Q", "gid"],
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&with_room).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // A plain caller, the shape every existing client sends.
    let bare = serde_json::json!({"code": "list [llength [names]] [channel]"});
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&bare).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["output"][0], "2 #coven",
        "the roster the bridge reported survives a request that reported \
         none: {json}"
    );
}

/// A request may name its own nick and mask, so those must never be the
/// ones the privilege check reads.
///
/// `handle_eval` builds `nick!host` from the AUTHORIZATION identity —
/// `user` plus the frontend's own `web` host — and matches it against
/// `privileged_users`. If the display fields fed that instead, an
/// ordinary caller could name `admin!anything` and be believed. The
/// admin token is still required, so this is the second gate, not the
/// first; it is also the one a bridge's own compromise would reach.
#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn a_request_cannot_name_its_own_privilege() {
    let (_temp, state_path) = create_temp_state();
    let app = create_router(create_test_app_state_with_tokens(state_path).await);

    let request_body = serde_json::json!({
        "code": PROBE_CODE,
        "user": UNPRIVILEGED_USER,
        // `privileged_users` in the test state is `admin!*@*` / `web!*`.
        "nick": "admin",
        "mask": "anything@anywhere",
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/eval")
                .header("content-type", "application/json")
                .header("authorization", "Bearer admin-token")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let output = json["output"][0].as_str().unwrap_or_default();
    assert!(
        output.contains("requires privileges"),
        "the display nick must not reach the hostmask check: {json}"
    );
}
