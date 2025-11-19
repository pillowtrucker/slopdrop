# Utility commands for common operations

proc lindex_random {list} {
    lindex $list [expr {int(rand() * [llength $list])}]
}

# Functional programming utilities

# map - apply a transform to each element of a list
# Usage: map {list} {varname {body}}
# Example: map {1 2 3} {x {expr {$x * 2}}}
proc map {list body} {
    set varname [lindex $body 0]
    set code [lindex $body 1]
    set result [list]
    foreach item $list {
        uplevel 1 [list set $varname $item]
        lappend result [uplevel 1 $code]
    }
    return $result
}

# lfilter - filter a list using a glob pattern
# Usage: lfilter pattern list
# Usage: lfilter -nocase pattern list
proc lfilter {args} {
    set nocase 0
    if {[lindex $args 0] eq "-nocase"} {
        set nocase 1
        set args [lrange $args 1 end]
    }

    if {[llength $args] != 2} {
        error "wrong # args: should be \"lfilter ?-nocase? pattern list\""
    }

    set pattern [lindex $args 0]
    set list [lindex $args 1]

    # Convert glob pattern to regexp
    set regexp [glob_to_regexp $pattern $nocase]

    lsearch -inline -all -regexp $list $regexp
}

# Helper: convert glob pattern to regexp
proc glob_to_regexp {pattern {nocase 0}} {
    # Escape regexp special chars, then convert glob wildcards
    set regexp $pattern
    # Escape all regexp metacharacters except * and ?
    set regexp [string map {
        "\\" "\\\\"
        "." "\\."
        "^" "\\^"
        "$" "\\$"
        "[" "\\["
        "]" "\\]"
        "(" "\\("
        ")" "\\)"
        "{" "\\{"
        "}" "\\}"
        "|" "\\|"
        "+" "\\+"
    } $regexp]
    # Convert glob wildcards to regexp
    set regexp [string map {
        "*" ".*"
        "?" "."
    } $regexp]

    if {$nocase} {
        return "(?i)$regexp"
    }
    return $regexp
}

# seq - generate a sequence of numbers
# Usage: seq start end ?step?
# Example: seq 1 10 -> {1 2 3 4 5 6 7 8 9 10}
proc seq {start end {step 1}} {
    set result [list]
    if {$step > 0} {
        for {set i $start} {$i <= $end} {incr i $step} {
            lappend result $i
        }
    } elseif {$step < 0} {
        for {set i $start} {$i >= $end} {incr i $step} {
            lappend result $i
        }
    } else {
        error "step cannot be 0"
    }
    return $result
}

# nlsplit - split string on newlines
proc nlsplit {str} {
    split $str "\n"
}

# second - get second element of a list
proc second {list} {
    lindex $list 1
}

# third - get third element of a list
proc third {list} {
    lindex $list 2
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

proc name {} {
    # Return a random nick from current channel
    set nicks [chanlist $::channel]
    if {[llength $nicks] == 0} {
        return ""
    }
    set idx [expr {int(rand() * [llength $nicks])}]
    lindex $nicks $idx
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

# Log commands - return log lines for channels/users
# Each log entry is a list: {timestamp nick mask message}
# The log is populated by the Rust side when messages arrive

# log - returns log lines for current channel
proc log {} {
    if {[info exists ::slopdrop_log_lines($::channel)]} {
        return $::slopdrop_log_lines($::channel)
    }
    return [list]
}

# log_for - returns log lines for a specific user
proc log_for {who} {
    set result [list]
    if {[info exists ::slopdrop_log_lines($::channel)]} {
        foreach entry $::slopdrop_log_lines($::channel) {
            # Entry format: {timestamp nick mask message}
            set nick [lindex $entry 1]
            if {[string equal -nocase $nick $who]} {
                lappend result $entry
            }
        }
    }
    return $result
}

# lastlog_text - get last N entries as plain text
proc lastlog_text {count} {
    set result [list]
    if {[info exists ::slopdrop_log_lines($::channel)]} {
        set entries $::slopdrop_log_lines($::channel)
        set start [expr {max(0, [llength $entries] - $count)}]
        foreach entry [lrange $entries $start end] {
            # Entry format: {timestamp nick mask message}
            lappend result [lindex $entry 3]
        }
    }
    return $result
}

# lgrep - grep through a list using a pattern
# Pattern format: "***:pattern" where *** is replaced with .*
proc lgrep {pattern list} {
    # Convert *** to .* for regexp
    set regexp [string map {"***" ".*"} $pattern]
    set result [list]
    foreach item $list {
        if {[regexp $regexp $item]} {
            lappend result $item
        }
    }
    return $result
}

# select - filter a list based on a condition
# Usage: select {list} {varname {expr}}
# Example: select {1 2 3 4 5} {x {expr {$x > 2}}}
proc select {list body} {
    set varname [lindex $body 0]
    set code [lindex $body 1]
    set result [list]
    foreach item $list {
        uplevel 1 [list set $varname $item]
        if {[uplevel 1 $code]} {
            lappend result $item
        }
    }
    return $result
}

# format_log_line - format a log entry for display
proc format_log_line {line} {
    if {$line eq ""} {
        return ""
    }
    set nick [lindex $line 1]
    set msg [lindex $line 3]
    return "<$nick> $msg"
}

# lastlink - get the last URL from recent channel history
proc lastlink {} {
    set msgs [lastlog_text 200]
    # Look for https first, then http
    set https_urls [lfilter {*https://*} $msgs]
    if {[llength $https_urls] > 0} {
        # Extract just the URL from the message
        set msg [last $https_urls]
        if {[regexp {https://[^\s]+} $msg url]} {
            return $url
        }
    }
    set http_urls [lfilter {*http://*} $msgs]
    if {[llength $http_urls] > 0} {
        set msg [last $http_urls]
        if {[regexp {https?://[^\s]+} $msg url]} {
            return $url
        }
    }
    return ""
}

# lastlog - get formatted last message from a user
proc lastlog {who} {
    set logline [last [log_for $who]]
    if {$logline eq ""} {
        return "No messages found for $who"
    }
    format_log_line $logline
}

# ^ - history lookup with optional nick and pattern filter
# Usage: ^ ?n? ?who? ?match?
#   n - how far back to look (default 1)
#   who - filter by nick (empty = all)
#   match - filter by pattern (empty = all)
proc ^ {{n 1} {who {}} {match {}}} {
    # Adjust n if looking at own messages or doing pattern search
    if {$who ne "" && [string toupper $who] eq [string toupper [nick]]} {
        set n [expr {$n + 1}]
    } elseif {$match ne "" && $who eq ""} {
        set n [expr {$n + 1}]
    }

    # Get filtered log entries
    if {$who eq ""} {
        set lines [lgrep "***:(?i)$match" [log]]
    } else {
        set lines [lgrep "***:(?i)$match" [log_for $who]]
    }

    # Return formatted entry
    set idx [expr {[llength $lines] - $n}]
    if {$idx < 0} {
        return ""
    }
    format_log_line [lindex $lines $idx]
}

# lastsaid - alias for ^ with nick
proc lastsaid {who {n 1}} {
    ^ $n $who
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
