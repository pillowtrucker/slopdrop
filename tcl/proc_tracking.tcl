# Procedure and variable modification tracking for slopdrop
# This intercepts the `proc` command and uses traces for variables

# Initialize global tracking variables
if {![info exists ::slopdrop_modified_procs]} {
    set ::slopdrop_modified_procs [list]
}
if {![info exists ::slopdrop_modified_vars]} {
    set ::slopdrop_modified_vars [list]
}
if {![info exists ::slopdrop_traced_vars]} {
    set ::slopdrop_traced_vars [list]
}

# Rename the built-in proc command
rename proc ::slopdrop::_original_proc

# Create wrapper that tracks proc definitions
# Must use ::slopdrop::_original_proc since we just renamed proc
::slopdrop::_original_proc proc {name args body} {
    # Get the caller's namespace to create proc in correct scope
    set caller_ns [uplevel 1 {namespace current}]

    # If caller is in a namespace, prepend it to the proc name if name isn't already qualified
    if {$caller_ns ne "::" && ![string match "::*" $name]} {
        set qualified_name "${caller_ns}::${name}"
    } else {
        set qualified_name $name
    }

    # Call original proc command in the caller's namespace using uplevel
    # Only track if the proc creation succeeds
    if {[catch {uplevel 1 [list ::slopdrop::_original_proc $name $args $body]} error]} {
        # Proc creation failed, propagate the error without tracking
        return -code error $error
    }

    # Track the fully qualified proc name (only reached if creation succeeded)
    global slopdrop_modified_procs
    if {[lsearch -exact $slopdrop_modified_procs $qualified_name] == -1} {
        lappend slopdrop_modified_procs $qualified_name
    }
}

# Helper proc to get and clear the modified procs list
::slopdrop::_original_proc ::slopdrop::get_modified_procs {} {
    global slopdrop_modified_procs

    # Filter out any invalid proc names before returning
    # This provides defense-in-depth in case invalid names got into the list somehow
    set validated [list]
    foreach procname $slopdrop_modified_procs {
        # Verify it's actually a valid procedure by testing info args
        if {![catch {info args $procname}]} {
            lappend validated $procname
        }
    }

    set slopdrop_modified_procs [list]
    return $validated
}

# Helper proc to mark all existing procs as modified
# Useful for migration/recovery after bot restart
::slopdrop::_original_proc ::slopdrop::mark_all_procs_modified {} {
    global slopdrop_modified_procs
    set slopdrop_modified_procs [list]

    # Get all procs and validate each one
    foreach procname [info procs] {
        # Only filter: verify it's actually a valid procedure by testing info args
        # If info args fails, it's not a real procedure (could be trigger data)
        if {[catch {info args $procname}]} {
            continue
        }

        lappend slopdrop_modified_procs $procname
    }

    return [llength $slopdrop_modified_procs]
}

# ====================
# Variable Tracking
# ====================

# Trace callback for variable writes
::slopdrop::_original_proc ::slopdrop::var_write_trace {varname index op} {
    global slopdrop_modified_vars

    # Debug: log when trace fires
    puts stderr "TRACE FIRED: var=$varname index=$index op=$op"

    # Add to modified list if not already there
    if {[lsearch -exact $slopdrop_modified_vars $varname] == -1} {
        lappend slopdrop_modified_vars $varname
        puts stderr "TRACE: Added $varname to modified list, count=[llength $slopdrop_modified_vars]"
    } else {
        puts stderr "TRACE: $varname already in modified list"
    }
}

# Helper to add trace to a variable
::slopdrop::_original_proc ::slopdrop::add_var_trace {varname} {
    global slopdrop_traced_vars
    if {[lsearch -exact $slopdrop_traced_vars $varname] == -1} {
        # Add write trace
        puts stderr "ADD_TRACE: Adding trace to variable '$varname' (::$varname)"
        trace add variable ::$varname write ::slopdrop::var_write_trace
        lappend slopdrop_traced_vars $varname
        puts stderr "ADD_TRACE: Successfully added trace, traced_vars count=[llength $slopdrop_traced_vars]"
    }
}

# Initialize traces for all existing global vars
::slopdrop::_original_proc ::slopdrop::init_var_traces {} {
    foreach varname [info globals] {
        # Skip internal tracking vars
        if {[string match "slopdrop_*" $varname]} {
            continue
        }
        ::slopdrop::add_var_trace $varname
    }
}

# Periodically update traces for new vars (called after each eval)
::slopdrop::_original_proc ::slopdrop::update_var_traces {} {
    set count 0
    set skipped 0
    foreach varname [info globals] {
        # Skip internal tracking vars
        if {[string match "slopdrop_*" $varname]} {
            incr skipped
            continue
        }
        ::slopdrop::add_var_trace $varname
        incr count
    }
    puts stderr "UPDATE_VAR_TRACES: Processed $count vars, skipped $skipped"
}

# Get and clear the modified vars list
::slopdrop::_original_proc ::slopdrop::get_modified_vars {} {
    global slopdrop_modified_vars

    # Debug logging
    puts stderr "GET_MODIFIED_VARS called, list has [llength $slopdrop_modified_vars] items: $slopdrop_modified_vars"

    # Don't filter - just return the list as-is
    # The trace only fires for valid variables, so we can trust the list
    set result $slopdrop_modified_vars
    set slopdrop_modified_vars [list]

    puts stderr "GET_MODIFIED_VARS returning [llength $result] items, cleared list"

    return $result
}

# Mark all existing vars as modified (for migration)
::slopdrop::_original_proc ::slopdrop::mark_all_vars_modified {} {
    global slopdrop_modified_vars
    set slopdrop_modified_vars [info globals]
    return [llength $slopdrop_modified_vars]
}

# Initialize traces for existing variables
::slopdrop::init_var_traces
