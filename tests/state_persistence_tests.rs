use slopdrop::state::{InterpreterState, StatePersistence, StateChanges, UserInfo};
use slopdrop::smeggdrop_commands;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use tcl::Interpreter;

/// Helper to create a temporary state directory
fn create_temp_state() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state");
    (temp_dir, state_path)
}

/// Helper to create a test interpreter
fn create_test_interp() -> Interpreter {
    Interpreter::new().unwrap()
}

#[test]
fn test_state_persistence_initialization() {
    let (_temp, state_path) = create_temp_state();

    let persistence = StatePersistence::with_repo(
        state_path.clone(),
        None,
        None,
    );

    persistence.ensure_initialized().unwrap();

    // Should create directories
    assert!(state_path.join("procs").exists());
    assert!(state_path.join("vars").exists());
    assert!(state_path.join("procs/_index").exists());
    assert!(state_path.join("vars/_index").exists());
}

#[test]
fn test_state_persistence_with_git() {
    let (_temp, state_path) = create_temp_state();

    let persistence = StatePersistence::with_repo(
        state_path.clone(),
        None,
        None,
    );

    persistence.ensure_initialized().unwrap();

    // Should initialize git repo
    assert!(state_path.join(".git").exists());
}

#[test]
fn test_capture_state() {
    let interp = create_test_interp();
    let state = InterpreterState::capture(&interp).unwrap();

    // TCL interpreter has built-in procs, so we just verify capture works
    // Procs will include things like: clock, open, close, etc.
    assert!(state.procs.len() > 0);
    // May or may not have vars depending on TCL version
}

#[test]
fn test_capture_state_with_procs() {
    let interp = create_test_interp();

    // Create some procs
    interp.eval("proc greet {name} { return \"Hello, $name!\" }").unwrap();
    interp.eval("proc add {a b} { expr {$a + $b} }").unwrap();

    let state = InterpreterState::capture(&interp).unwrap();

    // Check our procs are present (there will be built-in procs too)
    assert!(state.procs.contains("greet"));
    assert!(state.procs.contains("add"));
}

#[test]
fn test_capture_state_with_vars() {
    let interp = create_test_interp();

    // Set some variables
    interp.eval("set testvar1 \"value1\"").unwrap();
    interp.eval("set testvar2 42").unwrap();

    let state = InterpreterState::capture(&interp).unwrap();

    // Check our vars are present (there may be built-in vars too)
    assert!(state.vars.contains("testvar1"));
    assert!(state.vars.contains("testvar2"));
}

#[test]
fn test_state_diff_new_procs() {
    let interp = create_test_interp();

    let before = InterpreterState::capture(&interp).unwrap();

    interp.eval("proc test {} { return \"test\" }").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert!(changes.has_changes());
    assert_eq!(changes.new_procs.len(), 1);
    assert_eq!(changes.new_procs[0], "test");
    assert!(changes.deleted_procs.is_empty());
    assert!(changes.new_vars.is_empty());
    assert!(changes.deleted_vars.is_empty());
}

#[test]
fn test_state_diff_deleted_procs() {
    let interp = create_test_interp();

    interp.eval("proc test {} { return \"test\" }").unwrap();
    let before = InterpreterState::capture(&interp).unwrap();

    interp.eval("rename test {}").unwrap();
    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert!(changes.has_changes());
    assert_eq!(changes.deleted_procs.len(), 1);
    assert_eq!(changes.deleted_procs[0], "test");
    assert!(changes.new_procs.is_empty());
}

#[test]
fn test_state_diff_new_vars() {
    let interp = create_test_interp();

    let before = InterpreterState::capture(&interp).unwrap();

    interp.eval("set newvar \"value\"").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert!(changes.has_changes());
    assert_eq!(changes.new_vars.len(), 1);
    assert_eq!(changes.new_vars[0], "newvar");
    assert!(changes.deleted_vars.is_empty());
}

#[test]
fn test_state_diff_deleted_vars() {
    let interp = create_test_interp();

    interp.eval("set testvar \"value\"").unwrap();
    let before = InterpreterState::capture(&interp).unwrap();

    interp.eval("unset testvar").unwrap();
    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert!(changes.has_changes());
    assert_eq!(changes.deleted_vars.len(), 1);
    assert_eq!(changes.deleted_vars[0], "testvar");
    assert!(changes.new_vars.is_empty());
}

#[test]
fn test_state_diff_modified_proc() {
    let interp = create_test_interp();

    interp.eval("proc test {} { return \"v1\" }").unwrap();
    let before = InterpreterState::capture(&interp).unwrap();

    // Redefine the proc
    interp.eval("proc test {} { return \"v2\" }").unwrap();
    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    // NOTE: Redefining a proc doesn't change the proc list, so there are no changes
    // detected by the state capture. This is expected behavior because we only
    // track proc names, not their definitions. To detect modifications, we would
    // need to hash proc bodies, which is not currently implemented.
    assert!(!changes.has_changes());
}

#[test]
fn test_state_diff_no_changes() {
    let interp = create_test_interp();

    interp.eval("set var1 \"value\"").unwrap();
    let before = InterpreterState::capture(&interp).unwrap();
    let after = InterpreterState::capture(&interp).unwrap();

    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert!(!changes.has_changes());
}

#[test]
fn test_save_and_load_proc() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    // Capture initial state before creating any procs
    let before = InterpreterState::capture(&interp).unwrap();

    // Create and save a proc
    interp.eval("proc greet {name} { return \"Hello, $name!\" }").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    // Should only have the greet proc as new
    assert_eq!(changes.new_procs.len(), 1);
    assert_eq!(changes.new_procs[0], "greet");

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    persistence.save_changes(&interp, &changes, &user_info, "test code").unwrap();

    // Verify proc was indexed (check the _index file)
    let index_file = state_path.join("procs/_index");
    assert!(index_file.exists());
    let index_content = fs::read_to_string(index_file).unwrap();
    assert!(index_content.contains("greet"));
}

#[test]
fn test_save_and_load_var() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    // Capture initial state
    let before = InterpreterState::capture(&interp).unwrap();

    // Create and save a variable
    interp.eval("set testvar \"test value\"").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    // Should only have testvar as new
    assert!(changes.new_vars.contains(&"testvar".to_string()));

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    persistence.save_changes(&interp, &changes, &user_info, "test code").unwrap();

    // Verify var was indexed (check the _index file)
    let index_file = state_path.join("vars/_index");
    assert!(index_file.exists());
    let index_content = fs::read_to_string(index_file).unwrap();
    assert!(index_content.contains("testvar"));
}

#[test]
fn test_git_commit_returns_info() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    let before = InterpreterState::capture(&interp).unwrap();

    // Create a change
    interp.eval("set testvar \"value\"").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    let commit_info = persistence.save_changes(&interp, &changes, &user_info, "set testvar \"value\"").unwrap();

    assert!(commit_info.is_some());
    let info = commit_info.unwrap();

    // Verify commit info fields
    assert!(!info.commit_id.is_empty());
    assert_eq!(info.author, "testuser");
    assert!(info.message.contains("set testvar"));
    assert!(info.files_changed > 0);
}

#[test]
fn test_multiple_changes_single_commit() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);

    let before = InterpreterState::capture(&interp).unwrap();

    // Multiple changes
    interp.eval("proc test1 {} { return 1 }").unwrap();
    interp.eval("proc test2 {} { return 2 }").unwrap();
    interp.eval("set var1 \"value1\"").unwrap();
    interp.eval("set var2 \"value2\"").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert_eq!(changes.new_procs.len(), 2);
    assert_eq!(changes.new_vars.len(), 2);

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    let commit_info = persistence.save_changes(&interp, &changes, &user_info, "multiple changes").unwrap();

    assert!(commit_info.is_some());
    let info = commit_info.unwrap();

    // Should have multiple files changed
    assert!(info.files_changed >= 4); // 2 procs + 2 vars (+ indices)
}

#[test]
fn test_delete_proc() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    let state0 = InterpreterState::capture(&interp).unwrap();

    // Create and save a proc
    interp.eval("proc test {} { return \"test\" }").unwrap();
    let state1 = InterpreterState::capture(&interp).unwrap();
    let changes1 = state0.diff(&state1, &HashSet::new(), &HashSet::new());

    assert_eq!(changes1.new_procs.len(), 1);
    assert_eq!(changes1.new_procs[0], "test");

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    persistence.save_changes(&interp, &changes1, &user_info, "create proc").unwrap();

    // Delete the proc
    interp.eval("rename test {}").unwrap();
    let state2 = InterpreterState::capture(&interp).unwrap();
    let changes2 = state1.diff(&state2, &HashSet::new(), &HashSet::new());

    assert_eq!(changes2.deleted_procs.len(), 1);
    assert_eq!(changes2.deleted_procs[0], "test");

    persistence.save_changes(&interp, &changes2, &user_info, "delete proc").unwrap();

    // Verify proc was removed from index
    let index_file = state_path.join("procs/_index");
    let index_content = fs::read_to_string(index_file).unwrap();
    assert!(!index_content.contains("test\t"));
}

#[test]
fn test_user_info_to_signature() {
    let user_info = UserInfo::new("alice".to_string(), "example.com".to_string());
    let signature = user_info.to_signature().unwrap();

    assert_eq!(signature.name().unwrap(), "alice");
    assert_eq!(signature.email().unwrap(), "alice@example.com");
}

#[test]
fn test_state_changes_has_changes() {
    let mut changes = StateChanges { new_procs: vec![], deleted_procs: vec![], new_vars: vec![], deleted_vars: vec![] };
    assert!(!changes.has_changes());

    changes.new_procs.push("test".to_string());
    assert!(changes.has_changes());

    changes = StateChanges { new_procs: vec![], deleted_procs: vec![], new_vars: vec![], deleted_vars: vec![] };
    changes.deleted_procs.push("test".to_string());
    assert!(changes.has_changes());

    changes = StateChanges { new_procs: vec![], deleted_procs: vec![], new_vars: vec![], deleted_vars: vec![] };
    changes.new_vars.push("test".to_string());
    assert!(changes.has_changes());

    changes = StateChanges { new_procs: vec![], deleted_procs: vec![], new_vars: vec![], deleted_vars: vec![] };
    changes.deleted_vars.push("test".to_string());
    assert!(changes.has_changes());
}

#[test]
fn test_proc_with_special_characters() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    let before = InterpreterState::capture(&interp).unwrap();

    // Create proc with special chars in name (use underscores instead of :: for testing)
    interp.eval("proc test_with_underscores {} { return \"test\" }").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert_eq!(changes.new_procs.len(), 1);
    assert_eq!(changes.new_procs[0], "test_with_underscores");

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    persistence.save_changes(&interp, &changes, &user_info, "proc with underscores").unwrap();

    // Verify proc was indexed
    let index_file = state_path.join("procs/_index");
    let index_content = fs::read_to_string(index_file).unwrap();
    assert!(index_content.contains("test_with_underscores"));
}

#[test]
fn test_var_with_special_values() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    let before = InterpreterState::capture(&interp).unwrap();

    // Test various special values
    interp.eval(r#"set var1 "newlines\nand\ttabs""#).unwrap();
    interp.eval(r#"set var2 {braces and {nested} stuff}"#).unwrap();
    interp.eval(r#"set var3 [list 1 2 3]"#).unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    assert!(changes.new_vars.len() >= 3);

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    let result = persistence.save_changes(&interp, &changes, &user_info, "special values");

    assert!(result.is_ok());

    // Verify vars were indexed
    let index_file = state_path.join("vars/_index");
    let index_content = fs::read_to_string(index_file).unwrap();
    assert!(index_content.contains("var1"));
    assert!(index_content.contains("var2"));
    assert!(index_content.contains("var3"));
}

#[test]
fn test_multiple_commits_in_sequence() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());

    let state0 = InterpreterState::capture(&interp).unwrap();

    // First commit
    interp.eval("set var1 \"value1\"").unwrap();
    let state1 = InterpreterState::capture(&interp).unwrap();
    let changes1 = state0.diff(&state1, &HashSet::new(), &HashSet::new());
    let commit1 = persistence.save_changes(&interp, &changes1, &user_info, "commit 1").unwrap();
    assert!(commit1.is_some());

    // Second commit
    interp.eval("set var2 \"value2\"").unwrap();
    let state2 = InterpreterState::capture(&interp).unwrap();
    let changes2 = state1.diff(&state2, &HashSet::new(), &HashSet::new());
    let commit2 = persistence.save_changes(&interp, &changes2, &user_info, "commit 2").unwrap();
    assert!(commit2.is_some());

    // Verify commits are different
    assert_ne!(commit1.unwrap().commit_id, commit2.unwrap().commit_id);
}

#[test]
fn test_empty_changes_no_commit() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);

    let state = InterpreterState::capture(&interp).unwrap();
    let changes = state.diff(&state, &HashSet::new(), &HashSet::new()); // No changes

    assert!(!changes.has_changes());

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    // Note: This will still try to commit in current implementation
    // But we're testing that the StateChanges correctly reports no changes
}
// =============================================================================
// Proc Modification Tracking Tests
// =============================================================================

/// Helper to load proc tracking wrapper
fn load_proc_tracking(interp: &Interpreter) {
    let proc_tracking_code = include_str!("../tcl/proc_tracking.tcl");
    interp.eval(proc_tracking_code).unwrap();
}

#[test]
fn test_proc_tracking_wrapper_loads() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Verify the wrapper is loaded
    let result = interp.eval("info commands ::slopdrop::get_modified_procs").unwrap();
    assert_eq!(result.get_string(), "::slopdrop::get_modified_procs");
}

#[test]
fn test_proc_tracking_detects_new_proc() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Define a new proc
    interp.eval("proc test_proc {} { return 42 }").unwrap();
    
    // Check it was tracked
    let modified = InterpreterState::get_modified_procs(&interp).unwrap();
    assert!(modified.contains("test_proc"));
}

#[test]
fn test_proc_tracking_detects_modified_proc() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Define a proc
    interp.eval("proc test_proc {} { return 42 }").unwrap();
    
    // Clear tracking
    InterpreterState::get_modified_procs(&interp).unwrap();
    
    // Redefine the proc
    interp.eval("proc test_proc {} { return 99 }").unwrap();
    
    // Check it was tracked again
    let modified = InterpreterState::get_modified_procs(&interp).unwrap();
    assert!(modified.contains("test_proc"));
}

#[test]
fn test_proc_tracking_clears_after_get() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Define a proc
    interp.eval("proc test_proc {} { return 42 }").unwrap();
    
    // Get modified procs (should clear the list)
    let modified = InterpreterState::get_modified_procs(&interp).unwrap();
    assert!(modified.contains("test_proc"));
    
    // Get again (should be empty)
    let modified2 = InterpreterState::get_modified_procs(&interp).unwrap();
    assert!(modified2.is_empty());
}

#[test]
fn test_proc_modification_detected_in_diff() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Capture initial state
    let state_before = InterpreterState::capture(&interp).unwrap();
    
    // Define a new proc
    interp.eval("proc test_proc {} { return 42 }").unwrap();
    
    // Capture new state
    let state_after = InterpreterState::capture(&interp).unwrap();
    
    // Get modified procs
    let modified_procs = InterpreterState::get_modified_procs(&interp).unwrap();
    
    // Diff should detect new proc
    let changes = state_before.diff(&state_after, &modified_procs, &HashSet::new());
    assert!(changes.new_procs.contains(&"test_proc".to_string()));
}

#[test]
fn test_proc_modification_saved_to_disk() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();
    
    // Capture initial state
    let state_before = InterpreterState::capture(&interp).unwrap();
    
    // Define a new proc
    interp.eval("proc test_proc {x} { return [expr {$x * 2}] }").unwrap();
    
    // Capture new state
    let state_after = InterpreterState::capture(&interp).unwrap();
    
    // Get modified procs
    let modified_procs = InterpreterState::get_modified_procs(&interp).unwrap();
    
    // Get changes
    let changes = state_before.diff(&state_after, &modified_procs, &HashSet::new());
    assert!(changes.has_changes());
    
    // Save changes
    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    persistence.save_changes(&interp, &changes, &user_info, "proc test_proc {x} { return [expr {$x * 2}] }").unwrap();
    
    // Verify proc was saved
    let index_path = state_path.join("procs/_index");
    assert!(index_path.exists());
    let index_content = fs::read_to_string(&index_path).unwrap();
    assert!(index_content.contains("test_proc"));
}

#[test]
fn test_unchanged_proc_not_saved() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();
    
    // Define a proc
    interp.eval("proc test_proc {} { return 42 }").unwrap();
    
    // Capture states
    let state1 = InterpreterState::capture(&interp).unwrap();
    let modified1 = InterpreterState::get_modified_procs(&interp).unwrap();
    let changes1 = InterpreterState::capture(&interp).unwrap().diff(&state1, &modified1, &HashSet::new());
    
    // Save first time
    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    persistence.save_changes(&interp, &changes1, &user_info, "proc test_proc {} { return 42 }").unwrap();
    
    // Capture before/after with no changes
    let state2 = InterpreterState::capture(&interp).unwrap();
    let state3 = InterpreterState::capture(&interp).unwrap();
    let modified2 = InterpreterState::get_modified_procs(&interp).unwrap();
    
    // Should have no changes
    let changes2 = state2.diff(&state3, &modified2, &HashSet::new());
    assert!(!changes2.has_changes());
}

#[test]
fn test_internal_var_not_tracked() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Capture initial state
    let state_before = InterpreterState::capture(&interp).unwrap();
    
    // Set internal tracking var directly
    interp.eval("set slopdrop_modified_procs {foo bar}").unwrap();
    
    // Capture new state
    let state_after = InterpreterState::capture(&interp).unwrap();
    
    // Diff should not detect this as a change
    let modified_procs = HashSet::new();
    let changes = state_before.diff(&state_after, &modified_procs, &HashSet::new());
    
    // slopdrop_modified_procs should not be in new_vars
    assert!(!changes.new_vars.contains(&"slopdrop_modified_procs".to_string()));
}

#[test]
fn test_multiple_procs_modification_tracked() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Define multiple procs
    interp.eval("proc proc1 {} { return 1 }").unwrap();
    interp.eval("proc proc2 {} { return 2 }").unwrap();
    interp.eval("proc proc3 {} { return 3 }").unwrap();
    
    // Get modified procs
    let modified = InterpreterState::get_modified_procs(&interp).unwrap();
    
    // All three should be tracked
    assert!(modified.contains("proc1"));
    assert!(modified.contains("proc2"));
    assert!(modified.contains("proc3"));
    assert_eq!(modified.len(), 3);
}

// =============================================================================
// Variable Modification Tracking Tests
// =============================================================================

#[test]
fn test_var_tracking_detects_new_var() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Set a new var
    interp.eval("set test_var 42").unwrap();
    
    // Update traces to catch it
    interp.eval("::slopdrop::update_var_traces").unwrap();
    
    // Modify the var
    interp.eval("set test_var 99").unwrap();
    
    // Check it was tracked
    let modified = InterpreterState::get_modified_vars(&interp).unwrap();
    assert!(modified.contains("test_var"));
}

#[test]
fn test_var_tracking_detects_modification() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Set initial var
    interp.eval("set test_var initial").unwrap();
    
    // Update traces
    interp.eval("::slopdrop::update_var_traces").unwrap();
    
    // Clear tracking
    InterpreterState::get_modified_vars(&interp).unwrap();
    
    // Modify the var
    interp.eval("set test_var modified").unwrap();
    
    // Check it was tracked again
    let modified = InterpreterState::get_modified_vars(&interp).unwrap();
    assert!(modified.contains("test_var"));
}

#[test]
fn test_var_tracking_clears_after_get() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Set a var
    interp.eval("set test_var 123").unwrap();
    interp.eval("::slopdrop::update_var_traces").unwrap();
    interp.eval("set test_var 456").unwrap();
    
    // Get modified vars (should clear the list)
    let modified = InterpreterState::get_modified_vars(&interp).unwrap();
    assert!(modified.contains("test_var"));
    
    // Get again (should be empty)
    let modified2 = InterpreterState::get_modified_vars(&interp).unwrap();
    assert!(modified2.is_empty());
}

#[test]
fn test_var_modification_detected_in_diff() {
    let (_temp, _state_path) = create_temp_state();
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Set initial var
    interp.eval("set test_var initial").unwrap();
    
    // Capture initial state
    let state_before = InterpreterState::capture(&interp).unwrap();
    
    // Update traces
    interp.eval("::slopdrop::update_var_traces").unwrap();
    
    // Clear modified list
    InterpreterState::get_modified_vars(&interp).unwrap();
    
    // Modify var
    interp.eval("set test_var modified").unwrap();
    
    // Capture new state
    let state_after = InterpreterState::capture(&interp).unwrap();
    
    // Get modified vars
    let modified_vars = InterpreterState::get_modified_vars(&interp).unwrap();
    let modified_procs = HashSet::new();
    
    // Diff should detect modified var
    let changes = state_before.diff(&state_after, &modified_procs, &modified_vars);
    assert!(changes.new_vars.contains(&"test_var".to_string()));
}

#[test]
fn test_var_modification_saved_to_disk() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();
    
    // Set initial var and save it
    interp.eval("set test_var initial").unwrap();
    let state1 = InterpreterState::capture(&interp).unwrap();
    interp.eval("::slopdrop::update_var_traces").unwrap();
    InterpreterState::get_modified_vars(&interp).unwrap(); // Clear
    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    let changes1 = InterpreterState::capture(&interp).unwrap().diff(&state1, &HashSet::new(), &HashSet::new());
    if changes1.has_changes() {
        persistence.save_changes(&interp, &changes1, &user_info, "set test_var initial").unwrap();
    }
    
    // Modify var
    let state2 = InterpreterState::capture(&interp).unwrap();
    interp.eval("set test_var modified").unwrap();
    let state3 = InterpreterState::capture(&interp).unwrap();
    let modified_vars = InterpreterState::get_modified_vars(&interp).unwrap();
    
    // Get changes
    let changes2 = state2.diff(&state3, &HashSet::new(), &modified_vars);
    assert!(changes2.has_changes());
    
    // Save changes
    persistence.save_changes(&interp, &changes2, &user_info, "set test_var modified").unwrap();
    
    // Verify var was saved (check index updated)
    let index_path = state_path.join("vars/_index");
    assert!(index_path.exists());
    let index_content = fs::read_to_string(&index_path).unwrap();
    assert!(index_content.contains("test_var"));
}

#[test]
fn test_multiple_vars_modification_tracked() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Set multiple vars
    interp.eval("set var1 value1").unwrap();
    interp.eval("set var2 value2").unwrap();
    interp.eval("set var3 value3").unwrap();
    
    // Update traces
    interp.eval("::slopdrop::update_var_traces").unwrap();
    
    // Clear modified list
    InterpreterState::get_modified_vars(&interp).unwrap();
    
    // Modify all vars
    interp.eval("set var1 newvalue1").unwrap();
    interp.eval("set var2 newvalue2").unwrap();
    interp.eval("set var3 newvalue3").unwrap();
    
    // Get modified vars
    let modified = InterpreterState::get_modified_vars(&interp).unwrap();
    
    // All three should be tracked
    assert!(modified.contains("var1"));
    assert!(modified.contains("var2"));
    assert!(modified.contains("var3"));
    assert_eq!(modified.len(), 3);
}

#[test]
fn test_internal_var_slopdrop_not_tracked() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);
    
    // Capture initial state
    let state_before = InterpreterState::capture(&interp).unwrap();
    
    // Modify internal tracking vars directly
    interp.eval("::slopdrop::update_var_traces").unwrap();
    interp.eval("set slopdrop_modified_vars {foo bar}").unwrap();
    interp.eval("set slopdrop_modified_procs {baz}").unwrap();
    
    // Capture new state
    let state_after = InterpreterState::capture(&interp).unwrap();
    
    // Diff should not detect internal vars as changes
    let modified_vars = HashSet::new();
    let modified_procs = HashSet::new();
    let changes = state_before.diff(&state_after, &modified_procs, &modified_vars);
    
    // Internal slopdrop vars should not be in new_vars
    assert!(!changes.new_vars.iter().any(|v| v.starts_with("slopdrop_")));
}

#[test]
fn test_proc_name_with_special_characters() {
    let (_temp, _state_path) = create_temp_state();
    let mut interp = create_test_interp();

    // Load proc tracking module
    interp.eval(smeggdrop_commands::proc_tracking().as_str()).unwrap();

    // Create procs with special characters like unknown handlers
    interp.eval(r#"
        proc {unknown:2:cmd/^(.+)goon$/} {matches cmd args} {
            return "goon handler"
        }
    "#).unwrap();

    interp.eval(r#"
        proc {unknown:2:cmd/(.+)amid$/} {matches cmd args} {
            return "amid handler"
        }
    "#).unwrap();

    // Capture state - should include the procs without mangling the names
    let state_after = InterpreterState::capture(&interp).unwrap();

    // The proc names should be captured correctly WITHOUT extra braces
    assert!(state_after.procs.contains("unknown:2:cmd/^(.+)goon$/"),
            "Should contain the goon proc with special chars");
    assert!(state_after.procs.contains("unknown:2:cmd/(.+)amid$/"),
            "Should contain the amid proc with special chars");

    // Verify we can get both in the modified list
    let modified = InterpreterState::get_modified_procs(&interp).unwrap();
    assert!(modified.contains("unknown:2:cmd/^(.+)goon$/"),
            "Modified list should contain goon proc");
    assert!(modified.contains("unknown:2:cmd/(.+)amid$/"),
            "Modified list should contain amid proc");
}

// =============================================================================
// Array Tracking Tests
// =============================================================================

#[test]
fn test_array_captured_in_state() {
    let interp = create_test_interp();

    // Create an array
    interp.eval("set myarray(key1) value1").unwrap();
    interp.eval("set myarray(key2) value2").unwrap();

    let state = InterpreterState::capture(&interp).unwrap();

    // Array name should be in vars
    assert!(state.vars.contains("myarray"),
            "Array 'myarray' should be captured in state.vars");
}

#[test]
fn test_new_array_detected() {
    let interp = create_test_interp();

    let before = InterpreterState::capture(&interp).unwrap();

    // Create a new array
    interp.eval("set testarray(foo) bar").unwrap();

    let after = InterpreterState::capture(&interp).unwrap();
    let changes = before.diff(&after, &HashSet::new(), &HashSet::new());

    // Should detect new array
    assert!(changes.has_changes(), "Should detect array creation as a change");
    assert!(changes.new_vars.contains(&"testarray".to_string()),
            "New array 'testarray' should be in new_vars");
}

#[test]
fn test_array_modification_with_tracking() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);

    // Create initial array
    interp.eval("set agenda(ryan) {initial item}").unwrap();
    let state_before = InterpreterState::capture(&interp).unwrap();

    // Update traces to track the array
    interp.eval("::slopdrop::update_var_traces").unwrap();

    // Clear modified list
    InterpreterState::get_modified_vars(&interp).unwrap();

    // Add another item to the array
    interp.eval("lappend agenda(ryan) {second item}").unwrap();

    // Capture new state
    let state_after = InterpreterState::capture(&interp).unwrap();

    // Get modified vars
    let modified_vars = InterpreterState::get_modified_vars(&interp).unwrap();

    // Should have tracked the array modification
    assert!(modified_vars.contains("agenda"),
            "Array 'agenda' should be in modified_vars after element modification");

    // Diff should detect the modification
    let changes = state_before.diff(&state_after, &HashSet::new(), &modified_vars);
    assert!(changes.new_vars.contains(&"agenda".to_string()),
            "Modified array 'agenda' should appear in changes");
}

#[test]
fn test_agenda_command_tracking() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);

    // Define the +agenda command similar to user's implementation
    interp.eval(r#"
        proc +agenda {who args} {
            if {[info exists ::agenda($who)] != 1} {
                set ::agenda($who) {}
            }
            lappend ::agenda($who) $args
            return "Added $args to $who"
        }
    "#).unwrap();

    // Clear proc tracking
    InterpreterState::get_modified_procs(&interp).unwrap();

    // Capture initial state
    let state_before = InterpreterState::capture(&interp).unwrap();

    // Update traces
    interp.eval("::slopdrop::update_var_traces").unwrap();

    // Execute +agenda command (first time - creates array)
    interp.eval("+agenda ryan {have hbo documentary and book document hacks}").unwrap();

    // Update traces again to catch the new array
    interp.eval("::slopdrop::update_var_traces").unwrap();

    // Clear modified tracking from first command
    InterpreterState::get_modified_vars(&interp).unwrap();

    // Capture state after first command
    let state_after_first = InterpreterState::capture(&interp).unwrap();

    // Execute +agenda again (this time array exists, should be tracked)
    interp.eval("+agenda ryan {ransom trannies}").unwrap();

    // Capture final state
    let state_after_second = InterpreterState::capture(&interp).unwrap();

    // Get modified vars
    let modified_vars = InterpreterState::get_modified_vars(&interp).unwrap();

    // The array modification should be tracked
    assert!(modified_vars.contains("agenda"),
            "Array 'agenda' should be tracked when modified by +agenda command");

    // Diff should detect the change
    let changes = state_after_first.diff(&state_after_second, &HashSet::new(), &modified_vars);
    assert!(changes.has_changes(),
            "Should detect changes when +agenda modifies existing array");
    assert!(changes.new_vars.contains(&"agenda".to_string()),
            "Modified array 'agenda' should be in changes");
}

#[test]
fn test_array_modification_saved_to_disk() {
    let (_temp, state_path) = create_temp_state();
    let interp = create_test_interp();
    load_proc_tracking(&interp);

    let persistence = StatePersistence::with_repo(state_path.clone(), None, None);
    persistence.ensure_initialized().unwrap();

    // Create initial array
    interp.eval("set mydata(key1) value1").unwrap();
    let state1 = InterpreterState::capture(&interp).unwrap();

    // Update traces and save initial state
    interp.eval("::slopdrop::update_var_traces").unwrap();
    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());

    // Clear tracking
    InterpreterState::get_modified_vars(&interp).unwrap();

    // Modify array
    let state2 = InterpreterState::capture(&interp).unwrap();
    interp.eval("set mydata(key2) value2").unwrap();
    let state3 = InterpreterState::capture(&interp).unwrap();
    let modified_vars = InterpreterState::get_modified_vars(&interp).unwrap();

    // Get changes
    let changes = state2.diff(&state3, &HashSet::new(), &modified_vars);
    assert!(changes.has_changes(), "Array modification should be detected as a change");

    // Save changes
    let commit_info = persistence.save_changes(&interp, &changes, &user_info,
                                               "set mydata(key2) value2").unwrap();

    // Should create a commit
    assert!(commit_info.is_some(), "Array modification should result in a commit");

    // Verify array was saved
    let index_path = state_path.join("vars/_index");
    let index_content = fs::read_to_string(&index_path).unwrap();
    assert!(index_content.contains("mydata"),
            "Array 'mydata' should be in the vars index");
}

#[test]
fn test_multiple_array_elements_single_tracking() {
    let interp = create_test_interp();
    load_proc_tracking(&interp);

    // Create array and set up tracking
    interp.eval("set data(a) 1").unwrap();
    interp.eval("::slopdrop::update_var_traces").unwrap();
    InterpreterState::get_modified_vars(&interp).unwrap();

    // Modify multiple elements
    interp.eval("set data(b) 2").unwrap();
    interp.eval("set data(c) 3").unwrap();
    interp.eval("set data(a) 10").unwrap(); // Modify existing element

    // Get modified vars
    let modified = InterpreterState::get_modified_vars(&interp).unwrap();

    // Should contain the array name (not individual elements)
    assert!(modified.contains("data"),
            "Array 'data' should be tracked when any element is modified");
    // Should only appear once even though multiple elements were modified
    assert_eq!(modified.iter().filter(|v| *v == "data").count(), 1,
               "Array should only appear once in modified list");
}
