# HTTP Security Limits

The TCL HTTP client (`http::get`, `http::post`, `http::head`) implements multiple layers of security limits to prevent resource exhaustion and abuse.

## Overview

HTTP operations are among the most resource-intensive features available to users. To prevent abuse, the bot implements a comprehensive multi-layered protection system.

## Limit Layers

### 1. Per-Request Limits

**Transfer Size**: 150KB per individual request
- `http::get`: Response body limited to 150KB
- `http::post`: Response body limited to 150KB (request body has separate limit)
- `http::head`: Headers only (typically < 1KB)

**POST Body Size**: 150KB maximum
- Request body for POST operations limited separately
- Prevents large upload attempts

**Timeout**: 5 seconds per request
- Prevents hanging on slow servers
- Applies to entire request lifecycle including redirects

**Redirects**: Maximum 5 redirects per request
- Prevents redirect loops and chains
- Uses TCL's built-in `-maxredirects` parameter

### 2. Per-Eval Limits

**Request Count**: 5 HTTP requests maximum per eval
- Applies across all HTTP operations (GET/POST/HEAD combined)
- Prevents request flooding within single command

**Cumulative Transfer**: 500KB total per eval (NEW)
- Tracks combined bytes from ALL requests in single eval
- Includes both request and response bodies
- Example: 3x GET @ 100KB each + 2x POST @ 50KB each = 400KB total (allowed)
- Example: 5x GET @ 150KB each = 750KB total (denied after ~3 requests)

### 3. Per-Minute Limits (Per Channel)

**Request Count**: 25 HTTP requests per 60 seconds
- Tracked per IRC channel
- Prevents sustained request flooding
- Independent of per-eval limit

### 4. Global Limits

**Concurrent Connections**: Limited by TCL interpreter thread pool
- Only one eval runs at a time per interpreter thread
- Automatic thread restart on timeout/crash provides isolation

## How Limits Are Enforced

### Pre-Request Checking

Before making any HTTP request, `check_limits()` validates:
1. Per-eval request count (< 5)
2. Per-minute request count (< 25)
3. Cumulative transfer estimate (< 500KB remaining)

If any limit is exceeded, the request is denied with an error message.

### Post-Request Accounting

After successful request, `record_request()` tracks:
1. Request timestamp (for per-minute limit)
2. Actual bytes transferred (for cumulative limit)

Byte counting is accurate:
- **GET**: Response body size
- **POST**: Request body + response body
- **HEAD**: Estimated header size (key + value + separators)

### Automatic Cleanup

Old tracking data is automatically cleaned:
- Request history older than 60 seconds is discarded
- Byte counters from previous evals are removed
- Only current eval's data is retained

## Security Benefits

1. **Prevents Redirect Loops**: Max 5 redirects stops infinite redirect chains
2. **Prevents Accumulation Attacks**: Can't bypass per-request limits by making many small requests
3. **Prevents Request Flooding**: Per-eval and per-minute limits stop rapid-fire requests
4. **Prevents Large Transfers**: Multiple overlapping limits ensure bounded resource usage
5. **Channel Isolation**: Per-channel tracking prevents one channel from affecting others

## Example Scenarios

### Allowed Usage

```tcl
# Single large request (under 150KB)
set data [lindex [http::get "http://example.com/data"] 2]

# Multiple small requests (5 × 50KB = 250KB total, under 500KB)
foreach url $url_list {
    set result [http::get $url]
    # ... process ...
}
```

### Denied Usage

```tcl
# Too many requests in one eval
for {set i 0} {$i < 10} {incr i} {
    http::get "http://example.com/page$i"  # Fails after 5th request
}

# Cumulative transfer too large
for {set i 0} {$i < 5} {incr i} {
    http::get "http://example.com/large-file"  # Each 150KB, fails after ~3rd (450KB)
}

# Redirect loop (server redirects A → B → C → D → E → F → ...)
http::get "http://evil.com/redirect-loop"  # Fails after 5 redirects
```

## Error Messages

Users see clear error messages when limits are hit:

- **Too many requests per eval**: `too many HTTP requests in this eval (max 5 requests)`
- **Too many requests per minute**: `too many HTTP requests (max 25 requests in 60 seconds)`
- **Cumulative transfer exceeded**: `total transfer limit exceeded for this eval (max 500000 bytes, have 400000, trying 150000)`
- **Per-request transfer exceeded**: `transfer exceeded 150000 bytes`
- **POST body too large**: `post body exceeds 150000 bytes`
- **Timeout**: `HTTP request failed: timeout`

## Configuration

Current limits are hardcoded in `tcl/http.tcl`:

```tcl
variable requests_per_eval 5           # Max requests per eval
variable requests_per_minute 25        # Max requests per 60 seconds
variable request_interval 60           # Time window for rate limiting (seconds)
variable post_limit 150000             # Max POST body size (bytes)
variable transfer_limit 150000         # Max per-request transfer (bytes)
variable transfer_limit_per_eval 500000 # Max cumulative transfer per eval (bytes)
variable max_redirects 5               # Max redirects per request
variable time_limit 5000               # Request timeout (milliseconds)
```

To modify limits, edit these values in `tcl/http.tcl` and restart the bot.

## Implementation Notes

- Uses TCL's built-in `http` package with custom wrappers
- Leverages `-maxredirects`, `-timeout`, `-blocksize` parameters
- Channel tracking via `$::nick_channel` global variable
- Per-eval tracking via incrementing `$eval_count`
- Thread-safe (each TCL thread has independent state)

## Related Documentation

- [OOM Protection](OOM_PROTECTION.md) - Memory and recursion limits
- [TODO](TODO.md) - Feature roadmap and status
