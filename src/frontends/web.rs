//! Web frontend for slopdrop using axum
//!
//! Provides HTTP REST API and WebSocket interface

use crate::config::{SecurityConfig, TclConfig, WebToken};
use crate::frontend::Frontend;
use crate::state::CommitInfo;
use crate::tcl_service::{EvalContext, EvalResponse, TclService};
use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::{
    extract::{Query, Request, State as AxumState},
    Extension,
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;
use tracing::{error, info};

/// Web frontend configuration
#[derive(Clone, Debug)]
pub struct WebConfig {
    /// Bind address
    pub bind_address: String,
    /// Port
    pub port: u16,
    /// Bearer tokens and their privilege. Empty = no authentication,
    /// which `run_server` permits only on loopback.
    pub tokens: Vec<WebToken>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            tokens: Vec::new(),
        }
    }
}

impl WebConfig {
    /// Read the `[web]` section, falling back to the defaults.
    pub fn from_file(cfg: Option<&crate::config::WebConfigFile>) -> Self {
        let d = Self::default();
        match cfg {
            None => d,
            Some(w) => Self {
                bind_address: w.bind_address.clone().unwrap_or(d.bind_address),
                port: w.port.unwrap_or(d.port),
                tokens: w.tokens.clone(),
            },
        }
    }

    /// Does this bind reach past this machine?
    fn is_loopback_bind(&self) -> bool {
        matches!(
            self.bind_address.as_str(),
            "127.0.0.1" | "::1" | "localhost" | "[::1]"
        )
    }
}

/// What one authenticated caller may do.
///
/// Carried through the request as an extension rather than read from the
/// body: `is_admin` used to arrive in the JSON, which made privilege
/// self-service — whoever could reach `/api/eval` could ask for the
/// unrestricted interpreter (exec, file, socket) and be given it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caller {
    pub admin: bool,
}

/// Constant-time compare, so a token is not discoverable a byte at a
/// time. No new dependency for a 32-byte secret.
fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub tcl_service: Arc<Mutex<TclService>>,
    pub config: WebConfig,
}

/// Request to evaluate TCL code
#[derive(Debug, Deserialize)]
struct EvalRequest {
    code: String,
    /// Who this is being run FOR. Recorded as the git commit author, so
    /// a bridge (veles forwarding a channel line) can keep the real
    /// person's name on the state change rather than attributing every
    /// commit to the bridge. It is a LABEL, not a credential: privilege
    /// comes from the bearer token.
    #[serde(default)]
    user: Option<String>,
}

/// Response from evaluation
#[derive(Debug, Serialize)]
struct EvalResponseDto {
    output: Vec<String>,
    is_error: bool,
    commit_info: Option<CommitInfo>,
    more_available: bool,
}

impl From<EvalResponse> for EvalResponseDto {
    fn from(r: EvalResponse) -> Self {
        Self {
            output: r.output,
            is_error: r.is_error,
            commit_info: r.commit_info,
            more_available: r.more_available,
        }
    }
}

/// Request to get more output
#[derive(Debug, Deserialize)]
struct MoreRequest {
    #[serde(default)]
    user: Option<String>,
}

/// Rollback request
#[derive(Debug, Deserialize)]
struct RollbackRequest {
    commit_hash: String,
}

/// Generic response
#[derive(Debug, Serialize)]
struct GenericResponse {
    success: bool,
    message: String,
}

/// Web frontend implementation
pub struct WebFrontend {
    /// Frontend name (for trait implementation)
    #[allow(dead_code)]
    name: String,
    config: WebConfig,
    tcl_service: Arc<Mutex<TclService>>,
    running: Arc<RwLock<bool>>,
}

impl WebFrontend {
    /// Create a new web frontend
    pub fn new(
        config: WebConfig,
        security_config: SecurityConfig,
        tcl_config: TclConfig,
    ) -> Result<Self> {
        let channel_members = Arc::new(RwLock::new(HashMap::new()));
        let tcl_service = TclService::new(security_config, tcl_config, channel_members)?;

        Ok(Self {
            name: "Web".to_string(),
            config,
            tcl_service: Arc::new(Mutex::new(tcl_service)),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Build the axum router
    pub fn build_router(state: AppState) -> Router {
        // NO permissive CORS.
        //
        // This used to be `allow_origin(Any).allow_headers(Any)`, which
        // on a server with authentication off — the only shipped
        // configuration, since nothing could turn it on — meant any page
        // in any browser on this machine could POST to
        // 127.0.0.1:8080/api/eval and run TCL, `is_admin: true`
        // included. A JSON POST is preflighted, and that layer answered
        // the preflight yes.
        //
        // This is a server-to-server API. It has no browser origin to
        // allow, and the bundled index page is same-origin, so the
        // correct policy is no cross-origin policy at all.
        let router = Router::new()
            .route("/", get(serve_index))
            .route("/api/eval", post(handle_eval))
            .route("/api/more", get(handle_more))
            .route("/api/history", get(handle_history))
            .route("/api/rollback", post(handle_rollback))
            .route("/api/health", get(handle_health));

        // The auth layer runs ALWAYS, not "if enabled": with no tokens
        // configured it stamps every caller as a non-admin local process
        // (the loopback posture `run_server` enforces), and with tokens
        // it requires one and reads the privilege off it. A middleware
        // that could be skipped was a middleware that was skipped.
        router
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state)
    }

    /// Run the web server
    async fn run_server(&self) -> Result<()> {
        // No credential ⇒ loopback only, ENFORCED rather than merely
        // defaulted. The previous shape defaulted to 127.0.0.1 with auth
        // off and no way to change either; the moment the bind became
        // configurable, "no tokens" had to stop meaning "and serve the
        // network an eval endpoint".
        if self.config.tokens.is_empty() && !self.config.is_loopback_bind() {
            anyhow::bail!(
                "[web] bind_address = {:?} reaches past this machine and no [[web.tokens]] \
                 are configured — that would serve an unauthenticated TCL evaluator to the \
                 network. Add a token, or bind 127.0.0.1.",
                self.config.bind_address
            );
        }
        if self.config.tokens.is_empty() {
            info!(
                "Web API is UNAUTHENTICATED on loopback: every local process may evaluate \
                 (non-admin). Add [[web.tokens]] to require a bearer and to grant admin."
            );
        } else {
            let admins = self.config.tokens.iter().filter(|t| t.admin).count();
            info!(
                "Web API requires a bearer token ({} configured, {} with admin)",
                self.config.tokens.len(),
                admins
            );
        }
        let addr: SocketAddr = format!("{}:{}", self.config.bind_address, self.config.port)
            .parse()
            .context("Invalid bind address")?;

        let state = AppState {
            tcl_service: self.tcl_service.clone(),
            config: self.config.clone(),
        };

        let app = Self::build_router(state);

        info!("Web server listening on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("Failed to bind to address")?;

        axum::serve(listener, app)
            .await
            .context("Server error")?;

        Ok(())
    }
}

#[async_trait]
impl Frontend for WebFrontend {
    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting Web frontend on {}:{}", self.config.bind_address, self.config.port);
        *self.running.write().unwrap() = true;
        self.run_server().await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Web frontend");
        *self.running.write().unwrap() = false;
        let mut service = self.tcl_service.lock().await;
        service.shutdown();
        Ok(())
    }

    fn is_running(&self) -> bool {
        *self.running.read().unwrap()
    }
}

/// Authentication, and the privilege that comes with it.
///
/// Two postures, following the rule veles' own HTTP surfaces use:
///
///   - no tokens configured ⇒ loopback only (enforced at bind time),
///     and every caller is a non-admin local process. This is what
///     `--web` always did, minus the part where the caller could ask for
///     admin in the request body.
///   - tokens configured ⇒ a valid bearer is REQUIRED, and the token
///     says whether its holder is an admin.
///
/// `/api/health` stays open either way: it answers "is this process
/// alive" and nothing else, which is what a health check is for.
async fn auth_middleware(
    AxumState(state): AxumState<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    if request.uri().path() == "/api/health" {
        return next.run(request).await;
    }

    if state.config.tokens.is_empty() {
        // Unauthenticated loopback. Never admin: the unrestricted
        // interpreter is exec/file/socket on this host, and "a local
        // process asked nicely" is not an authorization decision.
        request.extensions_mut().insert(Caller { admin: false });
        return next.run(request).await;
    }

    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();

    // Every token is compared even after a match, so the work done does
    // not depend on WHICH token was presented.
    let mut found: Option<&WebToken> = None;
    for t in &state.config.tokens {
        if ct_eq(&t.token, presented) {
            found = Some(t);
        }
    }
    match found {
        Some(t) => {
            request.extensions_mut().insert(Caller { admin: t.admin });
            next.run(request).await
        }
        None => (StatusCode::UNAUTHORIZED, "Invalid authentication token").into_response(),
    }
}

/// Create a router for testing
/// NOTE: Used in web_frontend_tests.rs for integration testing
#[allow(dead_code)]
pub fn create_router(state: AppState) -> Router {
    WebFrontend::build_router(state)
}

/// Serve the index page
async fn serve_index() -> Html<String> {
    Html(INDEX_HTML.to_string())
}

/// Handle eval request
async fn handle_eval(
    AxumState(state): AxumState<AppState>,
    Extension(caller): Extension<Caller>,
    Json(req): Json<EvalRequest>,
) -> Result<Json<EvalResponseDto>, StatusCode> {
    let user = req.user.unwrap_or_else(|| "web".to_string());
    // Admin comes from the TOKEN. It used to come from the request body,
    // which meant the caller chose their own privilege and the answer
    // was always yes.
    let ctx = EvalContext::new(user, "web".to_string()).with_admin(caller.admin);

    let mut service = state.tcl_service.lock().await;

    match service.eval(&req.code, ctx).await {
        Ok(response) => Ok(Json(response.into())),
        Err(e) => {
            error!("Eval error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle more request
async fn handle_more(
    AxumState(state): AxumState<AppState>,
    Query(req): Query<MoreRequest>,
) -> Result<Json<EvalResponseDto>, StatusCode> {
    let user = req.user.unwrap_or_else(|| "web".to_string());
    let ctx = EvalContext::new(user, "web".to_string());

    let mut service = state.tcl_service.lock().await;

    match service.more(ctx).await {
        Ok(response) => Ok(Json(response.into())),
        Err(e) => {
            error!("More error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle history request
async fn handle_history(
    AxumState(state): AxumState<AppState>,
) -> Result<Json<Vec<CommitInfo>>, StatusCode> {
    let service = state.tcl_service.lock().await;

    match service.history(20).await {
        Ok(history) => Ok(Json(history)),
        Err(e) => {
            error!("History error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle rollback request
async fn handle_rollback(
    AxumState(state): AxumState<AppState>,
    Extension(caller): Extension<Caller>,
    Json(req): Json<RollbackRequest>,
) -> Result<Json<GenericResponse>, StatusCode> {
    // Rolling the state back rewrites everyone's procs and vars. On IRC
    // that is `tclAdmin rollback`, privileged-users only; here it had no
    // check at all, so it was reachable by anyone who could reach the
    // port.
    if !caller.admin {
        return Ok(Json(GenericResponse {
            success: false,
            message: "rollback requires an admin token".to_string(),
        }));
    }
    let mut service = state.tcl_service.lock().await;

    match service.rollback(&req.commit_hash).await {
        Ok(message) => Ok(Json(GenericResponse {
            success: true,
            message,
        })),
        Err(e) => {
            error!("Rollback error: {}", e);
            Ok(Json(GenericResponse {
                success: false,
                message: format!("Rollback failed: {}", e),
            }))
        }
    }
}

/// Health check endpoint
async fn handle_health() -> Json<GenericResponse> {
    Json(GenericResponse {
        success: true,
        message: "OK".to_string(),
    })
}

/// Simple HTML interface
const INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Slopdrop TCL Evalbot</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: #1e1e1e;
            color: #d4d4d4;
            padding: 20px;
            height: 100vh;
            display: flex;
            flex-direction: column;
        }
        h1 {
            color: #569cd6;
            margin-bottom: 20px;
        }
        .container {
            display: flex;
            flex: 1;
            gap: 20px;
            min-height: 0;
        }
        .main-panel {
            flex: 2;
            display: flex;
            flex-direction: column;
            gap: 10px;
            min-width: 0;
        }
        .side-panel {
            flex: 1;
            display: flex;
            flex-direction: column;
            gap: 10px;
            min-width: 300px;
        }
        .panel {
            background: #252526;
            border: 1px solid #3e3e42;
            border-radius: 5px;
            padding: 15px;
            overflow: auto;
        }
        .panel h2 {
            color: #4ec9b0;
            margin-bottom: 10px;
            font-size: 1.2em;
        }
        #code-editor {
            flex: 1;
            min-height: 200px;
        }
        textarea {
            width: 100%;
            height: 100%;
            background: #1e1e1e;
            color: #d4d4d4;
            border: 1px solid #3e3e42;
            border-radius: 3px;
            padding: 10px;
            font-family: 'Consolas', 'Courier New', monospace;
            font-size: 14px;
            resize: none;
        }
        textarea:focus {
            outline: none;
            border-color: #569cd6;
        }
        #output {
            flex: 1;
            min-height: 200px;
            overflow-y: auto;
        }
        #output pre {
            font-family: 'Consolas', 'Courier New', monospace;
            font-size: 14px;
            white-space: pre-wrap;
            word-wrap: break-word;
        }
        .error {
            color: #f48771;
        }
        .success {
            color: #4ec9b0;
        }
        .commit-info {
            color: #dcdcaa;
            font-style: italic;
        }
        button {
            background: #0e639c;
            color: white;
            border: none;
            padding: 10px 20px;
            border-radius: 3px;
            cursor: pointer;
            font-size: 14px;
            transition: background 0.2s;
        }
        button:hover {
            background: #1177bb;
        }
        button:active {
            background: #0d5a8f;
        }
        .button-group {
            display: flex;
            gap: 10px;
        }
        #history-list {
            list-style: none;
        }
        #history-list li {
            padding: 8px;
            margin-bottom: 5px;
            background: #1e1e1e;
            border-left: 3px solid #569cd6;
            font-family: 'Consolas', 'Courier New', monospace;
            font-size: 12px;
            cursor: pointer;
        }
        #history-list li:hover {
            background: #2d2d30;
        }
        .status {
            position: fixed;
            bottom: 20px;
            right: 20px;
            background: #0e639c;
            color: white;
            padding: 10px 20px;
            border-radius: 5px;
            display: none;
        }
    </style>
</head>
<body>
    <h1>🚀 Slopdrop TCL Evalbot - Web Interface</h1>

    <div class="container">
        <div class="main-panel">
            <div class="panel" id="code-editor">
                <h2>TCL Code</h2>
                <textarea id="code" placeholder="Enter TCL code here...&#10;&#10;Example:&#10;expr {1 + 1}&#10;set myvar &quot;hello&quot;&#10;proc greet {name} { return &quot;Hello, $name!&quot; }"></textarea>
            </div>

            <div class="button-group">
                <button onclick="evalCode()">Evaluate (Ctrl+Enter)</button>
                <button onclick="getMore()">More Output</button>
                <button onclick="clearOutput()">Clear Output</button>
            </div>

            <div class="panel" id="output">
                <h2>Output</h2>
                <pre id="output-content"></pre>
            </div>
        </div>

        <div class="side-panel">
            <div class="panel">
                <h2>Git History</h2>
                <ul id="history-list"></ul>
            </div>

            <div class="panel">
                <h2>Quick Help</h2>
                <p style="font-size: 12px; line-height: 1.6;">
                    <strong>Keyboard Shortcuts:</strong><br>
                    • Ctrl+Enter: Evaluate code<br>
                    • Ctrl+L: Clear output<br>
                    <br>
                    <strong>Admin Commands:</strong><br>
                    • Click history item to rollback<br>
                    • All evaluations are saved to git<br>
                </p>
            </div>
        </div>
    </div>

    <div id="status" class="status"></div>

    <script>
        const codeEditor = document.getElementById('code');
        const outputContent = document.getElementById('output-content');
        const historyList = document.getElementById('history-list');
        const statusDiv = document.getElementById('status');

        // Keyboard shortcuts
        codeEditor.addEventListener('keydown', (e) => {
            if (e.ctrlKey && e.key === 'Enter') {
                e.preventDefault();
                evalCode();
            }
        });

        document.addEventListener('keydown', (e) => {
            if (e.ctrlKey && e.key === 'l') {
                e.preventDefault();
                clearOutput();
            }
        });

        // Evaluate TCL code
        async function evalCode() {
            const code = codeEditor.value.trim();
            if (!code) return;

            showStatus('Evaluating...');

            try {
                const response = await fetch('/api/eval', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ code, is_admin: true })
                });

                const result = await response.json();

                // Display output
                outputContent.textContent += '> ' + code + '\n';
                result.output.forEach(line => {
                    outputContent.textContent += line + '\n';
                });

                if (result.more_available) {
                    outputContent.textContent += '... (more lines available - click "More Output")\n';
                }

                if (result.commit_info) {
                    const info = result.commit_info;
                    outputContent.textContent += `[Git] ${info.commit_id.substring(0, 8)} | ${info.files_changed} files (+${info.insertions} -${info.deletions})\n`;
                    loadHistory();
                }

                outputContent.textContent += '\n';
                outputContent.scrollTop = outputContent.scrollHeight;

                showStatus('Evaluation complete', 'success');

                // Clear editor
                codeEditor.value = '';
            } catch (error) {
                showStatus('Error: ' + error.message, 'error');
            }
        }

        // Get more paginated output
        async function getMore() {
            showStatus('Getting more output...');

            try {
                const response = await fetch('/api/more');
                const result = await response.json();

                result.output.forEach(line => {
                    outputContent.textContent += line + '\n';
                });

                if (result.more_available) {
                    outputContent.textContent += '... (more lines available)\n';
                }

                outputContent.textContent += '\n';
                outputContent.scrollTop = outputContent.scrollHeight;

                showStatus('Retrieved more output', 'success');
            } catch (error) {
                showStatus('Error: ' + error.message, 'error');
            }
        }

        // Clear output
        function clearOutput() {
            outputContent.textContent = '';
        }

        // Load git history
        async function loadHistory() {
            try {
                const response = await fetch('/api/history');
                const history = await response.json();

                historyList.innerHTML = '';
                history.forEach(commit => {
                    const li = document.createElement('li');
                    li.textContent = `${commit.commit_id.substring(0, 8)} - ${commit.author} - ${commit.message.split('\n')[0]}`;
                    li.title = 'Click to rollback to this commit';
                    li.onclick = () => rollback(commit.commit_id);
                    historyList.appendChild(li);
                });
            } catch (error) {
                console.error('Failed to load history:', error);
            }
        }

        // Rollback to commit
        async function rollback(commitHash) {
            if (!confirm(`Rollback to commit ${commitHash.substring(0, 8)}? This will restart the TCL interpreter.`)) {
                return;
            }

            showStatus('Rolling back...');

            try {
                const response = await fetch('/api/rollback', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ commit_hash: commitHash })
                });

                const result = await response.json();

                if (result.success) {
                    showStatus('Rollback successful', 'success');
                    outputContent.textContent += `[Rollback] ${result.message}\n\n`;
                    loadHistory();
                } else {
                    showStatus('Rollback failed: ' + result.message, 'error');
                }
            } catch (error) {
                showStatus('Error: ' + error.message, 'error');
            }
        }

        // Show status message
        function showStatus(message, type = 'info') {
            statusDiv.textContent = message;
            statusDiv.style.display = 'block';
            statusDiv.style.background = type === 'error' ? '#f48771' : type === 'success' ? '#4ec9b0' : '#0e639c';

            setTimeout(() => {
                statusDiv.style.display = 'none';
            }, 3000);
        }

        // Load history on page load
        loadHistory();
    </script>
</body>
</html>
"#;
