# Trigger/event binding system
# Similar to eggdrop's bind command

namespace eval triggers {
    # Storage for bindings: event_type -> list of {pattern proc_name}
    # Event types: JOIN, PART, QUIT, KICK, NICK, TEXT
    variable bindings
    array set bindings {}

    # Bind a proc to an event
    # Usage: triggers bind <event> <pattern> <proc>
    #   event: JOIN, PART, QUIT, KICK, NICK, TEXT
    #   pattern: channel pattern (e.g., "#channel" or "*" for all)
    #   proc: proc name to call
    #
    # For JOIN/PART: proc is called with: nick mask channel
    # For QUIT: proc is called with: nick mask message
    # For KICK: proc is called with: nick kicker channel reason
    # For NICK: proc is called with: old_nick new_nick mask
    # For TEXT: proc is called with: nick mask channel text
    proc bind {event pattern proc_name} {
        variable bindings

        # Normalize event type
        set event [string toupper $event]

        # Validate event type
        if {$event ni {JOIN PART QUIT KICK NICK TEXT}} {
            error "Unknown event type '$event'. Valid types: JOIN, PART, QUIT, KICK, NICK, TEXT"
        }

        # Initialize list if not exists
        if {![info exists bindings($event)]} {
            set bindings($event) [list]
        }

        # Add binding
        lappend bindings($event) [list $pattern $proc_name]
        return "Bound $proc_name to $event $pattern"
    }

    # Unbind a proc from an event
    # Usage: triggers unbind <event> <pattern> <proc>
    proc unbind {event pattern proc_name} {
        variable bindings

        set event [string toupper $event]

        if {![info exists bindings($event)]} {
            return "No bindings for $event"
        }

        set new_list [list]
        set found 0
        foreach binding $bindings($event) {
            if {[lindex $binding 0] eq $pattern && [lindex $binding 1] eq $proc_name} {
                set found 1
            } else {
                lappend new_list $binding
            }
        }

        if {$found} {
            set bindings($event) $new_list
            return "Unbound $proc_name from $event $pattern"
        } else {
            return "Binding not found"
        }
    }

    # List all bindings
    proc list_bindings {{event ""}} {
        variable bindings

        if {$event ne ""} {
            set event [string toupper $event]
            if {[info exists bindings($event)]} {
                return $bindings($event)
            } else {
                return [list]
            }
        }

        # Return all bindings
        set result [list]
        foreach {evt bindlist} [array get bindings] {
            foreach binding $bindlist {
                lappend result [list $evt [lindex $binding 0] [lindex $binding 1]]
            }
        }
        return $result
    }

    # Dispatch an event to registered handlers
    # Called by Rust when an event occurs
    # Returns list of {channel message} pairs for responses
    proc dispatch {event args} {
        variable bindings

        set event [string toupper $event]

        if {![info exists bindings($event)]} {
            return [list]
        }

        set results [list]

        # Determine channel for pattern matching
        switch $event {
            JOIN - PART - KICK - TEXT {
                # args: nick mask channel [text/reason]
                set channel [lindex $args 2]
            }
            QUIT - NICK {
                # No channel for these events
                set channel "*"
            }
        }

        foreach binding $bindings($event) {
            set pattern [lindex $binding 0]
            set proc_name [lindex $binding 1]

            # Check if pattern matches
            if {$pattern eq "*" || [string match -nocase $pattern $channel]} {
                # Call the proc
                if {[catch {
                    set response [uplevel #0 [list $proc_name {*}$args]]
                    if {$response ne ""} {
                        # Return response to the channel for relevant events
                        switch $event {
                            JOIN - PART - KICK - TEXT {
                                lappend results [list $channel $response]
                            }
                        }
                    }
                } err]} {
                    # Log error but continue processing other bindings
                    lappend results [list $channel "Error in $proc_name: $err"]
                }
            }
        }

        return $results
    }

    # Export commands
    namespace export bind unbind list_bindings dispatch
    namespace ensemble create
}

# Convenience aliases at global scope
proc bind {event pattern proc_name} {
    triggers bind $event $pattern $proc_name
}

proc unbind {event pattern proc_name} {
    triggers unbind $event $pattern $proc_name
}
