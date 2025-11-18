use slopdrop::config::SecurityConfig;
use slopdrop::state::CommitInfo;

#[test]
fn test_extract_admin_nicks_from_hostmasks() {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec![
            "alice!*@*.example.com".to_string(),
            "bob!~bobuser@*".to_string(),
            "charlie!*@192.168.1.*".to_string(),
            "*!*@*.admin.net".to_string(), // Wildcard nick should be skipped
        ],
        blacklisted_users: vec![],
        memory_limit_mb: 0, // Disabled for tests - RLIMIT_AS affects entire process
        max_recursion_depth: 1000,
    };

    // Extract nicks from patterns (this is the logic from tcl_plugin.rs)
    let mut admin_nicks = Vec::new();
    for pattern in &security_config.privileged_users {
        if let Some(nick_part) = pattern.split('!').next() {
            if nick_part != "*" && !nick_part.is_empty() {
                admin_nicks.push(nick_part.to_string());
            }
        }
    }

    assert_eq!(admin_nicks.len(), 3);
    assert!(admin_nicks.contains(&"alice".to_string()));
    assert!(admin_nicks.contains(&"bob".to_string()));
    assert!(admin_nicks.contains(&"charlie".to_string()));
}

#[test]
fn test_commit_info_format() {
    let commit = CommitInfo {
        commit_id: "abc123".to_string(),
        author: "testuser".to_string(),
        message: "Test commit".to_string(),
        files_changed: 2,
        insertions: 10,
        deletions: 5,
    };

    // Format as expected by PM notification: "[Git] <hash> committed by <author> | <files> files changed (+<ins> -<del>) | <message>"
    let formatted = format!(
        "[Git] {} committed by {} | {} files changed (+{} -{}) | {}",
        commit.commit_id,
        commit.author,
        commit.files_changed,
        commit.insertions,
        commit.deletions,
        commit.message
    );

    assert_eq!(
        formatted,
        "[Git] abc123 committed by testuser | 2 files changed (+10 -5) | Test commit"
    );
}

#[test]
fn test_commit_info_single_file() {
    let commit = CommitInfo {
        commit_id: "def456".to_string(),
        author: "admin".to_string(),
        message: "Fixed typo".to_string(),
        files_changed: 1,
        insertions: 1,
        deletions: 1,
    };

    // Should say "file" (singular) when files_changed == 1
    let word = if commit.files_changed == 1 {
        "file"
    } else {
        "files"
    };

    let formatted = format!(
        "[Git] {} committed by {} | {} {} changed (+{} -{}) | {}",
        commit.commit_id, commit.author, commit.files_changed, word, commit.insertions, commit.deletions, commit.message
    );

    assert_eq!(
        formatted,
        "[Git] def456 committed by admin | 1 file changed (+1 -1) | Fixed typo"
    );
}

#[test]
fn test_empty_admin_list() {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec![],
        blacklisted_users: vec![],
        memory_limit_mb: 0, // Disabled for tests - RLIMIT_AS affects entire process
        max_recursion_depth: 1000,
    };

    let mut admin_nicks = Vec::new();
    for pattern in &security_config.privileged_users {
        if let Some(nick_part) = pattern.split('!').next() {
            if nick_part != "*" && !nick_part.is_empty() {
                admin_nicks.push(nick_part.to_string());
            }
        }
    }

    assert_eq!(admin_nicks.len(), 0);
}

#[test]
fn test_wildcard_only_patterns() {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec![
            "*!*@*".to_string(),
            "*!admin@*".to_string(),
            "*!*@host.example.com".to_string(),
        ],
        blacklisted_users: vec![],
        memory_limit_mb: 0, // Disabled for tests - RLIMIT_AS affects entire process
        max_recursion_depth: 1000,
    };

    let mut admin_nicks = Vec::new();
    for pattern in &security_config.privileged_users {
        if let Some(nick_part) = pattern.split('!').next() {
            if nick_part != "*" && !nick_part.is_empty() {
                admin_nicks.push(nick_part.to_string());
            }
        }
    }

    // All patterns have wildcard nick, so no admins should be extracted
    assert_eq!(admin_nicks.len(), 0);
}

#[test]
fn test_complex_hostmask_patterns() {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec![
            "alice!~alice@*.example.com".to_string(),
            "bob!*@192.168.?.???".to_string(),
            "charlie!?admin@host*.net".to_string(),
        ],
        blacklisted_users: vec![],
        memory_limit_mb: 0, // Disabled for tests - RLIMIT_AS affects entire process
        max_recursion_depth: 1000,
    };

    let mut admin_nicks = Vec::new();
    for pattern in &security_config.privileged_users {
        if let Some(nick_part) = pattern.split('!').next() {
            if nick_part != "*" && !nick_part.is_empty() {
                admin_nicks.push(nick_part.to_string());
            }
        }
    }

    assert_eq!(admin_nicks.len(), 3);
    assert!(admin_nicks.contains(&"alice".to_string()));
    assert!(admin_nicks.contains(&"bob".to_string()));
    assert!(admin_nicks.contains(&"charlie".to_string()));
}

#[test]
fn test_duplicate_admin_nicks() {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec![
            "alice!*@host1.example.com".to_string(),
            "alice!*@host2.example.com".to_string(),
            "bob!*@*.example.com".to_string(),
        ],
        blacklisted_users: vec![],
        memory_limit_mb: 0, // Disabled for tests - RLIMIT_AS affects entire process
        max_recursion_depth: 1000,
    };

    let mut admin_nicks = Vec::new();
    for pattern in &security_config.privileged_users {
        if let Some(nick_part) = pattern.split('!').next() {
            if nick_part != "*" && !nick_part.is_empty() {
                admin_nicks.push(nick_part.to_string());
            }
        }
    }

    // Should have duplicates since we're not deduplicating
    assert_eq!(admin_nicks.len(), 3);
    assert_eq!(admin_nicks.iter().filter(|n| *n == "alice").count(), 2);
    assert_eq!(admin_nicks.iter().filter(|n| *n == "bob").count(), 1);
}
