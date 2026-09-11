//! The veles bridge — asking a model a question from inside Tcl.
//!
//! WHY THIS IS A NATIVE COMMAND AND NOT A `tcl/*.tcl` FILE
//!
//! Every other slopdrop command is Tcl, loaded from `tcl/`. This one
//! cannot be, and the reason is worth writing down because it looks like
//! gratuitous FFI otherwise.
//!
//! `SafeTclInterp` is ONE interpreter with the dangerous commands
//! removed — not a master/safe pair with aliases across the boundary.
//! There is therefore no variable, namespace or proc that user Tcl
//! cannot read and rewrite. A bridge implemented in Tcl would have to
//! keep veles' URL and bearer token somewhere in that interpreter, and
//! the first thing a channel would do is `puts $::veles::token`. Worse,
//! it could point the URL somewhere else and make the bot POST its own
//! credential to a listener.
//!
//! Holding both in Rust, behind `Tcl_CreateObjCommand`, is the only
//! arrangement where user code can USE the bridge without being able to
//! read it or aim it.
//!
//! # What it costs
//!
//! An `ai` call spends the operator's model tokens, and any proc can
//! make one. So it carries its own rate limits — per eval, per user, per
//! minute — in the same spirit as `tcl/http.tcl`'s, for the same reason:
//! the interpreter is a public surface.

use std::ffi::{c_int, c_void, CStr, CString};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// `[veles]` in config.toml.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct VelesConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Base URL of veles' A2A server, e.g. `http://127.0.0.1:9900`.
    #[serde(default)]
    pub url: Option<String>,
    /// Bearer token. veles reads `A2A_PEER_TOKENS="slopdrop:<tok>"`, and
    /// the peer NAME on that line is the identity its ACL resolves — so
    /// `[acl.roles]."a2a:slopdrop" = "user"` over there decides what
    /// this bridge may do. Empty means no credential, which veles
    /// accepts only on loopback.
    #[serde(default)]
    pub token: Option<String>,
    /// Env var to read the token from instead, so config.toml need not
    /// hold a secret.
    #[serde(default)]
    pub token_env: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Model calls one EVAL may make. Small on purpose: a proc that
    /// wants a dozen completions is a proc that wants a bill.
    #[serde(default = "default_per_eval")]
    pub max_per_eval: u32,
    /// Model calls per minute, across everybody.
    #[serde(default = "default_per_minute")]
    pub max_per_minute: u32,
}

fn default_timeout_ms() -> u64 {
    120_000
}
fn default_per_eval() -> u32 {
    2
}
fn default_per_minute() -> u32 {
    10
}

/// The bridge, as the Tcl command sees it.
pub struct VelesBridge {
    url: String,
    token: Option<String>,
    timeout: Duration,
    max_per_eval: u32,
    max_per_minute: u32,
    state: Mutex<Rates>,
}

#[derive(Default)]
struct Rates {
    /// Calls in the current eval, reset by `begin_eval`.
    this_eval: u32,
    /// Instants of the calls still inside the minute.
    recent: Vec<Instant>,
}

impl VelesBridge {
    /// `None` when the bridge is not configured, so the command is not
    /// registered at all and Tcl reports `invalid command name "ai"` —
    /// which is a better answer than a command that always errors.
    pub fn new(cfg: &VelesConfig) -> Option<Arc<Self>> {
        if !cfg.enabled {
            return None;
        }
        let url = cfg.url.as_deref().map(str::trim).filter(|u| !u.is_empty())?;
        let token = cfg
            .token
            .clone()
            .or_else(|| cfg.token_env.as_deref().and_then(|k| std::env::var(k).ok()))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        Some(Arc::new(Self {
            url: url.trim_end_matches('/').to_string(),
            token,
            timeout: Duration::from_millis(cfg.timeout_ms),
            max_per_eval: cfg.max_per_eval,
            max_per_minute: cfg.max_per_minute,
            state: Mutex::new(Rates::default()),
        }))
    }

    /// Called by the eval loop before each evaluation, so the per-eval
    /// budget is per EVAL and not per process.
    pub fn begin_eval(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.this_eval = 0;
        }
    }

    fn claim(&self) -> Result<(), String> {
        let mut s = self.state.lock().map_err(|_| "bridge lock poisoned")?;
        if s.this_eval >= self.max_per_eval {
            return Err(format!(
                "ai: {} model calls already made in this evaluation (the per-eval limit)",
                s.this_eval
            ));
        }
        let now = Instant::now();
        s.recent
            .retain(|t| now.duration_since(*t) < Duration::from_secs(60));
        if s.recent.len() as u32 >= self.max_per_minute {
            return Err(format!(
                "ai: {} model calls in the last minute (the per-minute limit) — try again shortly",
                s.recent.len()
            ));
        }
        s.this_eval += 1;
        s.recent.push(now);
        Ok(())
    }

    /// One `message/send`, blocking. Correct here: the Tcl interpreter
    /// runs on its own std thread, which is exactly where blocking IO
    /// belongs.
    fn ask(&self, prompt: &str, context_id: &str) -> Result<String, String> {
        self.claim()?;
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "message/send",
            "params": {"message": {
                "role": "user",
                "contextId": context_id,
                "parts": [{"kind": "text", "text": prompt}],
            }},
        });
        let mut req = ureq::post(&format!("{}/", self.url))
            .timeout(self.timeout)
            .set("content-type", "application/json");
        if let Some(t) = &self.token {
            req = req.set("authorization", &format!("Bearer {t}"));
        }
        let resp = match req.send_json(body) {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                if code == 401 {
                    return Err(
                        "ai: veles refused the bridge token — check [veles] token against \
                         veles' A2A_PEER_TOKENS"
                            .into(),
                    );
                }
                return Err(format!("ai: veles returned HTTP {code}: {body}"));
            }
            Err(e) => return Err(format!("ai: veles unreachable at {}: {e}", self.url)),
        };
        let json: serde_json::Value = resp
            .into_json()
            .map_err(|e| format!("ai: veles sent something that is not json: {e}"))?;
        if let Some(err) = json["error"]["message"].as_str() {
            return Err(format!("ai: veles refused: {err}"));
        }
        // `build_task`: the reply text is the status message's text
        // part, and (on completion) an artifact carrying the same. Read
        // the status message first and fall back, so a non-terminal
        // state still yields whatever was said.
        let task = &json["result"]["task"];
        let text = first_text(&task["status"]["message"]["parts"])
            .or_else(|| first_text(&task["artifacts"][0]["parts"]))
            .or_else(|| first_text(&json["result"]["message"]["parts"]))
            .unwrap_or_default();
        if text.trim().is_empty() {
            let state = task["status"]["state"].as_str().unwrap_or("?");
            return Err(format!("ai: veles answered nothing (task state: {state})"));
        }
        Ok(text)
    }
}

fn first_text(parts: &serde_json::Value) -> Option<String> {
    parts
        .as_array()?
        .iter()
        .find_map(|p| p["text"].as_str())
        .map(str::to_string)
}

// ── the Tcl side ─────────────────────────────────────────────────────

/// Set the interpreter result to `s`.
fn set_result(interp: *mut clib::Tcl_Interp, s: &str) {
    // A NUL cannot survive the C boundary; replacing beats truncating,
    // because a silently halved error message is worse than a visible
    // placeholder.
    let safe = s.replace('\0', "<nul>");
    if let Ok(c) = CString::new(safe) {
        unsafe {
            clib::Tcl_SetObjResult(
                interp,
                clib::Tcl_NewStringObj(c.as_ptr(), c.as_bytes().len() as c_int),
            );
        }
    }
}

/// `objv` is 0-based with `objv[0]` the command name; valid indices are
/// `0..objc`. Reading `objv[objc]` for an optional argument segfaults in
/// `Tcl_GetString` — bounds-check against `objc`, always.
fn arg_str(objc: c_int, objv: *const *mut clib::Tcl_Obj, i: c_int) -> Option<String> {
    if i >= objc {
        return None;
    }
    unsafe {
        let p = *objv.add(i as usize);
        if p.is_null() {
            return None;
        }
        Some(
            CStr::from_ptr(clib::Tcl_GetString(p))
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// `ai <prompt> ?context?` — ask veles, return the answer.
extern "C" fn ffi_ai(
    client_data: *mut c_void,
    interp: *mut clib::Tcl_Interp,
    objc: c_int,
    objv: *const *mut clib::Tcl_Obj,
) -> c_int {
    // Safety: the pointer came from `Arc::into_raw` in `register` and is
    // deliberately leaked for the process lifetime — one bridge per
    // interpreter, and the interpreter outlives nothing.
    let bridge = match unsafe { (client_data as *const VelesBridge).as_ref() } {
        Some(b) => b,
        None => {
            set_result(interp, "ai: bridge unavailable");
            return clib::TCL_ERROR as c_int;
        }
    };
    let Some(prompt) = arg_str(objc, objv, 1) else {
        set_result(interp, "wrong # args: should be \"ai prompt ?context?\"");
        return clib::TCL_ERROR as c_int;
    };
    if prompt.trim().is_empty() {
        set_result(interp, "ai: the prompt is empty");
        return clib::TCL_ERROR as c_int;
    }
    // A context id groups a conversation on veles' side. Default to one
    // shared thread so follow-ups work; a proc that wants isolation
    // passes its own.
    let context = arg_str(objc, objv, 2).unwrap_or_else(|| "slopdrop".to_string());
    match bridge.ask(&prompt, &context) {
        Ok(answer) => {
            set_result(interp, &answer);
            clib::TCL_OK as c_int
        }
        Err(e) => {
            set_result(interp, &e);
            clib::TCL_ERROR as c_int
        }
    }
}

// ── the process-global bridge ────────────────────────────────────────
//
// `TclThreadWorker::new` builds the interpreter from `TclConfig` and
// `SecurityConfig`, neither of which knows about `[veles]`, and the
// worker is constructed from three different places. Threading a fourth
// parameter through all of them to reach one optional command is a lot
// of signature churn for very little; veles solves the identical
// problem the identical way (`bridge::publish_global`), so this does
// too. `main` publishes what it read; the worker asks.

static GLOBAL: std::sync::OnceLock<Option<Arc<VelesBridge>>> = std::sync::OnceLock::new();

/// Publish the configured bridge, once, at startup. A second call is
/// ignored — there is one config and one process.
pub fn publish(cfg: Option<&VelesConfig>) {
    let _ = GLOBAL.set(cfg.and_then(VelesBridge::new));
}

/// The bridge, if one was configured. `None` means `ai` is never
/// registered and Tcl reports `invalid command name "ai"`, which says
/// more than a command that exists and always fails.
pub fn global() -> Option<Arc<VelesBridge>> {
    GLOBAL.get().cloned().flatten()
}

/// Register `ai` into the interpreter.
///
/// # Safety
/// The interpreter must outlive the Arc. It does: both are owned by the
/// Tcl thread, and the Arc is leaked into ClientData for the process.
pub unsafe fn register(bridge: Arc<VelesBridge>, interp: &tcl::Interpreter) {
    let ptr = Arc::into_raw(bridge) as *mut c_void;
    interp.def_proc_with_client_data("ai", ffi_ai, ptr, None);
    interp.def_proc_with_client_data("ai::ask", ffi_ai, ptr, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VelesConfig {
        VelesConfig {
            enabled: true,
            url: Some("http://127.0.0.1:9".into()),
            token: Some("tok".into()),
            token_env: None,
            timeout_ms: 1000,
            max_per_eval: 2,
            max_per_minute: 3,
        }
    }

    #[test]
    fn the_bridge_is_absent_until_configured() {
        let mut c = cfg();
        assert!(VelesBridge::new(&c).is_some());
        c.enabled = false;
        assert!(VelesBridge::new(&c).is_none(), "the flag decides");
        c.enabled = true;
        c.url = None;
        assert!(
            VelesBridge::new(&c).is_none(),
            "an enabled bridge with no url registers nothing, so Tcl says \
             `invalid command name \"ai\"` rather than erroring on every call"
        );
    }

    /// The A2A wire, against a real socket: the JSON-RPC envelope veles
    /// expects, the bearer that names this peer to its ACL, and the
    /// reply dug out of `build_task`'s shape.
    #[test]
    fn an_ask_speaks_a2a_and_reads_the_answer_out_of_the_task() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let srv = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            // ureq writes the head and the body separately, so a single
            // `read` captures only the headers and the body assertions
            // below silently pass on an empty string. Read until the
            // declared Content-Length has arrived.
            let mut raw: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = sock.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&chunk[..n]);
                let s = String::from_utf8_lossy(&raw);
                if let Some(hdr_end) = s.find("\r\n\r\n") {
                    let want: usize = s
                        .to_ascii_lowercase()
                        .split("content-length:")
                        .nth(1)
                        .and_then(|t| t.split("\r\n").next())
                        .and_then(|t| t.trim().parse().ok())
                        .unwrap_or(0);
                    if raw.len() >= hdr_end + 4 + want {
                        break;
                    }
                }
            }
            let req = String::from_utf8_lossy(&raw).to_string();
            let body = r#"{"jsonrpc":"2.0","id":1,"result":{"task":{"id":"t1",
                "contextId":"slopdrop","status":{"state":"completed",
                "message":{"parts":[{"kind":"text","text":"the answer is 42"}]}}}}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(resp.as_bytes()).unwrap();
            req
        });

        let mut c = cfg();
        c.url = Some(format!("http://127.0.0.1:{port}"));
        let b = VelesBridge::new(&c).unwrap();
        let answer = b.ask("what is six times seven", "slopdrop").unwrap();
        assert_eq!(answer, "the answer is 42");

        let req = srv.join().unwrap();
        assert!(req.contains("\"method\":\"message/send\""), "{req}");
        assert!(
            req.contains("Bearer tok") || req.contains("bearer tok"),
            "the peer token must ride — it is what names this bridge to \
             veles' ACL: {req}"
        );
        assert!(req.contains("what is six times seven"), "{req}");
    }

    /// A JSON-RPC error (veles refusing the peer, a lockdown, a rate
    /// limit) must surface as the reason, not as "no answer".
    #[test]
    fn a_refusal_from_veles_is_reported_as_the_reason() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32052,
                "message":"peer not in trusted list"}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let mut c = cfg();
        c.url = Some(format!("http://127.0.0.1:{port}"));
        let b = VelesBridge::new(&c).unwrap();
        let e = b.ask("hello", "slopdrop").unwrap_err();
        assert!(e.contains("peer not in trusted list"), "{e}");
    }

    /// The budgets are the point of a native command that spends money:
    /// any proc in a public interpreter can call this.
    #[test]
    fn the_per_eval_budget_resets_and_the_per_minute_one_does_not() {
        let b = VelesBridge::new(&cfg()).unwrap();
        assert!(b.claim().is_ok());
        assert!(b.claim().is_ok());
        let e = b.claim().unwrap_err();
        assert!(e.contains("per-eval"), "{e}");

        // A new evaluation gets a fresh per-eval budget…
        b.begin_eval();
        assert!(b.claim().is_ok());
        // …but the per-minute ledger carries across evals, which is what
        // stops "one call per eval, a thousand evals" from being free.
        b.begin_eval();
        let e = b.claim().unwrap_err();
        assert!(e.contains("per-minute"), "{e}");
    }
}
