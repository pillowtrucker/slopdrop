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
memory_limit_mb = 0  # Default: 0 (disabled)
```

**Important Limitation - RLIMIT_AS affects entire process:**

The `memory_limit_mb` option uses `setrlimit(RLIMIT_AS)` which limits the **entire process's virtual address space**, not just the TCL thread. This has significant implications:

- Small values (e.g., 256 MB) will crash the entire bot on startup
- The process needs ~150+ MB just for Rust runtime, TCL interpreter, and signal handling
- When the limit is hit, the allocator calls `abort()` and kills the whole process
- The thread restart mechanism never gets a chance to run

**Recommended approach - Use systemd instead:**

For production deployments, use systemd's `MemoryMax` directive which properly handles memory limits with graceful OOM killing:

```ini
# /etc/systemd/system/slopdrop.service
[Service]
ExecStart=/path/to/slopdrop
MemoryMax=512M
MemorySwapMax=0
```

This allows the OOM killer to terminate and restart the service gracefully.

**If you must use memory_limit_mb:**
- Set values >= 1024 MB to account for process overhead
- Understand that hitting the limit will crash the entire bot, not just restart the TCL thread
- 0 = no limit (default, recommended unless using systemd)

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

**Note:** Due to RLIMIT_AS limitations, memory bombs will crash the entire bot (not just restart the thread). Use systemd for proper OOM handling.

```tcl
# This will consume memory until systemd kills the process
set x {}
while 1 {
    append x [string repeat "AAAAAAAA" 1000000]
}
```

With systemd (`MemoryMax=512M`, `Restart=on-failure`): Process killed and restarted automatically.

### Manual Timeout Test

```tcl
# This will timeout after 30s and trigger restart
while 1 { }
```

Expected output: `error: evaluation timed out after 30s (thread restarted)`

## Best Practices

### For Public Bots (Recommended - systemd)

Use systemd for memory protection:

```ini
# /etc/systemd/system/slopdrop.service
[Service]
ExecStart=/path/to/slopdrop
MemoryMax=512M
MemorySwapMax=0
Restart=on-failure
RestartSec=5
```

```toml
# config.toml
[security]
memory_limit_mb = 0      # Let systemd handle memory
eval_timeout_ms = 30000  # 30s max
```

### For Containers (Docker/Podman)

```bash
docker run --memory=512m --memory-swap=512m slopdrop
```

```toml
[security]
memory_limit_mb = 0      # Let container runtime handle memory
eval_timeout_ms = 30000
```

### For Development

```toml
[security]
memory_limit_mb = 0      # No limit
eval_timeout_ms = 120000 # 2 minutes for debugging
```

## Limitations

1. **RLIMIT_AS is process-wide**: The built-in `memory_limit_mb` option affects the entire process
   - Cannot limit just the TCL thread's memory
   - Small values crash the bot on startup
   - Use systemd/containers for proper memory isolation
   
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
