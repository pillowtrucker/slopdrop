# Link Auto-Resolver

The link auto-resolver automatically detects and resolves URLs posted in IRC channels, providing rich metadata like titles, view counts, and other information. It includes an extensible API that allows you to create custom resolvers for specific sites like YouTube, Bluesky, Twitter, Reddit, and GitHub.

## Features

- **Automatic URL Detection**: Detects http:// and https:// URLs in channel messages
- **Smart Command Detection**: Skips URL resolution in TCL commands (messages starting with `tcl` or `tclAdmin`)
- **Extensible Resolver API**: Register custom resolvers for specific domains/patterns
- **Built-in Caching**: Prevents re-fetching the same URLs (1-hour cache by default)
- **Priority System**: Control which resolver runs first
- **Default HTML Title Extraction**: Falls back to extracting `<title>` tags for unknown sites
- **Rate Limiting**: Respects existing HTTP rate limits (5 requests per eval, 25 per minute per channel)
- **Security**: Uses existing SSRF protection and URL validation

## Quick Start

### Enable Auto-Resolution

```tcl
linkresolver enable
```

This will automatically start resolving links posted in channels.

### Disable Auto-Resolution

```tcl
linkresolver disable
```

## Persistence

**Custom resolvers are automatically persisted** and will survive bot restarts. When you register a custom resolver, it's stored in the bot's git-backed state and will be restored when the bot restarts.

**Built-in resolvers are NOT persisted** - they're registered fresh on each bot startup from `tcl/linkresolver_examples.tcl`. This keeps your git state clean and makes it easy to update built-in resolvers by editing the file.

### What Gets Persisted

✅ **Persisted (saved to git):**
- Custom resolvers you register via `linkresolver register`
- The resolver pattern, procedure name, and priority

❌ **NOT Persisted (transient):**
- Built-in resolvers (YouTube, Bluesky, etc.)
- Enable/disable state (defaults to enabled if configured in examples file)
- Cached URL results (cache is in-memory only)
- The resolver procedure code itself (must be defined in your state as a normal proc)

### Example

```tcl
# Define a custom resolver procedure (this gets persisted as a normal proc)
proc my_site_resolver {url nick channel} {
    return "My Site: $url"
}

# Register it (this registration gets persisted)
linkresolver register {mysite\.com} my_site_resolver 20

# After bot restart, both the proc and registration are restored!
```

**Note:** The resolver procedure itself must exist as a persisted proc in your bot's state. The link resolver only persists the *registration* (which pattern maps to which proc), not the procedure code.

## Built-in Example Resolvers

The following resolvers are included and can be enabled in `tcl/linkresolver_examples.tcl`:

### YouTube
Extracts video title, duration, and view count:
```
▶ YouTube: Amazing Video Title [3:45] (1.2M views)
```

### Bluesky
Shows author and post content:
```
🦋 Bluesky - username: This is the content of the post
```

### Twitter/X
Shows author and tweet text:
```
🐦 Twitter - @username: Tweet content goes here
```

### Reddit
Shows subreddit, post title, and score:
```
🔴 r/programming: Interesting post title (1.5K points)
```

### GitHub
Shows repository or issue/PR information:
```
🐙 GitHub Issue #123: Bug report title
```

## Custom Resolver API

### Register a Custom Resolver

```tcl
linkresolver register <pattern> <proc> [priority]
```

**Parameters:**
- `pattern`: Regular expression to match URLs (e.g., `{youtube\.com|youtu\.be}`)
- `proc`: Name of the procedure to call when pattern matches
- `priority`: Optional priority (default: 50, lower = higher priority)

**Example:**

```tcl
# Define a custom resolver procedure
proc my_youtube_resolver {url nick channel} {
    # Your resolution logic here
    # Return a string to post to channel, or "" to post nothing

    # Check cache first (recommended)
    set cached [::linkresolver::get_cached $url]
    if {$cached ne ""} {
        return $cached
    }

    # Fetch and parse the URL
    if {[catch {http get $url} content]} {
        return ""
    }

    # Extract information
    # ... your parsing logic ...

    set result "Your formatted result"

    # Cache the result (recommended)
    ::linkresolver::set_cached $url $result

    return $result
}

# Register the resolver
linkresolver register {youtube\.com/watch|youtu\.be/} my_youtube_resolver 10
```

**Resolver Procedure Signature:**
```tcl
proc resolver_name {url nick channel} {
    # url: The full URL that matched the pattern
    # nick: Nickname of user who posted the link
    # channel: Channel where link was posted

    # Return: String to post to channel, or "" to post nothing
}
```

### Unregister a Resolver

```tcl
linkresolver unregister <pattern>
```

### List Registered Resolvers

```tcl
linkresolver list
```

Shows all registered resolvers sorted by priority.

### Test a Resolver

```tcl
linkresolver test <url>
```

Test URL resolution without posting to a channel (useful for debugging).

## Helper Functions

The following helper functions are available for use in custom resolvers:

### Caching

```tcl
# Check if URL is in cache (returns "" if not found or expired)
set cached [::linkresolver::get_cached $url]

# Store result in cache (expires in 1 hour)
::linkresolver::set_cached $url $result
```

### HTML Entity Decoding

```tcl
# Decode HTML entities (&amp; &lt; &#39; etc.)
set decoded [::linkresolver::decode_html_entities $html_text]
```

### Number Formatting

```tcl
# Format large numbers (1500000 -> "1M", 1500 -> "1K")
set formatted [::linkresolver::format_number 1500000]
# Returns: "1M"
```

## Configuration

Edit `tcl/linkresolver_examples.tcl` to enable/disable specific resolvers:

```tcl
# Enable YouTube and Bluesky resolvers
::linkresolver::register {youtube\.com/watch|youtu\.be/} ::linkresolver::youtube_resolver 10
::linkresolver::register {bsky\.app/profile/.*/(post|feed)} ::linkresolver::bluesky_resolver 10

# Disabled by default (uncomment to enable):
# ::linkresolver::register {(twitter\.com|x\.com)/.*/(status|statuses)/} ::linkresolver::twitter_resolver 10
# ::linkresolver::register {reddit\.com/r/[^/]+/comments/} ::linkresolver::reddit_resolver 10
# ::linkresolver::register {github\.com/[^/]+/[^/]+} ::linkresolver::github_resolver 10

# Auto-enable on startup (comment out for manual control)
::linkresolver::enable
```

You can also adjust these variables in `tcl/linkresolver.tcl`:

```tcl
variable cache_expiry 3600          ;# Cache expiry in seconds (default: 1 hour)
variable max_title_length 200       ;# Maximum length of result messages
variable auto_resolve_enabled 0     ;# Auto-enable on init (0=no, 1=yes)
```

## Example: Custom Resolver for Wikipedia

```tcl
proc wikipedia_resolver {url nick channel} {
    # Check cache
    set cached [::linkresolver::get_cached $url]
    if {$cached ne ""} {
        return $cached
    }

    # Fetch the page
    if {[catch {http get $url} content]} {
        return ""
    }

    # Extract title and first paragraph
    set title ""
    set summary ""

    if {[regexp -nocase {<title>([^<]+)</title>} $content -> raw_title]} {
        # Wikipedia titles end with " - Wikipedia"
        set title [regsub { - Wikipedia$} $raw_title ""]
        set title [::linkresolver::decode_html_entities $title]
    }

    # Extract first paragraph from meta description
    if {[regexp -nocase {<meta property="og:description" content="([^"]+)"} $content -> desc]} {
        set summary [::linkresolver::decode_html_entities $desc]
        # Truncate to reasonable length
        if {[string length $summary] > 150} {
            set summary "[string range $summary 0 146]..."
        }
    }

    if {$title ne ""} {
        set result "📖 Wikipedia: $title"
        if {$summary ne ""} {
            append result " - $summary"
        }

        # Cache the result
        ::linkresolver::set_cached $url $result
        return $result
    }

    return ""
}

# Register with high priority
linkresolver register {wikipedia\.org/wiki/} wikipedia_resolver 5
```

## How It Works

1. **URL Detection**: The linkresolver binds to TEXT events and uses regex to extract URLs
2. **Command Filtering**: Skips processing if the message starts with `tcl` or `tclAdmin` (case insensitive)
3. **Pattern Matching**: Each URL is tested against registered resolver patterns in priority order
4. **Resolution**: The matching resolver is called with the URL, nick, and channel
5. **Caching**: Results are cached by URL hash to avoid redundant fetches
6. **Output**: Non-empty results are returned to the trigger system, which sends them to the channel

## Limitations

- Maximum 2 URLs resolved per message (to prevent spam)
- Rate limits apply (5 HTTP requests per eval, 25 per minute per channel)
- Some sites may block bot user-agents or require JavaScript
- Cache expires after 1 hour by default
- SSRF protection prevents fetching localhost/private IPs

## Troubleshooting

### Links aren't being resolved

```tcl
# Check if resolver is enabled
linkresolver list

# Enable it if needed
linkresolver enable
```

### Custom resolver not working

```tcl
# Test your resolver
linkresolver test https://example.com/your-url

# Check if it's registered
linkresolver list

# Verify your pattern matches
# Use a TCL regex tester or the test command
```

### Rate limit errors

The linkresolver respects existing HTTP rate limits. If you hit rate limits:
- Wait a minute for the rate limit window to reset
- Reduce the number of links being posted
- Increase cache expiry to reduce refetches

### Getting HTML instead of rich content

Some sites require:
- Specific user agents (modify the HTTP client if needed)
- JavaScript execution (not supported - use their API instead)
- Authentication (implement in your custom resolver)

For sites with APIs (YouTube, Reddit, etc.), consider using their API endpoints instead of HTML scraping for more reliable results.

## Advanced: Using External APIs

For sites with REST APIs, you can fetch JSON data instead of parsing HTML:

```tcl
proc github_api_resolver {url nick channel} {
    # Extract owner/repo from URL
    if {![regexp {github\.com/([^/]+)/([^/]+)} $url -> owner repo]} {
        return ""
    }

    # Use GitHub API
    set api_url "https://api.github.com/repos/$owner/$repo"

    if {[catch {http get $api_url} json]} {
        return ""
    }

    # Parse JSON (basic extraction)
    if {[regexp {"description":\s*"([^"]+)"} $json -> description]} {
        if {[regexp {"stargazers_count":\s*(\d+)} $json -> stars]} {
            set stars_fmt [::linkresolver::format_number $stars]
            return "🐙 GitHub: $owner/$repo - $description (⭐ $stars_fmt)"
        }
    }

    return ""
}
```

## Integration with User Commands

You can create convenience commands:

```tcl
# Create a command to manually resolve the last link
proc resolve_last {} {
    set url [lastlink]
    if {$url eq ""} {
        return "No recent links found"
    }

    set result [::linkresolver::test $url]
    if {$result eq ""} {
        return "Could not resolve: $url"
    }
    return $result
}
```

## Security Considerations

- All HTTP requests go through the existing security layer with SSRF protection
- Private IPs, localhost, and link-local addresses are blocked
- Rate limiting prevents abuse
- Timeout protection applies to all HTTP fetches
- Custom resolvers run in the sandboxed TCL environment

## Contributing Resolvers

If you create a useful resolver, consider:
1. Adding it to `linkresolver_examples.tcl`
2. Documenting the API endpoints used
3. Handling errors gracefully
4. Respecting the site's robots.txt and ToS
5. Using caching to minimize requests

---

**See also:**
- `tcl/linkresolver.tcl` - Core resolver implementation
- `tcl/linkresolver_examples.tcl` - Example resolvers
- `tcl/http.tcl` - HTTP client documentation
- `tcl/cache.tcl` - Cache system documentation
