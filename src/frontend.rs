//! Frontend trait and common types for multi-frontend architecture

use anyhow::Result;
use async_trait::async_trait;

/// Trait that all frontends must implement
#[async_trait]
pub trait Frontend: Send + Sync {
    /// Get the name of this frontend
    fn name(&self) -> &str;

    /// Start the frontend
    async fn start(&mut self) -> Result<()>;

    /// Stop the frontend gracefully
    async fn stop(&mut self) -> Result<()>;

    /// Check if the frontend is running
    fn is_running(&self) -> bool;
}
