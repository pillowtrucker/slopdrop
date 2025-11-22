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
    # Returns: List of {timestamp price} pairs
    proc history {symbol {days 7}} {
        return [uplevel #0 [list eval "stock::history $symbol $days"]]
    }

    # Generate ASCII art chart
    # Args: symbol - stock symbol (e.g., "AAPL")
    #       days - number of days (default: 7)
    # Returns: ASCII chart as string
    proc chart {symbol {days 7}} {
        return [uplevel #0 [list eval "stock::chart $symbol $days"]]
    }
}
