use slopdrop::tcl_wrapper::SafeTclInterp;
use tempfile::TempDir;
use std::path::PathBuf;

/// Helper to create a temporary state directory
fn create_temp_state() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state");
    (temp_dir, state_path)
}

// =============================================================================
// Timer Tests
// =============================================================================

#[test]
fn test_timer_schedule_basic() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule a timer
    let result = interp.eval("timers schedule #test \"Hello world\" 1000").unwrap();
    assert!(result.starts_with("timer_"));

    // Check count
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "1");
}

#[test]
fn test_timer_schedule_multiple() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule multiple timers
    interp.eval("timers schedule #test \"Message 1\" 1000").unwrap();
    interp.eval("timers schedule #test \"Message 2\" 2000").unwrap();
    interp.eval("timers schedule #test \"Message 3\" 3000").unwrap();

    // Check count
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "3");
}

#[test]
fn test_timer_cancel() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule a timer
    let id = interp.eval("timers schedule #test \"Hello\" 1000").unwrap();
    let id = id.trim();

    // Cancel it
    let result = interp.eval(&format!("timers cancel {}", id)).unwrap();
    assert_eq!(result.trim(), "1"); // Found and cancelled

    // Check count
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "0");
}

#[test]
fn test_timer_cancel_nonexistent() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Try to cancel nonexistent timer
    let result = interp.eval("timers cancel timer_999").unwrap();
    assert_eq!(result.trim(), "0"); // Not found
}

#[test]
fn test_timer_cancel_like() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule multiple timers
    interp.eval("timers schedule #test \"Message 1\" 1000").unwrap();
    interp.eval("timers schedule #test \"Message 2\" 2000").unwrap();
    interp.eval("timers schedule #test \"Message 3\" 3000").unwrap();

    // Cancel all
    let result = interp.eval("timers cancel_like timer_*").unwrap();
    assert_eq!(result.trim(), "3");

    // Check count
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "0");
}

#[test]
fn test_timer_list() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule timers
    interp.eval("timers schedule #test \"Message 1\" 1000").unwrap();
    interp.eval("timers schedule #chan2 \"Message 2\" 2000").unwrap();

    // List them
    let result = interp.eval("timers pending").unwrap();
    assert!(result.contains("#test"));
    assert!(result.contains("#chan2"));
    assert!(result.contains("Message 1"));
    assert!(result.contains("Message 2"));
}

#[test]
fn test_timer_clear() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule timers
    interp.eval("timers schedule #test \"Message 1\" 1000").unwrap();
    interp.eval("timers schedule #test \"Message 2\" 2000").unwrap();

    // Clear all
    interp.eval("timers clear").unwrap();

    // Check count
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "0");
}

#[test]
fn test_timer_check_empty() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check with no timers
    let result = interp.eval("timers check").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_timer_with_repeat() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Schedule with repeat
    interp.eval("timers schedule #test \"Repeat\" 1000 5 1000").unwrap();

    // Should be 1 timer scheduled
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "1");
}

#[test]
fn test_timer_convenience_after_ms() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Use convenience function
    let result = interp.eval("after_ms 5000 #test \"Delayed message\"").unwrap();
    assert!(result.starts_with("timer_"));

    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "1");
}

#[test]
fn test_timer_convenience_repeat_ms() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Use convenience function
    interp.eval("repeat_ms 1000 #test \"Repeating\" 3").unwrap();

    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "1");
}

// =============================================================================
// Trigger Tests
// =============================================================================

#[test]
fn test_trigger_bind_join() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a handler
    interp.eval("proc my_join_handler {nick mask channel} { return \"Welcome $nick!\" }").unwrap();

    // Bind it
    let result = interp.eval("bind JOIN * my_join_handler").unwrap();
    assert!(result.contains("Bound"));

    // Check bindings
    let bindings = interp.eval("triggers list_bindings JOIN").unwrap();
    assert!(bindings.contains("my_join_handler"));
}

#[test]
fn test_trigger_bind_text() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a handler
    interp.eval("proc my_text_handler {nick mask channel text} { return \"\" }").unwrap();

    // Bind it
    let result = interp.eval("bind TEXT #mychan my_text_handler").unwrap();
    assert!(result.contains("Bound"));
}

#[test]
fn test_trigger_bind_invalid_event() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Try invalid event
    let result = interp.eval("bind INVALID * my_handler");
    assert!(result.is_err());
}

#[test]
fn test_trigger_unbind() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create and bind
    interp.eval("proc my_handler {nick mask channel} { return \"\" }").unwrap();
    interp.eval("bind JOIN * my_handler").unwrap();

    // Unbind
    let result = interp.eval("unbind JOIN * my_handler").unwrap();
    assert!(result.contains("Unbound"));

    // Check bindings
    let bindings = interp.eval("triggers list_bindings JOIN").unwrap();
    assert!(!bindings.contains("my_handler"));
}

#[test]
fn test_trigger_unbind_nonexistent() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Try to unbind nonexistent
    let result = interp.eval("unbind JOIN * nonexistent_handler").unwrap();
    assert!(result.contains("not found"));
}

#[test]
fn test_trigger_list_bindings_all() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create handlers
    interp.eval("proc join_handler {nick mask channel} { return \"\" }").unwrap();
    interp.eval("proc part_handler {nick mask channel} { return \"\" }").unwrap();

    // Bind them
    interp.eval("bind JOIN * join_handler").unwrap();
    interp.eval("bind PART * part_handler").unwrap();

    // List all
    let bindings = interp.eval("triggers list_bindings").unwrap();
    assert!(bindings.contains("JOIN"));
    assert!(bindings.contains("PART"));
    assert!(bindings.contains("join_handler"));
    assert!(bindings.contains("part_handler"));
}

#[test]
fn test_trigger_dispatch_join() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create handler that returns a message
    interp.eval("proc welcome {nick mask channel} { return \"Hello $nick!\" }").unwrap();
    interp.eval("bind JOIN * welcome").unwrap();

    // Dispatch
    let result = interp.eval("triggers dispatch JOIN testuser user@host #test").unwrap();
    assert!(result.contains("#test"));
    assert!(result.contains("Hello testuser!"));
}

#[test]
fn test_trigger_dispatch_text() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create handler
    interp.eval(r#"
        proc echo_handler {nick mask channel text} {
            if {[string match "*hello*" $text]} {
                return "Hello to you too!"
            }
            return ""
        }
    "#).unwrap();
    interp.eval("bind TEXT * echo_handler").unwrap();

    // Dispatch with matching text
    let result = interp.eval("triggers dispatch TEXT testuser user@host #test {hello world}").unwrap();
    assert!(result.contains("Hello to you too!"));

    // Dispatch without matching text
    let result = interp.eval("triggers dispatch TEXT testuser user@host #test {goodbye}").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_trigger_dispatch_channel_pattern() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Unbind the default timtom_welcome handler first
    interp.eval("unbind JOIN * timtom_welcome").unwrap();

    // Create handler for specific channel
    interp.eval("proc specific_handler {nick mask channel} { return \"Specific!\" }").unwrap();
    interp.eval("bind JOIN #specific specific_handler").unwrap();

    // Dispatch to matching channel
    let result = interp.eval("triggers dispatch JOIN testuser user@host #specific").unwrap();
    assert!(result.contains("Specific!"));

    // Dispatch to non-matching channel
    let result = interp.eval("triggers dispatch JOIN testuser user@host #other").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_trigger_dispatch_multiple_handlers() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create multiple handlers
    interp.eval("proc handler1 {nick mask channel} { return \"Handler1\" }").unwrap();
    interp.eval("proc handler2 {nick mask channel} { return \"Handler2\" }").unwrap();

    interp.eval("bind JOIN * handler1").unwrap();
    interp.eval("bind JOIN * handler2").unwrap();

    // Both should fire
    let result = interp.eval("triggers dispatch JOIN testuser user@host #test").unwrap();
    assert!(result.contains("Handler1"));
    assert!(result.contains("Handler2"));
}

#[test]
fn test_trigger_dispatch_no_bindings() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Dispatch with no bindings
    let result = interp.eval("triggers dispatch JOIN testuser user@host #test").unwrap();
    assert_eq!(result.trim(), "");
}

#[test]
fn test_trigger_dispatch_handler_error() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create handler that errors
    interp.eval("proc error_handler {nick mask channel} { error \"Intentional error\" }").unwrap();
    interp.eval("bind JOIN * error_handler").unwrap();

    // Dispatch - should return error message
    let result = interp.eval("triggers dispatch JOIN testuser user@host #test").unwrap();
    assert!(result.contains("Error in error_handler"));
}

#[test]
fn test_trigger_all_event_types() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test all supported event types can be bound
    let events = vec!["JOIN", "PART", "QUIT", "KICK", "NICK", "TEXT"];

    for event in events {
        let proc_name = format!("{}_handler", event.to_lowercase());
        interp.eval(&format!("proc {} {{args}} {{ return \"\" }}", proc_name)).unwrap();
        let result = interp.eval(&format!("bind {} * {}", event, proc_name));
        assert!(result.is_ok(), "Failed to bind {}", event);
    }
}

// =============================================================================
// TIMTOM Integration Tests
// =============================================================================

#[test]
fn test_timtom_uses_general_timers() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Set context
    interp.eval("set ::nick testuser").unwrap();
    interp.eval("set ::channel #test").unwrap();
    interp.eval("set ::mask user@host").unwrap();

    // Call stare which schedules timers
    let result = interp.eval("timtom stare").unwrap();
    assert!(result.contains("STARING"));

    // Check that timers were scheduled using general timer system
    let count = interp.eval("timers count").unwrap();
    assert_eq!(count.trim(), "1");
}

#[test]
fn test_timtom_welcome_trigger() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Check that timtom_welcome is registered
    let bindings = interp.eval("triggers list_bindings JOIN").unwrap();
    assert!(bindings.contains("timtom_welcome"));
}

#[test]
fn test_timtom_cache_integration() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Set context
    interp.eval("set ::nick testuser").unwrap();
    interp.eval("set ::channel #test").unwrap();

    // Check money
    let result = interp.eval("timtom money").unwrap();
    assert!(result.contains("testuser"));
    assert!(result.contains("$"));
}

// =============================================================================
// Utility Function Tests
// =============================================================================

#[test]
fn test_utils_lindex_random() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Test lindex_random returns valid element
    let result = interp.eval("lindex_random {a b c d e}").unwrap();
    let result = result.trim();
    assert!(["a", "b", "c", "d", "e"].contains(&result));
}

#[test]
fn test_utils_map() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("map {1 2 3} {x {expr {$x * 2}}}").unwrap();
    assert_eq!(result.trim(), "2 4 6");
}

#[test]
fn test_utils_select() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("select {1 2 3 4 5} {x {expr {$x > 2}}}").unwrap();
    assert_eq!(result.trim(), "3 4 5");
}

#[test]
fn test_utils_seq() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("seq 1 5").unwrap();
    assert_eq!(result.trim(), "1 2 3 4 5");

    // With step
    let result = interp.eval("seq 0 10 2").unwrap();
    assert_eq!(result.trim(), "0 2 4 6 8 10");
}

#[test]
fn test_utils_file_operations() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // file extension
    let result = interp.eval("file extension /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), ".txt");

    // file dirname
    let result = interp.eval("file dirname /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), "/path/to");

    // file tail
    let result = interp.eval("file tail /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), "file.txt");

    // file rootname
    let result = interp.eval("file rootname /path/to/file.txt").unwrap();
    assert_eq!(result.trim(), "/path/to/file");

    // file join
    let result = interp.eval("file join /path to file.txt").unwrap();
    assert_eq!(result.trim(), "/path/to/file.txt");
}

#[test]
fn test_utils_lfilter() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("lfilter {*test*} {alpha test123 beta testing gamma}").unwrap();
    assert!(result.contains("test123"));
    assert!(result.contains("testing"));
    assert!(!result.contains("alpha"));
}

#[test]
fn test_utils_nlsplit() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("nlsplit \"line1\nline2\nline3\"").unwrap();
    assert!(result.contains("line1"));
    assert!(result.contains("line2"));
    assert!(result.contains("line3"));
}

// =============================================================================
// Cache System Tests
// =============================================================================

#[test]
fn test_cache_put_get() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("cache put testbucket testkey testvalue").unwrap();
    let result = interp.eval("cache get testbucket testkey").unwrap();
    assert_eq!(result.trim(), "testvalue");
}

#[test]
fn test_cache_exists() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("cache put testbucket testkey testvalue").unwrap();

    let result = interp.eval("cache exists testbucket testkey").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("cache exists testbucket nonexistent").unwrap();
    assert_eq!(result.trim(), "0");
}

#[test]
fn test_cache_delete() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("cache put testbucket testkey testvalue").unwrap();
    interp.eval("cache delete testbucket testkey").unwrap();

    let result = interp.eval("cache exists testbucket testkey").unwrap();
    assert_eq!(result.trim(), "0");
}

#[test]
fn test_cache_keys() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("cache put testbucket key1 value1").unwrap();
    interp.eval("cache put testbucket key2 value2").unwrap();
    interp.eval("cache put testbucket key3 value3").unwrap();

    let result = interp.eval("cache keys testbucket").unwrap();
    assert!(result.contains("key1"));
    assert!(result.contains("key2"));
    assert!(result.contains("key3"));
}
