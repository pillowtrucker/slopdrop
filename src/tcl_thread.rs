use crate::config::TclConfig;
use crate::state::{InterpreterState, StatePersistence, UserInfo};
use crate::tcl_wrapper::SafeTclInterp;
use crate::types::ChannelMembers;
use anyhow::Result;
use std::collections::HashSet;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

#[cfg(unix)]
use nix::sys::resource::{setrlimit, Resource};

/// Set memory limit for current process (Unix only)
#[cfg(unix)]
fn set_memory_limit(limit_mb: u64) -> Result<()> {
    if limit_mb == 0 {
        // 0 means no limit
        return Ok(());
    }

    let limit_bytes = limit_mb * 1024 * 1024;

    // Set virtual memory limit (RLIMIT_AS)
    setrlimit(Resource::RLIMIT_AS, limit_bytes, limit_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to set memory limit: {}", e))?;

    info!("Memory limit set to {} MB", limit_mb);
    Ok(())
}

#[cfg(not(unix))]
fn set_memory_limit(_limit_mb: u64) -> Result<()> {
    // Memory limits not supported on non-Unix platforms
    warn!("Memory limits not supported on this platform");
    Ok(())
}

/// Request to evaluate TCL code
#[derive(Debug)]
pub struct EvalRequest {
    pub code: String,
    pub is_admin: bool,
    pub nick: String,
    pub host: String,
    pub channel: String,
    pub network: String,
    /// What the room looks like, when a bridge reported it. Display
    /// only: the privilege check reads `nick` and `host` above, never
    /// this.
    pub room: crate::tcl_service::RoomContext,
    pub response_tx: oneshot::Sender<EvalResult>,
}

/// Result of TCL evaluation
#[derive(Debug, Clone)]
pub struct EvalResult {
    pub output: String,
    /// Indicates whether the output is an error message
    /// Currently not used but kept for future error handling improvements
    #[allow(dead_code)]
    pub is_error: bool,
    /// Git commit information (if state changed and was committed)
    pub commit_info: Option<crate::state::CommitInfo>,
}

/// Commands that can be sent to the TCL thread
pub enum TclThreadCommand {
    Eval(EvalRequest),
    LogMessage {
        channel: String,
        nick: String,
        mask: String,
        text: String,
    },
    Reload,
    UpdateConfig {
        tcl_config: TclConfig,
        security_config: crate::config::SecurityConfig,
    },
    Shutdown,
}

/// Handle to communicate with the TCL thread
pub struct TclThreadHandle {
    command_tx: mpsc::Sender<TclThreadCommand>,
    thread_handle: Option<thread::JoinHandle<()>>,
    timeout: Duration,
    tcl_config: TclConfig,
    security_config: crate::config::SecurityConfig,
    channel_members: ChannelMembers,
}

impl TclThreadHandle {
    /// Spawn a new TCL thread
    pub fn spawn(
        tcl_config: TclConfig,
        security_config: crate::config::SecurityConfig,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let (command_tx, command_rx) = mpsc::channel();
        let timeout = Duration::from_millis(security_config.eval_timeout_ms);

        let tcl_config_clone = tcl_config.clone();
        let security_config_clone = security_config.clone();
        let channel_members_clone = channel_members.clone();

        let thread_handle = thread::spawn(move || {
            // Set memory limit for this thread
            if let Err(e) = set_memory_limit(security_config_clone.memory_limit_mb) {
                error!("Failed to set memory limit: {}", e);
            }

            let worker = TclThreadWorker::new(
                tcl_config_clone,
                security_config_clone,
                channel_members_clone,
            );
            if let Err(e) = worker {
                error!("Failed to create TCL worker: {}", e);
                return;
            }

            worker.unwrap().run(command_rx);
        });

        info!("TCL thread spawned with {}ms timeout", timeout.as_millis());

        Ok(Self {
            command_tx,
            thread_handle: Some(thread_handle),
            timeout,
            tcl_config,
            security_config,
            channel_members,
        })
    }

    /// Restart the TCL thread (called after timeout/hang)
    fn restart(&mut self) -> Result<()> {
        warn!("Restarting hung TCL thread");

        // Drop old thread handle (abandon hung thread)
        if let Some(handle) = self.thread_handle.take() {
            // Don't wait for it - it's hung
            drop(handle);
        }

        // Create new channel
        let (command_tx, command_rx) = mpsc::channel();

        // Spawn new thread
        let tcl_config = self.tcl_config.clone();
        let security_config = self.security_config.clone();
        let channel_members = self.channel_members.clone();

        let thread_handle = thread::spawn(move || {
            // Set memory limit for this thread
            if let Err(e) = set_memory_limit(security_config.memory_limit_mb) {
                error!("Failed to set memory limit after restart: {}", e);
            }

            let worker = TclThreadWorker::new(tcl_config, security_config, channel_members);
            if let Err(e) = worker {
                error!("Failed to create TCL worker after restart: {}", e);
                return;
            }

            worker.unwrap().run(command_rx);
        });

        // Update handle
        self.command_tx = command_tx;
        self.thread_handle = Some(thread_handle);

        info!("TCL thread restarted successfully");
        Ok(())
    }

    /// Evaluate TCL code with timeout
    pub async fn eval(
        &mut self,
        code: String,
        is_admin: bool,
        nick: String,
        host: String,
        channel: String,
        network: String,
        room: crate::tcl_service::RoomContext,
    ) -> Result<EvalResult> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = EvalRequest {
            code,
            is_admin,
            nick,
            host,
            channel,
            network,
            room,
            response_tx,
        };

        // Send request to TCL thread
        if let Err(e) = self.command_tx.send(TclThreadCommand::Eval(request)) {
            // Channel closed - thread probably crashed/panicked
            error!("TCL thread channel closed (thread crashed): {}", e);

            // Restart the thread
            if let Err(restart_err) = self.restart() {
                error!("Failed to restart TCL thread after crash: {}", restart_err);
                return Ok(EvalResult {
                    output: format!("error: thread crashed and failed to restart: {}", restart_err),
                    is_error: true,
                    commit_info: None,
                });
            }

            return Ok(EvalResult {
                output: "error: thread crashed (likely out of memory), restarted".to_string(),
                is_error: true,
                commit_info: None,
            });
        }

        // Wait for response with timeout
        debug!("Waiting for TCL response with timeout of {}ms", self.timeout.as_millis());
        let start = Instant::now();
        match tokio::time::timeout(self.timeout, response_rx).await {
            Ok(Ok(result)) => {
                debug!("TCL response received after {}ms", start.elapsed().as_millis());
                Ok(result)
            }
            Ok(Err(e)) => {
                // Response channel closed unexpectedly - thread crashed
                error!("TCL thread died unexpectedly: {}", e);

                // Restart the thread
                if let Err(restart_err) = self.restart() {
                    error!("Failed to restart TCL thread after crash: {}", restart_err);
                    return Ok(EvalResult {
                        output: format!("error: thread died and failed to restart: {}", restart_err),
                        is_error: true,
                        commit_info: None,
                    });
                }

                Ok(EvalResult {
                    output: "error: thread died unexpectedly (likely out of memory), restarted".to_string(),
                    is_error: true,
                    commit_info: None,
                })
            }
            Err(_) => {
                // Timeout! The TCL thread is hung
                warn!("TCL evaluation timed out after {}ms - thread is hung, restarting", self.timeout.as_millis());

                // Restart the thread
                if let Err(e) = self.restart() {
                    error!("Failed to restart TCL thread: {}", e);
                    return Ok(EvalResult {
                        output: format!("error: timeout and failed to restart: {}", e),
                        is_error: true,
                        commit_info: None,
                    });
                }

                Ok(EvalResult {
                    output: format!("error: evaluation timed out after {}s (thread restarted)", self.timeout.as_secs()),
                    is_error: true,
                    commit_info: None,
                })
            }
        }
    }

    /// Simple eval for system-level operations (like timer checking)
    /// Uses a "system" context without user tracking
    pub async fn eval_simple(&mut self, code: String) -> Result<String> {
        let result = self.eval(
            code,
            false,
            "system".to_string(),
            "system@bot".to_string(),
            "system".to_string(),
            "system".to_string(),
            Default::default(),
        ).await?;

        Ok(result.output)
    }

    /// Log a message to the channel history
    pub fn log_message(&self, channel: String, nick: String, mask: String, text: String) {
        let _ = self.command_tx.send(TclThreadCommand::LogMessage {
            channel,
            nick,
            mask,
            text,
        });
    }

    /// Reload TCL modules from disk
    pub fn reload(&self) {
        info!("Sending reload command to TCL thread");
        let _ = self.command_tx.send(TclThreadCommand::Reload);
    }

    /// Update runtime configuration
    pub fn update_config(
        &mut self,
        tcl_config: TclConfig,
        security_config: crate::config::SecurityConfig,
    ) -> Result<()> {
        info!("Sending config update command to TCL thread");

        // Update handle's stored configs
        self.timeout = Duration::from_millis(security_config.eval_timeout_ms);
        self.tcl_config = tcl_config.clone();
        self.security_config = security_config.clone();

        // Send config update to thread
        self.command_tx
            .send(TclThreadCommand::UpdateConfig {
                tcl_config,
                security_config,
            })
            .map_err(|e| anyhow::anyhow!("Failed to send config update: {}", e))?;

        Ok(())
    }

    /// Shutdown the TCL thread
    pub fn shutdown(&mut self) {
        info!("Shutting down TCL thread");
        let _ = self.command_tx.send(TclThreadCommand::Shutdown);

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TclThreadHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Worker that runs in the TCL thread
struct TclThreadWorker {
    interp: SafeTclInterp,
    tcl_config: TclConfig,
    security_config: crate::config::SecurityConfig,
    timeout: Duration,
    channel_members: ChannelMembers,
}

impl TclThreadWorker {
    fn new(
        tcl_config: TclConfig,
        security_config: crate::config::SecurityConfig,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let interp = SafeTclInterp::new_with_options(
            security_config.eval_timeout_ms,
            &tcl_config.state_path,
            tcl_config.state_repo.clone(),
            tcl_config.ssh_key.clone(),
            security_config.max_recursion_depth,
            tcl_config.show_error_traces,
        )?;

        // Register chanlist command
        Self::register_chanlist_command(interp.interpreter(), channel_members.clone())?;

        // The veles bridge: a native `ai` command, when one is
        // configured. Native rather than Tcl because it holds a bearer
        // token, and this interpreter has no boundary a secret could
        // hide behind — see `veles_bridge`'s header.
        if let Some(bridge) = crate::veles_bridge::global() {
            // Safety: the interpreter is owned by this worker, which
            // owns it for the life of the thread; the Arc is leaked
            // into ClientData for the process.
            unsafe { crate::veles_bridge::register(bridge, interp.interpreter()) };
            tracing::info!("veles bridge registered: `ai <prompt>` is available");
        }

        let timeout = Duration::from_millis(security_config.eval_timeout_ms);

        Ok(Self {
            interp,
            tcl_config,
            security_config,
            timeout,
            channel_members,
        })
    }

    /// Register the chanlist command that reads from the synced channel members array
    fn register_chanlist_command(
        interp: &tcl::Interpreter,
        _channel_members: ChannelMembers,
    ) -> Result<()> {
        // Create a proc that reads from the ::slopdrop_channel_members array.
        // The array is synced before each eval by sync_channel_members() and is
        // keyed by composite "network:#channel" since channel members are tracked
        // per-network. The proc looks up the current network from ::network and
        // falls back to a bare channel-name lookup for callers that pre-built
        // the composite key themselves.
        interp.eval(r#"
            # chanlist command - returns list of nicks in a channel
            # Reads from ::slopdrop_channel_members which is synced before each eval
            proc chanlist {channel} {
                if {[info exists ::network] && $::network ne ""} {
                    set key "${::network}:${channel}"
                    if {[info exists ::slopdrop_channel_members($key)]} {
                        return $::slopdrop_channel_members($key)
                    }
                }
                if {[info exists ::slopdrop_channel_members($channel)]} {
                    return $::slopdrop_channel_members($channel)
                }
                return ""
            }
        "#).map_err(|e| anyhow::anyhow!("Failed to register chanlist command: {:?}", e))?;

        Ok(())
    }

    /// Sync channel members from Rust to TCL global array
    fn sync_channel_members(&self) {
        match self.channel_members.read() {
            Ok(members) => {
                for (channel, nicks) in members.iter() {
                    if !nicks.is_empty() {
                        let mut sorted: Vec<_> = nicks.iter().cloned().collect();
                        sorted.sort();

                        // Escape channel name and nicks for TCL
                        let escaped_channel = channel
                            .replace('\\', "\\\\")
                            .replace('{', "\\{")
                            .replace('}', "\\}");

                        let escaped_nicks: Vec<String> = sorted.iter()
                            .map(|n| n
                                .replace('\\', "\\\\")
                                .replace('{', "\\{")
                                .replace('}', "\\}"))
                            .collect();

                        let tcl_code = format!(
                            "set ::slopdrop_channel_members({}) {{{}}}",
                            escaped_channel,
                            escaped_nicks.join(" ")
                        );

                        if let Err(e) = self.interp.interpreter().eval(tcl_code.as_str()) {
                            warn!("Failed to sync channel members for {}: {:?}", channel, e);
                        }
                    } else {
                        // Empty channel - unset if exists
                        let escaped_channel = channel
                            .replace('\\', "\\\\")
                            .replace('{', "\\{")
                            .replace('}', "\\}");
                        let unset_cmd = format!(
                            "catch {{unset ::slopdrop_channel_members({})}}",
                            escaped_channel
                        );
                        let _ = self.interp.interpreter().eval(unset_cmd.as_str());
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read channel members: {:?}", e);
            }
        }
    }

    fn run(mut self, command_rx: mpsc::Receiver<TclThreadCommand>) {
        info!("TCL thread worker started");

        for command in command_rx {
            match command {
                TclThreadCommand::Eval(request) => {
                    self.handle_eval(request);
                }
                TclThreadCommand::LogMessage { channel, nick, mask, text } => {
                    self.handle_log_message(channel, nick, mask, text);
                }
                TclThreadCommand::Reload => {
                    self.handle_reload();
                }
                TclThreadCommand::UpdateConfig { tcl_config, security_config } => {
                    self.handle_config_update(tcl_config, security_config);
                }
                TclThreadCommand::Shutdown => {
                    info!("TCL thread worker shutting down");
                    break;
                }
            }
        }
    }

    fn handle_reload(&self) {
        info!("Reloading TCL modules");
        match self.interp.reload_modules() {
            Ok(()) => info!("TCL modules reloaded successfully"),
            Err(e) => error!("Failed to reload TCL modules: {}", e),
        }
    }

    fn handle_config_update(
        &mut self,
        _tcl_config: TclConfig,
        security_config: crate::config::SecurityConfig,
    ) {
        info!("Updating runtime configuration");

        // Update timeout for evaluations
        self.timeout = Duration::from_millis(security_config.eval_timeout_ms);
        info!("  Eval timeout updated to {}ms", security_config.eval_timeout_ms);

        // Update security config (used for blacklist checks, etc.)
        self.security_config = security_config.clone();

        // Note: max_recursion_depth requires recreating the interpreter
        // For now, we only update runtime-changeable settings
        // The interpreter's recursion limit cannot be changed after creation

        info!("Configuration updated successfully (some settings require restart)");
    }

    fn handle_log_message(&self, channel: String, nick: String, mask: String, text: String) {
        // Store message in ::slopdrop_log_lines($channel)
        // Format: {timestamp nick mask message}
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Escape TCL special characters in text
        let escaped_text = text
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('$', "\\$")
            .replace('"', "\\\"");

        let escaped_nick = nick
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}");

        let escaped_mask = mask
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}");

        let escaped_channel = channel
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}");

        // Add to log array with size limit (default 1000 lines per channel)
        // Channel name is wrapped in braces {#bottest} which handles # correctly
        // Use a TCL variable to avoid issues with # in array subscripts
        // This ensures the key matches what $::channel will be during eval
        let tcl_code = format!(r#"
            set _chan {{{}}}
            set entry [list {} {{{}}} {{{}}} {{{}}}]
            if {{![info exists ::slopdrop_log_lines($_chan)]}} {{
                set ::slopdrop_log_lines($_chan) [list]
            }}
            lappend ::slopdrop_log_lines($_chan) $entry
            # Keep only last 1000 entries
            if {{[llength $::slopdrop_log_lines($_chan)] > 1000}} {{
                set ::slopdrop_log_lines($_chan) [lrange $::slopdrop_log_lines($_chan) end-999 end]
            }}
        "#,
            escaped_channel,
            timestamp, escaped_nick, escaped_mask, escaped_text
        );

        debug!("Logging message to channel '{}': TCL code:\n{}", channel, tcl_code);

        if let Err(e) = self.interp.interpreter().eval(tcl_code.as_str()) {
            warn!("Failed to log message: {:?}", e);
        } else {
            debug!("Successfully logged message");
        }
    }

    /// Brace-quote one value for `set ::var {…}`.
    fn tcl_brace(v: &str) -> String {
        v.replace('\\', "\\\\").replace('{', "\\{").replace('}', "\\}")
    }

    /// Put the conversation into the interpreter, for the procs that read
    /// it out of globals.
    ///
    /// These used to be set by our own IRC connection, and a large part of
    /// what a channel has built reads them: `tcl/utils.tcl` publishes
    /// `[nick]`, `[names]`, `[name]` and `[hostmask]` on top of `::nick`,
    /// `::mask`, `::channel` and `chanlist`, and `tcl/timtom.tcl` builds
    /// `channel_nicks`, `nick_count` and `random_other_nick` on top of
    /// those. Headless — the `--web` deployment — the only process that
    /// still knows any of it is whichever bot is in the room, so a bridge
    /// reports it per request and this puts it back.
    ///
    /// TWO IDENTITIES, DELIBERATELY NOT ONE. `request.nick`/`request.host`
    /// are the AUTHORIZATION identity: `handle_eval` builds `nick!host`
    /// from them and matches `privileged_users`. `request.room` is the
    /// DISPLAY identity, and a caller can choose it. So the room wins for
    /// the globals and loses everywhere else, and nothing here is ever
    /// read by the privilege check.
    ///
    /// Runs for the admin path too. It did not before — `handle_eval`'s
    /// admin branch called `eval` where the ordinary one called
    /// `eval_with_context` — so `tclAdmin` saw whatever the previous
    /// evaluation had left in `::nick`.
    fn apply_room_context(&self, request: &EvalRequest) {
        let room = &request.room;

        // The speaker. Every frontend has a real one — an IRC nick, a web
        // `user`, a CLI username — so this is always set, and the room's
        // version wins when a bridge sent one.
        let nick = room.nick.clone().unwrap_or_else(|| request.nick.clone());
        let mask = room.mask.clone().unwrap_or_else(|| request.host.clone());
        for (var, val) in [("nick", &nick), ("mask", &mask)] {
            self.set_global(var, val);
        }

        // The room itself is different, and the difference is the whole
        // reason this function exists.
        //
        // `EvalContext.channel` is an Option that becomes the literal
        // string "default" one layer up, so a caller who said nothing
        // about a room is indistinguishable here from one who named a
        // channel called "default". Setting ::channel from it every time
        // means an unrelated API call — a cron, the bundled web page —
        // silently moves the room out from under the procs a bridge just
        // set up, and `names` then answers "nobody is here" with total
        // confidence.
        //
        // So: a request that REPORTS a room sets it; one that does not
        // only fills a hole. The hole has to be filled, because
        // `tcl/utils.tcl` does `chanlist $::channel` and an unset
        // variable is a Tcl error rather than an empty answer.
        match room.channel.as_deref().filter(|c| !c.is_empty()) {
            Some(c) => {
                self.set_global("channel", c);
                self.set_global("network", &request.network);
                // The topic belongs to the channel, so it is rewritten
                // exactly when the channel is — including to empty, which
                // is what a room with no topic must leave behind.
                self.set_global("topic", room.topic.as_deref().unwrap_or_default());
            }
            None => {
                self.fill_global("channel", &request.channel);
                self.fill_global("network", &request.network);
                self.fill_global("topic", "");
            }
        }

        // The roster, filed where `chanlist` looks for it: the composite
        // `network:#channel` key, because members are tracked per network
        // and a bare channel name collides across them.
        //
        // Written AFTER `sync_channel_members` and allowed to win. The
        // bridge is the process actually sitting in that room right now;
        // our own copy is whatever our IRC connection last saw, which
        // headless is nothing at all. An absent roster changes nothing —
        // an eval with no room behind it must not empty the channel.
        if !room.members.is_empty() {
            if let Some(channel) = room.channel.as_deref().filter(|c| !c.is_empty()) {
                let key = format!("{}:{}", request.network, channel);
                let names: Vec<String> = room.members.iter().map(|n| Self::tcl_brace(n)).collect();
                let code = format!(
                    "set ::slopdrop_channel_members({}) {{{}}}",
                    Self::tcl_brace(&key),
                    names.join(" ")
                );
                if let Err(e) = self.interp.interpreter().eval(code.as_str()) {
                    warn!("Failed to set channel members for {}: {:?}", key, e);
                }
            }
        }
    }

    /// `set ::<var> {<val>}`.
    fn set_global(&self, var: &str, val: &str) {
        let code = format!("set ::{} {{{}}}", var, Self::tcl_brace(val));
        if let Err(e) = self.interp.interpreter().eval(code.as_str()) {
            warn!("Failed to set ::{}: {:?}", var, e);
        }
    }

    /// The same, but only where there is no answer yet — a default for a
    /// fresh interpreter that never overwrites a real one.
    ///
    /// Unset AND empty both count as "no answer": `setup_safe_interp` does
    /// `set ::network {}` at boot precisely so `info exists` is true
    /// everywhere, so an existence check alone would never fill anything.
    fn fill_global(&self, var: &str, val: &str) {
        let code = format!(
            "if {{![info exists ::{0}] || ${{::{0}}} eq {{}}}} {{ set ::{0} {{{1}}} }}",
            var,
            Self::tcl_brace(val)
        );
        if let Err(e) = self.interp.interpreter().eval(code.as_str()) {
            warn!("Failed to default ::{}: {:?}", var, e);
        }
    }

    fn handle_eval(&self, request: EvalRequest) {
        debug!("TCL thread evaluating: {}", request.code);

        // Check privilege level using hostmask matching
        if request.is_admin {
            // Build full hostmask: nick!ident@host
            // host parameter contains "ident@host" as built in tcl_plugin
            let hostmask = format!("{}!{}", request.nick, request.host);

            // Check if hostmask matches any privileged pattern
            let is_privileged = self.security_config.privileged_users.iter().any(|pattern| {
                crate::hostmask::matches_hostmask(&hostmask, pattern)
            });

            if !is_privileged {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: tclAdmin requires privileges (your hostmask: {})", hostmask),
                    is_error: true,
                    commit_info: None,
                });
                return;
            }
        }

        // A new evaluation gets a fresh per-eval model-call budget. The
        // per-minute one deliberately does not reset, or "one call per
        // eval, a thousand evals" would be free.
        if let Some(b) = crate::veles_bridge::global() {
            b.begin_eval();
        }

        // Get eval count for rate limiting (needed for all commands)
        let eval_count_result = self.interp.interpreter().eval("::httpx::increment_eval");
        let eval_count = eval_count_result
            .ok()
            .and_then(|obj| obj.get_string().parse::<u64>().ok())
            .unwrap_or(0);

        // Set HTTP context variables (for rate limiting)
        let set_channel = format!("set ::nick_channel {{{}}}", request.channel);
        let _ = self.interp.interpreter().eval(set_channel.as_str());

        // `::network` moved into `apply_room_context`: it names the same
        // room `::channel` does — `chanlist` builds ONE composite key out
        // of the pair — so a caller that reports no room must not move
        // half of it. Set here unconditionally, a bare API call reset it
        // to "default" and the roster a bridge had just filed under
        // `irc4fun:#coven` became unreachable, which reads from the
        // channel as the bot forgetting who is in the room.

        // Set stock context for rate limiting
        crate::stock_commands::set_stock_context(request.nick.clone(), eval_count);

        // Sync channel members to TCL array before evaluation
        self.sync_channel_members();

        // …then whatever the bridge reported about the room, which is the
        // only source of it when we hold no IRC connection ourselves.
        self.apply_room_context(&request);

        // Check for special commands
        let code_trimmed = request.code.trim();
        if code_trimmed == "history" || code_trimmed.starts_with("history ") {
            self.handle_history_command(request);
            return;
        }
        if code_trimmed.starts_with("rollback ") {
            self.handle_rollback_command(request);
            return;
        }
        if code_trimmed.starts_with("chanlist ") {
            self.handle_chanlist_command(request);
            return;
        }
        // Intercept stock commands that need Rust backend
        if code_trimmed.starts_with("stock::quote ")
            || code_trimmed.starts_with("stock::price ")
            || code_trimmed.starts_with("stock::detail ")
            || code_trimmed.starts_with("stock::history ")
            || code_trimmed.starts_with("stock::chart ")
        {
            self.handle_stock_command(request);
            return;
        }

        // Capture state before evaluation
        let state_before = InterpreterState::capture(self.interp.interpreter());

        // Evaluate the code
        // `apply_room_context` has already set ::nick, ::mask and
        // ::channel — for BOTH paths, where `eval_with_context` set them
        // for only one, leaving `tclAdmin` reading the previous
        // evaluation's speaker.
        let result = self.interp.eval(&request.code);

        let output = match result {
            Ok(output) => EvalResult {
                output,
                is_error: false,
                commit_info: None,
            },
            Err(e) => EvalResult {
                output: format!("error: {}", e),
                is_error: true,
                commit_info: None,
            },
        };

        // Update var traces AFTER eval to catch any new variables that were created
        // This is more efficient than checking all 1000+ vars before every eval
        // New vars will get traces for the NEXT eval, existing vars already have traces
        let _ = self.interp.interpreter().eval("::slopdrop::update_var_traces");

        // Capture state after and save if changed
        let mut output = output;
        if let Ok(state_after) = InterpreterState::capture(self.interp.interpreter()) {
            if let Ok(state_before) = state_before {
                // Get list of procs and vars that were modified during this eval
                let modified_procs = InterpreterState::get_modified_procs(self.interp.interpreter())
                    .unwrap_or_else(|_| HashSet::new());
                let modified_vars = InterpreterState::get_modified_vars(self.interp.interpreter())
                    .unwrap_or_else(|_| HashSet::new());

                debug!("Modified procs: {:?}", modified_procs);
                debug!("Modified vars: {:?}", modified_vars);
                debug!("State before vars: {} procs: {}", state_before.vars.len(), state_before.procs.len());
                debug!("State after vars: {} procs: {}", state_after.vars.len(), state_after.procs.len());

                let changes = state_before.diff(&state_after, &modified_procs, &modified_vars);

                debug!("Changes detected: new_procs={}, new_vars={}, deleted_procs={}, deleted_vars={}",
                    changes.new_procs.len(), changes.new_vars.len(),
                    changes.deleted_procs.len(), changes.deleted_vars.len());

                // Skip state persistence for system evals (timers, triggers, etc.)
                // Only persist state changes from actual user interactions
                let is_system_eval = request.nick == "system";

                if changes.has_changes() && !is_system_eval {
                    debug!("State changed: {:?}", changes);

                    let user_info = UserInfo::new(request.nick.clone(), request.host.clone());
                    let persistence = StatePersistence::with_repo(
                        self.tcl_config.state_path.clone(),
                        self.tcl_config.state_repo.clone(),
                        self.tcl_config.ssh_key.clone(),
                    );

                    match persistence.save_changes(
                        self.interp.interpreter(),
                        &changes,
                        &user_info,
                        &request.code,
                    ) {
                        Ok(commit_info) => {
                            debug!("State saved successfully");
                            output.commit_info = commit_info;
                        }
                        Err(e) => {
                            warn!("Failed to save state: {}", e);
                        }
                    }
                } else if changes.has_changes() && is_system_eval {
                    debug!("Skipping state persistence for system eval (nick={})", request.nick);
                    // Clear modified tracking so system changes don't pollute user commits
                    let _ = self.interp.interpreter().eval("set ::slopdrop_modified_procs [list]");
                    let _ = self.interp.interpreter().eval("set ::slopdrop_modified_vars [list]");
                }
            }
        }

        // Send response back
        let _ = request.response_tx.send(output);
    }

    fn handle_history_command(&self, request: EvalRequest) {
        let code = request.code.trim();

        // Parse count from "history" or "history <count>"
        let count = if code == "history" {
            10 // default
        } else if let Some(count_str) = code.strip_prefix("history ") {
            count_str.trim().parse::<usize>().unwrap_or(10)
        } else {
            10
        };

        let persistence = StatePersistence::with_repo(
            self.tcl_config.state_path.clone(),
            self.tcl_config.state_repo.clone(),
            self.tcl_config.ssh_key.clone(),
        );

        match persistence.get_history(count) {
            Ok(commits) => {
                if commits.is_empty() {
                    let _ = request.response_tx.send(EvalResult {
                        output: "No commits found".to_string(),
                        is_error: false,
                        commit_info: None,
                    });
                    return;
                }

                // Format commits as TCL list
                let mut output = String::new();
                for (hash, timestamp, author, message) in commits {
                    // Format: {hash timestamp author message}
                    let date = chrono::DateTime::from_timestamp(timestamp, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| timestamp.to_string());

                    output.push_str(&format!("{} {} {} {}\n",
                        &hash[..8], date, author, message));
                }

                let _ = request.response_tx.send(EvalResult {
                    output: output.trim_end().to_string(),
                    is_error: false,
                    commit_info: None,
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }

    fn handle_rollback_command(&self, request: EvalRequest) {
        // Rollback is admin-only
        if !request.is_admin {
            let _ = request.response_tx.send(EvalResult {
                output: "error: rollback requires admin privileges (use tclAdmin)".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        }

        let code = request.code.trim();

        // Parse commit hash from "rollback <hash>"
        let hash = if let Some(h) = code.strip_prefix("rollback ") {
            h.trim()
        } else {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: rollback <commit-hash>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        };

        if hash.is_empty() {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: rollback <commit-hash>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        }

        let persistence = StatePersistence::with_repo(
            self.tcl_config.state_path.clone(),
            self.tcl_config.state_repo.clone(),
            self.tcl_config.ssh_key.clone(),
        );

        match persistence.rollback_to(hash) {
            Ok(()) => {
                // After rollback, state files have been reset via git
                // The TCL interpreter still has old state in memory
                // Restarting the bot (or just the TCL thread) loads fresh state from disk
                // Since rollback is an admin-only operation rarely used, manual restart is acceptable
                let _ = request.response_tx.send(EvalResult {
                    output: format!("Rolled back to commit {}. Note: Restart bot to reload state.", hash),
                    is_error: false,
                    commit_info: None,
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }

    fn handle_chanlist_command(&self, request: EvalRequest) {
        let code = request.code.trim();

        // Parse channel from "chanlist <channel>"
        let channel = if let Some(ch) = code.strip_prefix("chanlist ") {
            ch.trim()
        } else {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: chanlist <channel>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        };

        if channel.is_empty() {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: chanlist <channel>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        }

        // The ROOM the caller reported, before our own map.
        //
        // This command is intercepted in Rust and never reaches the Tcl
        // proc of the same name, so the roster the bridge writes into
        // `::slopdrop_channel_members` — which is what `[names]`,
        // `[name]` and every proc calling `chanlist $::channel` read —
        // is invisible from here. Headless that map is not merely stale
        // but permanently empty: nothing is filling it, because filling
        // it is what an IRC connection does. So a person typing the
        // plain `chanlist #coven` that this whole interception exists
        // to serve got back nothing, while the identical call from
        // inside a proc answered correctly.
        //
        // Same precedence rule as `apply_room_context`: the bridge is
        // the process actually sitting in that room right now, so its
        // roster wins when it names this channel. An unreported room,
        // or one for a different channel, falls through to the map
        // below and nothing changes for a slopdrop that still has its
        // own connection.
        let reported = request
            .room
            .channel
            .as_deref()
            .filter(|c| c.eq_ignore_ascii_case(channel))
            .map(|_| request.room.members.as_slice())
            .filter(|m| !m.is_empty());
        if let Some(nicks) = reported {
            let mut sorted: Vec<String> = nicks.to_vec();
            sorted.sort();
            let _ = request.response_tx.send(EvalResult {
                output: sorted.join(" "),
                is_error: false,
                commit_info: None,
            });
            return;
        }

        // Read from shared channel members. The map is keyed by the composite
        // "network:#channel" so we have to build the same key from the request.
        // Fall back to a bare channel-name lookup so callers that already
        // supplied a composite key (e.g. tests) keep working.
        let composite_key = format!("{}:{}", request.network, channel);
        match self.channel_members.read() {
            Ok(members) => {
                let lookup = members
                    .get(&composite_key)
                    .or_else(|| members.get(channel));
                if let Some(nicks) = lookup {
                    if nicks.is_empty() {
                        let _ = request.response_tx.send(EvalResult {
                            output: String::new(),
                            is_error: false,
                            commit_info: None,
                        });
                    } else {
                        let mut sorted: Vec<_> = nicks.iter().cloned().collect();
                        sorted.sort();
                        let _ = request.response_tx.send(EvalResult {
                            output: sorted.join(" "),
                            is_error: false,
                            commit_info: None,
                        });
                    }
                } else {
                    // Channel not found - return empty list
                    let _ = request.response_tx.send(EvalResult {
                        output: String::new(),
                        is_error: false,
                        commit_info: None,
                    });
                }
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: failed to read channel members: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }

    fn handle_stock_command(&self, request: EvalRequest) {
        let code = request.code.trim();

        // Special handling for stock::chart - needs to call TCL code with data
        if code.starts_with("stock::chart ") {
            let parts: Vec<&str> = code.split_whitespace().collect();
            if parts.len() < 2 {
                let _ = request.response_tx.send(EvalResult {
                    output: "error: Usage: stock::chart <symbol> [days] [interval]".to_string(),
                    is_error: true,
                    commit_info: None,
                });
                return;
            }

            let symbol = parts[1];
            let days = if parts.len() > 2 {
                parts[2].parse::<usize>().unwrap_or(7)
            } else {
                7
            };

            // Get optional interval (e.g., "1m", "5m", "1h", "1d")
            let interval_arg = if parts.len() > 3 {
                format!(" {}", parts[3])
            } else {
                String::new()
            };

            // Get historical data from Rust backend
            match crate::stock_commands::handle_stock_command(&format!("stock::history {} {}{}", symbol, days, interval_arg)) {
                Ok(history_data) => {
                    // Call TCL chart_from_data with the history
                    let tcl_code = format!("stock::chart_from_data {{{}}} {{{}}}", symbol, history_data);
                    match self.interp.interpreter().eval(tcl_code.as_str()) {
                        Ok(result) => {
                            let _ = request.response_tx.send(EvalResult {
                                output: result.to_string(),
                                is_error: false,
                                commit_info: None,
                            });
                        }
                        Err(e) => {
                            let _ = request.response_tx.send(EvalResult {
                                output: format!("error: Failed to generate chart: {:?}", e),
                                is_error: true,
                                commit_info: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ = request.response_tx.send(EvalResult {
                        output: format!("error: {}", e),
                        is_error: true,
                        commit_info: None,
                    });
                }
            }
            return;
        }

        // Call the stock command handler from stock_commands module
        match crate::stock_commands::handle_stock_command(code) {
            Ok(output) => {
                let _ = request.response_tx.send(EvalResult {
                    output,
                    is_error: false,
                    commit_info: None,
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }
}
