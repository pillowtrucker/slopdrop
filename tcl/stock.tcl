# Stock price tracking module using Yahoo Finance
# Uses existing http:: commands with aggressive caching to avoid rate limits
# Cache TTL: 60 seconds (good enough for 1-5 minute resolution)

namespace eval stock {
    variable cache
    array set cache {}
    variable cache_ttl 60  ;# 60 second cache
    variable max_cache_size 100  ;# Max symbols to cache

    proc now {} {
        clock seconds
    }

    proc cleanup_cache {} {
        variable cache
        variable cache_ttl
        variable max_cache_size

        set now_time [now]
        set threshold [expr {$now_time - $cache_ttl}]

        # Remove expired entries
        set keys_to_remove [list]
        foreach key [array names cache] {
            lassign $cache($key) timestamp price change volume
            if {$timestamp < $threshold} {
                lappend keys_to_remove $key
            }
        }

        foreach key $keys_to_remove {
            unset cache($key)
        }

        # If still too large, remove oldest entries
        if {[array size cache] > $max_cache_size} {
            set sorted_entries [list]
            foreach key [array names cache] {
                lassign $cache($key) timestamp price change volume
                lappend sorted_entries [list $timestamp $key]
            }
            set sorted_entries [lsort -integer -index 0 $sorted_entries]

            set to_remove [expr {[array size cache] - $max_cache_size}]
            foreach entry [lrange $sorted_entries 0 [expr {$to_remove - 1}]] {
                set key [lindex $entry 1]
                unset cache($key)
            }
        }
    }

    proc get_cached {symbol} {
        variable cache
        variable cache_ttl

        set symbol [string toupper $symbol]

        if {![info exists cache($symbol)]} {
            return ""
        }

        lassign $cache($symbol) timestamp price change volume

        set now_time [now]
        if {[expr {$now_time - $timestamp}] > $cache_ttl} {
            unset cache($symbol)
            return ""
        }

        return $cache($symbol)
    }

    proc put_cache {symbol price change volume} {
        variable cache

        set symbol [string toupper $symbol]
        cleanup_cache

        set now_time [now]
        set cache($symbol) [list $now_time $price $change $volume]
    }

    proc fetch_quote {symbol} {
        set symbol [string toupper $symbol]

        # Yahoo Finance query API endpoint
        # This provides real-time (15-20 min delayed) quotes without API key
        # Note: Using http instead of https due to TCL TLS limitations
        set url "http://query1.finance.yahoo.com/v8/finance/chart/${symbol}?interval=1d&range=2d"

        # Fetch data using existing http commands (already rate-limited)
        set response [http::get $url]

        lassign $response code headers body

        if {$code < 200 || $code >= 300} {
            error "HTTP error $code for symbol $symbol"
        }

        # Parse JSON response (simple extraction, not full JSON parser)
        # Look for current price in the response

        # Try to extract price from meta.regularMarketPrice
        if {![regexp {"regularMarketPrice":\s*([0-9.]+)} $body -> price]} {
            # Fallback: try to get from indicators
            if {![regexp {"close":\s*\[([^\]]+)\]} $body -> close_data]} {
                error "Could not parse price data for $symbol"
            }

            # Get last non-null value from close array
            set close_values [split [string map {"null" ""} $close_data] ","]
            set price ""
            foreach val [lreverse $close_values] {
                set val [string trim $val]
                if {$val ne ""} {
                    set price $val
                    break
                }
            }

            if {$price eq ""} {
                error "No valid price data found for $symbol"
            }
        }

        # Try to extract previous close for change calculation
        set prev_close $price
        if {[regexp {"previousClose":\s*([0-9.]+)} $body -> prev_close]} {
            # Calculate change percentage
            set change [expr {(($price - $prev_close) / $prev_close) * 100.0}]
        } else {
            set change 0.0
        }

        # Try to extract volume
        set volume 0
        if {[regexp {"regularMarketVolume":\s*([0-9]+)} $body -> volume]} {
            # Got volume
        } elseif {[regexp {"volume":\s*\[([^\]]+)\]} $body -> volume_data]} {
            # Get last non-null volume
            set volume_values [split [string map {"null" ""} $volume_data] ","]
            foreach val [lreverse $volume_values] {
                set val [string trim $val]
                if {$val ne ""} {
                    set volume $val
                    break
                }
            }
        }

        return [list $price $change $volume]
    }

    # Main API: stock::price <symbol>
    # Returns just the current price
    proc price {symbol} {
        set cached [get_cached $symbol]

        if {$cached ne ""} {
            lassign $cached timestamp price change volume
            return [format "%.2f" $price]
        }

        # Fetch from API
        lassign [fetch_quote $symbol] price change volume
        put_cache $symbol $price $change $volume

        return [format "%.2f" $price]
    }

    # Main API: stock::quote <symbol>
    # Returns formatted quote with symbol, price, and change
    proc quote {symbol} {
        set symbol [string toupper $symbol]
        set cached [get_cached $symbol]

        if {$cached ne ""} {
            lassign $cached timestamp price change volume
        } else {
            # Fetch from API
            lassign [fetch_quote $symbol] price change volume
            put_cache $symbol $price $change $volume
        }

        set sign [expr {$change >= 0 ? "+" : ""}]
        return [format "%s: \$%.2f (%s%.2f%%)" $symbol $price $sign $change]
    }

    # Main API: stock::detail <symbol>
    # Returns detailed quote as TCL dict
    proc detail {symbol} {
        set symbol [string toupper $symbol]
        set cached [get_cached $symbol]

        if {$cached ne ""} {
            lassign $cached timestamp price change volume
        } else {
            # Fetch from API
            lassign [fetch_quote $symbol] price change volume
            put_cache $symbol $price $change $volume
        }

        set sign [expr {$change >= 0 ? "+" : ""}]

        return [format "symbol {%s} price %.2f change {%s%.2f%%} volume %s" \
            $symbol $price $sign $change $volume]
    }

    # Utility: stock::clear
    # Clear the cache (admin/debug use)
    proc clear {} {
        variable cache
        array unset cache
        return "Stock cache cleared"
    }
}

# Export namespace
namespace export stock
