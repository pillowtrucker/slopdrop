# Out-of-Memory (OOM) Protection

This document describes the comprehensive memory protection mechanisms implemented in slopdrop.

## Overview

Slopdrop protects against three types of resource exhaustion:
1. **Infinite loops** - Handled by timeout mechanism
2. **Memory bombs** - Handled by OS-level memory limits
3. **Thread crashes/panics** - Handled by automatic restart

## Features

### 1. Memory Limits (Unix only)

**Configuration:**
```toml
[security]
memory_limit_mb = 256  # Default: 256 MB
```

**How it works:**
- Uses `setrlimit(RLIMIT_AS)` to cap virtual memory per TCL thread
- Set when TCL thread spawns  
- 0 = no limit (not recommended for public bots)
- Limit applies to entire thread (interpreter + all allocations)

**What happens on OOM:**
- Thread receives SIGKILL from OS
- Channel closes, triggering detection in main thread
- Thread automatically restarts
- State reloaded from git
- User sees: `"error: thread crashed (likely out of memory), restarted"`

### 2. Timeout Protection

**Configuration:**
```toml
[security]
eval_timeout_ms = 30000  # Default: 30 seconds
```

**How it works:**
- Each evaluation wrapped in `tokio::time::timeout()`
- If eval doesn't complete within timeout, thread is assumed hung
- Old thread abandoned, new thread spawned
- State reloaded from git

**What happens on timeout:**
- User sees: `"error: evaluation timed out after 30s (thread restarted)"`
- All pending TCL state lost (intentional - it's hung)
- Next evaluation starts fresh

### 3. Crash Recovery

**Automatic detection:**
- mpsc channel closing = thread died
- oneshot response channel closing = thread died mid-eval

**Recovery process:**
1. Detect channel closure
2. Log error
3. Spawn fresh TCL thread  
4. Set memory limits on new thread
5. Reload state from git
6. Return error to user

## Implementation Details

### Thread Lifecycle

```rust
// Initial spawn
TclThreadHandle::spawn() 
  -> thread::spawn()
    -> set_memory_limit()  // <-- Memory limit applied
    -> TclThreadWorker::new()
    -> worker.run()

// Auto-restart on crash/timeout
TclThreadHandle::restart()
  -> drop old thread handle
  -> spawn new thread
    -> set_memory_limit()  // <-- Memory limit reapplied
    -> load state from git
```

### Error Messages

| Scenario | Message |
|----------|---------|
| Timeout | `error: evaluation timed out after 30s (thread restarted)` |
| OOM/Crash (send fails) | `error: thread crashed (likely out of memory), restarted` |
| OOM/Crash (response channel closes) | `error: thread died unexpectedly (likely out of memory), restarted` |
| Restart fails | `error: thread crashed and failed to restart: {err}` |

### Platform Support

| Feature | Linux | macOS | Windows |
|---------|-------|-------|---------|
| Memory limits | ✅ | ✅ | ❌ |
| Timeout | ✅ | ✅ | ✅ |
| Crash recovery | ✅ | ✅ | ✅ |

**Note:** On Windows, memory limits are disabled with a warning. Consider using WSL for production deployments.

## Testing

### Manual OOM Test

```tcl
# This will hit memory limit and trigger restart (Unix only)
set x {}
while 1 {
    append x [string repeat "AAAAAAAA" 1000000]
}
```

Expected output: `error: thread crashed (likely out of memory), restarted`

### Manual Timeout Test

```tcl
# This will timeout after 30s and trigger restart
while 1 { }
```

Expected output: `error: evaluation timed out after 30s (thread restarted)`

## Best Practices

### For Public Bots

```toml
[security]
memory_limit_mb = 256    # Strict limit
eval_timeout_ms = 30000  # 30s max
```

### For Private/Trusted Channels

```toml
[security]
memory_limit_mb = 512    # More permissive
eval_timeout_ms = 60000  # 60s for complex scripts
```

### For Development

```toml
[security]
memory_limit_mb = 0      # No limit (careful!)
eval_timeout_ms = 120000 # 2 minutes for debugging
```

## Limitations

1. **Memory limit granularity**: OS enforces limit, not interpreter
   - Can't limit individual TCL commands
   - Entire thread shares the limit
   
2. **State loss on crash**: Intentional design
   - Crashed state is corrupt/untrusted
   - Always reload from last git commit
   - Unsaved changes lost (by design)

3. **No cross-eval quotas**: Each eval is independent
   - User can trigger 10 crashes in a row
   - Consider rate limiting at IRC/frontend level

## Related

- See `src/tcl_thread.rs` for implementation
- See `src/config.rs` for configuration schema
- See `TESTING_GUIDE.md` for testing procedures
