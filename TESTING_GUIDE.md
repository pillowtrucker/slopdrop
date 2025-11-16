# Testing Guide for New Features

This guide covers testing all the newly implemented features in the TCL evalbot.

## 1. Git Over SSH Testing

### Prerequisites
- A git repository accessible via SSH (e.g., `git@github.com:user/repo.git`)
- SSH key configured on your system
- SSH agent running OR explicit key file path

### Configuration

#### Option A: Using SSH Agent (Recommended)
```toml
[tcl]
state_path = "./state"
state_repo = "git@github.com:youruser/yourrepo.git"
# No ssh_key needed - will use SSH agent
```

Start SSH agent and add your key:
```bash
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_rsa
```

#### Option B: Using Explicit SSH Key
```toml
[tcl]
state_path = "./state"
state_repo = "git@github.com:youruser/yourrepo.git"
ssh_key = "/home/user/.ssh/id_rsa"
```

### Test Steps

1. **Configure remote repository**
   - Edit `config.toml` with SSH URL
   - Add SSH key configuration if needed

2. **Run the bot**
   ```bash
   cargo run
   ```

3. **Make a state change on IRC**
   ```
   <you> tcl set testvar "hello world"
   <bot> hello world
   ```

4. **Verify git commit and push**
   - Check bot logs for "Pushing to remote repository"
   - Check bot logs for "Successfully pushed to main" or "Successfully pushed to master"
   - Verify commit appears on remote repository

5. **Test with rollback** (admin only)
   ```
   <you> tclAdmin history
   <bot> <commit hash> <date> <author> <message>
   <you> tclAdmin rollback <hash>
   <bot> Rolled back to commit <hash>. Note: Restart bot to reload state.
   ```
   - Verify rollback is pushed to remote

### Expected Behavior

- On successful commit, bot pushes to remote automatically
- Push tries `main` branch first, falls back to `master`
- SSH authentication uses specified key or SSH agent
- Logs show authentication method used
- Remote repository reflects all commits and rollbacks

### Troubleshooting

**Error: "Failed to authenticate"**
- Ensure SSH key has correct permissions (600)
- Verify SSH key is added to remote git service
- Check SSH agent is running and has key loaded
- Test SSH connection: `ssh -T git@github.com`

**Error: "Failed to push to main: ... and master: ..."**
- Verify remote repository exists
- Ensure you have push permissions
- Check branch exists on remote (create `main` or `master` branch)

## 2. PM Notifications to Admins

### Configuration

```toml
[security]
# Admin hostmasks - use wildcards * and ?
privileged_users = [
    "alice!*@*.example.com",
    "bob!~bobident@*",
    "charlie!*@*"
]
```

### Test Steps

1. **Configure multiple admins**
   - Add 2+ hostmasks to privileged_users
   - Use different nick patterns

2. **Make a state change as one admin**
   ```
   <alice> tcl set notifytest "testing notifications"
   <bot> testing notifications
   ```

3. **Verify PM notifications**
   - All other admins (bob, charlie) receive PM from bot
   - PM format: `[Git] <hash> committed by <author> | <files> files changed (+<ins> -<del>) | <message>`
   - Alice (the author) does NOT receive a PM

### Expected Behavior

- Admins with specific nicks in hostmask patterns get PMs
- Wildcard-only patterns (`*!*@*`) are ignored
- Commit author doesn't get self-notification
- Notification includes commit summary and stats

### Example Notification
```
[Git] a1b2c3d4 committed by alice | 2 files changed (+3 -1) | Evaluated set notifytest "testing notifications"
```

## 3. Output Pagination with "more" Command

### Test Steps

1. **Generate long output** (more than 10 lines)
   ```
   <you> tcl for {set i 0} {$i < 30} {incr i} { puts "Line $i" }
   <bot> Line 0
   <bot> Line 1
   ...
   <bot> Line 9
   <bot> ... (20 more lines - type 'tcl more' to continue)
   ```

2. **Retrieve next chunk**
   ```
   <you> tcl more
   <bot> Line 10
   <bot> Line 11
   ...
   <bot> Line 19
   <bot> ... (10 more lines - type 'tcl more' to continue)
   ```

3. **Retrieve final chunk**
   ```
   <you> tcl more
   <bot> Line 20
   ...
   <bot> Line 29
   ```

4. **Try "more" when no cache**
   ```
   <you> tcl more
   <bot> No cached output. Run a tcl command first.
   ```

### Expected Behavior

- First response shows 10 lines (default max_output_lines)
- Remaining output cached per (channel, nick)
- Each "more" shows next 10 lines
- Final chunk shows no "more" message
- Cache expires after 5 minutes
- Different users have separate caches

### Configuration

Change pagination limit in `config.toml`:
```toml
[tcl]
max_output_lines = 20  # Show 20 lines at a time
```

## 4. SSL/TLS with Self-Signed Certificates

### Test Steps

1. **Set up test IRC server with self-signed cert**
   - Use ngircd, inspircd, or similar
   - Generate self-signed certificate

2. **Configure bot**
   ```toml
   [server]
   hostname = "irc.local.test"
   port = 6697
   use_tls = true
   ```

3. **Run bot**
   ```bash
   cargo run
   ```

4. **Verify connection**
   - Bot should connect successfully
   - Logs show "IRC client connected to irc.local.test:6697"
   - No certificate validation errors

### Expected Behavior

- Bot accepts self-signed certificates
- No manual certificate installation needed
- TLS connection established successfully
- All IRC features work normally

### Security Note

The bot is configured with `dangerously_accept_invalid_certs: true` which accepts all certificates including self-signed ones. This is intentional for testing and private IRC servers, but be aware of the security implications.

## 5. Integration Testing

### Complete Workflow Test

1. **Set up everything**
   - Configure SSH git repository
   - Add multiple admin hostmasks
   - Set output pagination limit
   - Use TLS connection

2. **Create TCL procs**
   ```
   <admin1> tcl proc greet {name} { return "Hello, $name!" }
   <bot> (empty output)
   <admin2> *receives PM about commit*
   ```

3. **Test pagination with proc**
   ```
   <user> tcl for {set i 0} {$i < 50} {incr i} { greet "User$i" }
   <bot> (first 10 lines)
   <bot> ... (40 more lines - type 'tcl more' to continue)
   <user> tcl more
   <bot> (next 10 lines)
   ```

4. **Verify git state**
   - Check remote repository for commits
   - Verify proc is saved in state
   - Check commit has correct author

5. **Test rollback**
   ```
   <admin> tclAdmin history 5
   <bot> (shows last 5 commits)
   <admin> tclAdmin rollback <old-commit>
   <bot> Rolled back to commit <hash>. Note: Restart bot to reload state.
   <admin2> *receives PM about rollback commit*
   ```

6. **Verify rollback pushed**
   - Check remote repository shows rollback commit
   - Force-push visible in git history

### Expected Results

✅ All features work together seamlessly
✅ Git commits pushed to remote via SSH
✅ Admins notified of all state changes
✅ Output pagination works for all commands
✅ SSL/TLS works with self-signed certs
✅ State persists across bot restarts

## Troubleshooting

### Git SSH Issues
- **Problem**: Authentication fails
- **Solution**: Check SSH key permissions, verify key is in git service, test with `ssh -T git@host`

### PM Notifications Not Received
- **Problem**: Admins not getting PMs
- **Solution**: Check hostmask patterns, verify nicks match patterns, check for wildcard-only patterns

### Pagination Cache Issues
- **Problem**: "more" shows wrong output
- **Solution**: Cache is per-user/channel - ensure using same nick/channel, cache expires after 5min

### SSL/TLS Connection Fails
- **Problem**: Certificate errors even with self-signed support
- **Solution**: Verify TLS is enabled in config, check port is correct (usually 6697), review bot logs

## Performance Notes

- Output cache cleanup runs on every command (negligible overhead)
- SSH authentication cached by git2 library per connection
- PM notifications sent asynchronously (non-blocking)
- Git commits are synchronous but typically fast (<100ms)

## Security Considerations

1. **Self-signed certificates**: Bot accepts all certs - use only with trusted servers
2. **SSH keys**: Protect private keys (chmod 600), use SSH agent when possible
3. **Admin hostmasks**: Use specific patterns when possible, avoid `*!*@*`
4. **Git credentials**: Never commit SSH keys to repository
5. **Output pagination**: Cache expires after 5 minutes, no sensitive data leakage

## Next Steps

After testing, consider:
- Adjusting `max_output_lines` based on channel preferences
- Tightening admin hostmask patterns for production
- Setting up automated git backups
- Monitoring git repository size
- Implementing rate limiting for PM notifications (if spammy)
