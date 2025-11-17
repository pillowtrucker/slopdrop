# HTTP command implementation with rate limiting
# Limits: 5 per eval, 25 per minute

package require http

namespace eval httpx {
    variable requests_per_eval 5
    variable requests_per_minute 25
    variable request_interval 60
    variable post_limit 150000
    variable transfer_limit 150000
    variable transfer_limit_per_eval 500000
    variable max_redirects 5
    variable time_limit 5000

    variable eval_count 0
    variable requests_history
    array set requests_history {}

    variable bytes_transferred
    array set bytes_transferred {}

    proc now {} {
        clock seconds
    }

    proc check_limits {bytes_to_transfer} {
        variable eval_count
        variable requests_history
        variable bytes_transferred
        variable requests_per_eval
        variable requests_per_minute
        variable request_interval
        variable transfer_limit_per_eval

        set channel [get_channel]
        if {$channel eq ""} {
            set channel "default"
        }

        # Initialize history for this channel if needed
        if {![info exists requests_history($channel)]} {
            set requests_history($channel) [list]
        }
        if {![info exists bytes_transferred($channel)]} {
            set bytes_transferred($channel) [dict create]
        }

        set now_time [now]
        set threshold [expr {$now_time - $request_interval}]

        # Clean old requests
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

        # Clean old byte counters
        dict for {eval_id bytes} $bytes_transferred($channel) {
            # Keep current eval's counter
            if {$eval_id == $eval_count} {
                dict set new_bytes_dict $eval_id $bytes
            }
        }

        set requests_history($channel) $new_history
        set bytes_transferred($channel) $new_bytes_dict

        # Check per-eval request limit
        if {$eval_count_current >= $requests_per_eval} {
            error "too many HTTP requests in this eval (max $requests_per_eval requests)"
        }

        # Check per-minute limit
        if {$recent_count >= $requests_per_minute} {
            error "too many HTTP requests (max $requests_per_minute requests in $request_interval seconds)"
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
        variable bytes_transferred

        set channel [get_channel]
        if {$channel eq ""} {
            set channel "default"
        }

        if {![info exists requests_history($channel)]} {
            set requests_history($channel) [list]
        }
        if {![info exists bytes_transferred($channel)]} {
            set bytes_transferred($channel) [dict create]
        }

        lappend requests_history($channel) [list [now] $eval_count]

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

    proc increment_eval {} {
        variable eval_count
        incr eval_count
    }

    proc http_get {url} {
        variable transfer_limit
        variable time_limit
        variable max_redirects

        # Pre-check (assume max transfer for limit checking)
        check_limits $transfer_limit

        # Configure http package to limit redirects
        set token [::http::geturl $url \
            -timeout $time_limit \
            -blocksize 1024 \
            -maxredirects $max_redirects]

        set result [handle_response $token]

        # Extract actual bytes transferred
        lassign $result code headers body
        set bytes [string length $body]
        record_request $bytes

        return $result
    }

    proc http_post {url body} {
        variable post_limit
        variable time_limit
        variable transfer_limit
        variable max_redirects

        set body_len [string length $body]

        if {$body_len > $post_limit} {
            error "post body exceeds $post_limit bytes"
        }

        # Pre-check (assume max transfer for limit checking)
        check_limits [expr {$body_len + $transfer_limit}]

        set token [::http::geturl $url \
            -timeout $time_limit \
            -blocksize 1024 \
            -query $body \
            -type "application/x-www-form-urlencoded" \
            -maxredirects $max_redirects]

        set result [handle_response $token]

        # Extract actual bytes transferred (request body + response body)
        lassign $result code headers resp_body
        set total_bytes [expr {$body_len + [string length $resp_body]}]
        record_request $total_bytes

        return $result
    }

    proc http_head {url} {
        variable time_limit
        variable max_redirects

        # HEAD requests don't transfer much data (headers only, ~1KB estimate)
        check_limits 1024

        set token [::http::geturl $url \
            -timeout $time_limit \
            -validate 1 \
            -maxredirects $max_redirects]

        upvar #0 $token state
        set headers $state(meta)

        # Estimate header size (rough approximation)
        set header_size 0
        dict for {key val} $headers {
            set header_size [expr {$header_size + [string length $key] + [string length $val] + 4}]
        }

        ::http::cleanup $token
        record_request $header_size

        return $headers
    }

    proc handle_response {token} {
        variable transfer_limit

        upvar #0 $token state

        # Check status
        set status $state(status)
        if {$status ne "ok"} {
            ::http::cleanup $token
            error "HTTP request failed: $status"
        }

        # Check transfer limit
        if {[info exists state(currentsize)] && $state(currentsize) > $transfer_limit} {
            ::http::cleanup $token
            error "transfer exceeded $transfer_limit bytes"
        }

        # Build result: [status_code, headers, body]
        set code [::http::ncode $token]
        set headers $state(meta)
        set body $state(body)

        # Check body size
        if {[string length $body] > $transfer_limit} {
            ::http::cleanup $token
            error "transfer exceeded $transfer_limit bytes"
        }

        ::http::cleanup $token

        return [list $code $headers $body]
    }
}

# Export http commands
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
}
