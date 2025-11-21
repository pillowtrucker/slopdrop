# Procedure modification tracking for slopdrop
# This intercepts the `proc` command to track when procedures are defined/modified

# Initialize global tracking variable
if {![info exists ::slopdrop_modified_procs]} {
    set ::slopdrop_modified_procs [list]
}

# Rename the built-in proc command
rename proc ::slopdrop::_original_proc

# Create wrapper that tracks proc definitions
proc proc {name args body} {
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
proc ::slopdrop::get_modified_procs {} {
    global slopdrop_modified_procs
    set result $slopdrop_modified_procs
    set slopdrop_modified_procs [list]
    return $result
}
