/// TCL code that implements HTTP commands with rate limiting
/// This uses TCL's built-in http package (safe within our timeout protection)
///
/// TCL script is stored in tcl/http.tcl and embedded at compile time

pub fn http_commands() -> &'static str {
    include_str!("../tcl/http.tcl")
}
