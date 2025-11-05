/// Smeggdrop TCL commands that need to be injected into the interpreter
/// These replicate functionality from the original bot

/// Returns the cache commands TCL code
pub fn cache_commands() -> &'static str {
    r#"
namespace eval cache {
  namespace eval buckets {
    proc import {bucket_name {as bucket}} {
      variable ::cache::buckets::$bucket_name
      if {![info exists ::cache::buckets::$bucket_name]} {
        array set ::cache::buckets::$bucket_name {}
      }
      uplevel [list upvar ::cache::buckets::$bucket_name $as]
    }
  }

  proc keys {bucket_name} {
    buckets::import $bucket_name
    array names bucket
  }

  proc exists {bucket_name key} {
    buckets::import $bucket_name
    info exists bucket($key)
  }

  proc get {bucket_name key} {
    buckets::import $bucket_name
    ensure_key_exists $bucket_name $key
    set bucket($key)
  }

  proc put {bucket_name key value} {
    buckets::import $bucket_name
    set bucket($key) $value
  }

  proc fetch {bucket_name key script} {
    if {[exists $bucket_name $key]} {
      get $bucket_name $key
    } else {
      set value [uplevel 1 $script]
      put $bucket_name $key $value
      set value
    }
  }

  proc delete {bucket_name key} {
    buckets::import $bucket_name
    ensure_key_exists $bucket_name $key
    unset bucket($key)
  }

  proc ensure_key_exists {bucket_name key} {
    if {![exists $bucket_name $key]} {
      error "bucket \"$bucket_name\" doesn't have key \"$key\""
    }
  }
}
"#
}

/// Returns utility commands TCL code
pub fn utility_commands() -> &'static str {
    r#"
# Helper procs for common operations

proc lindex_random {list} {
    lindex $list [expr {int(rand() * [llength $list])}]
}

proc pick args {
    # pick 1 {option1} 2 {option2} - weighted random choice
    set total 0
    foreach {weight _} $args {
        incr total $weight
    }
    set r [expr {rand() * $total}]
    set acc 0
    foreach {weight value} $args {
        set acc [expr {$acc + $weight}]
        if {$r < $acc} {
            return [uplevel 1 $value]
        }
    }
    return ""
}

proc ?? {list} {
    lindex_random $list
}

proc choose args {
    lindex_random $args
}

# String manipulation
proc upper {str} {
    string toupper $str
}

proc lower {str} {
    string tolower $str
}

# List operations
proc first {list} {
    lindex $list 0
}

proc last {list} {
    lindex $list end
}

proc rest {list} {
    lrange $list 1 end
}
"#
}

/// Returns the encoding commands TCL code
pub fn encoding_commands() -> &'static str {
    r#"
namespace eval encoding {
    # Base64 encoding (simple version)
    proc base64 {str} {
        binary encode base64 $str
    }

    proc unbase64 {str} {
        binary decode base64 $str
    }

    # URL encoding
    proc url {str} {
        set result ""
        foreach char [split $str ""] {
            scan $char %c code
            if {[string match {[a-zA-Z0-9_.~-]} $char]} {
                append result $char
            } else {
                append result [format %%%02X $code]
            }
        }
        return $result
    }
}
"#
}

/// Returns SHA1 hashing command
pub fn sha1_command() -> &'static str {
    r#"
proc sha1 {str} {
    # This is a placeholder - need to implement via Rust
    # For now, return a notice
    return "SHA1 not yet implemented in Rust version"
}
"#
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
