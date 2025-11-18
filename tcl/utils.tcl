# Utility commands for common operations

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

# IRC context commands - return info about current evaluation context
proc names {} {
    # Return list of nicks in current channel
    chanlist $::channel
}

proc hostmask {{who ""}} {
    # Return hostmask - for now just return the current mask
    # Full implementation would need getchanhost which requires IRC state
    return $::mask
}

# Meta namespace - info about evaluation context
namespace eval meta {
    proc uptime {} {
        # Return bot uptime in seconds
        if {[info exists ::slopdrop_start_time]} {
            expr {[clock seconds] - $::slopdrop_start_time}
        } else {
            return 0
        }
    }

    proc eval_count {} {
        if {[info exists ::eval_count]} {
            return $::eval_count
        } else {
            return 0
        }
    }

    proc line {} {
        if {[info exists ::line]} {
            return $::line
        } else {
            return ""
        }
    }

    namespace export uptime eval_count line
    namespace ensemble create
}

# Initialize start time if not set
if {![info exists ::slopdrop_start_time]} {
    set ::slopdrop_start_time [clock seconds]
}

# Log command - returns log lines for current channel
# This is a placeholder that returns empty list
# Full implementation would need log storage infrastructure
proc log {} {
    if {[info exists ::slopdrop_log_lines($::channel)]} {
        return $::slopdrop_log_lines($::channel)
    }
    return [list]
}

# Initialize log storage
if {![info exists ::slopdrop_log_lines]} {
    array set ::slopdrop_log_lines {}
}

# Safe file path operations (string-only, no filesystem access)
# These replace the blocked 'file' command for common path manipulation

proc file args {
    # Safe subset of TCL's file command - only string operations
    if {[llength $args] < 1} {
        error "wrong # args: should be \"file subcommand ?arg ...?\""
    }

    set subcmd [lindex $args 0]
    set rest [lrange $args 1 end]

    switch -- $subcmd {
        join {
            # Join path components
            if {[llength $rest] < 1} {
                error "wrong # args: should be \"file join name ?name ...?\""
            }
            set result ""
            foreach part $rest {
                if {$result eq "" || [string index $part 0] eq "/"} {
                    set result $part
                } elseif {[string index $result end] eq "/"} {
                    append result $part
                } else {
                    append result "/" $part
                }
            }
            return $result
        }
        extension {
            # Get file extension
            if {[llength $rest] != 1} {
                error "wrong # args: should be \"file extension name\""
            }
            set name [lindex $rest 0]
            set idx [string last "." $name]
            if {$idx == -1} {
                return ""
            }
            # Make sure the dot is after the last slash
            set slashidx [string last "/" $name]
            if {$slashidx > $idx} {
                return ""
            }
            return [string range $name $idx end]
        }
        rootname {
            # Get name without extension
            if {[llength $rest] != 1} {
                error "wrong # args: should be \"file rootname name\""
            }
            set name [lindex $rest 0]
            set idx [string last "." $name]
            if {$idx == -1} {
                return $name
            }
            # Make sure the dot is after the last slash
            set slashidx [string last "/" $name]
            if {$slashidx > $idx} {
                return $name
            }
            return [string range $name 0 [expr {$idx - 1}]]
        }
        dirname {
            # Get directory portion
            if {[llength $rest] != 1} {
                error "wrong # args: should be \"file dirname name\""
            }
            set name [lindex $rest 0]
            set idx [string last "/" $name]
            if {$idx == -1} {
                return "."
            } elseif {$idx == 0} {
                return "/"
            }
            return [string range $name 0 [expr {$idx - 1}]]
        }
        tail {
            # Get filename portion
            if {[llength $rest] != 1} {
                error "wrong # args: should be \"file tail name\""
            }
            set name [lindex $rest 0]
            set idx [string last "/" $name]
            if {$idx == -1} {
                return $name
            }
            return [string range $name [expr {$idx + 1}] end]
        }
        split {
            # Split path into components
            if {[llength $rest] != 1} {
                error "wrong # args: should be \"file split name\""
            }
            set name [lindex $rest 0]
            set parts [split $name "/"]
            # Handle absolute paths
            if {[string index $name 0] eq "/"} {
                set parts [lreplace $parts 0 0 "/"]
            }
            # Remove empty parts (from consecutive slashes)
            set result {}
            foreach part $parts {
                if {$part ne ""} {
                    lappend result $part
                }
            }
            return $result
        }
        default {
            error "bad option \"$subcmd\": must be dirname, extension, join, rootname, split, or tail"
        }
    }
}
