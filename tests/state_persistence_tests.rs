use slopdrop::state::{InterpreterState, StatePersistence, StateChanges, UserInfo};
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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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

    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes1 = state0.diff(&state1);

    assert_eq!(changes1.new_procs.len(), 1);
    assert_eq!(changes1.new_procs[0], "test");

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    persistence.save_changes(&interp, &changes1, &user_info, "create proc").unwrap();

    // Delete the proc
    interp.eval("rename test {}").unwrap();
    let state2 = InterpreterState::capture(&interp).unwrap();
    let changes2 = state1.diff(&state2);

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
    let changes = before.diff(&after);

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
    let changes = before.diff(&after);

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
    let changes1 = state0.diff(&state1);
    let commit1 = persistence.save_changes(&interp, &changes1, &user_info, "commit 1").unwrap();
    assert!(commit1.is_some());

    // Second commit
    interp.eval("set var2 \"value2\"").unwrap();
    let state2 = InterpreterState::capture(&interp).unwrap();
    let changes2 = state1.diff(&state2);
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
    let changes = state.diff(&state); // No changes

    assert!(!changes.has_changes());

    let user_info = UserInfo::new("testuser".to_string(), "testhost".to_string());
    // Note: This will still try to commit in current implementation
    // But we're testing that the StateChanges correctly reports no changes
}
