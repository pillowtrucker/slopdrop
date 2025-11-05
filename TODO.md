# TODO List for Slopdrop

## Critical Features (Must Have)

### 1. State Persistence System (HIGHEST PRIORITY)
The original had a sophisticated git-based versioned interpreter system that we're completely missing:

- [ ] Implement git-based state storage
  - [ ] Create state directory structure (procs/, vars/, .git)
  - [ ] SHA1-based file naming for procs and vars
  - [ ] Index files to track proc/var names to file mappings
- [ ] Save interpreter state after each evaluation
  - [ ] Detect proc creation/modification/deletion
  - [ ] Detect var creation/modification/deletion
  - [ ] Write changed procs/vars to files
  - [ ] Git commit with author info from IRC user
- [ ] Load interpreter state on startup
  - [ ] Read all procs from state/procs/
  - [ ] Read all vars from state/vars/
  - [ ] Restore interpreter to previous state
- [ ] Implement rollback command
  - [ ] `history` command to view git log
  - [ ] `rollback <revision>` to revert to previous state
- [ ] Push to remote git repository (optional)

**Files to study**: `versioned_interpreter.tcl`, `interpx.tcl`

### 2. Proper TCL Safe Interpreter
Current implementation just renames commands, not actually safe:

- [ ] Use TCL's actual safe interpreter mode
  - [ ] Research tcltk crate's safe interpreter support
  - [ ] If not supported, manually hide dangerous commands properly
- [ ] Implement proc tracking
  - [ ] Wrap `proc` command to detect new/modified procs
  - [ ] Wrap `rename` command to detect proc renames
  - [ ] Track which procs are user-defined vs built-in
- [ ] Implement variable tracking
  - [ ] Add traces to detect variable modifications
  - [ ] Track creation/modification/deletion
- [ ] Custom loop wrappers (for, foreach, while)
  - [ ] Ensure loops can be interrupted
  - [ ] Prevent infinite loops from hanging

**Files to study**: `interpx.tcl` lines 312-334

### 3. Timeout Mechanism
Currently no timeout protection at all:

- [ ] Research Rust alternatives to SIGALRM
  - [ ] Consider using tokio::time::timeout
  - [ ] May need separate thread for TCL execution
- [ ] Implement 30-second default timeout
- [ ] Make timeout configurable
- [ ] Handle timeout gracefully
  - [ ] Kill evaluation
  - [ ] Return error message
  - [ ] Clean up resources
- [ ] Test with infinite loops

## Important Features (Should Have)

### 4. Smeggdrop Command System
The original had many utility commands available in TCL:

- [ ] **cache** - Persistent key-value storage
  - [ ] `cache::get bucket key`
  - [ ] `cache::put bucket key value`
  - [ ] `cache::exists bucket key`
  - [ ] `cache::delete bucket key`
  - [ ] `cache::keys bucket`
  - [ ] `cache::fetch bucket key script` - Get or compute

- [ ] **http** - HTTP operations with rate limiting
  - [ ] `http::get url` - GET request
  - [ ] `http::post url body` - POST request
  - [ ] `http::head url` - HEAD request
  - [ ] Rate limiting (5 requests per eval, 25 per minute)
  - [ ] Transfer size limits
  - [ ] Timeout limits
  - [ ] Returns: [status_code, headers, body]

- [ ] **history** - Git commit history
  - [ ] `history ?start?` - Show last 10 commits
  - [ ] Format: [commit, date, author, summary]

- [ ] **dict** - Dictionary operations (if not built-in)

- [ ] **encoding** - Encoding utilities
  - [ ] Base64 encode/decode
  - [ ] URL encode/decode
  - [ ] HTML entity encode/decode

- [ ] **sha1** - SHA1 hashing
  - [ ] `sha1 string`

- [ ] **publish** - Publishing mechanism
  - [ ] Research what this did in original

- [ ] **meta** - Meta-programming features
  - [ ] Research what this did in original

- [ ] **log** - Logging utilities
  - [ ] Integration with Rust logging

- [ ] **irc** - IRC-related commands
  - [ ] `chanlist` - Get list of users in channel
  - [ ] Channel info access from TCL

**Files to study**: All files in `src/smeggdrop/smeggdrop/commands/`

### 5. IRC Feature Completeness

- [ ] **Channel member tracking**
  - [ ] Handle NAMES reply (353)
  - [ ] Track JOIN events
  - [ ] Track PART events
  - [ ] Track QUIT events
  - [ ] Track KICK events
  - [ ] Track NICK changes
  - [ ] Make channel list available to TCL via `chanlist` command

- [ ] **Auto-rejoin on kick**
  - [ ] Wait 10 seconds
  - [ ] Rejoin channel
  - [ ] Handle repeated kicks (increase delay?)

- [ ] **IRC formatting support**
  - [ ] Parse IRC color codes
  - [ ] Parse bold/italic/underline
  - [ ] Implement message splitting with formatting preservation
  - [ ] `split_lines` function from original

- [ ] **Better message handling**
  - [ ] Accurate IRC message length calculation (512 - prefix - command - CRLF)
  - [ ] Smart message splitting (don't break in middle of words)
  - [ ] Preserve formatting across splits

- [ ] **CTCP support**
  - [ ] VERSION reply
  - [ ] TIME reply
  - [ ] PING reply
  - [ ] ACTION handling (/me)

**Files to study**: `Carrion/Plugin/IO/IRC/Client.hs`, `smeggdrop.tcl` lines 5-155

### 6. Configuration Enhancements

- [ ] Add more config options
  - [ ] `command_prefix` - Default "tcl"
  - [ ] `admin_command_prefix` - Default "tclAdmin"
  - [ ] `max_message_length` - IRC message limit
  - [ ] `flood_protection` - Enable/disable
  - [ ] `owner` - Bot owner nick
- [ ] Per-channel configuration
  - [ ] Different privileges per channel
  - [ ] Different command prefixes per channel
- [ ] Multiple IRC servers support?
- [ ] Hot reload configuration (SIGHUP)

### 7. Better Error Handling

- [ ] Propagate TCL errorInfo properly
- [ ] Better error messages to users
- [ ] Log errors to file
- [ ] Don't crash on errors
- [ ] Handle network disconnections gracefully
- [ ] Reconnect logic for IRC

### 8. Resource Management

- [ ] Limit memory usage of TCL interpreter
- [ ] Limit recursion depth
- [ ] Clean up old state files
- [ ] Garbage collection for cache buckets
- [ ] Rate limiting per user

## Nice to Have Features

### 9. Testing

- [ ] Unit tests
  - [ ] validator::validate_brackets tests (already has some)
  - [ ] Config parsing tests
  - [ ] Message type tests
- [ ] Integration tests
  - [ ] TCL interpreter tests
  - [ ] IRC client tests (with mock server?)
- [ ] TCL script tests
  - [ ] Test all smeggdrop commands
  - [ ] Test state persistence
  - [ ] Test rollback
- [ ] CI/CD setup
  - [ ] GitHub Actions for tests
  - [ ] Automated builds
  - [ ] Docker image builds

### 10. Deployment

- [ ] Systemd service file
  - [ ] Auto-restart on crash
  - [ ] Logging to journald
  - [ ] User/group isolation
- [ ] Docker support
  - [ ] Dockerfile
  - [ ] Docker Compose example
  - [ ] Volume for state persistence
- [ ] Installation script
- [ ] Binary releases (GitHub Releases)
- [ ] Distribution packages (deb, rpm)

### 11. Documentation

- [ ] API documentation (rustdoc)
- [ ] User guide
  - [ ] How to install
  - [ ] How to configure
  - [ ] Available commands
  - [ ] Security best practices
- [ ] Development guide
  - [ ] How to build
  - [ ] Architecture overview
  - [ ] How to add new features
  - [ ] How to add TCL commands
- [ ] Migration guide from old bot

### 12. Observability

- [ ] Metrics
  - [ ] Number of evaluations
  - [ ] Evaluation duration
  - [ ] Error rate
  - [ ] IRC messages sent/received
- [ ] Prometheus exporter
- [ ] Health check endpoint
- [ ] Admin commands
  - [ ] `!status` - Bot status
  - [ ] `!stats` - Statistics
  - [ ] `!reload` - Reload config
  - [ ] `!restart` - Restart bot

### 13. Security Enhancements

- [ ] Hostmask-based authentication (not just nick)
- [ ] NickServ integration for auth
- [ ] Channel modes integration (op/voice)
- [ ] Blacklist/whitelist for users
- [ ] Per-user rate limiting
- [ ] Sandboxing at OS level (seccomp, containers)
- [ ] Security audit of TCL sandboxing

### 14. Additional Features

- [ ] Multiple channels with different privileges
- [ ] Private message support (already partially there)
- [ ] Ignored users list
- [ ] TCL package management
  - [ ] Allow loading safe TCL packages
  - [ ] Package whitelist
- [ ] Web interface
  - [ ] View state
  - [ ] View history
  - [ ] Manage configuration
- [ ] REST API
  - [ ] Submit TCL code via HTTP
  - [ ] Query state
- [ ] Webhook support
  - [ ] GitHub webhooks
  - [ ] CI notifications

## Code Quality

- [ ] Fix all compiler warnings
- [ ] Add documentation comments
- [ ] Error handling consistency
- [ ] Logging consistency
- [ ] Code formatting (rustfmt)
- [ ] Linting (clippy)
- [ ] Remove dead code
- [ ] Remove TODOs in code

## Files to Reference from Original

- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/versioned_interpreter.tcl` - State persistence
- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/interpx.tcl` - Safe interpreter
- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/commands.tcl` - Command system
- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/commands/*.tcl` - Individual commands
- `/home/user/old-tcl-evalbot/src/Carrion/Plugin/TCL.hs` - TCL plugin architecture
- `/home/user/old-tcl-evalbot/src/Carrion/Plugin/IO/IRC/Client.hs` - IRC client features

## Priority Order

1. **State Persistence** - Without this, the bot can't remember anything between sessions
2. **Timeout Mechanism** - Critical security feature
3. **Proper Safe Interpreter** - Better sandboxing
4. **Smeggdrop Commands** - Core functionality (at least cache and http)
5. **Channel Tracking** - Needed for chanlist command
6. **IRC Features** - Better user experience
7. **Testing** - Ensure reliability
8. **Documentation** - Help users and developers

---

**Estimated work**: This is probably 2-4 weeks of solid development work to reach feature parity with the original.
