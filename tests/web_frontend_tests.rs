use axum::body::Body;
use axum::http::{Request, StatusCode};
use slopdrop::config::{SecurityConfig, TclConfig};
use slopdrop::tcl_service::TclService;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tempfile::TempDir;
use tokio::sync::Mutex;
use tower::util::ServiceExt; // for oneshot

#[cfg(feature = "frontend-web")]
use slopdrop::frontends::web::{create_router, AppState};

/// Helper function to create a temporary state directory
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
        privileged_users: vec!["admin!*@*".to_string()],
    };

    let tcl_config = TclConfig {
        state_path,
        state_repo: None,
        ssh_key: None,
        max_output_lines: 10,
    };

    let channel_members = Arc::new(RwLock::new(HashMap::new()));
    let service = TclService::new(security_config, tcl_config, channel_members).unwrap();

    AppState {
        tcl_service: Arc::new(Mutex::new(service)),
        config: WebConfig::default(),
    }
}

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

    assert_eq!(json["status"], "ok");
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
    let app_state = create_test_app_state(state_path).await;
    let app = create_router(app_state.clone());

    // Define a procedure as admin
    let request_body = serde_json::json!({
        "code": "proc test {} { return 42 }",
        "is_admin": true
    });

    let response = app
        .clone()
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

    // Now call the procedure as non-admin
    let request_body = serde_json::json!({
        "code": "test",
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
    assert_eq!(json["output"][0], "42");
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_eval_endpoint_pagination() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;
    let app = create_router(app_state.clone());

    // Generate lots of output
    let request_body = serde_json::json!({
        "code": "for {set i 0} {$i < 20} {incr i} { puts \"Line $i\" }",
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
    assert_eq!(json["output"].as_array().unwrap().len(), 10);
    assert_eq!(json["more_available"], true);
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_more_endpoint() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;
    let app = create_router(app_state.clone());

    // First, generate output that will be paginated
    let request_body = serde_json::json!({
        "code": "for {set i 0} {$i < 20} {incr i} { puts \"Line $i\" }",
        "is_admin": false
    });

    let _ = app
        .clone()
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

    // Now get more output
    let response = app
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

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_history_endpoint() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;
    let app = create_router(app_state.clone());

    // Make some state changes
    let request_body = serde_json::json!({
        "code": "set x 1",
        "is_admin": true
    });

    let _ = app
        .clone()
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

    // Get history
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

    assert!(json["history"].is_array());
    assert!(json["history"].as_array().unwrap().len() > 0);
}

#[cfg(feature = "frontend-web")]
#[tokio::test]
async fn test_rollback_endpoint() {
    let (_temp, state_path) = create_temp_state();
    let app_state = create_test_app_state(state_path).await;
    let app = create_router(app_state.clone());

    // Create initial state
    let request_body = serde_json::json!({
        "code": "set x 100",
        "is_admin": true
    });

    let _ = app
        .clone()
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
    let history_response = app
        .clone()
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
    let commit_hash = history_json["history"][0]["commit_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Change state
    let request_body = serde_json::json!({
        "code": "set x 200",
        "is_admin": true
    });

    let _ = app
        .clone()
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

    // Rollback
    let rollback_body = serde_json::json!({
        "commit_hash": commit_hash
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rollback")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rollback_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["message"].as_str().unwrap().contains("Rolled back"));
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[cfg(not(feature = "frontend-web"))]
#[test]
fn test_web_frontend_not_enabled() {
    // This test just ensures the test file compiles when web frontend is disabled
    assert!(true);
}
