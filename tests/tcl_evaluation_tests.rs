use slopdrop::tcl_wrapper::SafeTclInterp;
use slopdrop::tcl_thread::TclThreadHandle;
use slopdrop::config::{SecurityConfig, TclConfig};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use tempfile::TempDir;
use std::path::PathBuf;

/// Helper to create a temporary state directory
fn create_temp_state() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state");
    (temp_dir, state_path)
}

#[test]
fn test_basic_eval() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let result = interp.eval("expr {1 + 1}");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "2");
}

#[test]
fn test_string_operations() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let result = interp.eval("string length \"hello\"").unwrap();
    assert_eq!(result.trim(), "5");

    let result = interp.eval("string toupper \"hello\"").unwrap();
    assert_eq!(result.trim(), "HELLO");

    let result = interp.eval("string tolower \"WORLD\"").unwrap();
    assert_eq!(result.trim(), "world");
}

#[test]
fn test_list_operations() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let result = interp.eval("llength {1 2 3 4 5}").unwrap();
    assert_eq!(result.trim(), "5");

    let result = interp.eval("lindex {a b c d e} 2").unwrap();
    assert_eq!(result.trim(), "c");

    let result = interp.eval("lappend mylist a b c; set mylist").unwrap();
    assert_eq!(result.trim(), "a b c");
}

#[test]
fn test_proc_creation_and_call() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // Create a proc
    let result = interp.eval("proc greet {name} { return \"Hello, $name!\" }");
    assert!(result.is_ok());

    // Call the proc
    let result = interp.eval("greet Alice").unwrap();
    assert_eq!(result.trim(), "Hello, Alice!");

    let result = interp.eval("greet Bob").unwrap();
    assert_eq!(result.trim(), "Hello, Bob!");
}

#[test]
fn test_dangerous_commands_blocked() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // exec should be blocked
    let result = interp.eval("exec ls");
    assert!(result.is_err());

    // open should be blocked
    let result = interp.eval("open /etc/passwd r");
    assert!(result.is_err());

    // file should be blocked
    let result = interp.eval("file exists /etc/passwd");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_timeout_handling() {
    let (_temp, state_path) = create_temp_state();

    // Create a TCL thread with a short timeout (500ms)
    let security_config = SecurityConfig {
        eval_timeout_ms: 500,
        privileged_users: vec![],
        memory_limit_mb: 256,
    };

    let tcl_config = TclConfig {
        state_path: state_path.clone(),
        state_repo: None,
        ssh_key: None,
        max_output_lines: 10,
    };

    // Create empty channel members
    let channel_members = Arc::new(RwLock::new(HashMap::new()));

    let mut tcl_thread = TclThreadHandle::spawn(
        tcl_config,
        security_config,
        channel_members,
    ).unwrap();

    // Give the thread time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // This should timeout (infinite loop)
    let result = tcl_thread.eval(
        "while {1} {}".to_string(),
        false,
        "testuser".to_string(),
        "testhost".to_string(),
        "#test".to_string(),
    ).await;

    assert!(result.is_ok()); // The method returns Ok with error message on timeout
    let eval_result = result.unwrap();
    assert!(eval_result.is_error);
    assert!(eval_result.output.contains("timeout") || eval_result.output.contains("timed out"));

    // Cleanup
    tcl_thread.shutdown();
}

#[test]
fn test_variable_persistence() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // Set a variable
    let _ = interp.eval("set counter 0");

    // Increment it
    let _ = interp.eval("incr counter");
    let result = interp.eval("set counter").unwrap();
    assert_eq!(result.trim(), "1");

    // Increment again
    let _ = interp.eval("incr counter");
    let result = interp.eval("set counter").unwrap();
    assert_eq!(result.trim(), "2");
}

#[test]
fn test_array_operations() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // Create an array
    let _ = interp.eval("set arr(key1) value1");
    let _ = interp.eval("set arr(key2) value2");

    // Get array values
    let result = interp.eval("set arr(key1)").unwrap();
    assert_eq!(result.trim(), "value1");

    // Get array names
    let result = interp.eval("array names arr").unwrap();
    assert!(result.contains("key1"));
    assert!(result.contains("key2"));

    // Get array size
    let result = interp.eval("array size arr").unwrap();
    assert_eq!(result.trim(), "2");
}

#[test]
fn test_foreach_loop() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        set result ""
        foreach item {a b c} {
            append result $item
        }
        set result
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "abc");
}

#[test]
fn test_for_loop() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        set sum 0
        for {set i 1} {$i <= 10} {incr i} {
            set sum [expr {$sum + $i}]
        }
        set sum
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "55"); // Sum of 1 to 10
}

#[test]
fn test_if_else() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        set x 10
        if {$x > 5} {
            set result "greater"
        } else {
            set result "lesser"
        }
        set result
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "greater");

    let code = r#"
        set x 3
        if {$x > 5} {
            set result "greater"
        } else {
            set result "lesser"
        }
        set result
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "lesser");
}

#[test]
fn test_switch_statement() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        set day "monday"
        switch $day {
            monday { set result "Start of week" }
            friday { set result "End of week" }
            default { set result "Midweek" }
        }
        set result
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "Start of week");
}

#[test]
fn test_nested_procs() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let _ = interp.eval("proc add {a b} { expr {$a + $b} }");
    let _ = interp.eval("proc multiply {a b} { expr {$a * $b} }");
    let _ = interp.eval("proc compute {x y} { multiply [add $x $y] 2 }");

    let result = interp.eval("compute 3 4").unwrap(); // (3+4) * 2 = 14
    assert_eq!(result.trim(), "14");
}

#[test]
fn test_error_handling() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // Division by zero
    let result = interp.eval("expr {1 / 0}");
    assert!(result.is_err());

    // Undefined variable
    let result = interp.eval("set nonexistent_var");
    assert!(result.is_err());

    // Invalid command
    let result = interp.eval("this_command_does_not_exist");
    assert!(result.is_err());
}

#[test]
fn test_catch_command() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        if {[catch {expr {1 / 0}} result]} {
            set result "error_caught"
        }
        set result
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "error_caught");
}

#[test]
fn test_return_values() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // Proc with return
    let _ = interp.eval("proc get_value {} { return 42 }");
    let result = interp.eval("get_value").unwrap();
    assert_eq!(result.trim(), "42");

    // Proc without explicit return (returns last expression)
    let _ = interp.eval("proc get_sum {a b} { expr {$a + $b} }");
    let result = interp.eval("get_sum 10 20").unwrap();
    assert_eq!(result.trim(), "30");
}

#[test]
fn test_global_variables() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        set globalvar "global_value"
        proc test_global {} {
            global globalvar
            return $globalvar
        }
        test_global
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "global_value");
}

#[test]
fn test_upvar_command() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        proc increment {varname} {
            upvar 1 $varname var
            incr var
        }
        set counter 5
        increment counter
        set counter
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "6");
}

#[test]
fn test_format_command() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let result = interp.eval("format \"Hello %s\" \"World\"").unwrap();
    assert_eq!(result.trim(), "Hello World");

    let result = interp.eval("format \"Number: %d\" 42").unwrap();
    assert_eq!(result.trim(), "Number: 42");
}

#[test]
fn test_regexp_matching() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let result = interp.eval("regexp {^[0-9]+$} \"12345\"").unwrap();
    assert_eq!(result.trim(), "1"); // true

    let result = interp.eval("regexp {^[0-9]+$} \"abc123\"").unwrap();
    assert_eq!(result.trim(), "0"); // false
}

#[test]
fn test_regsub_substitution() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = "regsub {world} \"hello world\" \"TCL\" result; set result";
    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "hello TCL");
}

#[test]
fn test_scan_command() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let code = r#"
        scan "John 25" "%s %d" name age
        list $name $age
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "John 25");
}

#[test]
fn test_join_and_split() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // Join
    let result = interp.eval("join {a b c} \",\"").unwrap();
    assert_eq!(result.trim(), "a,b,c");

    // Split
    let result = interp.eval("split \"a,b,c\" \",\"").unwrap();
    assert_eq!(result.trim(), "a b c");
}

#[test]
fn test_lsort() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let result = interp.eval("lsort {3 1 4 1 5 9 2 6}").unwrap();
    assert_eq!(result.trim(), "1 1 2 3 4 5 6 9");

    // Reverse sort
    let result = interp.eval("lsort -decreasing {3 1 4 1 5}").unwrap();
    assert_eq!(result.trim(), "5 4 3 1 1");
}

#[test]
fn test_lsearch() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    let result = interp.eval("lsearch {a b c d e} c").unwrap();
    assert_eq!(result.trim(), "2");

    let result = interp.eval("lsearch {a b c d e} z").unwrap();
    assert_eq!(result.trim(), "-1");
}

#[test]
fn test_dict_operations() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None).unwrap();

    // Create and query dict
    let code = r#"
        set mydict [dict create name Alice age 30]
        dict get $mydict name
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "Alice");

    // Dict size
    let code = r#"
        set mydict [dict create name Alice age 30 city NYC]
        dict size $mydict
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "3");
}
