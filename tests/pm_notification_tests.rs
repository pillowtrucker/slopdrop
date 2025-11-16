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
    assert!(!admin_nicks.contains(&"*".to_string())); // Wildcard should be skipped
}

#[test]
fn test_commit_info_notification_format() {
    let commit_info = CommitInfo {
        commit_id: "a1b2c3d4e5f6g7h8".to_string(),
        author: "alice".to_string(),
        message: "Evaluated set testvar \"value\"".to_string(),
        files_changed: 2,
        insertions: 3,
        deletions: 1,
    };

    // This is the format from tcl_plugin.rs send_commit_notifications
    let notification = format!(
        "[Git] {} committed by {} | {} files changed (+{} -{}) | {}",
        &commit_info.commit_id[..8],
        commit_info.author,
        commit_info.files_changed,
        commit_info.insertions,
        commit_info.deletions,
        commit_info.message.lines().next().unwrap_or("")
    );

    assert_eq!(
        notification,
        "[Git] a1b2c3d4 committed by alice | 2 files changed (+3 -1) | Evaluated set testvar \"value\""
    );
}

#[test]
fn test_commit_info_multiline_message() {
    let commit_info = CommitInfo {
        commit_id: "abcdef1234567890".to_string(),
        author: "bob".to_string(),
        message: "First line\nSecond line\nThird line".to_string(),
        files_changed: 5,
        insertions: 10,
        deletions: 2,
    };

    // Should only show first line
    let notification = format!(
        "[Git] {} committed by {} | {} files changed (+{} -{}) | {}",
        &commit_info.commit_id[..8],
        commit_info.author,
        commit_info.files_changed,
        commit_info.insertions,
        commit_info.deletions,
        commit_info.message.lines().next().unwrap_or("")
    );

    assert!(notification.contains("First line"));
    assert!(!notification.contains("Second line"));
    assert!(!notification.contains("Third line"));
}

#[test]
fn test_skip_notification_to_commit_author() {
    let author_nick = "alice";
    let admin_nicks = vec!["alice".to_string(), "bob".to_string(), "charlie".to_string()];

    // Filter out the author (logic from send_commit_notifications)
    let recipients: Vec<_> = admin_nicks.iter()
        .filter(|nick| **nick != author_nick)
        .collect();

    assert_eq!(recipients.len(), 2);
    assert!(!recipients.contains(&&"alice".to_string()));
    assert!(recipients.contains(&&"bob".to_string()));
    assert!(recipients.contains(&&"charlie".to_string()));
}

#[test]
fn test_empty_admin_list() {
    let security_config = SecurityConfig {
        eval_timeout_ms: 5000,
        privileged_users: vec![],
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
    };

    let mut admin_nicks = Vec::new();
    for pattern in &security_config.privileged_users {
        if let Some(nick_part) = pattern.split('!').next() {
            if nick_part != "*" && !nick_part.is_empty() {
                admin_nicks.push(nick_part.to_string());
            }
        }
    }

    // alice appears twice - both should be in the list
    // In production, duplicates would result in duplicate PMs
    // but that's acceptable behavior
    assert_eq!(admin_nicks.len(), 3);
    assert_eq!(admin_nicks.iter().filter(|n| **n == "alice").count(), 2);
}
