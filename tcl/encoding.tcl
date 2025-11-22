# Encoding command - wrapper around TCL's ::encoding
# Blocks system encoding modification for security

# First, rename the original encoding command to preserve it (only if not already done)
if {[llength [info commands _tcl_encoding_original]] == 0} {
    rename encoding _tcl_encoding_original
}

proc encoding args {
    # Block system encoding modification except for UTF-8
    # This allows proper Unicode handling while preventing security issues
    if {[string match s* [lindex $args 0]] && [llength $args] > 1} {
        set encoding_name [lindex $args 1]
        # Allow UTF-8 encoding (needed for proper HTTP response handling)
        if {[string tolower $encoding_name] eq "utf-8" ||
            [string tolower $encoding_name] eq "utf8"} {
            uplevel 1 [list _tcl_encoding_original {*}$args]
            return
        }
        error "can't modify system encoding (except to utf-8)"
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
