# HTTP command implementation with rate limiting using TclCurl
# Limits: 5 per eval, 25 per minute (channel), 10 per minute (user)

package require http
package require TclCurl

namespace eval httpx {
    variable requests_per_eval 5
    variable requests_per_minute 25
    variable requests_per_user_per_minute 10  ;# Per-user limit
    variable request_interval 60
    variable post_limit 150000
    variable transfer_limit 150000
    variable transfer_limit_per_eval 500000
    variable max_redirects 5
    variable time_limit 5000

    variable eval_count 0
    variable requests_history
    array set requests_history {}

    variable user_requests_history
    array set user_requests_history {}

    variable bytes_transferred
    array set bytes_transferred {}

    proc now {} {
        clock seconds
    }

    proc check_limits {bytes_to_transfer} {
        variable eval_count
        variable requests_history
        variable user_requests_history
        variable bytes_transferred
        variable requests_per_eval
        variable requests_per_minute
        variable requests_per_user_per_minute
        variable request_interval
        variable transfer_limit_per_eval

        set channel [get_channel]
        if {$channel eq ""} {
            set channel "default"
        }

        set user [get_user]
        set user_key "${channel}:${user}"

        # Initialize history for this channel if needed
        if {![info exists requests_history($channel)]} {
            set requests_history($channel) [list]
        }
        if {![info exists user_requests_history($user_key)]} {
            set user_requests_history($user_key) [list]
        }
        if {![info exists bytes_transferred($channel)]} {
            set bytes_transferred($channel) [dict create]
        }

        set now_time [now]
        set threshold [expr {$now_time - $request_interval}]

        # Clean old requests (channel-level)
        set new_history [list]
        set recent_count 0
        set eval_count_current 0
        set new_bytes_dict [dict create]

        foreach req $requests_history($channel) {
            lassign $req timestamp eval_id
            if {$timestamp >= $threshold} {
                lappend new_history $req
                incr recent_count
                if {$eval_id == $eval_count} {
                    incr eval_count_current
                }
            }
        }

        # Clean old requests (user-level)
        set new_user_history [list]
        set user_recent_count 0

        foreach req $user_requests_history($user_key) {
            lassign $req timestamp
            if {$timestamp >= $threshold} {
                lappend new_user_history $req
                incr user_recent_count
            }
        }

        # Clean old byte counters
        dict for {eval_id bytes} $bytes_transferred($channel) {
            # Keep current eval's counter
            if {$eval_id == $eval_count} {
                dict set new_bytes_dict $eval_id $bytes
            }
        }

        set requests_history($channel) $new_history
        set user_requests_history($user_key) $new_user_history
        set bytes_transferred($channel) $new_bytes_dict

        # Check per-eval request limit
        if {$eval_count_current >= $requests_per_eval} {
            error "too many HTTP requests in this eval (max $requests_per_eval requests)"
        }

        # Check per-minute limit (channel-level)
        if {$recent_count >= $requests_per_minute} {
            error "too many HTTP requests (max $requests_per_minute requests in $request_interval seconds)"
        }

        # Check per-user per-minute limit
        if {$user_recent_count >= $requests_per_user_per_minute} {
            error "you have made too many HTTP requests (max $requests_per_user_per_minute requests per user in $request_interval seconds)"
        }

        # Check accumulated transfer limit for this eval
        set current_bytes 0
        if {[dict exists $bytes_transferred($channel) $eval_count]} {
            set current_bytes [dict get $bytes_transferred($channel) $eval_count]
        }

        if {[expr {$current_bytes + $bytes_to_transfer}] > $transfer_limit_per_eval} {
            error "total transfer limit exceeded for this eval (max $transfer_limit_per_eval bytes, have $current_bytes, trying $bytes_to_transfer)"
        }
    }

    proc record_request {bytes} {
        variable eval_count
        variable requests_history
        variable user_requests_history
        variable bytes_transferred

        set channel [get_channel]
        if {$channel eq ""} {
            set channel "default"
        }

        set user [get_user]
        set user_key "${channel}:${user}"

        if {![info exists requests_history($channel)]} {
            set requests_history($channel) [list]
        }
        if {![info exists user_requests_history($user_key)]} {
            set user_requests_history($user_key) [list]
        }
        if {![info exists bytes_transferred($channel)]} {
            set bytes_transferred($channel) [dict create]
        }

        set now_time [now]
        lappend requests_history($channel) [list $now_time $eval_count]
        lappend user_requests_history($user_key) [list $now_time]

        # Track bytes transferred
        set current_bytes 0
        if {[dict exists $bytes_transferred($channel) $eval_count]} {
            set current_bytes [dict get $bytes_transferred($channel) $eval_count]
        }
        dict set bytes_transferred($channel) $eval_count [expr {$current_bytes + $bytes}]
    }

    proc get_channel {} {
        if {[info exists ::nick_channel]} {
            return $::nick_channel
        }
        return ""
    }

    proc get_user {} {
        if {[info exists ::nick]} {
            return $::nick
        }
        return "unknown"
    }

    proc increment_eval {} {
        variable eval_count
        incr eval_count
    }

    proc normalize_url {url} {
        # Auto-prepend http:// if no protocol specified
        set url_lower [string tolower $url]
        if {![string match "http://*" $url_lower] && ![string match "https://*" $url_lower]} {
            set url "http://$url"
        }
        return $url
    }

    proc validate_url {url} {
        # Convert to lowercase for case-insensitive matching
        set url_lower [string tolower $url]

        # Block non-HTTP(S) schemes (should already have protocol from normalize_url)
        if {![string match "http://*" $url_lower] && ![string match "https://*" $url_lower]} {
            error "only http:// and https:// URLs are allowed"
        }

        # Extract hostname from URL
        # Remove protocol
        set url_no_proto [regsub {^https?://} $url_lower ""]
        # Get everything before first / or : (port)
        regexp {^([^/:]+)} $url_no_proto -> hostname

        # Block localhost and loopback
        if {[string match "localhost*" $hostname] ||
            [string match "127.*" $hostname] ||
            [string match "::1" $hostname] ||
            [string match "0.0.0.0" $hostname]} {
            error "requests to localhost are not allowed"
        }

        # Block private IP ranges (RFC 1918)
        if {[string match "10.*" $hostname] ||
            [string match "192.168.*" $hostname] ||
            [regexp {^172\.(1[6-9]|2[0-9]|3[01])\.} $hostname]} {
            error "requests to private IP addresses are not allowed"
        }

        # Block link-local addresses
        if {[string match "169.254.*" $hostname]} {
            error "requests to link-local addresses are not allowed"
        }

        # Block IPv6 private/local addresses
        if {[string match "fc*" $hostname] ||
            [string match "fd*" $hostname] ||
            [string match "fe80:*" $hostname]} {
            error "requests to private IPv6 addresses are not allowed"
        }

        return 1
    }

    proc http_get {url} {
        variable transfer_limit
        variable time_limit

        # Normalize URL (add http:// if missing)
        set url [normalize_url $url]

        # Validate URL for security (SSRF prevention)
        validate_url $url

        # Pre-check (assume max transfer for limit checking)
        check_limits $transfer_limit

        # Use TclCurl for the request
        set curlHandle [curl::init]
        set html {}
        array set http_resp_header [list]

        $curlHandle configure \
            -url $url \
            -nosignal 1 \
            -bodyvar html \
            -headervar http_resp_header \
            -timeout [expr {$time_limit / 1000}] \
            -followlocation 1 \
            -maxredirs 5

        catch { $curlHandle perform } curlErrorNumber

        if { $curlErrorNumber != 0 } {
            $curlHandle cleanup
            error [curl::easystrerror $curlErrorNumber]
        }

        set ret [list]
        lappend ret [$curlHandle getinfo responsecode]
        lappend ret [array get http_resp_header]
        lappend ret $html

        array unset http_resp_header
        $curlHandle cleanup

        # Record actual bytes transferred
        set bytes [string length $html]
        record_request $bytes

        return $ret
    }

    proc http_post {url body} {
        variable post_limit
        variable time_limit
        variable transfer_limit

        # Normalize URL (add http:// if missing)
        set url [normalize_url $url]

        # Validate URL for security (SSRF prevention)
        validate_url $url

        set body_len [string length $body]

        if {$body_len > $post_limit} {
            error "post body exceeds $post_limit bytes"
        }

        # Pre-check (assume max transfer for limit checking)
        check_limits [expr {$body_len + $transfer_limit}]

        # Use TclCurl for the request
        set curlHandle [curl::init]
        set html {}

        $curlHandle configure \
            -url $url \
            -nosignal 1 \
            -bodyvar html \
            -post 1 \
            -postfields $body \
            -timeout [expr {$time_limit / 1000}] \
            -followlocation 1 \
            -maxredirs 5

        catch { $curlHandle perform } curlErrorNumber

        if { $curlErrorNumber != 0 } {
            $curlHandle cleanup
            error [curl::easystrerror $curlErrorNumber]
        }

        set ret [list]
        lappend ret [$curlHandle getinfo responsecode]
        lappend ret {}  ;# headers not captured in post
        lappend ret $html

        $curlHandle cleanup

        # Record actual bytes transferred (request body + response body)
        set total_bytes [expr {$body_len + [string length $html]}]
        record_request $total_bytes

        return $ret
    }

    proc http_head {url} {
        variable time_limit

        # Normalize URL (add http:// if missing)
        set url [normalize_url $url]

        # Validate URL for security (SSRF prevention)
        validate_url $url

        # HEAD requests don't transfer much data (headers only, ~1KB estimate)
        check_limits 1024

        # Use http get but return only headers
        set resp [http_get $url]

        # Return just the headers
        return [lindex $resp 1]
    }
}

# Export http commands as ensemble
# This creates "http get", "http post", "http head" commands
namespace eval http {
    proc get {url} {
        ::httpx::http_get $url
    }

    proc post {url body} {
        ::httpx::http_post $url $body
    }

    proc head {url} {
        ::httpx::http_head $url
    }

    namespace export get post head
    namespace ensemble create
}
