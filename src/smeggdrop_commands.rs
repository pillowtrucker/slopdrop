/// Smeggdrop TCL commands that need to be injected into the interpreter
/// These replicate functionality from the original bot
///
/// TCL scripts are stored in tcl/ directory and embedded at compile time

/// Returns the cache commands TCL code
pub fn cache_commands() -> &'static str {
    include_str!("../tcl/cache.tcl")
}

/// Returns utility commands TCL code
pub fn utility_commands() -> &'static str {
    include_str!("../tcl/utils.tcl")
}

/// Returns the encoding commands TCL code
pub fn encoding_commands() -> &'static str {
    include_str!("../tcl/encoding.tcl")
}

/// Returns SHA1 hashing command
pub fn sha1_command() -> &'static str {
    include_str!("../tcl/sha1.tcl")
}

/// Initialize all smeggdrop commands in the interpreter
pub fn inject_commands(interp: &tcl::Interpreter) -> anyhow::Result<()> {
    use tracing::debug;

    debug!("Injecting smeggdrop commands");

    // Inject cache commands
    interp.eval(cache_commands())
        .map_err(|e| anyhow::anyhow!("Failed to inject cache commands: {:?}", e))?;

    // Inject utility commands
    interp.eval(utility_commands())
        .map_err(|e| anyhow::anyhow!("Failed to inject utility commands: {:?}", e))?;

    // Inject encoding commands
    interp.eval(encoding_commands())
        .map_err(|e| anyhow::anyhow!("Failed to inject encoding commands: {:?}", e))?;

    // Inject SHA1 command (placeholder)
    interp.eval(sha1_command())
        .map_err(|e| anyhow::anyhow!("Failed to inject SHA1 command: {:?}", e))?;

    debug!("Smeggdrop commands injected successfully");

    Ok(())
}
