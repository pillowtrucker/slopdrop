# Cache commands - persistent key-value storage
namespace eval cache {
  # Security limits
  variable max_keys_per_bucket 1000
  variable max_value_size 100000  ;# 100KB per value
  variable max_total_size_per_bucket 1000000  ;# 1MB total per bucket

  namespace eval buckets {
    proc import {bucket_name {as bucket}} {
      variable ::cache::buckets::$bucket_name
      if {![info exists ::cache::buckets::$bucket_name]} {
        array set ::cache::buckets::$bucket_name {}
      }
      uplevel [list upvar ::cache::buckets::$bucket_name $as]
    }
  }

  proc check_bucket_limits {bucket_name new_key new_value} {
    variable max_keys_per_bucket
    variable max_value_size
    variable max_total_size_per_bucket

    buckets::import $bucket_name

    # Check value size
    set value_size [string length $new_value]
    if {$value_size > $max_value_size} {
      error "cache value exceeds maximum size (max $max_value_size bytes)"
    }

    # Check if this is a new key
    set is_new_key [expr {![info exists bucket($new_key)]}]

    # Check number of keys
    if {$is_new_key} {
      set current_keys [array size bucket]
      if {$current_keys >= $max_keys_per_bucket} {
        error "cache bucket \"$bucket_name\" has too many keys (max $max_keys_per_bucket keys)"
      }
    }

    # Check total size
    set total_size 0
    foreach {key val} [array get bucket] {
      if {$key ne $new_key} {
        set total_size [expr {$total_size + [string length $val]}]
      }
    }
    set total_size [expr {$total_size + $value_size}]

    if {$total_size > $max_total_size_per_bucket} {
      error "cache bucket \"$bucket_name\" exceeds total size limit (max $max_total_size_per_bucket bytes)"
    }

    return 1
  }

  proc keys {bucket_name} {
    buckets::import $bucket_name
    array names bucket
  }

  proc exists {bucket_name key} {
    buckets::import $bucket_name
    info exists bucket($key)
  }

  proc get {bucket_name key} {
    buckets::import $bucket_name
    ensure_key_exists $bucket_name $key
    set bucket($key)
  }

  proc put {bucket_name key value} {
    # Check limits before storing
    check_bucket_limits $bucket_name $key $value

    buckets::import $bucket_name
    set bucket($key) $value
  }

  proc fetch {bucket_name key script} {
    if {[exists $bucket_name $key]} {
      get $bucket_name $key
    } else {
      set value [uplevel 1 $script]
      put $bucket_name $key $value
      set value
    }
  }

  proc delete {bucket_name key} {
    buckets::import $bucket_name
    ensure_key_exists $bucket_name $key
    unset bucket($key)
  }

  proc ensure_key_exists {bucket_name key} {
    if {![exists $bucket_name $key]} {
      error "bucket \"$bucket_name\" doesn't have key \"$key\""
    }
  }
}
