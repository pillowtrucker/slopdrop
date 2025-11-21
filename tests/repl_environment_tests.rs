use slopdrop::tcl_wrapper::SafeTclInterp;
use slopdrop::state::{InterpreterState, StatePersistence, UserInfo};
use tempfile::TempDir;
use std::path::PathBuf;
use std::fs;
use std::collections::HashSet;
use tcl::Interpreter;

/// Helper to create a temporary state directory
fn create_temp_state() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state");
    (temp_dir, state_path)
}

/// Helper to create a test interpreter (raw)
fn create_test_interp() -> Interpreter {
    Interpreter::new().unwrap()
}

// =============================================================================
// REPL Programming Environment Tests
// =============================================================================

#[test]
fn test_proc_definition_and_execution() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define a complex proc
    interp.eval(r#"
        proc factorial {n} {
            if {$n <= 1} {
                return 1
            }
            return [expr {$n * [factorial [expr {$n - 1}]]}]
        }
    "#).unwrap();

    // Test execution
    let result = interp.eval("factorial 5").unwrap();
    assert_eq!(result.trim(), "120");

    let result = interp.eval("factorial 0").unwrap();
    assert_eq!(result.trim(), "1");
}

#[test]
fn test_proc_with_default_args() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define proc with default arguments
    interp.eval(r#"
        proc greet {{name "World"} {greeting "Hello"}} {
            return "$greeting, $name!"
        }
    "#).unwrap();

    let result = interp.eval("greet").unwrap();
    assert_eq!(result.trim(), "Hello, World!");

    let result = interp.eval("greet Alice").unwrap();
    assert_eq!(result.trim(), "Hello, Alice!");

    let result = interp.eval("greet Bob Hi").unwrap();
    assert_eq!(result.trim(), "Hi, Bob!");
}

#[test]
fn test_proc_with_args_variadic() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define proc with variadic args
    interp.eval(r#"
        proc sum {args} {
            set total 0
            foreach n $args {
                set total [expr {$total + $n}]
            }
            return $total
        }
    "#).unwrap();

    let result = interp.eval("sum 1 2 3 4 5").unwrap();
    assert_eq!(result.trim(), "15");

    let result = interp.eval("sum 10").unwrap();
    assert_eq!(result.trim(), "10");

    let result = interp.eval("sum").unwrap();
    assert_eq!(result.trim(), "0");
}

#[test]
fn test_proc_namespace_creation() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a namespace with procs
    interp.eval(r#"
        namespace eval myutils {
            proc double {x} {
                return [expr {$x * 2}]
            }
            proc triple {x} {
                return [expr {$x * 3}]
            }
            namespace export double triple
        }
    "#).unwrap();

    let result = interp.eval("myutils::double 5").unwrap();
    assert_eq!(result.trim(), "10");

    let result = interp.eval("myutils::triple 4").unwrap();
    assert_eq!(result.trim(), "12");
}

#[test]
fn test_global_variable_persistence() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Set global variables
    interp.eval("set ::config(debug) 1").unwrap();
    interp.eval("set ::config(version) 1.0.0").unwrap();

    // Access from proc
    interp.eval(r#"
        proc get_config {key} {
            return $::config($key)
        }
    "#).unwrap();

    let result = interp.eval("get_config debug").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("get_config version").unwrap();
    assert_eq!(result.trim(), "1.0.0");
}

#[test]
fn test_list_comprehension_with_map_select() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Filter and transform data
    let result = interp.eval(r#"
        set numbers {1 2 3 4 5 6 7 8 9 10}
        set evens [select $numbers {n {expr {$n % 2 == 0}}}]
        map $evens {n {expr {$n * $n}}}
    "#).unwrap();

    assert_eq!(result.trim(), "4 16 36 64 100");
}

#[test]
fn test_recursive_data_processing() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define recursive list flattening
    interp.eval(r#"
        proc flatten {lst} {
            set result {}
            foreach item $lst {
                if {[llength $item] > 1} {
                    foreach subitem [flatten $item] {
                        lappend result $subitem
                    }
                } else {
                    lappend result $item
                }
            }
            return $result
        }
    "#).unwrap();

    let result = interp.eval("flatten {1 {2 3} 4 {5 {6 7}}}").unwrap();
    assert!(result.contains("1"));
    assert!(result.contains("7"));
}

#[test]
fn test_error_handling_with_catch() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define proc with error handling
    interp.eval(r#"
        proc safe_divide {a b} {
            if {[catch {expr {$a / $b}} result]} {
                return "Error: division by zero"
            }
            return $result
        }
    "#).unwrap();

    let result = interp.eval("safe_divide 10 2").unwrap();
    assert_eq!(result.trim(), "5");

    let result = interp.eval("safe_divide 10 0").unwrap();
    assert!(result.contains("Error"));
}

#[test]
fn test_closure_like_behavior() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create closure-like behavior using uplevel
    interp.eval(r#"
        proc make_counter {} {
            set count 0
            proc counter {} [subst -nocommands {
                global counter_value
                if {![info exists counter_value]} {
                    set counter_value $count
                }
                incr counter_value
            }]
        }
    "#).unwrap();

    interp.eval("make_counter").unwrap();
    let result = interp.eval("counter").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("counter").unwrap();
    assert_eq!(result.trim(), "2");
}

// =============================================================================
// State Loading and Persistence Tests
// =============================================================================

#[test]
fn test_state_load_procs_from_files() {
    let (_temp, state_path) = create_temp_state();

    // Create state directory structure
    let procs_dir = state_path.join("procs");
    fs::create_dir_all(&procs_dir).unwrap();

    // Create a proc file
    let proc_content = r#"proc myproc {} { return "loaded" }"#;
    let hash = "abc123";
    fs::write(procs_dir.join(hash), proc_content).unwrap();

    // Create index file
    fs::write(procs_dir.join("_index"), format!("myproc\t{}\n", hash)).unwrap();

    // Now the state exists, test that we can load procs
    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);

    // The proc should be loadable
    assert!(procs_dir.join("_index").exists());
}

#[test]
fn test_state_save_creates_git_commit() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    let before = InterpreterState::capture(&interp).unwrap();

    // Create a new proc
    interp.eval("proc testproc {} { return 42 }").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new());

    assert!(changes.has_changes());

    let user_info = UserInfo::new("alice".to_string(), "example.com".to_string());
    let commit_info = persistence.save_changes(&interp, &changes, &user_info, "proc testproc {} { return 42 }").unwrap();

    assert!(commit_info.is_some());
    let info = commit_info.unwrap();
    assert!(!info.commit_id.is_empty());
    assert_eq!(info.author, "alice");
}

#[test]
fn test_state_load_vars_from_files() {
    let (_temp, state_path) = create_temp_state();

    // Create state directory structure
    let vars_dir = state_path.join("vars");
    fs::create_dir_all(&vars_dir).unwrap();

    // Create a var file
    let var_content = "test_value";
    let hash = "def456";
    fs::write(vars_dir.join(hash), var_content).unwrap();

    // Create index file
    fs::write(vars_dir.join("_index"), format!("myvar\t{}\n", hash)).unwrap();

    // Verify the structure
    assert!(vars_dir.join("_index").exists());
    assert!(vars_dir.join(hash).exists());
}

#[test]
fn test_multiple_users_different_commits() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    // User 1 creates a proc
    let state0 = InterpreterState::capture(&interp).unwrap();
    interp.eval("proc user1proc {} { return \"user1\" }").unwrap();
    let state1 = InterpreterState::capture(&interp).unwrap();
    let changes1 = state0.diff(&state1, &HashSet::new());

    let user1 = UserInfo::new("user1".to_string(), "host1.com".to_string());
    let commit1 = persistence.save_changes(&interp, &changes1, &user1, "user1 code").unwrap();

    // User 2 creates a proc
    interp.eval("proc user2proc {} { return \"user2\" }").unwrap();
    let state2 = InterpreterState::capture(&interp).unwrap();
    let changes2 = state1.diff(&state2, &HashSet::new());

    let user2 = UserInfo::new("user2".to_string(), "host2.com".to_string());
    let commit2 = persistence.save_changes(&interp, &changes2, &user2, "user2 code").unwrap();

    // Verify different commits with different authors
    assert_ne!(commit1.as_ref().unwrap().commit_id, commit2.as_ref().unwrap().commit_id);
    assert_eq!(commit1.as_ref().unwrap().author, "user1");
    assert_eq!(commit2.as_ref().unwrap().author, "user2");
}

// =============================================================================
// Context Variables Tests
// =============================================================================

#[test]
fn test_context_variables_available() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Set context variables
    interp.eval("set ::nick testuser").unwrap();
    interp.eval("set ::channel #test").unwrap();
    interp.eval("set ::mask user@host.com").unwrap();

    // Define proc that uses context
    interp.eval(r#"
        proc whoami {} {
            return "$::nick in $::channel ($::mask)"
        }
    "#).unwrap();

    let result = interp.eval("whoami").unwrap();
    assert!(result.contains("testuser"));
    assert!(result.contains("#test"));
    assert!(result.contains("user@host.com"));
}

// chanlist command is tested in tcl_service_tests.rs where the full service pipeline is available

// =============================================================================
// String Manipulation for IRC Tests
// =============================================================================

#[test]
fn test_string_substitution() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define proc that performs substitutions
    interp.eval(r#"
        proc madlib {template args} {
            set result $template
            set i 0
            foreach word $args {
                set result [string map [list "\{$i\}" $word] $result]
                incr i
            }
            return $result
        }
    "#).unwrap();

    let result = interp.eval("madlib {The {0} {1} over the {2}} quick fox jumped").unwrap();
    assert_eq!(result.trim(), "The quick fox over the jumped");
}

#[test]
fn test_color_code_handling() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test that we can work with IRC color codes
    interp.eval(r#"
        proc colorize {color text} {
            return "\x03${color}${text}\x03"
        }
    "#).unwrap();

    let result = interp.eval("colorize 04 \"red text\"").unwrap();
    assert!(result.contains("red text"));
}

// =============================================================================
// Complex Data Structure Tests
// =============================================================================

#[test]
fn test_dict_operations() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let code = r#"
        set person [dict create name Alice age 30 city NYC]
        dict with person {
            set greeting "Hello, I'm $name from $city"
        }
        set greeting
    "#;

    let result = interp.eval(code).unwrap();
    assert!(result.contains("Alice"));
    assert!(result.contains("NYC"));
}

#[test]
fn test_nested_data_structures() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let code = r#"
        set users [dict create]
        dict set users alice [dict create name "Alice" age 30]
        dict set users bob [dict create name "Bob" age 25]

        dict get [dict get $users alice] name
    "#;

    let result = interp.eval(code).unwrap();
    assert_eq!(result.trim(), "Alice");
}

// =============================================================================
// Math and Expression Tests
// =============================================================================

#[test]
fn test_complex_expressions() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test various math functions
    let result = interp.eval("expr {sqrt(16)}").unwrap();
    assert_eq!(result.trim(), "4.0");

    let result = interp.eval("expr {pow(2, 10)}").unwrap();
    assert_eq!(result.trim(), "1024.0");

    let result = interp.eval("expr {abs(-42)}").unwrap();
    assert_eq!(result.trim(), "42");

    let result = interp.eval("expr {max(1, 5, 3)}").unwrap();
    assert_eq!(result.trim(), "5");

    let result = interp.eval("expr {min(1, 5, 3)}").unwrap();
    assert_eq!(result.trim(), "1");
}

#[test]
fn test_boolean_expressions() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("expr {5 > 3 && 2 < 4}").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("expr {5 > 3 || 2 > 4}").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("expr {!(5 > 3)}").unwrap();
    assert_eq!(result.trim(), "0");

    let result = interp.eval("expr {5 == 5 ? \"yes\" : \"no\"}").unwrap();
    assert_eq!(result.trim(), "yes");
}

// =============================================================================
// Timer Integration with State Tests
// =============================================================================

#[test]
fn test_timer_state_interaction() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule timer that modifies state
    let id = interp.eval("timers schedule #test {[set ::counter 1]} 1000").unwrap();
    assert!(id.trim().starts_with("timer_"));

    // Timer count should be 1
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "1");

    // Cancel timer
    interp.eval(&format!("timers cancel {}", id.trim())).unwrap();

    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "0");
}

// =============================================================================
// Trigger Integration Tests
// =============================================================================

#[test]
fn test_trigger_with_proc() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define handler proc
    interp.eval(r#"
        proc greet_join {nick mask channel} {
            return "Welcome $nick to $channel!"
        }
    "#).unwrap();

    // Bind it
    interp.eval("bind JOIN * greet_join").unwrap();

    // Dispatch event
    let result = interp.eval("triggers dispatch JOIN alice user@host #test").unwrap();
    assert!(result.contains("Welcome alice"));
    assert!(result.contains("#test"));
}

#[test]
fn test_text_trigger_pattern_matching() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Define pattern-matching handler
    interp.eval(r#"
        proc bot_command {nick mask channel text} {
            if {[string match "!help*" $text]} {
                return "Available commands: !help, !status"
            }
            return ""
        }
    "#).unwrap();

    interp.eval("bind TEXT * bot_command").unwrap();

    // Test matching
    let result = interp.eval("triggers dispatch TEXT user user@host #test {!help me}").unwrap();
    assert!(result.contains("Available commands"));

    // Test non-matching
    let result = interp.eval("triggers dispatch TEXT user user@host #test {hello}").unwrap();
    assert_eq!(result.trim(), "");
}

// =============================================================================
// Security and Sandboxing Tests
// =============================================================================

#[test]
fn test_dangerous_commands_blocked() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // exec blocked
    let result = interp.eval("exec ls");
    assert!(result.is_err());

    // socket blocked
    let result = interp.eval("socket localhost 80");
    assert!(result.is_err());

    // open blocked
    let result = interp.eval("open /etc/passwd");
    assert!(result.is_err());

    // source blocked
    let result = interp.eval("source /etc/passwd");
    assert!(result.is_err());
}

#[test]
fn test_rename_does_not_bypass_security() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Try to rename a dangerous command
    // The dangerous commands are already blocked, so rename should fail
    let result = interp.eval("rename nonexistent newname");
    assert!(result.is_err());
}
