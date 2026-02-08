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
// Trigger disable_for / enable_for
// =============================================================================

#[test]
fn test_trigger_disable_for_all() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("triggers disable_for libera #test").unwrap();
    assert!(result.contains("Disabled"));
    assert!(result.contains("all"));
    assert!(result.contains("libera:#test"));
}

#[test]
fn test_trigger_disable_for_specific_proc() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("triggers disable_for libera #test ::linkresolver::on_text").unwrap();
    assert!(result.contains("Disabled"));
    assert!(result.contains("::linkresolver::on_text"));
}

#[test]
fn test_trigger_disable_already_disabled() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("triggers disable_for libera #test").unwrap();
    let result = interp.eval("triggers disable_for libera #test").unwrap();
    assert!(result.contains("Already disabled"));
}

#[test]
fn test_trigger_enable_for() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("triggers disable_for libera #test").unwrap();
    let result = interp.eval("triggers enable_for libera #test").unwrap();
    assert!(result.contains("Enabled"));
}

#[test]
fn test_trigger_enable_not_disabled() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("triggers enable_for libera #test").unwrap();
    assert!(result.contains("No disabled triggers"));
}

#[test]
fn test_trigger_enable_wrong_proc() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("triggers disable_for libera #test ::linkresolver::on_text").unwrap();
    let result = interp.eval("triggers enable_for libera #test ::other_proc").unwrap();
    assert!(result.contains("Not disabled"));
}

// =============================================================================
// Trigger is_disabled checks
// =============================================================================

#[test]
fn test_trigger_is_disabled_exact_match() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("triggers disable_for libera #test ::linkresolver::on_text").unwrap();

    // Exact match should be disabled
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #test").unwrap();
    assert_eq!(result.trim(), "1");

    // Different channel should not be disabled
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #other").unwrap();
    assert_eq!(result.trim(), "0");

    // Different network should not be disabled
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text oftc #test").unwrap();
    assert_eq!(result.trim(), "0");

    // Different proc should not be disabled
    let result = interp.eval("triggers is_disabled ::other_proc libera #test").unwrap();
    assert_eq!(result.trim(), "0");
}

#[test]
fn test_trigger_is_disabled_all_keyword() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Disable all triggers for libera:#test
    interp.eval("triggers disable_for libera #test").unwrap();

    // Any proc should be disabled on that network+channel
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #test").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("triggers is_disabled ::any_other_proc libera #test").unwrap();
    assert_eq!(result.trim(), "1");

    // But not on different network/channel
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text oftc #test").unwrap();
    assert_eq!(result.trim(), "0");
}

#[test]
fn test_trigger_is_disabled_network_wildcard() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Disable for all channels on libera
    interp.eval("triggers disable_for libera * ::linkresolver::on_text").unwrap();

    // Should be disabled on any channel on libera
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #test").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #other").unwrap();
    assert_eq!(result.trim(), "1");

    // But not on another network
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text oftc #test").unwrap();
    assert_eq!(result.trim(), "0");
}

#[test]
fn test_trigger_is_disabled_channel_wildcard() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Disable for #test on all networks
    interp.eval("triggers disable_for * #test ::linkresolver::on_text").unwrap();

    // Should be disabled on #test on any network
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #test").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("triggers is_disabled ::linkresolver::on_text oftc #test").unwrap();
    assert_eq!(result.trim(), "1");

    // But not on a different channel
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #other").unwrap();
    assert_eq!(result.trim(), "0");
}

#[test]
fn test_trigger_is_disabled_global_wildcard() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Disable globally
    interp.eval("triggers disable_for * * ::linkresolver::on_text").unwrap();

    // Should be disabled everywhere
    let result = interp.eval("triggers is_disabled ::linkresolver::on_text libera #test").unwrap();
    assert_eq!(result.trim(), "1");

    let result = interp.eval("triggers is_disabled ::linkresolver::on_text oftc #other").unwrap();
    assert_eq!(result.trim(), "1");

    // But different proc should not be disabled
    let result = interp.eval("triggers is_disabled ::other_proc libera #test").unwrap();
    assert_eq!(result.trim(), "0");
}

// =============================================================================
// Trigger status
// =============================================================================

#[test]
fn test_trigger_status_empty() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    let result = interp.eval("triggers status").unwrap();
    assert!(result.contains("No triggers are disabled"));
}

#[test]
fn test_trigger_status_shows_disabled() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval("triggers disable_for libera #test ::linkresolver::on_text").unwrap();

    let result = interp.eval("triggers status").unwrap();
    assert!(result.contains("libera:#test"));
    assert!(result.contains("::linkresolver::on_text"));
}

// =============================================================================
// Trigger dispatch with network parameter
// =============================================================================

#[test]
fn test_trigger_dispatch_with_network() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a simple trigger handler
    interp.eval(r#"
        proc test_greeter {nick mask channel} {
            return "Hello $nick!"
        }
    "#).unwrap();

    // Bind the trigger
    interp.eval("triggers bind JOIN * test_greeter").unwrap();

    // Dispatch with network - should fire
    let result = interp.eval("triggers dispatch JOIN libera testuser user@host #test").unwrap();
    assert!(result.contains("Hello testuser!"));
}

#[test]
fn test_trigger_dispatch_respects_disable() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a simple trigger handler
    interp.eval(r#"
        proc test_greeter {nick mask channel} {
            return "Hello $nick!"
        }
    "#).unwrap();

    // Bind the trigger
    interp.eval("triggers bind JOIN * test_greeter").unwrap();

    // Disable for libera
    interp.eval("triggers disable_for libera * test_greeter").unwrap();

    // Dispatch on libera - should NOT fire (disabled)
    let result = interp.eval("triggers dispatch JOIN libera testuser user@host #test").unwrap();
    assert!(!result.contains("Hello testuser!"));

    // Dispatch on oftc - should fire (not disabled)
    let result = interp.eval("triggers dispatch JOIN oftc testuser user@host #test").unwrap();
    assert!(result.contains("Hello testuser!"));
}

#[test]
fn test_trigger_dispatch_disable_specific_channel() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create trigger handler
    interp.eval(r#"
        proc test_greeter {nick mask channel} {
            return "Hi $nick in $channel!"
        }
    "#).unwrap();

    interp.eval("triggers bind JOIN * test_greeter").unwrap();

    // Disable only for libera:#quiet
    interp.eval("triggers disable_for libera #quiet test_greeter").unwrap();

    // Should NOT fire on libera:#quiet
    let result = interp.eval("triggers dispatch JOIN libera testuser user@host #quiet").unwrap();
    assert!(!result.contains("Hi testuser"));

    // Should fire on libera:#active
    let result = interp.eval("triggers dispatch JOIN libera testuser user@host #active").unwrap();
    assert!(result.contains("Hi testuser in #active!"));
}

#[test]
fn test_trigger_disable_then_enable_restores() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    interp.eval(r#"
        proc test_greeter {nick mask channel} {
            return "Hello $nick!"
        }
    "#).unwrap();

    interp.eval("triggers bind JOIN * test_greeter").unwrap();

    // Disable
    interp.eval("triggers disable_for libera #test test_greeter").unwrap();

    // Verify disabled
    let result = interp.eval("triggers dispatch JOIN libera testuser user@host #test").unwrap();
    assert!(!result.contains("Hello testuser!"));

    // Re-enable
    interp.eval("triggers enable_for libera #test test_greeter").unwrap();

    // Should fire again
    let result = interp.eval("triggers dispatch JOIN libera testuser user@host #test").unwrap();
    assert!(result.contains("Hello testuser!"));
}

#[test]
fn test_trigger_dispatch_text_event_with_network() {
    let (_temp, state_path) = create_temp_state();
    let interp = SafeTclInterp::new(5000, &state_path, None, None, 1000).unwrap();

    // Create a TEXT event handler
    interp.eval(r#"
        proc test_echo {nick mask channel text} {
            if {[string match "!echo *" $text]} {
                return [string range $text 6 end]
            }
            return ""
        }
    "#).unwrap();

    interp.eval("triggers bind TEXT * test_echo").unwrap();

    // Should fire
    let result = interp.eval(r#"triggers dispatch TEXT libera testuser user@host #test "!echo hello""#).unwrap();
    assert!(result.contains("hello"));

    // Disable on libera
    interp.eval("triggers disable_for libera * test_echo").unwrap();

    // Should NOT fire on libera
    let result = interp.eval(r#"triggers dispatch TEXT libera testuser user@host #test "!echo hello""#).unwrap();
    assert!(!result.contains("hello") || result.trim().is_empty() || result.trim() == "{}");
}
