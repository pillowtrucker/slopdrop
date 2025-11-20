# Encoding command - wrapper around TCL's ::encoding
# Blocks system encoding modification for security

# First, rename the original encoding command to preserve it (only if not already done)
if {[llength [info commands _tcl_encoding_original]] == 0} {
    rename encoding _tcl_encoding_original
}

proc encoding args {
    # Block system encoding modification
    if {[string match s* [lindex $args 0]] && [llength $args] > 1} {
        error "can't modify system encoding"
    }
    # Call TCL's built-in encoding command (renamed above)
    uplevel 1 [list _tcl_encoding_original {*}$args]
}

# Additional encoding utilities as separate commands
proc base64 {str} {
    binary encode base64 $str
}

proc unbase64 {str} {
    binary decode base64 $str
}

proc url_encode {str} {
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
