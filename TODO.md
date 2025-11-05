# TODO List for Slopdrop

**Last Updated:** 2025-11-05

## ✅ Completed Features

### 1. State Persistence System ✅ COMPLETE
- [x] Implement git-based state storage
  - [x] Create state directory structure (procs/, vars/, .git)
  - [x] SHA1-based file naming for procs and vars
  - [x] Index files to track proc/var names to file mappings
- [x] Save interpreter state after each evaluation
  - [x] Detect proc creation/modification/deletion
  - [x] Detect var creation/modification/deletion
  - [x] Write changed procs/vars to files
  - [x] Git commit with author info from IRC user
- [x] Load interpreter state on startup
  - [x] Read all procs from state/procs/
  - [x] Read all vars from state/vars/
  - [x] Restore interpreter to previous state
  - [x] Bootstrap loading (stolen-treasure.tcl + overrides)

**Status:** Fully functional git-based versioned interpreter with automatic commits

### 2. Timeout Mechanism ✅ COMPLETE
- [x] Thread-based timeout using std::mpsc + tokio::time::timeout
- [x] Implement 30-second default timeout
- [x] Make timeout configurable (eval_timeout_ms in config)
- [x] Handle timeout gracefully
  - [x] Return error message to user
  - [x] Bot doesn't hang on user side
- [x] Test with infinite loops

**Status:** Working thread-based timeout. Known limitation: thread doesn't restart (documented)

### 3. Smeggdrop Command System ✅ MOSTLY COMPLETE
- [x] **cache** - Persistent key-value storage
  - [x] `cache::get bucket key`
  - [x] `cache::put bucket key value`
  - [x] `cache::exists bucket key`
  - [x] `cache::delete bucket key`
  - [x] `cache::keys bucket`
  - [x] `cache::fetch bucket key script` - Get or compute

- [x] **http** - HTTP operations with rate limiting
  - [x] `http::get url` - GET request
  - [x] `http::post url body` - POST request
  - [x] `http::head url` - HEAD request
  - [x] Rate limiting (5 requests per eval, 25 per minute)
  - [x] Transfer size limits (150KB)
  - [x] Timeout limits (5s)
  - [x] Returns: [status_code, headers, body]

- [x] **encoding** - Encoding utilities
  - [x] Base64 encode/decode
  - [x] URL encode/decode

- [x] **sha1** - SHA1 hashing
  - [x] `sha1 string` (requires tcllib)

- [x] **Utility commands**
  - [x] `pick` - Weighted random choice
  - [x] `choose` - Random choice from args
  - [x] `??` - Random element from list
  - [x] `first`, `last`, `rest` - List operations
  - [x] `upper`, `lower` - String operations

**Status:** Core commands complete. Only minor utilities missing.

### 4. Auto-rejoin on kick ✅ COMPLETE
- [x] Wait 10 seconds
- [x] Rejoin channel

---

## 🚧 High Priority (Next Steps)

### 5. Git History Commands
The state is saved to git, but users can't view/rollback:

- [ ] **history** - Git commit history
  - [ ] `history ?start?` - Show last 10 commits
  - [ ] Format: [commit, date, author, summary]
  - [ ] Use git2 crate to read log

- [ ] **rollback** - Revert to previous state
  - [ ] `rollback <revision>` - Git reset to commit
  - [ ] Reload interpreter state after rollback
  - [ ] Warn about uncommitted changes

**Estimated time:** 1 day

### 6. Thread Restart on Timeout
Currently timeout is detected but thread keeps running:

- [ ] Detect when timeout occurs
- [ ] Kill hung TCL thread
- [ ] Spawn new TCL thread
- [ ] Reload interpreter state
- [ ] Maintain channel communication

**Estimated time:** 1-2 days
**Challenge:** Need to handle channel management carefully

---

## 📋 Medium Priority (Polish)

### 7. Channel Member Tracking
Enable `chanlist` command and track who's in channels:

- [ ] **Channel member tracking**
  - [ ] Handle NAMES reply (353)
  - [ ] Track JOIN events
  - [ ] Track PART events
  - [ ] Track QUIT events
  - [ ] Track KICK events
  - [ ] Track NICK changes
  - [ ] Make channel list available to TCL via `chanlist` command

**Estimated time:** 2-3 days

### 8. IRC Formatting Support
Better message handling and formatting:

- [ ] **IRC formatting support**
  - [ ] Parse IRC color codes
  - [ ] Parse bold/italic/underline
  - [ ] Implement message splitting with formatting preservation

- [ ] **Better message handling**
  - [ ] Accurate IRC message length calculation (512 - prefix - command - CRLF)
  - [ ] Smart message splitting (don't break in middle of words)
  - [ ] Preserve formatting across splits

- [ ] **CTCP support**
  - [ ] VERSION reply
  - [ ] TIME reply
  - [ ] PING reply
  - [ ] ACTION handling (/me)

**Estimated time:** 2-3 days

### 9. Better TCL Safe Interpreter
Current implementation renames dangerous commands, could be better:

- [ ] Research TCL safe interpreter mode in tcltk crate
- [ ] Implement proper command hiding (not just rename)
- [ ] Add proc tracking wrapper for better state detection
- [ ] Add variable traces for fine-grained tracking
- [ ] Custom loop wrappers that can be interrupted

**Estimated time:** 3-5 days

---

## 🎯 Lower Priority (Nice to Have)

### 10. Configuration Enhancements
- [ ] Add more config options
  - [ ] `command_prefix` - Default "tcl"
  - [ ] `admin_command_prefix` - Default "tclAdmin"
  - [ ] `max_message_length` - IRC message limit
  - [ ] `flood_protection` - Enable/disable
  - [ ] `owner` - Bot owner nick
- [ ] Per-channel configuration
- [ ] Hot reload configuration (SIGHUP)

### 11. Better Error Handling
- [ ] Propagate TCL errorInfo properly (partially done)
- [ ] Better error messages to users
- [ ] Log errors to file
- [ ] Handle network disconnections gracefully
- [ ] Reconnect logic for IRC

### 12. Resource Management
- [ ] Limit memory usage of TCL interpreter
- [ ] Limit recursion depth
- [ ] Clean up old state files (git gc)
- [ ] Garbage collection for cache buckets
- [ ] Rate limiting per user (not just per channel)

### 13. Additional Commands
- [ ] **dict** - Dictionary operations (TCL 8.5+ has built-in)
- [ ] **HTML entity encoding** - For encoding command
- [ ] **publish/meta/log** - Research original implementation

---

## 🧪 Testing & Deployment

### 14. Testing
- [ ] Unit tests
  - [ ] validator::validate_brackets tests (already has some)
  - [ ] Config parsing tests
  - [ ] HTTP rate limiter tests
  - [ ] State persistence tests
- [ ] Integration tests
  - [ ] TCL interpreter tests
  - [ ] IRC client tests (with mock server?)
  - [ ] End-to-end eval tests
- [ ] TCL script tests
  - [ ] Test all smeggdrop commands
  - [ ] Test state persistence
  - [ ] Test rollback
- [ ] CI/CD setup
  - [ ] GitHub Actions for tests
  - [ ] Automated builds

**Estimated time:** 1 week

### 15. Deployment
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

### 16. Documentation
- [ ] API documentation (rustdoc)
- [ ] User guide
  - [ ] How to install
  - [ ] How to configure
  - [ ] Available commands
  - [ ] Security best practices
- [ ] Development guide
  - [ ] Architecture overview
  - [ ] How to add new features
  - [ ] How to add TCL commands
- [ ] Migration guide from old bot

### 17. Observability
- [ ] Metrics
  - [ ] Number of evaluations
  - [ ] Evaluation duration
  - [ ] Error rate
  - [ ] HTTP requests
- [ ] Prometheus exporter
- [ ] Health check endpoint
- [ ] Admin commands (`!status`, `!stats`, `!reload`)

### 18. Security Enhancements
- [ ] Hostmask-based authentication (not just nick)
- [ ] NickServ integration for auth
- [ ] Channel modes integration (op/voice)
- [ ] Blacklist/whitelist for users
- [ ] Per-user rate limiting
- [ ] Sandboxing at OS level (seccomp, containers)

---

## 📊 Current Status Summary

**Core Functionality:** ✅ COMPLETE
- State persistence with git versioning
- Thread-based timeout (30s)
- HTTP commands with rate limiting
- Cache commands (key-value storage)
- Encoding commands (base64, URL)
- SHA1 hashing
- Utility commands

**Production Ready:** ~85%
- Missing: history/rollback commands, thread restart, channel tracking
- Everything else works and is tested in practice

**Timeline to Full Feature Parity:** ~1-2 weeks
- History/rollback: 1 day
- Thread restart: 1-2 days
- Channel tracking: 2-3 days
- IRC polish: 2-3 days
- Testing: 1 week

---

## References

**Files to study from original:**
- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/versioned_interpreter.tcl` - State persistence
- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/interpx.tcl` - Safe interpreter
- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/commands.tcl` - Command system
- `/home/user/old-tcl-evalbot/src/smeggdrop/smeggdrop/commands/*.tcl` - Individual commands
- `/home/user/old-tcl-evalbot/src/Carrion/Plugin/TCL.hs` - TCL plugin architecture
- `/home/user/old-tcl-evalbot/src/Carrion/Plugin/IO/IRC/Client.hs` - IRC features
