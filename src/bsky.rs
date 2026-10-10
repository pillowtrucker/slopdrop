//! Bluesky context for an evaluation, and the native `bsky::query`.
//!
//! veles sends a `bsky` object with an evaluation that came from a
//! Bluesky post:
//!
//! ```json
//! {"post": {...}, "thread": [{...}], "query": {"url": "...", "token": "...", "max": 5}}
//! ```
//!
//! [`apply`] turns it into the `::bsky::*` variables `tcl/bsky.tcl`
//! documents, BEFORE the code runs, and empties `::bsky::out`; [`collect`]
//! reads `::bsky::out` back afterwards for the eval response. Every
//! evaluation goes through [`apply`], Bluesky or not, so one post's
//! context never leaks into the next evaluation from IRC.
//!
//! WHY `bsky::query` IS NATIVE. The reads go through the bot's own
//! AppView session on the veles side. veles hands each Bluesky
//! evaluation a short-lived capability (a URL, a token, a read budget);
//! this interpreter has no boundary a token could hide behind (see
//! `veles_bridge`'s header), so it lives here in Rust, set by [`apply`]
//! and cleared by the next one, and Tcl can use it without reading it.

use std::cell::RefCell;
use std::ffi::{c_int, c_void, CStr};
use std::time::Duration;

use serde_json::Value;
use tcl::{Interpreter, Obj};

/// A JSON value as a native Tcl object: an object is a dict, an array a
/// list, a bool 1/0, null the empty string. Native objects, never a
/// quoted string, so a post's text survives whatever braces it holds.
pub fn json_to_obj(v: &Value) -> Obj {
    match v {
        Value::Object(m) => {
            let d = Obj::new_dict();
            for (k, v) in m {
                let _ = d.dict_put(Obj::from(k.clone()), json_to_obj(v));
            }
            d
        }
        Value::Array(a) => Obj::new_list(a.iter().map(json_to_obj)),
        Value::String(s) => Obj::from(s.clone()),
        Value::Number(n) => Obj::from(n.to_string()),
        Value::Bool(b) => Obj::from(if *b { "1".to_string() } else { "0".to_string() }),
        Value::Null => Obj::from(String::new()),
    }
}

/// The per-evaluation read capability veles granted, if any.
#[derive(Debug, Clone)]
struct QueryCap {
    url: String,
    token: String,
    remaining: u32,
}

thread_local! {
    /// Per THREAD, not per process: an interpreter lives on the one
    /// thread that created it, and `apply` and the command both run
    /// there. A process-global would let two interpreters (the test
    /// suite runs dozens) spend each other's capability.
    static QUERY: RefCell<Option<QueryCap>> = const { RefCell::new(None) };
}

/// Reads one evaluation may make when veles does not say.
const DEFAULT_READS: u32 = 5;

/// Set the `::bsky::*` variables for the evaluation about to run.
/// `ctx` is the request's `bsky` object; `None` is every other surface.
pub fn apply(interp: &Interpreter, ctx: Option<&Value>) {
    let active = ctx.is_some_and(|c| c.get("post").is_some_and(Value::is_object));
    let empty = || Obj::from(String::new());
    interp.set(
        "::bsky::active",
        Obj::from(if active { "1" } else { "0" }.to_string()),
    );
    interp.set(
        "::bsky::post",
        ctx.and_then(|c| c.get("post")).filter(|_| active).map(json_to_obj).unwrap_or_else(empty),
    );
    interp.set(
        "::bsky::thread",
        ctx.and_then(|c| c.get("thread")).filter(|_| active).map(json_to_obj).unwrap_or_else(empty),
    );
    interp.set("::bsky::event", empty());
    interp.set("::bsky::out", empty());

    let cap = ctx
        .filter(|_| active)
        .and_then(|c| c.get("query"))
        .and_then(|q| {
            let url = q.get("url")?.as_str()?.trim().trim_end_matches('/').to_string();
            let token = q.get("token")?.as_str()?.trim().to_string();
            if url.is_empty() || token.is_empty() {
                return None;
            }
            let max = q
                .get("max")
                .and_then(Value::as_u64)
                .map(|m| m.min(20) as u32)
                .unwrap_or(DEFAULT_READS);
            Some(QueryCap {
                url,
                token,
                remaining: max,
            })
        });
    QUERY.with(|q| *q.borrow_mut() = cap);
}

/// What the code recorded in `::bsky::out`, as a JSON array of flat
/// string objects (`{"kind": "link", "text": …, "uri": …}`). `None` when
/// this was not a Bluesky evaluation or nothing was recorded; a record
/// that is not a dict is skipped, never fatal.
pub fn collect(interp: &Interpreter) -> Option<Value> {
    let active = interp
        .get("::bsky::active")
        .ok()
        .is_some_and(|o| o.get_string() == "1");
    if !active {
        return None;
    }
    let out = interp.get("::bsky::out").ok()?;
    let mut records = Vec::new();
    for rec in out.get_elements().ok()? {
        let Ok(iter) = rec.dict_iter() else { continue };
        let mut m = serde_json::Map::new();
        for (k, v) in iter {
            m.insert(k.get_string(), Value::String(v.get_string()));
        }
        if !m.is_empty() {
            records.push(Value::Object(m));
        }
    }
    (!records.is_empty()).then_some(Value::Array(records))
}

/// The Tcl that sets `::bsky::event` for one trigger dispatch, from the
/// event's flat fields (kind, did, handle, uri, cid, subject, text).
/// Escaped word by word: this goes into a command string.
pub fn event_setup(ev: &Value) -> String {
    use crate::tcl_escape::tcl_escape_arg;
    let mut words = Vec::new();
    if let Some(m) = ev.as_object() {
        for (k, v) in m {
            let val = match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            words.push(tcl_escape_arg(k));
            words.push(tcl_escape_arg(&val));
        }
    }
    // `active` stays 0: a handler's answer is plain text relayed as-is,
    // with no spec beside it, so the procs must render the IRC way
    // (a link keeps its URL in the text).
    format!("set ::bsky::active 0\nset ::bsky::event [dict create {}]", words.join(" "))
}

// ── the native command ───────────────────────────────────────────────

fn set_result_str(interp: *mut clib::Tcl_Interp, s: &str) {
    let o = Obj::from(s.replace('\0', "<nul>"));
    unsafe { clib::Tcl_SetObjResult(interp, o.as_ptr()) };
}

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

/// Take one read from this evaluation's capability.
fn claim() -> Result<QueryCap, String> {
    QUERY.with(|q| {
        let mut q = q.borrow_mut();
        let Some(cap) = q.as_mut() else {
            return Err(
                "bsky::query: no Bluesky read is available here — only an evaluation that \
                 came from a Bluesky post may read"
                    .into(),
            );
        };
        if cap.remaining == 0 {
            return Err("bsky::query: this evaluation has used all its reads".into());
        }
        cap.remaining -= 1;
        Ok(cap.clone())
    })
}

/// One read, blocking (the interpreter runs on its own thread).
/// `params` are ordered pairs, not a map: an AppView query may repeat a
/// key (`getPosts` takes `uris` once per post).
fn read(cap: &QueryCap, method: &str, params: Vec<(String, String)>) -> Result<Value, String> {
    let body = serde_json::json!({"method": method, "params": params});
    let resp = ureq::post(&cap.url)
        .timeout(Duration::from_secs(15))
        .set("content-type", "application/json")
        .set("authorization", &format!("Bearer {}", cap.token))
        .send_json(body);
    let json: Value = match resp {
        Ok(r) => r
            .into_json()
            .map_err(|e| format!("bsky::query: veles sent something that is not json: {e}"))?,
        Err(ureq::Error::Status(_, r)) => r
            .into_json()
            .unwrap_or_else(|_| serde_json::json!({"error": "refused"})),
        Err(e) => return Err(format!("bsky::query: veles unreachable: {e}")),
    };
    if let Some(err) = json.get("error").and_then(Value::as_str) {
        return Err(format!("bsky::query: {err}"));
    }
    Ok(json.get("result").cloned().unwrap_or(Value::Null))
}

/// `bsky::query method ?key value …?` — one AppView read as the bot.
extern "C" fn ffi_query(
    _client_data: *mut c_void,
    interp: *mut clib::Tcl_Interp,
    objc: c_int,
    objv: *const *mut clib::Tcl_Obj,
) -> c_int {
    let fail = |msg: &str| {
        set_result_str(interp, msg);
        clib::TCL_ERROR as c_int
    };
    let Some(method) = arg_str(objc, objv, 1).filter(|m| !m.trim().is_empty()) else {
        return fail("wrong # args: should be \"bsky::query method ?key value ...?\"");
    };
    if objc % 2 != 0 {
        return fail("bsky::query: the parameters must be key value pairs");
    }
    let mut params = Vec::new();
    let mut i = 2;
    while i + 1 < objc {
        let (Some(k), Some(v)) = (arg_str(objc, objv, i), arg_str(objc, objv, i + 1)) else {
            return fail("bsky::query: unreadable argument");
        };
        params.push((k, v));
        i += 2;
    }
    let cap = match claim() {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    match read(&cap, method.trim(), params) {
        Ok(v) => {
            let o = json_to_obj(&v);
            unsafe { clib::Tcl_SetObjResult(interp, o.as_ptr()) };
            clib::TCL_OK as c_int
        }
        Err(e) => fail(&e),
    }
}

/// Register `bsky::query` into the interpreter. After the stock modules
/// load (`tcl/bsky.tcl` makes the namespace), before any evaluation.
///
/// # Safety
/// The interpreter must be a live one owned by the calling thread.
pub unsafe fn register(interp: &Interpreter) {
    interp.def_proc_with_client_data("::bsky::query", ffi_query, std::ptr::null_mut(), None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcl_wrapper::SafeTclInterp;
    use std::path::PathBuf;

    fn interp(name: &str) -> SafeTclInterp {
        let dir = std::env::temp_dir().join(format!("slopdrop_bsky_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        SafeTclInterp::new(5000, &PathBuf::from(&dir), None, None, 1000).unwrap()
    }

    fn post() -> Value {
        // "hi @alice.test 🙂 see https://x.test #tcl {braces}"
        let text = "hi @alice.test 🙂 see https://x.test #tcl {braces}";
        let b = |s: &str| text.find(s).unwrap();
        serde_json::json!({
            "uri": "at://did:plc:a/app.bsky.feed.post/1",
            "cid": "bafy",
            "text": text,
            "did": "did:plc:a",
            "handle": "a.test",
            "langs": ["en"],
            "facets": [
                {"start": b("@alice"), "end": b("@alice") + "@alice.test".len(), "type": "mention", "value": "did:plc:alice"},
                {"start": b("https"), "end": b("https") + "https://x.test".len(), "type": "link", "value": "https://x.test"},
                {"start": b("#tcl"), "end": b("#tcl") + 4, "type": "tag", "value": "tcl"},
            ],
            "embed": {"type": "images", "images": [["https://cdn.test/1", ""]]},
            "reply": {"root": "at://r", "parent": "at://p"},
        })
    }

    #[test]
    fn a_post_reaches_tcl_whole_and_its_facets_read_back() {
        let i = interp("read");
        apply(i.interpreter(), Some(&serde_json::json!({"post": post(), "thread": [post()]})));
        assert_eq!(i.eval("set ::bsky::active").unwrap(), "1");
        assert_eq!(
            i.eval("bsky::text").unwrap(),
            "hi @alice.test 🙂 see https://x.test #tcl {braces}",
            "text with braces and emoji survives"
        );
        assert_eq!(i.eval("bsky::author").unwrap(), "a.test");
        assert_eq!(i.eval("bsky::links").unwrap(), "https://x.test");
        assert_eq!(i.eval("bsky::tags").unwrap(), "tcl");
        assert_eq!(i.eval("lindex [bsky::mentions] 0").unwrap(), "@alice.test did:plc:alice");
        assert_eq!(
            i.eval("bsky::facet_text [lindex [bsky::facets link] 0]").unwrap(),
            "https://x.test",
            "byte offsets past a 4-byte emoji land on the right characters"
        );
        assert_eq!(i.eval("bsky::strip_facets").unwrap(), "hi 🙂 see {braces}");
        assert_eq!(i.eval("bsky::strip_facets mention").unwrap(), "hi 🙂 see https://x.test #tcl {braces}");
        assert_eq!(i.eval("llength $::bsky::thread").unwrap(), "1");
        assert_eq!(i.eval("bsky::is_reply").unwrap(), "1");
        assert_eq!(i.eval("lindex [lindex [bsky::images] 0] 0").unwrap(), "https://cdn.test/1");
    }

    #[test]
    fn the_output_procs_record_and_collect_hands_them_back() {
        let i = interp("write");
        apply(i.interpreter(), Some(&serde_json::json!({"post": post()})));
        let shown = i
            .eval("set r \"see [bsky::link docs https://d.test] [bsky::tag #tcl] [bsky::mention @b.test][bsky::newpost]more\"")
            .unwrap();
        assert_eq!(shown, "see docs #tcl @b.test\u{c}more");
        i.eval("bsky::quote at://q; bsky::lang en de").unwrap();
        let out = collect(i.interpreter()).expect("records");
        let kinds: Vec<&str> = out
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["link", "tag", "mention", "quote", "lang", "lang"]);
        assert_eq!(out[0]["uri"], "https://d.test");
        assert_eq!(out[0]["text"], "docs");
        assert_eq!(out[2]["handle"], "b.test");
    }

    #[test]
    fn off_bluesky_the_procs_read_as_irc_and_nothing_is_collected() {
        let i = interp("irc");
        // A Bluesky eval first, then an IRC one: nothing may leak across.
        apply(i.interpreter(), Some(&serde_json::json!({"post": post()})));
        i.eval("bsky::link x https://x.test").unwrap();
        apply(i.interpreter(), None);
        assert_eq!(i.eval("set ::bsky::active").unwrap(), "0");
        assert_eq!(i.eval("bsky::text").unwrap(), "", "the post is gone");
        assert_eq!(i.eval("bsky::link docs https://d.test").unwrap(), "docs <https://d.test>");
        assert_eq!(i.eval("bsky::link https://d.test").unwrap(), "https://d.test");
        assert_eq!(i.eval("bsky::quote at://q").unwrap(), "at://q");
        assert_eq!(i.eval("string length [bsky::newpost]").unwrap(), "1");
        assert!(collect(i.interpreter()).is_none());
    }

    #[test]
    fn query_without_a_capability_fails_with_the_reason() {
        let i = interp("noq");
        apply(i.interpreter(), Some(&serde_json::json!({"post": post()})));
        unsafe { register(i.interpreter()) };
        let err = i.eval("bsky::profile alice.test").unwrap_err().to_string();
        assert!(err.contains("no Bluesky read is available"), "{err}");
    }

    #[test]
    fn query_spends_the_evaluation_budget_and_converts_the_answer() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        // Read a whole request: headers, then content-length bytes of
        // body, which may arrive in a later packet than the headers.
        fn read_request(s: &mut std::net::TcpStream) -> String {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = s.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    if buf.len() >= h + 4 + len {
                        return text;
                    }
                }
            }
            String::from_utf8_lossy(&buf).to_string()
        }
        let server = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = listener.accept().unwrap();
                seen.push(read_request(&mut s));
                let body = r#"{"result":{"handle":"alice.test","followersCount":7,"labels":[]}}"#;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
            }
            seen
        });
        let i = interp("q");
        unsafe { register(i.interpreter()) };
        apply(
            i.interpreter(),
            Some(&serde_json::json!({"post": post(),
                "query": {"url": format!("http://127.0.0.1:{port}/r"), "token": "T0K", "max": 2}})),
        );
        assert_eq!(i.eval("dict get [bsky::profile @alice.test] followersCount").unwrap(), "7");
        assert_eq!(i.eval("dict get [bsky::profile alice.test] handle").unwrap(), "alice.test");
        let err = i.eval("bsky::profile alice.test").unwrap_err().to_string();
        assert!(err.contains("used all its reads"), "{err}");
        assert_eq!(
            i.eval("set all {}; foreach v [info vars ::bsky::*] { append all [set $v] }; string first T0K $all")
                .unwrap(),
            "-1",
            "the token is in no variable Tcl can read"
        );
        let seen = server.join().unwrap();
        assert!(seen[0].contains("authorization: Bearer T0K") || seen[0].contains("Authorization: Bearer T0K"));
        assert!(seen[0].contains("\"method\":\"app.bsky.actor.getProfile\""), "{}", seen[0]);
        assert!(
            seen[0].contains("\"params\":[[\"actor\",\"alice.test\"]]"),
            "ordered pairs, the @ trimmed: {}",
            seen[0]
        );
        // The next evaluation starts without it — even a grant with reads
        // left is gone once a line from anywhere else is evaluated.
        apply(
            i.interpreter(),
            Some(&serde_json::json!({"post": post(),
                "query": {"url": "http://127.0.0.1:9/r", "token": "T2", "max": 3}})),
        );
        apply(i.interpreter(), None);
        let err = i.eval("bsky::profile alice.test").unwrap_err().to_string();
        assert!(err.contains("no Bluesky read is available"), "{err}");
    }

    #[test]
    fn an_event_sets_its_dict_for_the_handlers() {
        let i = interp("ev");
        apply(i.interpreter(), None);
        let code = event_setup(&serde_json::json!({"kind": "LIKE", "did": "did:plc:x", "text": "a } [b] $c"}));
        i.eval(&code).unwrap();
        assert_eq!(i.eval("dict get $::bsky::event text").unwrap(), "a } [b] $c");
        assert_eq!(i.eval("dict get $::bsky::event kind").unwrap(), "LIKE");
    }
}
