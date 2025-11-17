# Security Guide for Public IRC Deployment

This document outlines the security protections implemented in Slopdrop and best practices for deploying to public IRC channels.

## Table of Contents

1. [Overview](#overview)
2. [Multi-Layered Security Model](#multi-layered-security-model)
3. [Attack Vectors & Protections](#attack-vectors--protections)
4. [Configuration Best Practices](#configuration-best-practices)
5. [Monitoring & Incident Response](#monitoring--incident-response)
6. [Known Limitations](#known-limitations)

---

## Overview

Slopdrop is designed with **defense in depth**: multiple overlapping security layers protect against abuse even in untrusted, public IRC channels.

### Security Philosophy

- **Least privilege**: TCL interpreter has minimal capabilities
- **Resource limits**: Hard limits on memory, CPU time, network, and storage
- **Rate limiting**: Multi-layered throttling (per-eval, per-user, per-channel)
- **Input validation**: All external inputs (URLs, values) are validated
- **Output sanitization**: Error messages don't leak sensitive paths
- **Automatic recovery**: Bot restarts gracefully on crashes/timeouts

---

## Multi-Layered Security Model

### Layer 1: Execution Limits

**Purpose**: Prevent runaway code from consuming resources

- **Timeout**: 30 seconds (configurable via `eval_timeout_ms`)
- **Memory**: 256 MB (configurable via `memory_limit_mb`, Unix only)
- **Recursion**: 1000 levels (configurable via `max_recursion_depth`)
- **Auto-restart**: Thread automatically restarts on timeout/OOM with state reload

**Configuration**:
```toml
[security]
eval_timeout_ms = 30000
memory_limit_mb = 256
max_recursion_depth = 1000
```

**See**: `OOM_PROTECTION.md` for memory limit details

### Layer 2: HTTP/Network Protection

**Purpose**: Prevent SSRF, network scanning, and bandwidth exhaustion

#### URL Validation (SSRF Prevention)
- ✅ Only `http://` and `https://` allowed (no `file://`, `ftp://`, etc.)
- ✅ Blocks localhost/127.0.0.1/::1
- ✅ Blocks private IP ranges (10.x.x.x, 192.168.x.x, 172.16-31.x.x)
- ✅ Blocks link-local addresses (169.254.x.x - AWS metadata endpoint)
- ✅ Blocks IPv6 private/local (fc00::/7, fe80::/10)

#### Rate Limits
- **Per-eval**: 5 requests maximum per command
- **Per-channel**: 25 requests per 60 seconds
- **Per-user**: 10 requests per 60 seconds (NEW - prevents one user from DoS)
- **Cumulative transfer**: 500KB total per eval (prevents accumulation attacks)

#### Transfer Limits
- **Per-request**: 150KB maximum
- **POST body**: 150KB maximum
- **Redirects**: 5 maximum (prevents redirect loops)
- **Timeout**: 5 seconds per request

**Error Messages**:
- `only http:// and https:// URLs are allowed`
- `requests to localhost are not allowed`
- `requests to private IP addresses are not allowed`
- `you have made too many HTTP requests (max 10 requests per user in 60 seconds)`

**See**: `HTTP_LIMITS.md` for complete HTTP security details

### Layer 3: Cache Protection

**Purpose**: Prevent memory exhaustion via cache abuse

- **Max keys per bucket**: 1000
- **Max value size**: 100KB per entry
- **Max total size per bucket**: 1MB

**Implementation**: `tcl/cache.tcl`

**Error Messages**:
- `cache value exceeds maximum size (max 100000 bytes)`
- `cache bucket "X" has too many keys (max 1000 keys)`
- `cache bucket "X" exceeds total size limit (max 1000000 bytes)`

### Layer 4: Error Sanitization

**Purpose**: Prevent information disclosure via error messages

All error messages are sanitized to remove:
- Filesystem paths (`/home/user/...` → `[PATH]`)
- Windows paths (`C:\Users\...` → `[PATH]`)
- File URLs (`file:///...` → `[FILE-URL]`)

**Implementation**: `src/tcl_wrapper.rs::sanitize_error_message()`

Users see generic errors instead of sensitive paths:
```
TCL Error: can't read "foo": no such variable
    while executing
"set foo"
```

Instead of exposing paths like `/home/user/slopdrop/tcl/...`

### Layer 5: State Management

**Purpose**: Control repository growth and enable rollback

- **Git garbage collection**: Automatic `git gc --auto` every 100 commits
- **Rollback**: Admin command to revert to previous state
- **Commit tracking**: Every eval creates a git commit with author info

**Admin Commands**:
```
tclAdmin rollback <commit-hash>
```

**See**: `TODO.md` section 13 for resource management details

### Layer 6: Admin Authentication

**Purpose**: Protect privileged commands

- **Hostmask-based**: Admin commands require matching hostmask patterns
- **Configurable**: Define patterns in `config.toml`

**Configuration**:
```toml
[security]
privileged_users = [
    "admin!*@trusted.example.com",
    "alice!~alice@*.admin.net"
]
```

**Limitation**: Currently uses simple nick matching extracted from hostmasks. For production, consider:
- NickServ integration
- Certificate-based authentication (SASL EXTERNAL)
- Dedicated admin channel

### Layer 7: User Blacklist

**Purpose**: Block abusive users from running eval commands

- **Hostmask-based blocking**: Blacklist users by hostmask pattern
- **Runtime management**: Admin commands to add/remove blacklisted users
- **Persistent config**: Initial blacklist loaded from `config.toml`

**Configuration**:
```toml
[security]
blacklisted_users = [
    "spammer!*@*",
    "*!*@evil.example.com",
    "abuser!*@192.168.1.*"
]
```

**Admin Commands** (use `tclAdmin`):
```
tclAdmin blacklist list                    # Show all blacklisted hostmasks
tclAdmin blacklist add baduser!*@*         # Add hostmask to blacklist
tclAdmin blacklist remove baduser!*@*      # Remove hostmask from blacklist
```

**How it works**:
1. Initial blacklist loaded from `config.toml` at startup
2. Admin can add/remove entries at runtime (not saved to config)
3. Before evaluating any TCL code, user's hostmask is checked against blacklist
4. If match found, evaluation is denied with message: `error: you are blacklisted and cannot use this bot`

**Use cases**:
- Quickly ban persistent abusers
- Block entire hostnames (e.g., `*!*@spam.example.com`)
- Block IP ranges (e.g., `*!*@192.168.1.*`)
- Temporary bans (add at runtime, removed on bot restart)

**Important notes**:
- Runtime blacklist changes are NOT saved to config file
- To make blacklist permanent, add to `config.toml` and restart bot
- Blacklist check happens BEFORE TCL evaluation (no resource consumption)
- Uses same wildcard matching as admin hostmask patterns

---

## Attack Vectors & Protections

### 1. Resource Exhaustion

**Attack**: Infinite loops, memory allocation, deep recursion

**Protection**:
- Timeout (30s) with auto-restart
- Memory limit (256MB) via `setrlimit(RLIMIT_AS)` on Unix
- Recursion limit (1000 levels) via TCL's `interp recursionlimit`
- Thread-based isolation (hung thread doesn't block bot)

**Result**: Bot remains responsive, attacker gets error message

### 2. SSRF (Server-Side Request Forgery)

**Attack**: `http::get "http://localhost:6379/..."` to scan internal services

**Protection**:
- URL validation blocks localhost, private IPs, link-local addresses
- DNS resolution happens in TCL's http package (we validate the URL string)

**Limitation**: Does NOT protect against DNS rebinding (domain resolves to private IP after validation). Consider:
- Network-level firewall rules
- Running bot in isolated network namespace
- Using a filtering HTTP proxy

### 3. HTTP-Based DoS

**Attack**: Rapid-fire HTTP requests to exhaust bandwidth or flood targets

**Protection**:
- Per-user limit (10/minute) prevents one user from monopolizing
- Per-channel limit (25/minute) prevents coordinated attacks
- Per-eval limit (5) prevents loops
- Cumulative transfer limit (500KB/eval) prevents accumulation
- Timeout (5s/request) prevents hanging on slow responses

**Result**: Attacker hits rate limit, other users unaffected

### 4. State Vandalism

**Attack**: Define malicious procs, overwrite useful functions, pollute namespace

**Protection**:
- Git versioning tracks all changes with commit author
- Rollback capability (`tclAdmin rollback`) to revert vandalism
- Admin-only rollback prevents unauthorized reversion

**Limitation**:
- No automatic protection against vandalism
- Admins must monitor and manually rollback
- Consider: Read-only mode for untrusted channels
- Consider: Namespace isolation (separate state per channel)

**Mitigation**:
- Monitor git history (`history` command)
- Rollback quickly when vandalism detected
- Consider deploying separate bot instances per channel

### 5. Cache Pollution

**Attack**: Fill cache with garbage to exhaust memory

**Protection**:
- Max 1000 keys per bucket
- Max 100KB per value
- Max 1MB total per bucket

**Result**: Attacker hits limits, cache remains bounded

### 6. Path Disclosure

**Attack**: Trigger errors to reveal filesystem structure

**Protection**:
- Error sanitization replaces all paths with `[PATH]`
- Regex-based replacement handles Unix/Windows paths

**Result**: Users see generic errors without sensitive paths

### 7. Redirect Loops

**Attack**: `http::get "http://evil.com/loop"` where server redirects A→B→C→...→A

**Protection**:
- Max 5 redirects per request
- Uses TCL http package's `-maxredirects` parameter

**Result**: Request fails after 5th redirect with timeout

### 8. Nested HTTP Requests

**Attack**: `for {set i 0} {$i < 100} {incr i} {http::get ...}` to bypass per-request limits

**Protection**:
- Per-eval request count (max 5)
- Cumulative transfer limit (500KB total)
- Loop still subject to timeout (30s)

**Result**: Fails after 5th request or 500KB transferred

### 9. Social Engineering

**Attack**: Define proc named `http::get` that does something malicious

**Protection**:
- TCL namespace resolution (can shadow commands)
- Git history shows who defined what
- Rollback available

**Limitation**:
- No automatic prevention
- Relies on community oversight and admin intervention
- Consider: Protected namespace for critical commands

---

## Configuration Best Practices

### Production config.toml

```toml
[server]
hostname = "irc.example.net"
port = 6697
use_tls = true
nickname = "tclbot"
channels = ["#tcl"]

[security]
# Conservative timeout (30 seconds)
eval_timeout_ms = 30000

# Memory limit (Unix only, 256MB)
memory_limit_mb = 256

# Recursion limit (1000 levels)
max_recursion_depth = 1000

# Admin hostmasks (IMPORTANT: Use specific patterns!)
privileged_users = [
    "admin!*@admin.example.com",
    # Do NOT use wildcards like "*!*@*"
]

[tcl]
# Git-versioned state storage
state_path = "./state"

# Max lines in output (prevent flooding)
max_output_lines = 10

# Optional: Remote state repository
state_repo = "https://github.com/user/bot-state.git"

# Optional: SSH key for push authentication
ssh_key = "/path/to/ssh/key"
```

### Security Checklist

- [ ] Use TLS (`use_tls = true`) to protect IRC traffic
- [ ] Set conservative timeout (30s or less)
- [ ] Enable memory limits on Unix systems
- [ ] Use specific admin hostmasks (not `*!*@*`)
- [ ] Monitor git history regularly (`history` command)
- [ ] Keep backups of state directory
- [ ] Run bot as unprivileged user (not root)
- [ ] Consider network isolation (firewall, containers)
- [ ] Consider separate instances per channel
- [ ] Monitor for unusual activity (rapid evals, large commits)

### Network Isolation (Recommended)

**Option 1: Firewall Rules** (iptables/nftables)
```bash
# Allow outbound HTTPS to public internet only
iptables -A OUTPUT -p tcp --dport 443 -d 10.0.0.0/8 -j REJECT
iptables -A OUTPUT -p tcp --dport 443 -d 192.168.0.0/16 -j REJECT
iptables -A OUTPUT -p tcp --dport 443 -d 172.16.0.0/12 -j REJECT
iptables -A OUTPUT -p tcp --dport 443 -d 127.0.0.0/8 -j REJECT
iptables -A OUTPUT -p tcp --dport 443 -j ACCEPT

# Block outbound HTTP to private ranges
iptables -A OUTPUT -p tcp --dport 80 -d 10.0.0.0/8 -j REJECT
# ... (repeat for other private ranges)
```

**Option 2: Network Namespaces** (Linux)
```bash
# Create isolated network namespace
ip netns add tclbot
ip netns exec tclbot ./slopdrop config.toml
```

**Option 3: Docker** (with network restrictions)
```dockerfile
# Dockerfile with USER directive (don't run as root)
FROM rust:1.70
WORKDIR /app
COPY . .
RUN cargo build --release
USER 1000:1000
CMD ["./target/release/slopdrop", "config.toml"]
```

### Monitoring

**Key Metrics to Watch**:
1. Eval frequency (evals per minute)
2. Error rate (errors per eval)
3. HTTP request rate (requests per minute)
4. Memory usage (TCL thread size)
5. Git repository size (disk usage)
6. Failed admin auth attempts

**Logging**:
- Enable `tracing` output to monitor activity
- Log all admin commands
- Alert on rollback usage
- Monitor for repeated errors from same user

**Git History Monitoring**:
```bash
# Check recent commits
cd state && git log --oneline -20

# Check commits by specific user
git log --author="suspicious_nick" --oneline

# Check file changes
git log --stat
```

---

## Known Limitations

### 1. DNS Rebinding

**Issue**: URL validation checks the hostname string, but DNS resolution happens later in TCL's http package. An attacker could use a malicious DNS server that initially resolves to a public IP (passes validation) then later resolves to a private IP (bypasses protection).

**Mitigation**:
- Network-level firewall rules
- DNS filtering/validation
- HTTP proxy with SSRF protection

**Risk**: Low (requires DNS control, complex attack)

### 2. State Vandalism

**Issue**: No automatic protection against malicious proc definitions. Users can overwrite useful functions or create harmful code.

**Mitigation**:
- Git history tracking (all changes logged with author)
- Rollback capability
- Admin monitoring
- Community oversight

**Risk**: Medium (requires manual intervention)

**Future**: Consider read-only mode or namespace isolation

### 3. Admin Authentication

**Issue**: Simple nick-based matching from hostmasks. No NickServ integration or certificate-based auth.

**Mitigation**:
- Use specific hostmask patterns
- Dedicated admin channel
- Monitor admin command usage

**Risk**: Low-Medium (depends on IRC network security)

**Future**: NickServ integration, SASL EXTERNAL

### 4. Cache/State Growth

**Issue**: Cache buckets limited per-bucket, but unlimited number of buckets. State repository grows with every commit.

**Mitigation**:
- Git GC runs automatically every 100 commits
- Admins can manually clean state directory
- Monitor disk usage

**Risk**: Low (gc handles most growth)

**Future**: Global cache size limit, state pruning

### 5. Encoding Attacks

**Issue**: Unicode/encoding edge cases might bypass filters.

**Mitigation**:
- TCL handles UTF-8 natively
- URL validation uses string matching (not regex)
- IRC formatting stripped on input

**Risk**: Very Low

### 6. Zip Bomb / Decompression

**Issue**: If TCL gains decompression capability, compressed payloads could expand beyond limits.

**Mitigation**:
- Memory limit (256MB) caps expansion
- Currently no built-in compression support

**Risk**: Very Low (hypothetical)

### 7. Time-of-Check Time-of-Use (TOCTOU)

**Issue**: URL could redirect to private IP after validation but before request.

**Mitigation**:
- Redirect limit (5 max)
- Timeout (5s total)
- URL validation on initial request

**Risk**: Low (limited by redirect count and timeout)

---

## Incident Response Playbook

### Scenario 1: Malicious Proc Detected

**Detection**: User reports broken functionality, git log shows suspicious commit

**Response**:
1. Check git history: `history 20`
2. Identify malicious commit hash
3. Rollback: `tclAdmin rollback <hash>`
4. Restart bot to reload state
5. Consider banning offending user (IRC-level)

### Scenario 2: HTTP Flood or Persistent Abuse

**Detection**: Bot becomes unresponsive, logs show HTTP rate limit errors, or user repeatedly abusing bot

**Response**:
1. Check if single user is responsible (logs will show nick and hostmask)
2. User hitting per-user limit (10/min) - self-limiting, no action needed
3. **Quick ban**: `tclAdmin blacklist add user!*@host` to block user immediately
4. Coordinated attack hitting per-channel limit (25/min) - wait for rate limit window to expire
5. If persistent, consider IRC-level measures (quiet, kick, ban)
6. Add permanent ban to `config.toml` blacklist if needed

**Blacklist commands**:
```
tclAdmin blacklist list                  # See current blacklist
tclAdmin blacklist add baduser!*@*       # Ban user
tclAdmin blacklist remove baduser!*@*    # Unban user
```

### Scenario 3: OOM/Crash Loop

**Detection**: Bot repeatedly restarts with "out of memory" errors

**Response**:
1. Check memory_limit_mb (maybe too low for workload)
2. Check recent commits for memory-intensive procs
3. Rollback to known-good state
4. Increase memory_limit_mb if legitimate usage
5. Investigate proc causing OOM

### Scenario 4: SSRF Attempt

**Detection**: Logs show "requests to localhost are not allowed" errors

**Response**:
1. Note the user attempting SSRF
2. Protection is working - no action needed
3. If repeated attempts, consider warning or ban
4. Verify network-level firewall rules are in place

### Scenario 5: State Repository Full

**Detection**: Disk full errors, git operations failing

**Response**:
1. Check repository size: `du -sh state/`
2. Run manual GC: `cd state && git gc --aggressive`
3. If still large, consider pruning old commits (careful!)
4. Increase disk quota
5. Verify automatic gc is working (runs every 100 commits)

---

## Testing Your Deployment

Before going fully public, test with these scenarios:

### 1. Resource Limits
```tcl
# Should timeout after 30s
while {1} {}

# Should hit memory limit (if on Unix)
set biglist {}
while {1} {lappend biglist [string repeat "x" 1000000]}

# Should hit recursion limit
proc recurse {} {recurse}
recurse
```

### 2. HTTP Protections
```tcl
# Should block localhost
http::get "http://localhost:8080"

# Should block private IP
http::get "http://192.168.1.1"

# Should block link-local
http::get "http://169.254.169.254/latest/meta-data"

# Should hit per-eval limit after 5 requests
for {set i 0} {$i < 10} {incr i} {
    http::get "http://example.com"
}

# Should hit per-user limit after 10 requests in 60s
# (run multiple evals rapidly)
```

### 3. Cache Limits
```tcl
# Should hit value size limit
cache::put test key [string repeat "x" 200000]

# Should hit key count limit
for {set i 0} {$i < 2000} {incr i} {
    cache::put test $i "value"
}
```

### 4. Error Sanitization
```tcl
# Error should NOT show filesystem paths
source /nonexistent/file.tcl

# Should show [PATH] instead
```

### 5. Admin Commands
```tcl
# As non-admin, should fail
tclAdmin rollback abc123

# As admin, should succeed
tclAdmin rollback <valid-hash>
```

---

## Conclusion

Slopdrop implements defense-in-depth security suitable for public IRC deployment. The multi-layered approach ensures that even if one protection fails, others will contain the damage.

**Key Takeaways**:
- Resource limits prevent DoS
- HTTP validation prevents SSRF
- Rate limiting prevents abuse
- Error sanitization prevents info disclosure
- Git versioning enables rollback
- Automatic recovery maintains availability

**Remaining Risks**:
- State vandalism requires manual intervention
- DNS rebinding (mitigate with network firewall)
- Admin auth could be strengthened (NickServ)

**Recommended Additional Protections**:
- Network-level firewall rules
- Separate bot instances per channel
- Regular state backups
- Monitoring and alerting

For questions or security reports, see `README.md`.
