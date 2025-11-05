# Utility commands for common operations

proc lindex_random {list} {
    lindex $list [expr {int(rand() * [llength $list])}]
}

proc pick args {
    # pick 1 {option1} 2 {option2} - weighted random choice
    set total 0
    foreach {weight _} $args {
        incr total $weight
    }
    set r [expr {rand() * $total}]
    set acc 0
    foreach {weight value} $args {
        set acc [expr {$acc + $weight}]
        if {$r < $acc} {
            return [uplevel 1 $value]
        }
    }
    return ""
}

proc ?? {list} {
    lindex_random $list
}

proc choose args {
    lindex_random $args
}

# String manipulation
proc upper {str} {
    string toupper $str
}

proc lower {str} {
    string tolower $str
}

# List operations
proc first {list} {
    lindex $list 0
}

proc last {list} {
    lindex $list end
}

proc rest {list} {
    lrange $list 1 end
}
