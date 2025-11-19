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

/// Context for a TCL evaluation request
#[derive(Debug, Clone)]
pub struct EvalContext {
    /// User identifier (nick, username, session ID, etc.)
    pub user: String,
    /// Host/origin (hostname, IP, "local", etc.)
    pub host: String,
    /// Optional channel/room identifier
    pub channel: Option<String>,
    /// Whether the user has admin privileges
    pub is_admin: bool,
}

impl EvalContext {
    pub fn new(user: String, host: String) -> Self {
        Self {
            user,
            host,
            channel: None,
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
        let tcl_thread = TclThreadHandle::spawn(
            tcl_config.clone(),
            security_config.clone(),
            channel_members,
        )?;

        Ok(Self {
            tcl_thread,
            security_config,
            tcl_config,
            output_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Evaluate TCL code
    pub async fn eval(&mut self, code: &str, ctx: EvalContext) -> Result<EvalResponse> {
        let channel = ctx.channel.clone().unwrap_or_else(|| "default".to_string());

        // Evaluate the code
        let result = self.tcl_thread.eval(
            code.to_string(),
            ctx.is_admin,
            ctx.user.clone(),
            ctx.host.clone(),
            channel.clone(),
        ).await?;

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
        let channel = ctx.channel.clone().unwrap_or_else(|| "default".to_string());
        let cache_key = format!("{}:{}", channel, ctx.user);

        let mut cache = self.output_cache.write()
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
                files_changed: 0,  // Not available from git history
                insertions: 0,
                deletions: 0,
                changes_summary: String::new(),  // Not available from git history
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

        Ok(format!("Rolled back to commit {}. TCL thread restarted with new state.", &commit_hash[..8]))
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
        self.security_config.privileged_users.iter()
            .any(|pattern| crate::hostmask::matches_hostmask(hostmask, pattern))
    }

    /// Shutdown the service gracefully
    /// NOTE: Used by frontends in their stop() methods during graceful shutdown
    #[allow(dead_code)]
    pub fn shutdown(&mut self) {
        self.tcl_thread.shutdown();
    }
}
