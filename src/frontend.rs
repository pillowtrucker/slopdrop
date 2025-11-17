//! Frontend trait and common types for multi-frontend architecture

use anyhow::Result;
use async_trait::async_trait;

/// Trait that all frontends must implement
#[async_trait]
pub trait Frontend: Send + Sync {
    /// Get the name of this frontend
    /// NOTE: Currently unused but part of the trait interface for future use (e.g., logging, status)
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Start the frontend
    async fn start(&mut self) -> Result<()>;

    /// Stop the frontend gracefully
    /// NOTE: Implemented by all frontends but not yet wired to signal handling (TODO)
    #[allow(dead_code)]
    async fn stop(&mut self) -> Result<()>;

    /// Check if the frontend is running
    /// NOTE: Currently unused but part of the trait interface for status checks
    #[allow(dead_code)]
    fn is_running(&self) -> bool;
}
