//! Core TCL evaluation service
//!
//! This module provides a frontend-agnostic TCL evaluation service
//! that can be used by multiple frontends (IRC, CLI, TUI, Web, etc.)
//!
//! NOTE: Currently unused - frontends use TclThreadHandle directly.
//! This abstraction is kept for future unified frontend management
//! where multiple frontends share a single TCL service instance.

#![allow(dead_code)]

use crate::config::{SecurityConfig, TclConfig};
use crate::state::{CommitInfo, StatePersistence};
use crate::tcl_thread::TclThreadHandle;
use crate::types::ChannelMembers;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// The room an evaluation is happening in, as reported by a bridge.
///
/// WHY THIS IS NOT `EvalContext.user` / `.host`
///
/// Those two are the AUTHORIZATION identity: `handle_eval` builds
/// `nick!host` from them and matches it against `privileged_users`. They
/// must keep coming from the frontend's own knowledge of the caller —
/// over the web that is the bearer token plus the recorded subject.
///
/// This is the DISPLAY identity: what `[nick]`, `[names]`, `[channel]`
/// and `[hostmask]` answer inside the interpreter. Those globals used to
/// be filled by our own IRC connection; headless, the only process that
/// still knows them is whichever bot is actually in the channel, so it
/// sends them. A caller can therefore choose them — which is exactly why
/// they are kept away from the privilege check, and why nothing here is
/// ever consulted by `matches_hostmask`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomContext {
    /// The speaker's nick — `$::nick`, `[nick]`.
    pub nick: Option<String>,
    /// `ident@host` — `$::mask`, `[hostmask]`.
    pub mask: Option<String>,
    /// The channel, or the other party's nick in a query — `$::channel`.
    pub channel: Option<String>,
    /// The channel topic — `$::topic`. New: this bot has never had one.
    pub topic: Option<String>,
    /// The channel roster, bare nicks — what `chanlist` (and so `[names]`
    /// and `[name]`) answers with.
    pub members: Vec<String>,
}

impl RoomContext {
    pub fn is_empty(&self) -> bool {
        self.nick.is_none()
            && self.mask.is_none()
            && self.channel.is_none()
            && self.topic.is_none()
            && self.members.is_empty()
    }
}

/// Context for a TCL evaluation request
#[derive(Debug, Clone)]
pub struct EvalContext {
    /// User identifier (nick, username, session ID, etc.)
    pub user: String,
    /// Host/origin (hostname, IP, "local", etc.)
    pub host: String,
    /// Optional channel/room identifier
    pub channel: Option<String>,
    /// Network identifier (used to disambiguate channels across networks)
    pub network: String,
    /// The room the line was typed in, when a bridge told us. Display
    /// only — never consulted by the privilege check.
    pub room: RoomContext,
    /// Whether the user has admin privileges
    pub is_admin: bool,
}

impl EvalContext {
    pub fn new(user: String, host: String) -> Self {
        Self {
            user,
            host,
            channel: None,
            network: "default".to_string(),
            room: RoomContext::default(),
            is_admin: false,
        }
    }

    /// Builder pattern to set channel
    /// NOTE: Currently unused but part of fluent builder API
    #[allow(dead_code)]
    pub fn with_channel(mut self, channel: String) -> Self {
        self.channel = Some(channel);
        self
    }

    /// Builder pattern to set network
    #[allow(dead_code)]
    pub fn with_network(mut self, network: String) -> Self {
        self.network = network;
        self
    }

    pub fn with_admin(mut self, is_admin: bool) -> Self {
        self.is_admin = is_admin;
        self
    }
}

/// Response from a TCL evaluation
#[derive(Debug, Clone)]
pub struct EvalResponse {
    /// Lines of output
    pub output: Vec<String>,
    /// Whether this was an error
    pub is_error: bool,
    /// Git commit info if state was changed
    pub commit_info: Option<CommitInfo>,
    /// Whether more output is available via pagination
    pub more_available: bool,
}

/// Core TCL evaluation service
///
/// This service manages the TCL interpreter thread and provides
/// a clean API for frontends to evaluate TCL code.
pub struct TclService {
    tcl_thread: TclThreadHandle,
    security_config: SecurityConfig,
    tcl_config: TclConfig,
    /// Cache for paginated output per user/channel
    output_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl TclService {
    /// Create a new TCL service
    pub fn new(
        security_config: SecurityConfig,
        tcl_config: TclConfig,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let tcl_thread =
            TclThreadHandle::spawn(tcl_config.clone(), security_config.clone(), channel_members)?;

        Ok(Self {
            tcl_thread,
            security_config,
            tcl_config,
            output_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Evaluate TCL code
    pub async fn eval(&mut self, code: &str, ctx: EvalContext) -> Result<EvalResponse> {
        // The frontends that name a channel through `EvalContext` — the
        // CLI, the TUI, the service API — are reporting a room just as
        // much as a bridge is, so their `channel` becomes one. It has to
        // happen HERE, before the Option collapses: one line below,
        // `None` becomes the literal string "default", and from there
        // "the caller said nothing" and "the caller named a channel
        // called default" are the same value.
        let mut room = ctx.room.clone();
        if room.channel.is_none() {
            room.channel = ctx.channel.clone();
        }

        let channel = ctx.channel.clone().unwrap_or_else(|| "default".to_string());

        // Evaluate the code
        let result = self
            .tcl_thread
            .eval(
                code.to_string(),
                ctx.is_admin,
                ctx.user.clone(),
                ctx.host.clone(),
                channel.clone(),
                ctx.network.clone(),
                room,
            )
            .await?;

        // Split output into lines
        let all_lines: Vec<String> = if result.output.is_empty() {
            vec![]
        } else {
            result.output.lines().map(|s| s.to_string()).collect()
        };

        // Apply pagination
        let max_lines = self.tcl_config.max_output_lines;
        let (output, more_available) = if all_lines.len() > max_lines {
            // Cache remaining lines
            let cache_key = format!("{}:{}", channel, ctx.user);
            let shown = all_lines[..max_lines].to_vec();
            let remaining = all_lines[max_lines..].to_vec();

            if let Ok(mut cache) = self.output_cache.write() {
                cache.insert(cache_key, remaining);
            }

            (shown, true)
        } else {
            (all_lines, false)
        };

        Ok(EvalResponse {
            output,
            is_error: result.is_error,
            commit_info: result.commit_info,
            more_available,
        })
    }

    /// Get more paginated output
    pub async fn more(&mut self, ctx: EvalContext) -> Result<EvalResponse> {
        // The frontends that name a channel through `EvalContext` — the
        // CLI, the TUI, the service API — are reporting a room just as
        // much as a bridge is, so their `channel` becomes one. It has to
        // happen HERE, before the Option collapses: one line below,
        // `None` becomes the literal string "default", and from there
        // "the caller said nothing" and "the caller named a channel
        // called default" are the same value.
        let mut room = ctx.room.clone();
        if room.channel.is_none() {
            room.channel = ctx.channel.clone();
        }

        let channel = ctx.channel.clone().unwrap_or_else(|| "default".to_string());
        let cache_key = format!("{}:{}", channel, ctx.user);

        let mut cache = self
            .output_cache
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to access output cache: {}", e))?;

        if let Some(remaining) = cache.get_mut(&cache_key) {
            if remaining.is_empty() {
                return Ok(EvalResponse {
                    output: vec!["No more output available.".to_string()],
                    is_error: false,
                    commit_info: None,
                    more_available: false,
                });
            }

            let max_lines = self.tcl_config.max_output_lines;
            let chunk_size = std::cmp::min(max_lines, remaining.len());
            let output = remaining.drain(..chunk_size).collect::<Vec<_>>();
            let more_available = !remaining.is_empty();

            // Clean up empty cache entries
            if !more_available {
                cache.remove(&cache_key);
            }

            Ok(EvalResponse {
                output,
                is_error: false,
                commit_info: None,
                more_available,
            })
        } else {
            Ok(EvalResponse {
                output: vec!["No cached output. Run a command first.".to_string()],
                is_error: false,
                commit_info: None,
                more_available: false,
            })
        }
    }

    /// Get git history
    pub async fn history(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        let persistence = StatePersistence::with_repo(
            self.tcl_config.state_path.clone(),
            self.tcl_config.state_repo.clone(),
            self.tcl_config.ssh_key.clone(),
        );

        let history = persistence.get_history(limit)?;

        // Convert tuples (hash, timestamp, author, message) to CommitInfo
        Ok(history
            .into_iter()
            .map(|(commit_id, _timestamp, author, message)| CommitInfo {
                commit_id,
                author,
                message,
                files_changed: 0, // Not available from git history
                insertions: 0,
                deletions: 0,
                changes_summary: String::new(), // Not available from git history
            })
            .collect())
    }

    /// Rollback to a specific commit
    pub async fn rollback(&mut self, commit_hash: &str) -> Result<String> {
        let persistence = StatePersistence::with_repo(
            self.tcl_config.state_path.clone(),
            self.tcl_config.state_repo.clone(),
            self.tcl_config.ssh_key.clone(),
        );

        persistence.rollback_to(commit_hash)?;

        // Need to restart the TCL thread to reload state
        self.restart_tcl_thread().await?;

        Ok(format!(
            "Rolled back to commit {}. TCL thread restarted with new state.",
            &commit_hash[..8]
        ))
    }

    /// Restart the TCL thread
    async fn restart_tcl_thread(&mut self) -> Result<()> {
        self.tcl_thread.shutdown();

        // Create empty channel members for now
        // In the future, frontends can provide their own channel members
        let channel_members = Arc::new(RwLock::new(HashMap::new()));

        self.tcl_thread = TclThreadHandle::spawn(
            self.tcl_config.clone(),
            self.security_config.clone(),
            channel_members,
        )?;

        Ok(())
    }

    /// Check if a user is admin based on hostmask pattern matching
    /// NOTE: Used in tests; IRC frontend uses TclPlugin's auth instead
    #[allow(dead_code)]
    pub fn is_admin(&self, hostmask: &str) -> bool {
        self.security_config
            .privileged_users
            .iter()
            .any(|pattern| crate::hostmask::matches_hostmask(hostmask, pattern))
    }

    /// Shutdown the service gracefully
    /// NOTE: Used by frontends in their stop() methods during graceful shutdown
    #[allow(dead_code)]
    pub fn shutdown(&mut self) {
        self.tcl_thread.shutdown();
    }

    /// Dispatch one IRC event to the trigger engine and return what the
    /// handlers said, WITHOUT sending it anywhere (headless mode).
    ///
    /// This is `tcl_plugin::handle_event` minus the `send_to_network`
    /// call: a bridge (veles) holds the IRC connection, forwards the
    /// line over HTTP, and relays the `{channel, message}` pairs it gets
    /// back through its own connection. The evaluation runs as the
    /// "system" user via `eval_simple`, which is load-bearing twice:
    ///
    ///  - no state persistence — a hundred channel lines must not mint a
    ///    hundred commits in the state repo ("Evaluated triggers
    ///    dispatch …" burying real history was the stated reason this
    ///    is a dedicated endpoint and not `POST /api/eval`);
    ///  - no pagination cache fill — the reply belongs to the bridge,
    ///    not to a `more` key nobody will ever read.
    ///
    /// `log` additionally appends the line to the channel LOG the same
    /// way the IRC frontend's `LogMessage` command did, so headless
    /// procs reading `utils.tcl`'s `log` see a channel that is alive.
    pub async fn dispatch_event(
        &mut self,
        event: &str,
        network: &str,
        args: &[String],
        log: Option<(&str, &str, &str, &str)>, // (channel, nick, mask, text)
    ) -> Result<Vec<(String, String)>> {
        use crate::tcl_escape::tcl_escape_arg;

        let event = event.to_ascii_uppercase();
        if !matches!(
            event.as_str(),
            "JOIN" | "PART" | "QUIT" | "KICK" | "NICK" | "TEXT"
        ) {
            anyhow::bail!("unknown event type '{event}'");
        }

        // Log BEFORE dispatch, the way the IRC frontend did it
        // (`LogMessage` then `UserText` on the same line): a handler
        // that reads the log must see the line it is reacting to.
        if let Some((channel, nick, mask, text)) = log {
            self.tcl_thread.log_message(
                channel.to_string(),
                nick.to_string(),
                mask.to_string(),
                text.to_string(),
            );
        }

        let tcl_args: Vec<String> = args.iter().map(|s| tcl_escape_arg(s)).collect();
        let dispatch_cmd = format!(
            "triggers dispatch {} {} {}",
            tcl_escape_arg(&event),
            tcl_escape_arg(network),
            tcl_args.join(" "),
        );

        let result = self.tcl_thread.eval_simple(dispatch_cmd).await?;
        Ok(crate::tcl_plugin::parse_tcl_response_list(&result))
    }

    /// Fire every due timer and return its `{channel, message}` pairs
    /// without sending them (headless mode). Same `check_timers` shape
    /// as the IRC plugin's, minus the network routing: the CHANNEL may
    /// carry a `network:` prefix in slopdrop's own spelling, and
    /// deciding what to do with that is the bridge's job, not ours —
    /// we no longer hold a connection to any network.
    pub async fn check_timers_headless(&mut self) -> Result<Vec<(String, String)>> {
        let result = self
            .tcl_thread
            .eval_simple("timers check".to_string())
            .await?;
        Ok(crate::tcl_plugin::parse_tcl_response_list(&result))
    }

    /// The live trigger bindings and disable rules, as data rather than
    /// as one Tcl dump line. The mask a bridge derives (which events
    /// does this channel actually want) is computed from THIS, not
    /// restated beside it — the house rule.
    pub async fn triggers_state(&mut self) -> Result<TriggerState> {
        let bindings_raw = self
            .tcl_thread
            .eval_simple("triggers list_bindings".to_string())
            .await?;
        let disabled_raw = self
            .tcl_thread
            .eval_simple("triggers status".to_string())
            .await?;

        // `list_bindings` answers a Tcl list of {event pattern proc}
        // triples; `status` answers lines of "key -> proc".
        let bindings = parse_binding_triples(&bindings_raw);
        let disabled = parse_disabled_lines(&disabled_raw);
        Ok(TriggerState { bindings, disabled })
    }

    /// Everything on disk under `procs/` — the index and every blob —
    /// read WITHOUT touching the interpreter, for a mirror that cannot
    /// be starved by a running eval (item 3). Blobs are `{args} {body}`;
    /// names come from the index. Unparseable blobs are skipped rather
    /// than erroring the whole listing: one corrupted entry must not
    /// take down the bridge's sync with it.
    pub fn procs_from_disk(&self) -> Result<Vec<ProcEntry>> {
        let state_path = &self.tcl_config.state_path;
        let index_path = state_path.join("procs/_index");
        if !index_path.exists() {
            return Ok(Vec::new());
        }
        let index = std::fs::read_to_string(&index_path)?;
        let mut out = Vec::new();
        for line in index.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let name = parts[0].to_string();
            let hash = parts[1].to_string();
            let Ok(content) = std::fs::read_to_string(state_path.join("procs").join(&hash)) else {
                continue; // a referenced blob vanished: skip, do not fail
            };
            // `{args} {body}` — the first two top-level elements.
            let words = split_tcl_words(&content);
            let args = words.first().cloned().unwrap_or_default();
            let body = words.get(1).cloned().unwrap_or_default();
            out.push(ProcEntry {
                name,
                args,
                body,
                hash,
            });
        }
        Ok(out)
    }

    /// Everything on disk under `vars/`, the same off-interpreter rule.
    /// Blobs are `scalar {value}` or `array {k v …}`.
    pub fn vars_from_disk(&self) -> Result<Vec<VarEntry>> {
        let state_path = &self.tcl_config.state_path;
        let index_path = state_path.join("vars/_index");
        if !index_path.exists() {
            return Ok(Vec::new());
        }
        let index = std::fs::read_to_string(&index_path)?;
        let mut out = Vec::new();
        for line in index.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let name = parts[0].to_string();
            let hash = parts[1].to_string();
            let Ok(content) = std::fs::read_to_string(state_path.join("vars").join(&hash)) else {
                continue;
            };
            let words = split_tcl_words(&content);
            let (kind, value) = match words.first().map(String::as_str) {
                Some("scalar") => ("scalar", words.get(1).cloned().unwrap_or_default()),
                Some("array") => ("array", words.get(1).cloned().unwrap_or_default()),
                _ => ("scalar", content),
            };
            out.push(VarEntry {
                name,
                kind: kind.to_string(),
                value,
            });
        }
        Ok(out)
    }

    /// Deploy one proc through the live interpreter — the ONLY write
    /// path that live-updates and commits correctly (item 3). The code
    /// is built with `tcl_escape_arg`, so a body containing braces or
    /// semicolons stays one literal script. `user` rides as the git
    /// author; `nick`/`host` default to bridge-style values, so the
    /// commit is never attributed to a nick that does not exist.
    pub async fn deploy_proc(
        &mut self,
        name: &str,
        args: &str,
        body: &str,
        user: Option<&str>,
    ) -> Result<String> {
        use crate::tcl_escape::tcl_escape_arg;
        let user = user.unwrap_or("web");
        let nick = user
            .split('@')
            .next()
            .filter(|n| !n.is_empty())
            .unwrap_or("web");
        let host = user.split_once('@').map(|(_, h)| h).unwrap_or("local");
        let ctx = EvalContext::new(nick.to_string(), host.to_string());
        let code = format!(
            // A `proc` REDEFINITION replaces the live entry and keeps the
            // modified-proc tracker pointing at this name, so saving here
            // persists the new blob and updates the index — the same path
            // typing it in the channel takes. Not `is_admin`: adminship
            // does not decide whether a caller may define a proc; the
            // bearer token already did.
            "proc {} {} {}",
            tcl_escape_arg(name),
            tcl_escape_arg(args),
            tcl_escape_arg(body),
        );
        let response = self.eval(&code, ctx).await?;
        if response.is_error {
            // The interpreter refused the definition (bad args, a body
            // it cannot parse). Return the refusal as an ERROR so the
            // caller reports success:false — reporting it as a success
            // message would let a broken deploy read as "deployed".
            return Err(anyhow::anyhow!(
                "the interpreter refused the proc definition: {}",
                response.output.join("\n")
            ));
        }
        let mut msg = format!("{} deployed", name);
        if let Some(ci) = &response.commit_info {
            let hash = &ci.commit_id;
            msg.push_str(&format!(" (commit {})", &hash[..hash.len().min(8)]));
        }
        Ok(msg)
    }
}

/// One proc, as the mirror needs it: the real name, the args-string and
/// body-string, and the content hash at fetch time.
#[derive(Debug, Clone)]
pub struct ProcEntry {
    pub name: String,
    pub args: String,
    pub body: String,
    pub hash: String,
}

/// One persistent variable, scalar or array.
#[derive(Debug, Clone)]
pub struct VarEntry {
    pub name: String,
    pub kind: String,
    pub value: String,
}

/// The trigger engine's live state, as the bridge needs it.
#[derive(Debug, Clone, Default)]
pub struct TriggerState {
    /// Every binding: (event, channel pattern, proc name).
    pub bindings: Vec<TriggerBinding>,
    /// Disable rules, keyed `network:channel` in SLOPDROP's spelling.
    pub disabled: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone)]
pub struct TriggerBinding {
    pub event: String,
    pub pattern: String,
    pub proc_name: String,
}

/// Split a Tcl list of `{event pattern proc}` triples.
///
/// The same brace-walker idea as `parse_tcl_response_list`, one element
/// deeper: each top-level element is itself a braced list of three
/// words. Hand-rolled rather than reaching for a Tcl parser crate
/// because the shapes here are flat and the source is our own ensemble.
fn parse_binding_triples(raw: &str) -> Vec<TriggerBinding> {
    let mut out = Vec::new();
    for element in split_tcl_words(raw) {
        let words = split_tcl_words(&element);
        if words.len() == 3 {
            out.push(TriggerBinding {
                event: words[0].clone(),
                pattern: words[1].clone(),
                proc_name: words[2].clone(),
            });
        }
    }
    out
}

/// Parse `status`'s lines: `key -> proc`, one rule per line.
fn parse_disabled_lines(raw: &str) -> Vec<(String, Vec<String>)> {
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if let Some((key, proc_name)) = line.split_once(" -> ") {
            let key = key.trim().to_string();
            let proc_name = proc_name.trim().to_string();
            if key.is_empty() || proc_name.is_empty() {
                continue;
            }
            if !map.contains_key(&key) {
                order.push(key.clone());
            }
            map.entry(key).or_default().push(proc_name);
        }
    }
    order
        .into_iter()
        .filter_map(|k| map.remove(&k).map(|procs| (k, procs)))
        .collect()
}

/// Split a Tcl list into its words/elements: braced elements are
/// de-quoted (honouring nesting and backslash escapes), bare words run
/// to the next whitespace. Good enough for the flat lists our own
/// ensembles emit; not a Tcl parser.
fn split_tcl_words(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = raw.trim().chars().peekable();
    loop {
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
        let Some(&c) = chars.peek() else { break };
        if c == '{' {
            chars.next(); // consume the opening brace
            let mut depth = 1usize;
            let mut w = String::new();
            while let Some(c2) = chars.next() {
                match c2 {
                    '\\' => {
                        w.push(c2);
                        if let Some(c3) = chars.next() {
                            w.push(c3);
                        }
                    }
                    '{' => {
                        depth += 1;
                        w.push(c2);
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        w.push(c2);
                    }
                    _ => w.push(c2),
                }
            }
            out.push(w);
        } else {
            let mut w = String::new();
            while let Some(&c2) = chars.peek() {
                if c2.is_whitespace() {
                    break;
                }
                w.push(c2);
                chars.next();
            }
            out.push(w);
        }
    }
    out
}
