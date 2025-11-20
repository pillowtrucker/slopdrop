// Library interface for integration tests

pub mod config;
pub mod file_watcher;
pub mod hostmask;
pub mod http_commands;
pub mod http_tcl_commands;
pub mod irc_client;
pub mod irc_formatting;
pub mod smeggdrop_commands;
pub mod state;
pub mod stock_commands;
pub mod tcl_plugin;
pub mod tcl_thread;
pub mod tcl_wrapper;
pub mod types;
pub mod validator;

// Multi-frontend architecture
pub mod frontend;
pub mod tcl_service;

// Frontend implementations
pub mod frontends;
