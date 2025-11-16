//! Frontend implementations
//!
//! This module contains implementations of various frontends that use the core TCL service.

#[cfg(feature = "frontend-irc")]
pub mod irc;

#[cfg(feature = "frontend-cli")]
pub mod cli;

#[cfg(feature = "frontend-tui")]
pub mod tui;

#[cfg(feature = "frontend-web")]
pub mod web;
