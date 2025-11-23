# Link Auto-Resolver Module
# Automatically resolves links posted in channels with extensible resolver API

namespace eval ::linkresolver {
    # Configuration variables
    variable enabled 0
    variable resolvers {}
    variable cache_expiry 3600 ;# 1 hour cache
    variable max_title_length 200
    variable auto_resolve_enabled 0

    # Initialize the link resolver
    proc init {} {
        variable enabled
        variable auto_resolve_enabled

        # Register default resolvers
        register_builtin_resolvers

        # Auto-enable if configured
        if {$auto_resolve_enabled} {
            enable
        }
    }

    # Enable automatic link resolution
    proc enable {} {
        variable enabled
        if {$enabled} {
            return "Link resolver already enabled"
        }

        # Bind to TEXT events to catch all messages with links
        bind TEXT * ::linkresolver::on_text
        set enabled 1
        return "Link resolver enabled - will auto-resolve URLs posted in channels"
    }

    # Disable automatic link resolution
    proc disable {} {
        variable enabled
        if {!$enabled} {
            return "Link resolver already disabled"
        }

        # Unbind from TEXT events
        unbind TEXT * ::linkresolver::on_text
        set enabled 0
        return "Link resolver disabled"
    }

    # Register a custom resolver
    # pattern: regex pattern to match URLs (e.g., {youtube\.com|youtu\.be})
    # proc: procedure name to call with (url, nick, channel)
    # priority: lower numbers = higher priority (default: 50)
    proc register {pattern proc_name {priority 50}} {
        variable resolvers

        # Validate that the proc exists (check both global and namespaced procs)
        set proc_exists 0
        if {[llength [info procs $proc_name]]} {
            set proc_exists 1
        } elseif {[string match "::*" $proc_name] && [llength [info procs $proc_name]]} {
            set proc_exists 1
        } elseif {[catch {namespace which -command $proc_name} result] == 0 && $result ne ""} {
            set proc_exists 1
        }

        if {!$proc_exists} {
            return -code error "Procedure $proc_name does not exist"
        }

        # Check if pattern already registered
        set idx [lsearch -index 0 $resolvers $pattern]
        if {$idx >= 0} {
            # Update existing resolver
            lset resolvers $idx [list $pattern $proc_name $priority]
            set resolvers [lsort -integer -index 2 $resolvers]
            return "Updated resolver for pattern: $pattern"
        }

        # Add new resolver and sort by priority
        lappend resolvers [list $pattern $proc_name $priority]
        set resolvers [lsort -integer -index 2 $resolvers]
        return "Registered resolver for pattern: $pattern (priority: $priority)"
    }

    # Unregister a custom resolver
    proc unregister {pattern} {
        variable resolvers
        set idx [lsearch -exact -index 0 $resolvers $pattern]
        if {$idx < 0} {
            return -code error "No resolver registered for pattern: $pattern"
        }
        set resolvers [lreplace $resolvers $idx $idx]
        return "Unregistered resolver for pattern: $pattern"
    }

    # List all registered resolvers
    proc list_resolvers {} {
        variable resolvers
        if {[llength $resolvers] == 0} {
            return "No custom resolvers registered"
        }

        set result "Registered resolvers (by priority):\n"
        foreach resolver $resolvers {
            lassign $resolver pattern proc_name priority
            append result "  \[$priority\] $pattern -> $proc_name\n"
        }
        return $result
    }

    # Extract URLs from text
    proc extract_urls {text} {
        set urls {}
        # Match http:// and https:// URLs
        set pattern {https?://[^\s\)\]<>"']+}
        set matches [regexp -all -inline $pattern $text]
        foreach url $matches {
            # Clean up trailing punctuation that's likely not part of URL
            regsub {[.,;:!?]+$} $url {} url
            lappend urls $url
        }
        return $urls
    }

    # Get cache key for URL
    # Note: Using URL encoding instead of SHA1 to avoid dependency
    proc get_cache_key {url} {
        # Simple URL encoding for cache key
        return "linkresolver:url:[url_encode $url]"
    }

    # Check if URL is in cache
    proc get_cached {url} {
        variable cache_expiry
        set key [get_cache_key $url]

        if {[catch {cache get default $key} result]} {
            return ""
        }

        # Check expiry
        lassign $result timestamp data
        if {[clock seconds] - $timestamp > $cache_expiry} {
            cache delete default $key
            return ""
        }

        return $data
    }

    # Store URL result in cache
    proc set_cached {url data} {
        set key [get_cache_key $url]
        set value [list [clock seconds] $data]
        cache put default $key $value
    }

    # Find matching resolver for URL
    proc find_resolver {url} {
        variable resolvers

        foreach resolver $resolvers {
            lassign $resolver pattern proc_name priority
            if {[regexp $pattern $url]} {
                return $proc_name
            }
        }

        # No custom resolver found, use default
        return "::linkresolver::default_resolver"
    }

    # Default resolver: extract title from HTML
    proc default_resolver {url nick channel} {
        variable max_title_length

        # Check cache first
        set cached [get_cached $url]
        if {$cached ne ""} {
            return $cached
        }

        # Fetch the URL
        if {[catch {http get $url} content]} {
            return ""
        }

        # Extract title from HTML
        set title ""
        if {[regexp -nocase {<title[^>]*>([^<]+)</title>} $content -> title]} {
            # Decode HTML entities
            set title [decode_html_entities $title]
            # Clean up whitespace
            set title [regsub -all {\s+} [string trim $title] " "]
            # Truncate if too long
            if {[string length $title] > $max_title_length} {
                set title "[string range $title 0 [expr {$max_title_length - 4}]]..."
            }

            if {$title ne ""} {
                set result "Title: $title"
                set_cached $url $result
                return $result
            }
        }

        return ""
    }

    # Decode common HTML entities
    proc decode_html_entities {text} {
        set entities {
            &quot; "\"" &amp; "&" &lt; "<" &gt; ">" &nbsp; " "
            &apos; "'" &copy; "©" &reg; "®" &trade; "™"
            &#39; "'" &#34; "\""
        }

        set result $text
        foreach {entity char} $entities {
            set result [string map [list $entity $char] $result]
        }

        # Decode numeric entities (&#NNN;)
        while {[regexp {&#(\d+);} $result -> code]} {
            set char [format %c $code]
            regsub {&#\d+;} $result $char result
        }

        return $result
    }

    # Handle TEXT events
    proc on_text {nick mask channel text} {
        variable enabled

        if {!$enabled} {
            return
        }

        # Extract URLs from the message
        set urls [extract_urls $text]

        if {[llength $urls] == 0} {
            return
        }

        # Process each URL (but limit to first 2 to avoid spam)
        set count 0
        foreach url $urls {
            if {$count >= 2} {
                break
            }
            incr count

            # Find appropriate resolver
            set resolver [find_resolver $url]

            # Call resolver (catch errors to prevent crashes)
            if {[catch {$resolver $url $nick $channel} result]} {
                # Resolver failed, skip silently
                continue
            }

            # If resolver returned something, send it to channel
            if {$result ne ""} {
                # Use the send command to post to channel
                send $channel $result
            }
        }
    }

    # Test URL resolution (for debugging)
    proc test {url} {
        set resolver [find_resolver $url]
        puts "Resolver: $resolver"

        if {[catch {$resolver $url "testuser" "#test"} result]} {
            return "Error: $result"
        }

        if {$result eq ""} {
            return "No result from resolver"
        }

        return $result
    }

    # Register built-in resolvers with examples
    proc register_builtin_resolvers {} {
        # YouTube resolver is registered if proc exists
        if {[llength [info procs ::linkresolver::youtube_resolver]]} {
            register {youtube\.com/watch|youtu\.be/} ::linkresolver::youtube_resolver 10
        }

        # Bluesky resolver
        if {[llength [info procs ::linkresolver::bluesky_resolver]]} {
            register {bsky\.app/profile/.*/(post|feed)} ::linkresolver::bluesky_resolver 10
        }
    }
}

# Export public API commands
proc linkresolver {args} {
    if {[llength $args] == 0} {
        return "Usage: linkresolver <enable|disable|register|unregister|list|test> \[args\]"
    }

    set cmd [lindex $args 0]
    set rest [lrange $args 1 end]

    switch -exact -- $cmd {
        enable {
            return [::linkresolver::enable]
        }
        disable {
            return [::linkresolver::disable]
        }
        register {
            if {[llength $rest] < 2} {
                return "Usage: linkresolver register <pattern> <proc> \[priority\]"
            }
            return [::linkresolver::register {*}$rest]
        }
        unregister {
            if {[llength $rest] < 1} {
                return "Usage: linkresolver unregister <pattern>"
            }
            return [::linkresolver::unregister {*}$rest]
        }
        list {
            return [::linkresolver::list_resolvers]
        }
        test {
            if {[llength $rest] < 1} {
                return "Usage: linkresolver test <url>"
            }
            return [::linkresolver::test {*}$rest]
        }
        default {
            return "Unknown command: $cmd. Use: enable, disable, register, unregister, list, test"
        }
    }
}

# Initialize on load
::linkresolver::init
