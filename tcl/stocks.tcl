# Stock price tracking and ASCII chart visualization
# Provides both stock data retrieval (via Rust backend) and charting

namespace eval stock {

    #=========================================================================
    # PUBLIC API - Rust-backed stock commands
    # These wrappers allow TCL code to call the Rust stock backend
    #=========================================================================

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

    #=========================================================================
    # INTERNAL HELPERS - Chart generation and visualization
    #=========================================================================

    # Generate ASCII art chart from stock history data
    # Args: symbol - stock symbol
    #       history_data - list of {timestamp price} pairs
    # Returns: ASCII chart as a string
    # Note: This is called from Rust with pre-fetched historical data
    proc chart_from_data {symbol history_data} {
        if {[llength $history_data] == 0} {
            error "No historical data available for $symbol"
        }

        # Parse data into separate lists
        set prices [list]
        set timestamps [list]

        foreach datapoint $history_data {
            lassign $datapoint timestamp price
            lappend timestamps $timestamp
            lappend prices $price
        }

        # Generate the chart
        set chart_output [generate_ascii_chart $symbol $prices $timestamps]

        return $chart_output
    }

    # Internal: Generate ASCII chart from price data
    proc generate_ascii_chart {symbol prices timestamps} {
        set num_points [llength $prices]

        if {$num_points == 0} {
            return "No data to chart"
        }

        # Find min and max prices for scaling
        set min_price [lindex $prices 0]
        set max_price [lindex $prices 0]

        foreach price $prices {
            if {$price < $min_price} { set min_price $price }
            if {$price > $max_price} { set max_price $price }
        }

        # Add some padding to the range
        set price_range [expr {$max_price - $min_price}]
        if {$price_range < 0.01} {
            # If prices are very close, add artificial range
            set price_range 1.0
            set min_price [expr {$min_price - 0.5}]
            set max_price [expr {$max_price + 0.5}]
        } else {
            set padding [expr {$price_range * 0.1}]
            set min_price [expr {$min_price - $padding}]
            set max_price [expr {$max_price + $padding}]
            set price_range [expr {$max_price - $min_price}]
        }

        # Chart dimensions
        set chart_height 10
        set chart_width [expr {min($num_points, 60)}]

        # Calculate current price and change
        set current_price [lindex $prices end]
        set first_price [lindex $prices 0]
        set change [expr {$current_price - $first_price}]
        set change_pct [expr {($change / $first_price) * 100.0}]

        # Build the chart header with data point count
        set sign [expr {$change >= 0 ? "+" : ""}]
        append result "\002$symbol\002 \$[format "%.2f" $current_price] ($sign[format "%.2f" $change_pct]%) - $num_points data points\n"

        # Create the chart grid
        set chart_lines [list]
        for {set row 0} {$row < $chart_height} {incr row} {
            lappend chart_lines [string repeat " " $chart_width]
        }

        # Sample data points to fit chart width if needed
        set sample_indices [list]
        if {$num_points <= $chart_width} {
            for {set i 0} {$i < $num_points} {incr i} {
                lappend sample_indices $i
            }
        } else {
            # Sample evenly across the data
            for {set i 0} {$i < $chart_width} {incr i} {
                set idx [expr {int(($i * ($num_points - 1)) / double($chart_width - 1))}]
                lappend sample_indices $idx
            }
        }

        # Plot the points and connect with lines
        set prev_row -1
        set prev_col -1

        foreach col_idx [lrange [lsearch -all -integer [lrepeat [llength $sample_indices] 1] 1] 0 end] sample_idx $sample_indices {
            set price [lindex $prices $sample_idx]

            # Convert price to row (inverted: high price = low row number)
            set normalized [expr {($price - $min_price) / $price_range}]
            set row [expr {$chart_height - 1 - int($normalized * ($chart_height - 1))}]

            # Set the character at this position
            set line [lindex $chart_lines $row]
            set chart_lines [lreplace $chart_lines $row $row \
                [string replace $line $col_idx $col_idx "*"]]

            # Draw connecting line if not first point
            if {$prev_row >= 0} {
                # Draw vertical line between points
                set start_row [expr {min($prev_row, $row)}]
                set end_row [expr {max($prev_row, $row)}]

                for {set r [expr {$start_row + 1}]} {$r < $end_row} {incr r} {
                    set line [lindex $chart_lines $r]
                    set current_char [string index $line $col_idx]
                    if {$current_char eq " "} {
                        set chart_lines [lreplace $chart_lines $r $r \
                            [string replace $line $col_idx $col_idx "|"]]
                    }
                }

                # Draw horizontal line if same row
                if {$prev_row == $row && $prev_col >= 0} {
                    for {set c [expr {$prev_col + 1}]} {$c < $col_idx} {incr c} {
                        set line [lindex $chart_lines $row]
                        set current_char [string index $line $c]
                        if {$current_char eq " "} {
                            set chart_lines [lreplace $chart_lines $row $row \
                                [string replace $line $c $c "-"]]
                        }
                    }
                }
            }

            set prev_row $row
            set prev_col $col_idx
        }

        # Add Y-axis labels and render chart
        for {set row 0} {$row < $chart_height} {incr row} {
            # Calculate price for this row
            set normalized [expr {($chart_height - 1 - $row) / double($chart_height - 1)}]
            set row_price [expr {$min_price + ($normalized * $price_range)}]

            # Format the price label (right-aligned)
            set label [format "%7.2f" $row_price]
            append result "$label |[lindex $chart_lines $row]\n"
        }

        # Add X-axis
        append result "        [string repeat "-" $chart_width]\n"

        # Add time range label
        set start_date [clock format [lindex $timestamps 0] -format "%m/%d"]
        set end_date [clock format [lindex $timestamps end] -format "%m/%d"]
        append result "        $start_date[string repeat " " [expr {$chart_width - 10}]]$end_date\n"

        return $result
    }
}
