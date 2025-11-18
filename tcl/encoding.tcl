# Encoding command - wrapper around TCL's ::encoding
# Blocks system encoding modification for security

proc encoding args {
    # Block system encoding modification
    if {[string match s* [lindex $args 0]] && [llength $args] > 1} {
        error "can't modify system encoding"
    }
    # Call TCL's built-in encoding command
    uplevel [concat ::encoding $args]
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
