# SHA1 hashing command
# Requires tcllib sha1 package (standard TCL library)

# Try to load sha1 package
if {[catch {package require sha1}]} {
    # Package not available, define error proc
    proc sha1 {str} {
        error "SHA1 not available: tcllib sha1 package not installed"
    }
} else {
    # Package loaded successfully, create wrapper
    proc sha1 {str} {
        ::sha1::sha1 -hex $str
    }
}
