# TCL wrappers for Rust-backed stock commands
# These allow any TCL code to call stock functionality

namespace eval stock {
    # Get current quote for a symbol
    # Args: symbol - stock symbol (e.g., "AAPL")
    # Returns: Formatted quote string
    proc quote {symbol} {
        # Use uplevel to eval at top level where Rust interception happens
        return [uplevel #0 [list eval "stock::quote $symbol"]]
    }

    # Get current price for a symbol
    # Args: symbol - stock symbol (e.g., "AAPL")
    # Returns: Price as string
    proc price {symbol} {
        return [uplevel #0 [list eval "stock::price $symbol"]]
    }

    # Get detailed quote information
    # Args: symbol - stock symbol (e.g., "AAPL")
    # Returns: Detailed quote as dict-formatted string
    proc detail {symbol} {
        return [uplevel #0 [list eval "stock::detail $symbol"]]
    }

    # Get historical quotes
    # Args: symbol - stock symbol (e.g., "AAPL")
    #       days - number of days (default: 7)
    #       interval - optional interval ("1m", "5m", "15m", "30m", "1h", "1d", "1wk", "1mo")
    #                  If not specified, uses smart defaults based on time range:
    #                  1 day: 5m, 2-7 days: 1h, 8-60 days: 1d, 60+ days: 1wk
    # Returns: List of {timestamp price} pairs
    proc history {symbol {days 7} {interval ""}} {
        if {$interval eq ""} {
            return [uplevel #0 [list eval "stock::history $symbol $days"]]
        } else {
            return [uplevel #0 [list eval "stock::history $symbol $days $interval"]]
        }
    }

    # Generate ASCII art chart
    # Args: symbol - stock symbol (e.g., "AAPL")
    #       days - number of days (default: 7)
    #       interval - optional interval ("1m", "5m", "15m", "30m", "1h", "1d", "1wk", "1mo")
    #                  If not specified, uses smart defaults based on time range
    # Returns: ASCII chart as string
    # Examples:
    #   stock::chart AAPL          # 7 days with hourly data
    #   stock::chart AAPL 1        # 1 day with 5-minute data
    #   stock::chart AAPL 1 15m    # 1 day with 15-minute data
    #   stock::chart AAPL 30       # 30 days with daily data
    #   stock::chart AAPL 30 1h    # 30 days with hourly data
    proc chart {symbol {days 7} {interval ""}} {
        if {$interval eq ""} {
            return [uplevel #0 [list eval "stock::chart $symbol $days"]]
        } else {
            return [uplevel #0 [list eval "stock::chart $symbol $days $interval"]]
        }
    }
}
