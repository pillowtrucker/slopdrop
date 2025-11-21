# Procedure modification tracking for slopdrop
# This intercepts the `proc` command to track when procedures are defined/modified

# Initialize global tracking variable
if {![info exists ::slopdrop_modified_procs]} {
    set ::slopdrop_modified_procs [list]
}

# Rename the built-in proc command
rename proc ::slopdrop::_original_proc

# Create wrapper that tracks proc definitions
# Must use ::slopdrop::_original_proc since we just renamed proc
::slopdrop::_original_proc proc {name args body} {
    # Call original proc command
    ::slopdrop::_original_proc $name $args $body

    # Track that this proc was modified
    # Use dict to avoid duplicates efficiently
    global slopdrop_modified_procs
    if {[lsearch -exact $slopdrop_modified_procs $name] == -1} {
        lappend slopdrop_modified_procs $name
    }
}

# Helper proc to get and clear the modified procs list
::slopdrop::_original_proc ::slopdrop::get_modified_procs {} {
    global slopdrop_modified_procs
    set result $slopdrop_modified_procs
    set slopdrop_modified_procs [list]
    return $result
}

# Helper proc to mark all existing procs as modified
# Useful for migration/recovery after bot restart
::slopdrop::_original_proc ::slopdrop::mark_all_procs_modified {} {
    global slopdrop_modified_procs
    set slopdrop_modified_procs [info procs]
    return [llength $slopdrop_modified_procs]
}
