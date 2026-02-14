# Trigger/event binding system
# Similar to eggdrop's bind command
# Supports per-network and per-channel toggling

namespace eval triggers {
    # Storage for bindings: event_type -> list of {pattern proc_name}
    # Event types: JOIN, PART, QUIT, KICK, NICK, TEXT
    variable bindings
    array set bindings {}

    # Storage for disabled triggers: "network:channel" -> list of proc_names
    # Special values: "*" matches any network/channel, "all" disables all triggers
    variable disabled
    array set disabled {}

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

    # Disable a trigger proc for a specific network/channel combination
    # Usage: triggers disable_for <network|*> <channel|*> [proc_name|all]
    #   network: network name or "*" for all networks
    #   channel: channel name or "*" for all channels
    #   proc_name: proc to disable, or "all" to disable all triggers (default: "all")
    proc disable_for {network channel {proc_name "all"}} {
        variable disabled

        set key "${network}:${channel}"

        if {![info exists disabled($key)]} {
            set disabled($key) [list]
        }

        # Check if already disabled
        if {$proc_name in $disabled($key)} {
            return "Already disabled: $proc_name on $key"
        }

        lappend disabled($key) $proc_name
        return "Disabled $proc_name on $key"
    }

    # Enable (re-enable) a trigger proc for a specific network/channel combination
    # Usage: triggers enable_for <network|*> <channel|*> [proc_name|all]
    #   Removes the disable rule. Use proc_name "all" to remove the blanket disable.
    proc enable_for {network channel {proc_name "all"}} {
        variable disabled

        set key "${network}:${channel}"

        if {![info exists disabled($key)]} {
            return "No disabled triggers for $key"
        }

        set new_list [list]
        set found 0
        foreach p $disabled($key) {
            if {$p eq $proc_name} {
                set found 1
            } else {
                lappend new_list $p
            }
        }

        if {$found} {
            if {[llength $new_list] == 0} {
                unset disabled($key)
            } else {
                set disabled($key) $new_list
            }
            return "Enabled $proc_name on $key"
        } else {
            return "Not disabled: $proc_name on $key"
        }
    }

    # Show current disable status
    proc status {} {
        variable disabled

        if {[array size disabled] == 0} {
            return "No triggers are disabled"
        }

        set result [list]
        foreach {key procs} [array get disabled] {
            foreach p $procs {
                lappend result "$key -> $p"
            }
        }
        return [join $result "\n"]
    }

    # Check if a proc is disabled for a given network/channel
    proc is_disabled {proc_name network channel} {
        variable disabled

        # Check exact match: "network:channel"
        set key "${network}:${channel}"
        if {[info exists disabled($key)]} {
            if {"all" in $disabled($key) || $proc_name in $disabled($key)} {
                return 1
            }
        }

        # Check network wildcard: "network:*"
        set key "${network}:*"
        if {[info exists disabled($key)]} {
            if {"all" in $disabled($key) || $proc_name in $disabled($key)} {
                return 1
            }
        }

        # Check channel wildcard: "*:channel"
        set key "*:${channel}"
        if {[info exists disabled($key)]} {
            if {"all" in $disabled($key) || $proc_name in $disabled($key)} {
                return 1
            }
        }

        # Check global wildcard: "*:*"
        if {[info exists disabled(*:*)]} {
            if {"all" in $disabled(*:*) || $proc_name in $disabled(*:*)} {
                return 1
            }
        }

        return 0
    }

    # Dispatch an event to registered handlers
    # Called by Rust when an event occurs
    # dispatch <event> <network> <args...>
    # Returns list of {channel message} pairs for responses
    proc dispatch {event network args} {
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
                # Check if this trigger is disabled for this network/channel
                if {[is_disabled $proc_name $network $channel]} {
                    continue
                }

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
    namespace export bind unbind list_bindings dispatch disable_for enable_for status is_disabled
    namespace ensemble create
}

# Convenience aliases at global scope
proc bind {event pattern proc_name} {
    triggers bind $event $pattern $proc_name
}

proc unbind {event pattern proc_name} {
    triggers unbind $event $pattern $proc_name
}
