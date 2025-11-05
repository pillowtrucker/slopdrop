# HTTP command implementation with rate limiting
# Limits: 5 per eval, 25 per minute

package require http

namespace eval httpx {
    variable requests_per_eval 5
    variable requests_per_minute 25
    variable request_interval 60
    variable post_limit 150000
    variable transfer_limit 150000
    variable time_limit 5000

    variable eval_count 0
    variable requests_history
    array set requests_history {}

    proc now {} {
        clock seconds
    }

    proc check_limits {} {
        variable eval_count
        variable requests_history
        variable requests_per_eval
        variable requests_per_minute
        variable request_interval

        set channel [get_channel]
        if {$channel eq ""} {
            set channel "default"
        }

        # Initialize history for this channel if needed
        if {![info exists requests_history($channel)]} {
            set requests_history($channel) [list]
        }

        set now_time [now]
        set threshold [expr {$now_time - $request_interval}]

        # Clean old requests
        set new_history [list]
        set recent_count 0
        set eval_count_current 0

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

        set requests_history($channel) $new_history

        # Check per-eval limit
        if {$eval_count_current >= $requests_per_eval} {
            error "too many HTTP requests in this eval (max $requests_per_eval requests)"
        }

        # Check per-minute limit
        if {$recent_count >= $requests_per_minute} {
            error "too many HTTP requests (max $requests_per_minute requests in $request_interval seconds)"
        }
    }

    proc record_request {} {
        variable eval_count
        variable requests_history

        set channel [get_channel]
        if {$channel eq ""} {
            set channel "default"
        }

        if {![info exists requests_history($channel)]} {
            set requests_history($channel) [list]
        }

        lappend requests_history($channel) [list [now] $eval_count]
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

        check_limits

        set token [::http::geturl $url \
            -timeout $time_limit \
            -blocksize 1024]

        set result [handle_response $token]
        record_request

        return $result
    }

    proc http_post {url body} {
        variable post_limit
        variable time_limit
        variable transfer_limit

        check_limits

        if {[string length $body] > $post_limit} {
            error "post body exceeds $post_limit bytes"
        }

        set token [::http::geturl $url \
            -timeout $time_limit \
            -blocksize 1024 \
            -query $body \
            -type "application/x-www-form-urlencoded"]

        set result [handle_response $token]
        record_request

        return $result
    }

    proc http_head {url} {
        variable time_limit

        check_limits

        set token [::http::geturl $url \
            -timeout $time_limit \
            -validate 1]

        upvar #0 $token state
        set headers $state(meta)
        ::http::cleanup $token
        record_request

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
