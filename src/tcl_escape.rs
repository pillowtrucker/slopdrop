//! Escape strings for safe interpolation into Tcl command source.
//!
//! The trigger dispatch path used to wrap arguments in `{...}` and splice
//! user-controlled text straight into the command string. Any `}` (or `{`
//! that left things unbalanced) terminated the brace early, splitting one
//! argument into several — which at best produced a "wrong # args" error
//! from the handler, and at worst let the trailing fragment be evaluated
//! as Tcl outside of any quoting.

/// Escape a string so that it can appear as a single argument in a Tcl
/// command. The output is backslash-escaped, which works regardless of
/// the surrounding parsing context. Tcl's word parser turns each `\<c>`
/// back into the literal `<c>`, with `\n`/`\r`/`\t` decoded to the
/// corresponding control characters.
pub fn tcl_escape_arg(s: &str) -> String {
    if s.is_empty() {
        return "{}".to_string();
    }
    let mut result = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            ' ' => result.push_str("\\ "),
            '\t' => result.push_str("\\t"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            ';' => result.push_str("\\;"),
            '$' => result.push_str("\\$"),
            '[' => result.push_str("\\["),
            ']' => result.push_str("\\]"),
            '{' => result.push_str("\\{"),
            '}' => result.push_str("\\}"),
            '"' => result.push_str("\\\""),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcl_wrapper::SafeTclInterp;
    use std::path::PathBuf;

    fn temp_state(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("slopdrop_tcl_escape_{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn roundtrip(s: &str) -> String {
        let interp = SafeTclInterp::new(5000, &temp_state(&format!("rt_{}", s.len())), None, None, 1000).unwrap();
        interp.eval(r#"proc _echo {x} {return $x}"#).unwrap();
        let cmd = format!("_echo {}", tcl_escape_arg(s));
        interp.eval(&cmd).unwrap().trim_end_matches('\n').to_string()
    }

    #[test]
    fn empty_string() {
        assert_eq!(tcl_escape_arg(""), "{}");
    }

    #[test]
    fn plain_text() {
        assert_eq!(tcl_escape_arg("hello"), "hello");
    }

    #[test]
    fn closing_brace_does_not_terminate_arg() {
        // The original report: "i should fix the ] and } thing" caused
        // dispatch to misparse and call the handler with too many args.
        assert_eq!(roundtrip("i should fix the ] and } thing"),
                   "i should fix the ] and } thing");
    }

    #[test]
    fn unbalanced_open_brace() {
        assert_eq!(roundtrip("foo { bar"), "foo { bar");
    }

    #[test]
    fn variable_substitution_is_inert() {
        // $::env(HOME) must not actually expand.
        assert_eq!(roundtrip("$::env(HOME)"), "$::env(HOME)");
    }

    #[test]
    fn command_substitution_is_inert() {
        // [info commands] must not run as a Tcl command.
        assert_eq!(roundtrip("[info commands]"), "[info commands]");
    }

    #[test]
    fn semicolon_does_not_break_command() {
        // ; would otherwise end the current command and start a new one.
        assert_eq!(roundtrip("; puts gotcha"), "; puts gotcha");
    }

    #[test]
    fn backslash_passes_through() {
        assert_eq!(roundtrip("a\\b"), "a\\b");
        assert_eq!(roundtrip("\\n is two chars"), "\\n is two chars");
    }

    #[test]
    fn newline_is_preserved() {
        assert_eq!(roundtrip("line1\nline2"), "line1\nline2");
    }

    #[test]
    fn whitespace_keeps_arg_together() {
        // Without escaping the spaces, the value would split into multiple args.
        let interp = SafeTclInterp::new(5000, &temp_state("ws"), None, None, 1000).unwrap();
        interp.eval(r#"proc _two {a b} {return "$a|$b"}"#).unwrap();
        // Pass a single value containing spaces; should arrive as one arg
        // and the second positional parameter should be missing.
        let cmd = format!("_two {} after", tcl_escape_arg("hello world space"));
        let result = interp.eval(&cmd).unwrap();
        assert_eq!(result.trim_end_matches('\n'), "hello world space|after");
    }

    #[test]
    fn injection_attempt_cannot_execute_code() {
        // Adversarial input designed to break out of brace-quoting and run
        // arbitrary Tcl. Without escaping this would have set ::pwned=1.
        let interp = SafeTclInterp::new(5000, &temp_state("inj"), None, None, 1000).unwrap();
        interp.eval(r#"proc _sink {x} {return $x}"#).unwrap();
        interp.eval("set ::pwned 0").unwrap();
        let payload = "} ; set ::pwned 1 ; {";
        let cmd = format!("_sink {}", tcl_escape_arg(payload));
        let echoed = interp.eval(&cmd).unwrap();
        assert_eq!(echoed.trim_end_matches('\n'), payload);
        let pwned = interp.eval("set ::pwned").unwrap();
        assert_eq!(pwned.trim_end_matches('\n'), "0");
    }
}
