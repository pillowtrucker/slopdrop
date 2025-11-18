# General timer infrastructure for scheduled/repeating tasks
# Available to all TCL code, not just timtom

namespace eval timers {
    # Internal state
    variable bucket "timers"
    variable counter 0

    # Schedule a timer
    # Usage: timers schedule <channel> <message> <delay_ms> ?repeat? ?interval_ms?
    #   channel: where to send the message (or nick for PM)
    #   message: text to send when timer fires
    #   delay_ms: initial delay in milliseconds
    #   repeat: number of times to repeat (default 1, -1 for infinite)
    #   interval_ms: delay between repeats (default same as delay_ms)
    # Returns: timer ID
    proc schedule {channel message delay_ms {repeat 1} {interval_ms 0}} {
        variable bucket
        variable counter
        incr counter
        set id "timer_$counter"

        if {$interval_ms == 0} {
            set interval_ms $delay_ms
        }

        set fire_time [expr {[clock milliseconds] + $delay_ms}]
        set timer_data [list $id $channel $message $fire_time $repeat $interval_ms]

        # Store in timers list
        set timers_key "active"
        if {[cache exists $bucket $timers_key]} {
            set timers [cache get $bucket $timers_key]
        } else {
            set timers [list]
        }
        lappend timers $timer_data
        cache put $bucket $timers_key $timers

        return $id
    }

    # Cancel a timer by ID
    # Usage: timers cancel <id>
    proc cancel {id} {
        variable bucket
        set timers_key "active"
        if {![cache exists $bucket $timers_key]} {
            return 0
        }
        set timers [cache get $bucket $timers_key]
        set new_timers [list]
        set found 0
        foreach timer $timers {
            if {[lindex $timer 0] ne $id} {
                lappend new_timers $timer
            } else {
                set found 1
            }
        }
        cache put $bucket $timers_key $new_timers
        return $found
    }

    # Cancel all timers matching a glob pattern
    # Usage: timers cancel_like <pattern>
    proc cancel_like {pattern} {
        variable bucket
        set timers_key "active"
        if {![cache exists $bucket $timers_key]} {
            return 0
        }
        set timers [cache get $bucket $timers_key]
        set new_timers [list]
        set count 0
        foreach timer $timers {
            if {![string match $pattern [lindex $timer 0]]} {
                lappend new_timers $timer
            } else {
                incr count
            }
        }
        cache put $bucket $timers_key $new_timers
        return $count
    }

    # Check for ready timers and return their messages
    # Called by Rust timer polling
    # Returns list of {channel message} pairs
    proc check {} {
        variable bucket
        set timers_key "active"
        if {![cache exists $bucket $timers_key]} {
            return [list]
        }

        set timers [cache get $bucket $timers_key]
        set now [clock milliseconds]
        set ready [list]
        set remaining [list]

        foreach timer $timers {
            lassign $timer id channel message fire_time repeat interval
            if {$now >= $fire_time} {
                # Timer is ready
                lappend ready [list $channel $message]
                # Check if it should repeat
                if {$repeat > 1 || $repeat == -1} {
                    set new_repeat [expr {$repeat == -1 ? -1 : $repeat - 1}]
                    set new_fire [expr {$now + $interval}]
                    lappend remaining [list $id $channel $message $new_fire $new_repeat $interval]
                }
            } else {
                lappend remaining $timer
            }
        }

        cache put $bucket $timers_key $remaining
        return $ready
    }

    # Get count of pending timers
    # Usage: timers count
    proc count {} {
        variable bucket
        set timers_key "active"
        if {![cache exists $bucket $timers_key]} {
            return 0
        }
        return [llength [cache get $bucket $timers_key]]
    }

    # List all pending timers
    # Usage: timers list
    proc list {} {
        variable bucket
        set timers_key "active"
        if {![cache exists $bucket $timers_key]} {
            return [list]
        }
        return [cache get $bucket $timers_key]
    }

    # Clear all timers
    # Usage: timers clear
    proc clear {} {
        variable bucket
        cache put $bucket "active" [list]
        return "All timers cleared"
    }

    # Export and create ensemble
    namespace export schedule cancel cancel_like check count list clear
    namespace ensemble create
}

# Convenience aliases at global scope
proc after_ms {delay_ms channel message} {
    timers schedule $channel $message $delay_ms
}

proc repeat_ms {interval_ms channel message {count -1}} {
    timers schedule $channel $message $interval_ms $count $interval_ms
}
