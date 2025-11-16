//! Web frontend for slopdrop using axum
//!
//! Provides HTTP REST API and WebSocket interface

use crate::config::{SecurityConfig, TclConfig};
use crate::frontend::Frontend;
use crate::state::CommitInfo;
use crate::tcl_service::{EvalContext, EvalResponse, TclService};
use crate::types::ChannelMembers;
use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::{
    extract::{Query, State as AxumState, WebSocketUpgrade},
    http::{Method, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

/// Web frontend configuration
#[derive(Clone, Debug)]
pub struct WebConfig {
    /// Bind address
    pub bind_address: String,
    /// Port
    pub port: u16,
    /// Enable authentication
    pub enable_auth: bool,
    /// Auth token (if auth enabled)
    pub auth_token: Option<String>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            enable_auth: false,
            auth_token: None,
        }
    }
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
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    is_admin: bool,
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
        // CORS configuration
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST])
            .allow_headers(Any);

        Router::new()
            .route("/", get(serve_index))
            .route("/api/eval", post(handle_eval))
            .route("/api/more", get(handle_more))
            .route("/api/history", get(handle_history))
            .route("/api/rollback", post(handle_rollback))
            .route("/api/health", get(handle_health))
            .layer(cors)
            .with_state(state)
    }

    /// Run the web server
    async fn run_server(&self) -> Result<()> {
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

/// Create a router for testing
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
    Json(req): Json<EvalRequest>,
) -> Result<Json<EvalResponseDto>, StatusCode> {
    let user = req.user.unwrap_or_else(|| "web".to_string());
    let ctx = EvalContext::new(user, "web".to_string()).with_admin(req.is_admin);

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
    Json(req): Json<RollbackRequest>,
) -> Result<Json<GenericResponse>, StatusCode> {
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
