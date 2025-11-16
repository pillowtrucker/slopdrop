# Encoding commands
namespace eval encoding {
    # Base64 encoding (simple version)
    proc base64 {str} {
        binary encode base64 $str
    }

    proc unbase64 {str} {
        binary decode base64 $str
    }

    # URL encoding
    proc url {str} {
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
}
