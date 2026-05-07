# TIMTOM IRC Bot - TCL Port
# Original mIRC script by gamme (2011-2017)
# Ported to TCL framework under timtom:: namespace
# Licensed under GNU GPL v3
#
# All command procs accept an optional explicit nick; when omitted they fall
# back to $::nick (set by the dispatching framework). State variables are
# initialised lazily through the cache and use mIRC-style "undefined or
# nonzero" semantics for permission flags (spin, flip, etc).

namespace eval timtom {
    variable bucket "timtom"

    # ========================================================================
    # Helpers - identity, formatting, state access
    # ========================================================================

    # IRC colour escape (mIRC palette). Both args are decimal in 0..15.
    # Use one-arg form for foreground only.
    # Implemented inline in messages as bare \003FG,BG sequences when there's
    # only one colour in the line; this helper is for procedurally-built ones.
    proc clr {fg {bg ""}} {
        if {$bg eq ""} { return "\003$fg" }
        return "\003$fg,$bg"
    }

    proc whoami {nick} {
        # Resolve implicit nick to caller, falling back to $::nick when unset.
        if {$nick ne ""} { return $nick }
        if {[info exists ::nick]} { return $::nick }
        return ""
    }

    proc current_channel {} {
        if {[info exists ::channel]} { return $::channel }
        return ""
    }

    # Channel name for use as a `timers schedule` target. The Rust timer
    # dispatch reads "network:channel" composite keys and routes them to
    # the right network connection; without the prefix it falls back to
    # picking an arbitrary first network, which silently misroutes
    # recurring messages on multi-network bots. Prefix only when ::network
    # is non-empty so the bare-channel fallback still works in tests and
    # single-network deployments.
    proc _timer_chan {} {
        set ch [current_channel]
        if {$ch eq ""} { return "" }
        if {[info exists ::network] && $::network ne ""} {
            return "${::network}:${ch}"
        }
        return $ch
    }

    proc format_with_commas {num} {
        set num [expr {entier($num)}]
        set negative 0
        if {$num < 0} { set negative 1; set num [expr {-$num}] }
        set str [format "%d" $num]
        set len [string length $str]
        if {$len <= 3} {
            if {$negative} { return "-$str" }
            return $str
        }
        set result ""
        set count 0
        for {set i [expr {$len - 1}]} {$i >= 0} {incr i -1} {
            if {$count > 0 && $count % 3 == 0} { set result ",$result" }
            set result "[string index $str $i]$result"
            incr count
        }
        if {$negative} { return "-$result" }
        return $result
    }

    proc format_money {amount} {
        if {$amount eq "" || $amount == 0} { return "\$0" }
        # Normalise to a string with at most 2 decimal places.
        if {[string is integer -strict $amount]} {
            return "\$[format_with_commas $amount]"
        }
        # Floating point: split on decimal point.
        set s [format "%.2f" $amount]
        set parts [split $s "."]
        set whole [lindex $parts 0]
        set decimal [lindex $parts 1]
        if {$decimal eq "00"} {
            return "\$[format_with_commas $whole]"
        }
        return "\$[format_with_commas $whole].$decimal"
    }

    proc _key {stat nick} { return "${stat}_[string tolower $nick]" }

    proc get_money {nick} {
        variable bucket
        set k [_key money $nick]
        if {[cache exists $bucket $k]} { return [cache get $bucket $k] }
        return 0
    }
    proc set_money {nick amount} {
        variable bucket
        cache put $bucket [_key money $nick] $amount
    }
    proc add_money {nick amount} {
        set v [expr {[get_money $nick] + $amount}]
        set_money $nick $v
        return $v
    }

    proc get_stat {nick stat} {
        variable bucket
        set k [_key $stat $nick]
        if {[cache exists $bucket $k]} { return [cache get $bucket $k] }
        return 0
    }
    proc set_stat {nick stat value} {
        variable bucket
        cache put $bucket [_key $stat $nick] $value
    }
    proc add_stat {nick stat amount} {
        set v [expr {[get_stat $nick $stat] + $amount}]
        set_stat $nick $stat $v
        return $v
    }
    proc del_stat {nick stat} {
        variable bucket
        set k [_key $stat $nick]
        if {[cache exists $bucket $k]} { cache delete $bucket $k }
    }

    # mIRC-style permission flag: undefined OR non-zero == allowed.
    proc stat_allows {nick stat} {
        variable bucket
        set k [_key $stat $nick]
        if {![cache exists $bucket $k]} { return 1 }
        set v [cache get $bucket $k]
        if {$v eq "" || $v eq "0" || $v == 0} { return 0 }
        return 1
    }

    # Clear all keys matching a glob pattern (for mIRC `unset %X*`).
    proc clear_glob {pattern} {
        variable bucket
        foreach key [cache keys $bucket] {
            if {[string match $pattern $key]} { cache delete $bucket $key }
        }
    }

    proc get_state {key} {
        variable bucket
        if {[cache exists $bucket $key]} { return [cache get $bucket $key] }
        return ""
    }
    proc set_state {key value} {
        variable bucket
        cache put $bucket $key $value
    }
    proc del_state {key} {
        variable bucket
        if {[cache exists $bucket $key]} { cache delete $bucket $key }
    }
    proc inc_state {key amount} {
        set cur [get_state $key]
        if {$cur eq ""} { set cur 0 }
        set v [expr {$cur + $amount}]
        set_state $key $v
        return $v
    }

    # State that distinguishes "unset" from "0".
    proc state_allows {key} {
        variable bucket
        if {![cache exists $bucket $key]} { return 1 }
        set v [cache get $bucket $key]
        if {$v eq "" || $v eq "0" || $v == 0} { return 0 }
        return 1
    }

    # ========================================================================
    # Channel helpers
    # ========================================================================

    proc channel_nicks {} {
        set ch [current_channel]
        if {$ch eq ""} { return [list] }
        if {[catch {chanlist $ch} nicks]} { return [list] }
        return $nicks
    }

    proc nick_count {} { llength [channel_nicks] }

    proc random_chan_nick {} {
        set ns [channel_nicks]
        if {[llength $ns] == 0} { return "" }
        return [lindex $ns [expr {int(rand() * [llength $ns])}]]
    }

    proc random_other_nick {nick} {
        set ns [channel_nicks]
        set others [list]
        foreach n $ns { if {[string equal -nocase $n $nick]} continue; lappend others $n }
        if {[llength $others] == 0} { return "" }
        return [lindex $others [expr {int(rand() * [llength $others])}]]
    }

    proc random_int {min max} {
        # mIRC $rand(min, max) is inclusive on both ends.
        return [expr {int(rand() * ($max - $min + 1)) + $min}]
    }

    proc pick {list} {
        if {[llength $list] == 0} { return "" }
        return [lindex $list [expr {int(rand() * [llength $list])}]]
    }

    # ========================================================================
    # Trivial / single-message commands
    # ========================================================================

    proc greet {{nick ""}} {
        set nick [whoami $nick]
        return "\0034,13${nick}, this is TIMTOM. How may I serve you?\003"
    }

    proc sex {{nick ""}} {
        set nick [whoami $nick]
        if {[random_int 1 68] == 41} {
            return "\0034,12ok, ${nick}, I will have sex with you now.\003"
        }
        return "\0037,12${nick}, I cannot perform sex on you at this moment.\003"
    }

    proc horses {{nick ""}} {
        set nick [whoami $nick]
        return "\0033,8${nick}, I like horses too.\003"
    }

    proc jesus {{nick ""}} {
        set nick [whoami $nick]
        set ch [current_channel]
        return "\0033,8${nick}, Jesus loves you more than ${ch}.  I'm sorry.  $ch just doesn't compare.\003"
    }

    proc wheel {{nick ""}} {
        set nick [whoami $nick]
        if {[stat_allows $nick spin]} {
            return "\00311,1I think it would be a good idea if $nick would spin the wheel.\003"
        }
        return "\0034,11${nick}, please let someone else spin.\003"
    }

    proc more_soup {{nick ""}} {
        set n [whoami $nick]
        return "\00310,4Sorry, ${nick}, here's some more soup.\003"
    }
    proc more_tea {{nick ""}} {
        set n [whoami $nick]
        return "\0031,12Sorry, ${nick}, here's some more tea.\003"
    }
    proc more_coffee {{nick ""}} {
        set n [whoami $nick]
        return "\00310,8Sorry, ${nick}, here's some more coffee.\003"
    }

    proc admissions {} {
        return "\0031,8Download the Hertford College Admissions iPhone app here: \003\0034,11http://itunes.apple.com/us/app/hertford-college-admissions/id382253306?mt=8\003"
    }

    proc whats_new {} {
        return "\0034,11Hey all!  It's been a joy serving you these past few days.  Though we've lost a bit of ground in terms of new memberships, we've all become closer and better good friends and that's what counts here. The wheel is still #1 as it should be.\003\n\0035,11Some of our newest features include: \003\0031,13death!\003\0035,11  \003\0030,3life!\003\0035,11  \003\0031,7piegs!\003\0035,11  and after a great deal of fighting, we've finally decided to allow our members to use the word \"buffelo\".  However, one must include the phrase \"msl strip_buf\" somewhere in his message in order for TIMTOM to agree.  Enjoy friends.\003"
    }

    proc help_cmd {} {
        return "\0037,10Hi, how are you doing?  My name is TIMTOM and you are in ${ch}.  I am a servant to the people, and like to fancy myself as quite the capable gentleman.  If there's anything you need don't hesitate to ask.  Everyone has a voice here and we treat everyone with love and kindness.\003\n\0034,11Some of our popular features include hot soup and hot tea, horses, and NEVER FEAR: we offer the sacrament of marriage and also deal in divorces, and Wheel of Fortune is always on.  Kick off your shoes, relax, and don't worry about a thing.  The internet cannot hurt you now.  You are in ${ch}.\003"
    }

    # Broadcast servings: list nicks (or non-ops) and serve them.
    proc _serve_broadcast {item color_intro} {
        set ns [channel_nicks]
        if {[llength $ns] == 0} { return "${color_intro}.  Enjoy friends.\003" }
        return "${color_intro} [join $ns " "]  Enjoy friends.\003"
    }

    proc soup {} {
        _serve_broadcast soup "\0038,6TIMTOM brings out the hot soup for"
    }
    proc tea {} {
        _serve_broadcast tea "\0034,12TIMTOM brings out the hot tea for"
    }
    proc coffee {} {
        _serve_broadcast coffee "\0032,7TIMTOM brings out the hot coffee for"
    }
    proc rings {} {
        _serve_broadcast rings "\0038,6TIMTOM brings out the rings for"
    }

    # ========================================================================
    # State / country trivia (single fixed messages)
    # ========================================================================

    proc check_states {text {nick ""} {result_var ""}} {
        # Backward-compat: original signature took {text nick resultVar}.
        set nick [whoami $nick]
        set ch [current_channel]
        set t [string tolower $text]
        set states [dict create \
            "alabama" "3,8Alabama eats my children." \
            "alaska" "3,8Alaska is a cotton gin." \
            "arizona" "3,8Arizona is the land of the forsaken bee hives." \
            "arkansas" "3,8Arkansas is a potato rally." \
            "california" "3,8The capital of California is Los Angeles." \
            "colorado" "3,8Colorado was the missing egg in the blue carton." \
            "connecticut" "3,8Connecticut is a wild stallion." \
            "delaware" "3,8Delaware is a label-making compartment of beauty." \
            "florida" "3,8The capital of Florida is Disney World." \
            "georgia" "3,8Georgia plates early, makes space for Willy." \
            "hawaii" "3,8The capital of Hawaii is dog." \
            "idaho" "3,8Idaho is a flowing mountain." \
            "illinois" "3,8The capital of Illinois is Deal Or No Deal." \
            "indiana" "3,8Indiana rests softly in my left breast pocket sandwich player machine box heavy." \
            "iowa" "3,8Friends make pottery in Iowa." \
            "kansas" "3,8Kansas is a candy cane land in ${ch}." \
            "kentucky" "3,8The capital of Kentucky is horse." \
            "louisiana" "3,8Louisiana is a bubble paper pepper boy." \
            "maine" "3,8Maine is the capital of France." \
            "maryland" "3,8The capital of Maryland is inside the fried pickled answering machine tape." \
            "massachusetts" "3,8Massachusettes is the capital of Happy time." \
            "michigan" "3,8$nick I love you more than the sun and the sky.  I want you to be my forever." \
            "minnesota" "3,8Minnesota, will you be my road puppy?" \
            "mississippi" "3,8Glad tidings to you, ${nick}, wherever you are." \
            "missouri" "3,8The fly sunk to the bottom of the jar of oil.  The fly's name was Montel." \
            "montana" "7,10You didn't press enter hard enough." \
            "nebraska" "3,8Spinning sunflower wreath, you come in the morning and leave by nightfall." \
            "nevada" "3,8Nevada is first in my peeegy back machine-eeeeeeeeeeee-ooooooooooo." \
            "new hampshire" "3,8I hear shovels.  Lock the doors. NOW NOOOOWWWW 1,4NOOOOOOOOOWWWWWWWWWWWWWWWWWWWWWWWW!!!!!!!!" \
            "new jersey" "3,8We all eat pots and pans." \
            "new mexico" "3,8Thunder, ice, and twins joined at the hip, make my day a solid whip?  Whippie!" \
            "new york" "3,8The capital of New York is New York City." \
            "north carolina" "3,8North Carolina makes $ch a happy land for you." \
            "north dakota" "3,8I am the willing partner in your N. Dakota movement." \
            "ohio" "3,8Ohio is diabetes." \
            "oklahoma" "3,8Oklahoma is my tea set." \
            "oregon" "3,8There's plenty of lightbulbs in the furnace." \
            "pennsylvania" "3,8The capital of Pennsylvania is cheddar." \
            "rhode island" "3,8How could I forget you, Rhode Island?  You are a gentle beauty." \
            "south carolina" "3,8South Carolina is poppy." \
            "south dakota" "3,11Do you really think you own me, ${nick}?" \
            "tennessee" "3,8Tennessee is a puppy cage." \
            "texas" "3,8Feast on these berries.  They were created through honor, diligence, and musk." \
            "utah" "3,8The little pieces of paper need to be evaluated." \
            "vermont" "3,8Vermont is a picnic tree." \
            "virginia" "3,8Virginia is a glue cow." \
            "washington" "3,8Let's roll up another traffic ordinance and place it beneath the Bubber Tree." \
            "west virginia" "3,8Them tree trunks look like legs." \
            "wisconsin" "3,8If we connect the brown pipe to the gray pipe we make famous grandwich butter spread." \
            "wyoming" "3,8Claw me to death with pear skins." \
            "africa" "3,8Africa is a lollipop for you." \
            "canada" "3,8Canada is made of copper and sand." \
            "china" "3,8Thank you for relaxing in ${ch}.  China." \
            "france" "3,8France is a boat." \
            "sweden" "3,8The capital of Sweden is pah-pah."
        ]

        if {[dict exists $states $t]} {
            set msg [dict get $states $t]
            if {$result_var ne ""} {
                upvar 1 $result_var r
                set r $msg
                return 1
            }
            return $msg
        }
        if {$result_var ne ""} { return 0 }
        return ""
    }

    # ========================================================================
    # Money command
    # ========================================================================

    proc money {{nick ""}} {
        set nick [whoami $nick]
        set amount [get_money $nick]
        set formatted [format_money $amount]
        set r [random_int 1 10]
        set upper [string toupper $nick]
        if {$amount == 0} {
            switch -- $r {
                1  { return "\0031,7Hey, how are you doing, ${nick}?  It's TIMTOM.  You currently have \0031,7\$0.  Sorry about that.\003" }
                2  { return "\0033,8TIMTOM here!  Are you having fun yet, ${nick}?  I sure hope you are.  You currently have \0033,8\$0.  :(\003" }
                3  { return "\0032,10What's the good word, there, ${nick}?  It's TIMTOM.  You currently have \0032,10\$0.  I'm so sorry.\003" }
                4  { return "\0036,9Howdy Doodie ${nick}!  You currently have \0036,9\$0.  That's too bad.\003" }
                5  { return "\00311,7TIMTOM here!  Responding to the one and only ${nick}.  You currently have \00311,7\$0.  Ah well.\003" }
                6  { return "\0033,5IT'S SO NICE TO HEAR FROM YOU, ${upper}!  You want to know about your money, eh, ${nick}?  Well, you've got \0033,5\$0.  Let's hope you do better.\003" }
                7  { return "\0038,12TIMTOM here!  Reporting for duty.  ${nick}, you currently have \0038,12\$0.  Uh oh!\003" }
                8  { return "\0031,4Hello! Hello!  You've got \0031,4\$0, ${nick}.  You can do better than that!\003" }
                9  { return "\0031,7Hey, how are you doing, ${nick}?  It's TIMTOM.  You currently have \0031,7\$0.  Sorry.  :(\003" }
                10 { return "\00310,5TIMTOM here with your bank statement.  You currently have \00310,5\$0.  :(  Good day, ${nick}.\003" }
            }
        } else {
            switch -- $r {
                1  { return "\0031,7Hey, how are you doing, ${nick}?  It's TIMTOM.  You currently have \0031,7${formatted}.  Good luck!\003" }
                2  { return "\0033,8TIMTOM here!  Are you having fun yet, ${nick}?  I sure hope you are.  You currently have \0033,8${formatted}.\003" }
                3  { return "\0032,10What's the good word, there, ${nick}?  It's TIMTOM.  You currently have \0032,10${formatted}.  Use it wisely.\003" }
                4  { return "\0036,9Howdy Doodie ${nick}!  You currently have \0036,9${formatted}.\003" }
                5  { return "\00311,7TIMTOM here!  Responding to the one and only ${nick}.  You currently have \00311,7${formatted}.\003" }
                6  { return "\0033,5IT'S SO NICE TO HEAR FROM YOU, ${upper}!  You want to know about your money, eh, ${nick}?  Well, you've got \0033,5${formatted}.\003" }
                7  { return "\0038,12TIMTOM here!  Reporting for duty.  ${nick}, you currently have \0038,12${formatted}.  Be good.\003" }
                8  { return "\0031,4Hello! Hello!  You've got \0031,4${formatted}, ${nick}.  Very well then!\003" }
                9  { return "\0031,7Hey, how are you doing, ${nick}?  It's TIMTOM.  You currently have \0031,7${formatted}.  Have a good day.\003" }
                10 { return "\00310,5TIMTOM here with your bank statement.  You currently have \00310,5${formatted}.  Good day, ${nick}.\003" }
            }
        }
        return ""
    }

    proc give {target amount {nick ""}} {
        set nick [whoami $nick]
        # mIRC: "Please, $nick, only whole dollar transfers." if "." isin \$3
        if {[string match "*.*" $amount]} {
            return "\0034,11Please, $nick, only whole dollar transfers.\003"
        }
        if {![string is integer -strict $amount]} {
            return "\0034,11Please, $nick, only whole dollar transfers.\003"
        }
        if {$amount < 0} { return "" }
        set cur [get_money $nick]
        if {$cur < $amount} { return "" }
        # Transfer with \$2 fee that goes into the pot.
        add_money $nick [expr {-$amount}]
        add_money $nick -2
        add_stat $nick pot 2
        inc_state pot 2
        add_money $target $amount
        return "\0038,6HELLO!  HELLO!  TIMTOM here!  Why sure, $nick, I'll transfer \0038,6\$[format_with_commas $amount] over to ${target}'s account.  We also deduct a \0038,6\$2 transfer fee that will be added to the pot.  Thank you for banking with TIMTOM!\003"
    }

    # ========================================================================
    # Spin / Wheel of Fortune
    # ========================================================================

    proc spin {{nick ""}} {
        set nick [whoami $nick]
        if {![stat_allows $nick spin]} {
            return "\0034,11${nick}, please let someone else spin.\003"
        }
        set r [random_int 1 40]
        set msg ""
        switch -- $r {
            1 - 10 - 20 - 30 - 40 {
                set_money $nick 0
                clear_glob "spin_*"
                set_stat $nick spin 0
                set msg "\0034,11${nick}, you get a BANKRUPT!!!\003"
            }
            2 { add_money $nick 500; clear_glob "spin_*"; set msg "\0035,12${nick}, you get \0035,12\$500\003" }
            3 { add_money $nick 400; clear_glob "spin_*"; set msg "\0031,7${nick}, you get \0031,7\$400\003" }
            5 { add_money $nick 5000; clear_glob "spin_*"; set msg "\00311,6${nick}, you get \0038,4\$5000!!!\00311,6 WOW!!!\003" }
            6 { add_money $nick 250; clear_glob "spin_*"; set msg "\0033,7${nick}, you get \0033,7\$250\003" }
            7 { add_money $nick 800; clear_glob "spin_*"; set msg "\0034,1${nick}, you get \0034,1\$800\003" }
            8 { add_money $nick 666; clear_glob "spin_*"; set msg "\0031,7${nick}, you get \0031,7\$666.  That's scary business.\003" }
            11 { add_money $nick 47; clear_glob "spin_*"; set msg "\00314,4${nick}, you get \00314,4\$47.  That's ok, it's better than nothing.\003" }
            12 { add_money $nick 900; clear_glob "spin_*"; set msg "\0038,9${nick}, you get \0038,9\$900\003" }
            15 { add_money $nick 251; clear_glob "spin_*"; set msg "\00311,10${nick}, you get \00311,10\$251\003" }
            16 { add_money $nick 300; clear_glob "spin_*"; set msg "\0033,7${nick}, you get \0033,7\$300\003" }
            17 { add_money $nick 450; clear_glob "spin_*"; set msg "\0034,1${nick}, you get \0034,1\$450\003" }
            18 { add_money $nick 9000; clear_glob "spin_*"; set msg "\0034,14${nick}, you get \0034,14\$9000.  That's a nice hefty amount.\003" }
            21 { add_money $nick 5000; clear_glob "spin_*"; set msg "\0034,11${nick}, you win a trip to Detroit, Michigan!  Good for you!\003" }
            22 { add_money $nick 11000; clear_glob "spin_*"; set msg "\0035,12${nick}, you get \0035,12\$11,000\003" }
            23 { add_money $nick 50; clear_glob "spin_*"; set msg "\0031,7${nick}, you get fifty dollars.\003" }
            25 { add_money $nick 999.99; clear_glob "spin_*"; set msg "\00311,6${nick}, you get \0038,4\$999.99!!!\00311,6 WOW!!!\003" }
            26 { add_money $nick 5000; clear_glob "spin_*"; set msg "\0033,7${nick}, you win a trip to Kenya, Africa.\003" }
            27 { add_money $nick 700; clear_glob "spin_*"; set msg "\0034,1${nick}, you get \0034,1\$700\003" }
            28 { add_money $nick 100; clear_glob "spin_*"; set msg "\0031,7${nick}, you get \0031,7\$100.  Maybe you can buy us all tacos later.\003" }
            31 { add_money $nick 680; clear_glob "spin_*"; set msg "\00314,4${nick}, you get \00314,4\$680.  Do you remember the time you got a million?  That was crazy.  Not this time though.\003" }
            32 { add_money $nick 900; clear_glob "spin_*"; set msg "\0038,9${nick}, you get \0038,9\$900\003" }
            33 { add_money $nick 5000; clear_glob "spin_*"; set msg "\0035,7${nick}, you win a trip to Hawaii!!!!\003" }
            35 { add_money $nick 255; clear_glob "spin_*"; set msg "\00311,10${nick}, you get \00311,10\$255\003" }
            36 { add_money $nick 390; clear_glob "spin_*"; set msg "\0033,7${nick}, you get \0033,7\$390\003" }
            38 { add_money $nick 9000; clear_glob "spin_*"; set msg "\0034,14${nick}, you get \0034,14\$9000.  That's a nice HEEEEFTY amount.\003" }
            4 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\0038,4${nick}, you get LOSE A TURN!!  Sorry about that.\003" }
            9 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\0038,4${nick}, you get LOSE A TURN!!  Sorry about that.\003" }
            14 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\0038,4${nick}, you get LOSE A TURN!!  Still better than bankrupt.\003" }
            19 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\00311,6${nick}, you get LOSE A TURN!!  Whoops, I guess the wheel is rigged.\003" }
            24 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\0038,4${nick}, you get LOSE A TURN!!  Let your secret crush spin next.\003" }
            29 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\0038,4${nick}, you get LOSE A TURN!!  I'm \00311,6 NOT \003\0038,4sorry about that.\003" }
            34 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\0038,4${nick}, you get LOSE A TURN!!  Sorry.\003" }
            39 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "\00311,6${nick}, you get LOSE A TURN!!  Whoops, I guess the wheel is rigged.\003" }
            13 {
                add_money $nick 1000000
                clear_glob "spin_*"
                set_state ok 1
                set msg "\0035,7${nick}, you get \0035,7$who,000,000!!! THAT'S AMAZING!!!\003"
            }
            37 { clear_glob "spin_*"; set msg "\0034,1${nick}, you get \0034,1\$000. LOL.\003" }
            default { set msg "" }
        }
        return $msg
    }

    proc enable_spin {{nick ""}} {
        set nick [whoami $nick]
        del_stat $nick spin
    }

    # ========================================================================
    # Coin flip
    # ========================================================================

    proc flip {{nick ""}} {
        set nick [whoami $nick]
        if {![stat_allows $nick flip]} {
            return "\0034,11$nick, you got your million.  Please let someone else flip now.\003"
        }
        if {[random_int 1 2] == 1} {
            set h [add_stat $nick heads 1]
            set_stat $nick tails 0
            clear_glob "flip_*"
            if {$h == 7} {
                add_money $nick 1000000
                set_stat $nick heads 0
                set_stat $nick flip 0
                set_state ok 1
                return "\0038,7$nick flips HEADS.\003\n\0038,7Wow $nick!!  You got 7 heads in a row!  Here's \0038,7\$1,000,000!\003"
            }
            return "\0038,7$nick flips HEADS.\003"
        } else {
            set t [add_stat $nick tails 1]
            set_stat $nick heads 0
            clear_glob "flip_*"
            if {$t == 7} {
                add_money $nick 1000000
                set_stat $nick tails 0
                set_stat $nick flip 0
                set_state ok 1
                return "\00311,6$nick flips TAILS.\003\n\00311,6Wow $nick!!  You got 7 tails in a row!  Here's \00311,6\$1,000,000!\003"
            }
            return "\00311,6$nick flips TAILS.\003"
        }
    }

    proc bonus {{nick ""}} {
        set nick [whoami $nick]
        if {[get_state ok] eq "1"} {
            add_money $nick 5000
            set_state ok 0
            return "\0030,13OK [string toupper $nick], HERE'S \0030,13\$5000\003"
        }
        return ""
    }

    # ========================================================================
    # Keek - 64 random messages
    # ========================================================================

    proc keek {{nick ""}} {
        set nick [whoami $nick]
        set msgs [list \
            "\0034,11Your name is ${nick}.  My life is a life of horse castles.\003" \
            "\0038,2Your name is ${nick}.  Party favorites go well with wiggly fins.\003" \
            "\0038,1Your name is ${nick}.  We force scarecrows into the space behind cotton pilgrims.\003" \
            "\0037,5Your name is ${nick}.  Do your feet make flat Rothchilds?\003" \
            "\0033,8Your name is ${nick}.  Skeleton handlebars are fastly becoming bacon tin cup freedom tunics.\003" \
            "\0038,2timtom eats a pine tree.\003" \
            "\0036,11timtom likes working in his railroad pants.\003" \
            "\0032,10timtom gives a toy truck to his neighbor Santa.\003" \
            "\0034,11All the fat young giraffes are gathered for a bag-off.\003" \
            "\0031,12Your name is $nick and my name is Geoffrey Giraffee.\003" \
            "\0037,5Your name is ${nick}.  All the piglets are crying over their nose bleeds.\003" \
            "\0033,8Your name is ${nick}.  Cold bowls of rice are waiting just above the paint can rim.\003" \
            "\0038,2timtom made a cloth figurine symbolizing the process of mitosis.\003" \
            "\0036,11timtom spends his Saturdays reading hot sauce packets.\003" \
            "\0032,10TIMTOM HERE!  We make Ronald McDonald bibs for sailors.\003" \
            "\0034,11Will you place your gentle fingers on my spikey larva?\003" \
            "\0031,12Your name is $nick and I just made frosted egg whites.\003" \
            "\0032,10TIMTOM HERE!  The chocolate monsignor is signing autographs in the vestibule.\003" \
            "\0032,10TIMTOM HERE!  The current weather in Austria-Hungary is 11.\003" \
            "\0032,10TIMTOM HERE!  When the toilet paper rolls first came out I was skeptical too.\003" \
            "\0032,10TIMTOM HERE!  GARBAGE! GARBAGE! GARBAGE! JASJAJFADSJFSDFDSBFSDHFSDHFSDHFSHJSFDHJDFSJHFSDHJSDHJSDHJFSDHFSNVNVDSNSUFEYFENDSNJVVllllllllllllllllllllllllllllllll\003" \
            "\0032,10TIMTOM HERE!  These are the days when wine smells like sweet roses.\003" \
            "\0032,10TIMTOM HERE!  The horrible elevator is next to the unhorrible escalator (in happy candy).\003" \
            "\0032,10TIMTOM HERE!  Elton is a pride.\003" \
            "\0032,10TIMTOM HERE!  Cheese can be white, yellow, or orange and your fingers eat themselves dry as a bone my flakiest one.\003" \
            "\0032,10TIMTOM HERE!  O the bells go snap with a white lion cap and the chins of the thrills of kooray.  Take your belt on feet let it pick up the meat and spatula stands for a maid.  O the bells go snap with a white lion cap!\003" \
            "\0032,10TIMTOM HERE!  We make honky horns.\003" \
            "\0031,12Your name is ${nick}.  We all grew up over the icing truck.\003" \
            "\0037,5Your name is ${nick}.  Forget what the sunbirds told you during spanking weather.\003" \
            "\0033,8Your name is ${nick}.  Flowing monster eat my daddy, pull my ribbons over my webbing.  Polar monster they call Vaxmonsky, tear me down from this wall of rabbit bark.\003" \
            "\0038,2timtom discovered that jeans can be used to make paper.\003" \
            "\0036,11timtom spends his Saturdays reading fiction.\003" \
            "\0032,10TIMTOM HERE!  The glass is always perched on a ledge near Franklin's Gower.\003" \
            "\0034,11Ah, the salty bells are ringing again.  Time to waste another helium balloon.\003" \
            "\0031,12Your name is $nick and everyone wants to climb on you.\003" \
            "\0032,10TIMTOM HERE!  LET'S PUT SOME TABLECLOTH ON TOP OF THE PIANO!\003" \
            "\0032,10TIMTOM HERE!  FROZEN HOTDOGS MAKE THE FUNNEST FUNNEL CAKES SINCE I DON'T KNOW NEXT TUESDAY!\003" \
            "\0032,10TIMTOM HERE!  HOSE DOWN THE GARBAGE CAN LID BEFORE WE GET ICY.\003" \
            "\0032,10TIMTOM HERE!  THESE SMALL PARASITES ARE NAMED AFTER ZONING BOARD NOBLEMEN.\003" \
            "\0032,10TIMTOM HERE!  WITHOUT YELLOW LEGS THE SUN WOULD ONLY PROTECT THE 8 PERCENT OF THE POPULATION THAT ACTUALLY TAKES THE TIME TO BREATHE IN THE CHICKEN POX VACCINE THAT WAS STUMBLING EYESHADOW.\003" \
            "\0032,10TIMTOM HERE!  IT'S HARD FOR HITCHHIKERS TO GET PICKED UP ANYMORE.\003" \
            "\0037,2Every hooved animal with pink flesh has a soda function.\003" \
            "\0036,11Your name is ${nick}, and it's no wonder that the wooden play pieces are so smooth.\003" \
            "\0032,4Your name is ${nick}, do you know where the wires to the cabinet are?\003" \
            "\0034,11Your name is ${nick}, \003\0034,1KETCHUP\003\0034,11 AND \003\0038,1MUSTARD\003\0034,11 ::::: \003\0038,1MUSTARD\003\0034,11 AND \003\0037,1MAYONAISE\003" \
            "\0032,10TIMTOM HERE!  I THINK WE ALL LOOK GOOD IN OUR TREEHOUSES.\003" \
            "\0032,10TIMTOM HERE!  QUESTIONS OF SKY AND SEA ARE NOT TO BE ADDRESSED BEFORE MORNING TEA.\003" \
            "\0032,10TIMTON HERE!  EGG YOLKS AND EGG WHITES, JUST SNIFFIN'.\003" \
            "\0033,8Your name is ${nick}, and I do believe you've perfected the leap year.\003" \
            "\0034,2timtom wants a cleaner bathtub for Halloween this year.\003" \
            "\0032,4timtom holds a piglet zygote in the palm of his aqua marine green handshire.\003" \
            "\0034,2timtom rides the lightning bolt upwards.\003" \
            "\0038,2timtom likes when palm readers dictate.\003" \
            "\0032,6TIMTOM HERE!  HOW'S THE CONCRETE STATUE COMING?\003" \
            "\00311,1Your name is ${nick}, and every tungsten tooth will be grinded accordingly.\003" \
            "\0034,11timtom needs a fireproof tee shirt.\003" \
            "\0035,9Your name is ${nick}, $nick the quick.  Welcome to Quackers.\003" \
            "\0038,2timtom eats a bell.\003" \
            "\0038,2timtom eats a bulb.\003" \
            "\0038,2timtom eats a bolt.\003" \
            "\0038,2timtom eats a beet.\003" \
            "\0034,11Your name is ${nick}, \003\0031,4KETCHUP\003\0034,11 AND \003\0031,8MUSTARD\003\0034,11 ::::: \003\0038,1MUSTARD\003\0034,11 AND \003\0037,1MAYONAISE\003" \
            "\0034,11Your name is ${nick}, \003\0034,1KETCHUP\003\0034,11 AND \003\0031,8MUSTARD\003\0034,11 ::::: \003\0031,8MUSTARD\003\0034,11 AND \003\0037,1MAYONAISE\003" \
            "\0034,11Your name is ${nick}, \003\0031,4KETCHUP\003\0034,11 AND \003\0038,1MUSTARD\003\0034,11 ::::: \003\0031,8MUSTARD\003\0034,11 AND \003\0031,7MAYONAISE\003"
        ]
        # 64 entries: index 0..63
        return [lindex $msgs [random_int 0 63]]
    }

    # ========================================================================
    # Drinks (self / for someone)
    # ========================================================================

    proc _drink_msg {nick r} {
        switch -- $r {
            1  { return "4,11timtom brings an ice cold beer for ${nick}" }
            2  { return "8,2timtom pours $nick a shot of whiskey" }
            3  { return "8,1timtom pours $nick a shot of bourbon" }
            4  { return "7,5timtom pours $nick a glass of lemonade" }
            5  { return "3,8timtom brings a bloody mary out for ${nick}" }
            6  { return "8,2timtom pours $nick a glass of milk" }
            7  { return "6,11timtom pours $nick a shot of vodka" }
            8  { return "2,10timtom gives $nick a sip from his beer" }
            9  { return "4,11timtom pours $nick a shot of Robitussin" }
            10 { return "1,12timtom brings $nick a tall glass of water" }
            11 { return "2,7timtom administers a few droplets of GHB to ${target}" }
        }
        return ""
    }

    proc _drink_for_msg {nick target r} {
        switch -- $r {
            1  { return "4,11timtom brings an ice cold beer for ${target}.  You can thank $nick down there." }
            2  { return "8,2timtom pours $target a shot of whiskey.  It's on ${nick}." }
            3  { return "8,1timtom pours $target a shot of bourbon.  This one's on ${nick}." }
            4  { return "7,5timtom pours $target a glass of lemonade" }
            5  { return "3,8timtom brings a bloody mary out for ${target}" }
            6  { return "8,2timtom pours $target a glass of milk" }
            7  { return "6,11timtom pours $target a shot of vodka.  $nick sent it over, by the way." }
            8  { return "2,10timtom gives $target a sip from his beer" }
            9  { return "4,11timtom pours $target a shot of Robitussin" }
            10 { return "1,12timtom brings $target a tall glass of water" }
            11 { return "2,7timtom administers a few droplets of GHB to ${target}." }
        }
        return ""
    }

    proc drink {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2} {
            return "\0034,11Sorry, $nick, you don't have enough money to buy a drink.\003"
        }
        add_money $nick -2
        return [_drink_msg $nick [random_int 1 11]]
    }

    proc drink_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2} {
            return "\0034,11Sorry, $nick, you don't have enough money to buy a drink.\003"
        }
        add_money $nick -2
        return [_drink_for_msg $nick $target [random_int 1 11]]
    }

    # ========================================================================
    # Crab (self / for someone)
    # ========================================================================

    proc crab {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 5} {
            return "\0034,11Sorry, $nick, you don't have enough money to buy a crab dinner.\003"
        }
        add_money $nick -5
        switch -- [random_int 1 7] {
            1 { return "4,11timtom ushers out a steaming plate of crab legs for ${nick}." }
            2 { return "8,2timtom hands $nick a crab rangoon." }
            3 { return "8,1timtom cracks open some crawdads for ${nick}." }
            4 { return "7,5timtom is here with the crab chowda for ${nick}." }
            5 { return "3,8timtom dollops out a healthy serving of crab dip for ${nick}." }
            6 { return "8,2timtom sprinkles some Old Bay seasoning on a plate of trash fische for ${nick}." }
            7 { return "2,10timtom gives $nick some spicy crab fries.  YUUUUMMMMMMMMMMM." }
        }
        return ""
    }

    proc crab_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 5} {
            return "\0034,11Sorry, $nick, you don't have enough money to buy a crab dinner.\003"
        }
        add_money $nick -5
        switch -- [random_int 1 7] {
            1 { return "4,11timtom ushers out a steaming plate of crab legs for ${target}. You can thank $nick down there." }
            2 { return "8,2timtom hands $target a crab rangoon.  It's on ${nick}." }
            3 { return "8,1timtom cracks open some crawdads for ${target}. This one's on ${nick}." }
            4 { return "7,5timtom is here with the crab chowda for ${target}." }
            5 { return "3,8timtom dollops out a healthy serving of crab dip for ${target}. " }
            6 { return "8,2timtom sprinkles some Old Bay seasoning on a plate of trash fische for ${target}. You can high five $nick down there. " }
            7 { return "2,10timtom gives $target some spicy crab fries.  YUUUUMMMMMMMMMMM." }
        }
        return ""
    }

    # ========================================================================
    # Cake / pizza / nachos / lasagna (self & for-target variants)
    # ========================================================================

    proc cake {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 1.95} {
            return "\0034,11Sorry, $nick, but you don't have enough for a piece of cake.  :(\003"
        }
        add_money $nick -1.95
        switch -- [random_int 1 10] {
            1 { return "5,11TIMTOM brings $nick some chocolate cake." }
            2 { return "1,8TIMTOM brings $nick some vanilla cake." }
            3 { return "8,1TIMTOM brings $nick some strawberry shortcake." }
            4 { return "1,10Since $nick has been such an angel, TIMTOM brings $nick some angel food cake." }
            5 { return "8,4TIMTOM brings $nick some banana cake." }
            6 { return "8,2TIMTOM brings $nick a bunt cake." }
            7 { return "6,11TIMTOM brings $nick some delicious cheesecake." }
            8 { return "2,10TIMTOM hands $nick a strawberry cupcake.  Enjoy!" }
            9 { return "1,4Since $nick has been such a little devil, TIMTOM brings $nick some devil's food cake." }
            10 { return "4,12TIMTOM brings $nick a beautiful slice of marble cake." }
        }
        return ""
    }

    proc cake_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 1.95} {
            return "\0034,11Sorry, $nick, but you don't have enough for a piece of cake.  :(\003"
        }
        add_money $nick -1.95
        switch -- [random_int 1 10] {
            1 { return "5,11TIMTOM brings $target some chocolate cake, courtesy of ${nick}." }
            2 { return "1,8TIMTOM brings $target some vanilla cake, courtesy of ${nick}." }
            3 { return "8,1TIMTOM brings $target some strawberry shortcake, courtesy of ${nick}." }
            4 { return "1,10Since $target has been such an angel, TIMTOM brings $target some angel food cake; courtesy of ${nick}." }
            5 { return "8,4TIMTOM brings $target some banana cake, courtesy of ${nick}." }
            6 { return "8,2TIMTOM brings $target a bunt cake, courtesy of ${nick}." }
            7 { return "6,11TIMTOM brings $target some delicious cheesecake, courtesy of ${nick}." }
            8 { return "2,10TIMTOM hands $target a strawberry cupcake, courtesy of ${nick}.  Enjoy!" }
            9 { return "1,4Since $target has been such a little devil, TIMTOM brings $target some devil's food cake; courtesy of ${nick}." }
            10 { return "4,12TIMTOM brings $target a beautiful slice of marble cake, courtesy of ${nick}." }
        }
        return ""
    }

    proc pizza {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.22} {
            return "\0034,11Sorry, $nick, but you don't have enough for a slice of pizza.  :(\003"
        }
        add_money $nick -2.22
        switch -- [random_int 1 8] {
            1 { return "5,11TIMTOM hands $nick a slice of cheese pizza." }
            2 { return "1,8TIMTOM hands $nick a slice of pepperoni pizza." }
            3 { return "8,1TIMTOM hands $nick a slice of sausage pizza." }
            4 { return "8,4TIMTOM hands $nick a slice of ham and pineapple pizza." }
            5 { return "1,13TIMTOM hands $nick a slice of anchovy pizza." }
            6 { return "6,11TIMTOM hands $nick a slice of bacon and black olive pizza." }
            7 { return "2,10TIMTOM hands $nick a slice of extra cheese pizza.  Enjoy!" }
            8 { return "4,12TIMTOM hands $nick a slice of white pizza.  Mangia!" }
        }
        return ""
    }

    proc pizza_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.22} {
            return "\0034,11Sorry, $nick, but you don't have enough for a slice of pizza.  :(\003"
        }
        add_money $nick -2.22
        switch -- [random_int 1 8] {
            1 { return "5,11TIMTOM hands $target a slice of cheese pizza, courtesy of ${nick}." }
            2 { return "1,8TIMTOM hands $target a slice of pepperoni pizza, courtesy of ${nick}." }
            3 { return "8,1TIMTOM hands $target a slice of sausage pizza, courtesy of ${nick}." }
            4 { return "8,4TIMTOM hands $target a slice of ham and pineapple pizza, courtesy of ${nick}." }
            5 { return "1,13TIMTOM hands $target a slice of anchovy pizza, courtesy of ${nick}." }
            6 { return "6,11TIMTOM hands $target a slice of bacon and black olive pizza, courtesy of ${nick}." }
            7 { return "2,10TIMTOM hands $target a slice of extra cheese pizza, courtesy of ${nick}.  Enjoy!" }
            8 { return "4,12TIMTOM hands $target a slice of white pizza, courtesy of ${nick}.  Mangia!" }
        }
        return ""
    }

    proc nachos {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 3.95} {
            return "\0034,11Sorry, ${nick}, but you don't have enough for a Nachos Fun Pack.\003"
        }
        add_money $nick -3.95
        return "\0031,8TIMTOM tosses $nick a Nachos Fun Pack, complete with hot cheese sauce.  Please enjoy!\003"
    }

    proc nachos_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 3.95} {
            return "\0034,11Sorry, ${nick}, but you don't have enough for a Nachos Fun Pack.\003"
        }
        add_money $nick -3.95
        return "\0031,8TIMTOM tosses $target a Nachos Fun Pack, complete with hot cheese sauce; courtesy of ${nick}.  Please enjoy!\003"
    }

    proc lasagna {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.50} {
            return "\0034,11Sorry, ${nick}, but you don't have enough for lasagna.\003"
        }
        add_money $nick -2.50
        return "\0038,4TIMTOM brings out a delicious slice of homemade lasagna for ${nick}.  Bon appetit!\003"
    }

    proc lasagna_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.50} {
            return "\0034,11Sorry, ${nick}, but you don't have enough for lasagna\003"
        }
        add_money $nick -2.50
        return "\0038,4TIMTOM brings out a delicious slice of homemade lasagna for ${target}, courtesy of ${nick}.  Bon appetit!\003"
    }

    proc sauce {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 0.25} {
            return "\0034,11Sorry, $nick, but you don't even have enough to buy sauce.  I'm so sorry. :(\003"
        }
        add_money $nick -0.25
        switch -- [random_int 1 4] {
            1 { return "0,12TIMTOM hands $nick a packet of 1,8MILD0,12 sauce." }
            2 { return "0,12TIMTOM hands $nick a packet of 1,7HOT0,12 sauce." }
            3 { return "0,12TIMTOM hands $nick a packet of 1,4FIRE0,12 sauce." }
            4 { return "0,12TIMTOM places a dollop of 1,3SOUR CREAM0,12 on ${nick}'s taco." }
        }
        return ""
    }

    proc sauce_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 0.25} {
            return "\0034,11Sorry, $nick, but you don't even have enough to buy sauce.  I'm so sorry. :(\003"
        }
        add_money $nick -0.25
        switch -- [random_int 1 4] {
            1 { return "0,12TIMTOM hands $target a packet of 1,8MILD0,12 sauce." }
            2 { return "0,12TIMTOM hands $target a packet of 1,7HOT0,12 sauce." }
            3 { return "0,12TIMTOM hands $target a packet of 1,4FIRE0,12 sauce." }
            4 { return "0,12TIMTOM places a dollop of 1,3SOUR CREAM0,12 on ${target}'s taco." }
        }
        return ""
    }

    proc tacos {{nick ""}} {
        set nick [whoami $nick]
        set ns [channel_nicks]
        set count [llength $ns]
        if {$count == 0} { set count 1 }
        set cost [expr {$count * 0.79}]
        if {[get_money $nick] < $cost} {
            return "\0034,11Sorry, ${nick}, but you don't have enough to buy tacos for everyone.  It was noble of you to be thinking of everyone though.  Maybe try your luck on the wheel?\003"
        }
        add_money $nick [expr {-$cost}]
        set who [join $ns " "]
        if {[random_int 1 2] == 1} {
            return "\0037,5TIMTOM brings out the crunchy tacos for $who¡Buen provecho, amigos!"
        }
        return "\0037,5TIMTOM brings out the soft tacos for $who¡Buen provecho, amigos!"
    }

    proc empanadas {{nick ""}} {
        set nick [whoami $nick]
        set ns [channel_nicks]
        set count [llength $ns]
        if {$count == 0} { set count 1 }
        set cost [expr {$count * 0.89}]
        if {[get_money $nick] < $cost} {
            return "\0034,11Sorry, ${nick}, but you don't have enough to buy empanadas for everyone.  It was noble of you to be thinking of everyone though.  Maybe try your luck on the wheel?\003"
        }
        add_money $nick [expr {-$cost}]
        set who [join $ns " "]
        if {[random_int 1 2] == 1} {
            return "\0037,5TIMTOM brings out the \003\0032,9EMPANADAS\003\0037,5 for $who¡Buen provecho, amigos!"
        }
        return "\0037,5TIMTOM brings out the \003\00311,3EMPANADAS\003\0037,5 for $who¡Buen provecho, amigos!"
    }

    # ========================================================================
    # Dice (line 2653 in TIMTOM.txt)
    #
    # The mIRC version pre-rolls a pair of dice when "dice" is typed,
    # opens a 40-second betting window, schedules a series of
    # countdown/result/payout messages spanning 80 seconds, and clears
    # the per-game state via a timer at +80s. The trigger framework here
    # can dispatch a string at a delay, but it can't run TCL code on
    # timer fire, so the per-game cleanup is done lazily on the next
    # `dice` call (any state older than 80 s is treated as expired) and
    # money payouts are credited immediately rather than at +40s. The
    # announcement and result messages still arrive at the correct
    # mIRC-faithful offsets.
    # ========================================================================

    proc dice {{nick ""}} {
        set nick [whoami $nick]
        set tchan [_timer_chan]
        # Cooldown: refuse to start a new game until the previous one's
        # 80 s window has elapsed.
        set started [get_state dice_started]
        if {$started ne ""} {
            set elapsed [expr {[clock seconds] - $started}]
            if {$elapsed < 80} { return "" }
        }
        # Pre-roll. Bets placed during the next 40 s know the outcome
        # already; users don't, because the result is announced at +40s.
        set d1 [random_int 1 6]
        set d2 [random_int 1 6]
        set total [expr {$d1 + $d2}]
        set_state dice1 $d1
        set_state dice2 $d2
        set_state dicetotal $total
        set_state dice 1
        set_state edice 1
        set_state dice_started [clock seconds]
        clear_glob "bet*_*"

        if {$tchan ne ""} {
            timers schedule $tchan "\0031,7Let's get those bets in friends!\003" 5000
            timers schedule $tchan "\0031,730 seconds to get those bets in!\003" 10000
            timers schedule $tchan "\0031,720 seconds to bet and counting!  Hurry hurry hurry!\003" 20000
            timers schedule $tchan "\0038,410 seconds to bet!!!!!  Last call!!!!!  Get em in, friends!!!!!\003" 30000
            timers schedule $tchan "\0031,7Ok, all bets are in!\003" 40000
            timers schedule $tchan "\0031,7I rolled $d1 and $d2 for a total of ${total}...Calculating payoffs....Please wait a moment....\003" 40000
            timers schedule $tchan "\0032,8Thanks for playing dice, friends!  I'll be waiting for you to play again real soon!\003" 80000
        }

        return "\0031,7Welcome to dice!  My name is TIMTOM and I will be throwing one pair of dice.  Everyone is encouraged to make bets on the total value.  The totals range from 2 to 12.  The min bet is \0031,7\$5000 and the max bet is \0031,7\$20,000 (only whole dollar amounts please).  The betting syntax is \"bet 5000 on 7\" for example.  The payouts are: 35:1 on 2 or 12, 17:1 on 3 or 11, 11:1 on 4 or 10, 8:1 on 5 or 9, 6:1 on 6 or 8, and 5:1 on 7.\003\n\0034,11Please make your bets now.  You can bet on as many numbers as you'd like, but please make only one bet per number.  TIMTOM gets confused easily ;P\003"
    }

    # Returns the number of milliseconds left until the +40 s "result" mark.
    # Used to schedule per-bet outcome lines so they arrive at the right time.
    proc _dice_payout_delay {} {
        set started [get_state dice_started]
        if {$started eq ""} { return 0 }
        set elapsed_ms [expr {([clock seconds] - $started) * 1000}]
        set remaining [expr {40000 - $elapsed_ms}]
        if {$remaining < 0} { return 0 }
        return $remaining
    }

    # `bet <amount> on <total>` - dice bet placement.
    proc dice_bet {amount target {nick ""}} {
        set nick [whoami $nick]
        set tchan [_timer_chan]

        # Sanity-check amount and target before hitting the game state.
        if {[string match "*.*" $amount]} {
            return "\0034,11Please, $nick, only whole dollar bets. :)\003"
        }
        if {![string is integer -strict $amount]} {
            return "\0034,11Please, $nick, only whole dollar bets. :)\003"
        }
        if {[string match "*.*" $target] || ![string is integer -strict $target] || $target < 2 || $target > 12} {
            return "\0034,11Sorry, $nick, that is not a valid dice total.  Use whole numbers ranging from 2 to 12.\003"
        }
        if {[get_state dice] ne "1"} { return "" }
        if {$amount < 5000 || $amount > 20000} {
            return "\0034,11$nick, the min bet is \0034,11\$5000 and the max bet is \0034,11\$20,000.\003"
        }
        if {[get_money $nick] < $amount} {
            return "\0034,11Sorry, $nick, you don't have enough money to make that bet. :(\003"
        }
        set bet_key "bet${target}_[string tolower $nick]"
        if {[get_state $bet_key] == 1} {
            return "\0034,11Um...$nick....I'm pretty sure you already bet on $target.  I could be wrong though. ;P\003"
        }

        set_state $bet_key 1
        add_money $nick [expr {-$amount}]

        set total [get_state dicetotal]
        set delay [_dice_payout_delay]
        set amount_str [format_with_commas $amount]
        if {$total == $target} {
            # Win: payout multiplier from the mIRC schedule.
            if {$total == 2 || $total == 12} {
                set mul 35
            } elseif {$total == 3 || $total == 11} {
                set mul 17
            } elseif {$total == 4 || $total == 10} {
                set mul 11
            } elseif {$total == 5 || $total == 9} {
                set mul 8
            } elseif {$total == 6 || $total == 8} {
                set mul 6
            } else {
                set mul 5
            }
            set winnings [expr {$amount * $mul}]
            # The framework can't run code on timer fire, so credit the
            # win immediately. Result line still arrives at +40 s.
            add_money $nick [expr {$amount + $winnings}]
            set winnings_str [format_with_commas $winnings]
            if {$tchan ne ""} {
                timers schedule $tchan "\0038,5Let's see here...$nick bet \0038,5\$$amount_str on $target. \00311,6You win $nick!\0038,5 TIMTOM pays $mul:1 odds for a total of \0038,5\$${winnings_str}.\003" $delay
            }
        } else {
            # Loss: amount goes to the pot.
            add_stat $nick pot $amount
            inc_state pot $amount
            if {$tchan ne ""} {
                timers schedule $tchan "\0038,5Let's see here...$nick bet \0038,5\$$amount_str on $target.  Sorry, $nick, that's a losing bet.  TIMTOM puts \0038,5\$${amount_str} into the pot.\003" $delay
            }
        }

        return "\0034,11$nick bets \0034,11\$$amount_str on $target.\003"
    }

    proc unicorn_buy {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 5000} {
            return "\0034,11Sorry, ${nick}, but you don't have enough right now for a unicorn.  I know, unicorns are expensive, but they are well worth it in the end.  Hopefully you'll have enough to buy one soon.\003"
        }
        add_money $nick -5000
        add_stat $nick unicorn 1
        return [_unicorn_message $nick]
    }

    proc _unicorn_message {nick} {
        switch -- [random_int 1 4] {
            1 { return "0,2Suddenly, out pops a beautiful unicorn for ${nick}. 4,4.8,8.12,12.2,2.9,9.6,6.4,4.5,5.11,11.6,6.7,7.11,11.2,2." }
            2 { return "0,2Flying through the sky is a magical unicorn for ${nick}. 4,4.6,6.7,7.11,11.2,2.10,10.9,9.3,3.12,12.5,5.6,6.11,11." }
            3 { return "0,12Here comes your very own unicorn, ${nick}! 11,11.4,4.7,7.6,6.3,3.10,10.8,8.2,2.6,6.11,11.7,7.9,9." }
            4 { return "0,2Hmmm...how about....a.....UNICORN??!! 12,12.3,3.4,4.5,5.6,6.11,11.7,7.6,6.3,3.10,10.8,8.2,2.4,4." }
        }
        return "\0030,12Here comes your very own unicorn, $nick!\003"
    }

    proc prices {{nick ""}} {
        set nick [whoami $nick]
        set ch [current_channel]
        return "\0035,10Hello ${nick}!  These are the current market prices in ${ch}:  drinks are \0035,10\$2 a piece, cake is \0035,10$who.95 a piece, and pizza is \0035,10\$2.22 a slice.  Homemade lasagna is \0035,10\$2.50.  Tacos are \0035,10$.79 \0035,10/ person (all non-ops served), and sauce is \0035,10$.25 extra.  A Nachos Fun Pack costs \0035,10\$3.95.  A pony will cost you \0035,10\$1000 and a unicorn will cost you \0035,10\$5000.  As always; soup, coffee, and tea are free; as are all of our other services.  Enjoy!\003"
    }

    proc hedges {{nick ""}} {
        set nick [whoami $nick]
        set u [string toupper $nick]
        switch -- [random_int 1 3] {
            1 { return "0,3THE HEDGES ARE HIGH TODAY, [string toupper $nick].  I'LL GO AHEAD AND TRIM THEM THEN." }
            2 { return "0,3THE HEDGES ARE LOOKIN' A BIT LOW TODAY, [string toupper $nick].  I'D BETTER WATER THEM SOME." }
            3 { return "0,3THE HEDGES ARE JUST FINE TODAY, [string toupper $nick].  SUCH BEAUTIFUL HEDGES WE HAVE." }
        }
        return ""
    }

    proc end_msg {} {
        switch -- [random_int 1 17] {
            1 { return "4,11Yes is no." }
            2 { return "8,2The truth is a lie." }
            3 { return "8,1Black is white." }
            4 { return "7,5Weak is strong." }
            5 { return "3,8Love is hate." }
            6 { return "8,2Life is death." }
            7 { return "6,11To win is to lose." }
            8 { return "2,10The beginning is the end." }
            9 { return "4,11First is last." }
            10 { return "1,12All are none." }
            11 { return "7,5Slavery is freedom." }
            12 { return "3,8To be blind is to see." }
            13 { return "8,2Everything is nothing." }
            14 { return "6,11Night is day." }
            15 { return "2,10Day is night." }
            16 { return "4,11Pain is pleasure." }
            17 { return "1,12The best is the worst." }
        }
        return ""
    }

    proc life {{nick ""}} {
        set nick [whoami $nick]
        set r [random_int 1 40]
        if {$r == 1} { return "$nick gets 1 life point.  Sorry :(" }
        return "$nick gets $r life points."
    }

    # ========================================================================
    # Ponies / unicorns inventory
    # ========================================================================

    proc my_ponies {{nick ""}} {
        set nick [whoami $nick]
        set count [get_stat $nick pony]
        if {$count == 0} {
            return "\0038,12HELLO HELLO HELLO HELLO $nick!  Right now you don't have any ponies.  I really want you to get one though!!\003"
        } elseif {$count == 1} {
            return "\0038,12HELLO HELLO HELLO HELLO $nick!  Currently you have 1 pony.  Do you love your pony?\003"
        }
        return "\0038,12HELLO HELLO HELLO HELLO $nick!  Currently you have [format_with_commas $count] ponies.\003"
    }

    proc my_unicorns {{nick ""}} {
        set nick [whoami $nick]
        set count [get_stat $nick unicorn]
        if {$count == 0} {
            return "\0030,2Hey $nick.  Right now you don't have any unicorns.  :(  Keep saving up!  Unicorns are well worth the wait.\003"
        } elseif {$count == 1} {
            if {[random_int 1 2] == 1} {
                return "\0030,2Hey $nick.  Currently you have 1 unicorn.  And what a beautiful horn she has!\003"
            }
            return "\0030,2Hey $nick.  Currently you have 1 unicorn.  And what a beautiful horn he has!\003"
        }
        return "\0030,2Hey $nick.  Currently you have [format_with_commas $count] unicorns.\003"
    }

    proc check_others_ponies {target {nick ""}} {
        set nick [whoami $nick]
        set count [get_stat $target pony]
        if {$count == 0} {
            return "\0038,12HELLO HELLO HELLO HELLO $nick!  Right now $target doesn't have any ponies.  We're all pulling for $target right now!!\003"
        } elseif {$count == 1} {
            return "\0038,12HELLO HELLO HELLO HELLO $nick!  Currently $target has 1 pony.  What a cute little pony!\003"
        }
        return "\0038,12HELLO HELLO HELLO HELLO $nick!  Currently $target has [format_with_commas $count] ponies.\003"
    }

    proc check_others_unicorns {target {nick ""}} {
        set nick [whoami $nick]
        set count [get_stat $target unicorn]
        if {$count == 0} {
            return "\0030,2Hey $nick.  Right now, $target doesn't have any unicorns.  :(\003"
        } elseif {$count == 1} {
            if {[random_int 1 2] == 1} {
                return "\0030,2Currently $target has 1 unicorn.  And what a beautiful horn she has!\003"
            }
            return "\0030,2Currently $target has 1 unicorn.  And what a beautiful horn he has!\003"
        }
        return "\0030,2Currently $target has [format_with_commas $count] unicorns.\003"
    }

    proc check_others_money {target {nick ""}} {
        set nick [whoami $nick]
        set amount [get_money $target]
        if {$amount == 0} {
            return "\0038,6HELLO $nick! Right now $target doesn't have any money. We're all pulling for $target right now!!\003"
        }
        return "\0038,6HELLO $nick! Currently $target has [format_money $amount].\003"
    }

    proc buy_pony {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 1000} {
            return "\0034,11Sorry, ${nick}, but you don't have enough for a pony.  I really want you to have one, though.  Try your luck at the wheel!\003"
        }
        add_money $nick -1000
        add_stat $nick pony 1
        return "\0037,10Finally, $nick gets a pony.\003"
    }

    proc timtom_unicorns {{nick ""}} {
        return "\0030,2$who always has 8 unicorns.  Never fear!\003"
    }
    proc timtom_pony {{nick ""}} {
        set nick [whoami $nick]
        return "\0033,11HELLO HELLO HELLO HELLO ${nick}!  Don't you know that $who always has 8 ponies?  That's why he gives so many away.\003"
    }

    # ========================================================================
    # Marry / divorce
    # ========================================================================

    proc marry {{nick ""}} {
        set nick [whoami $nick]
        set k [nick_count]
        if {$k < 1} { set k 1 }
        set j [expr {$k + 2}]
        set i [expr {$k + 1}]
        set r [random_int 1 $j]
        if {$r == $j} { return "\0038,4${nick} marries the Dark Lord Satan\003" }
        if {$r == $i} { return "\0034,1${nick} marries a garbage can\003" }
        set partner [random_other_nick $nick]
        if {$partner eq ""} { set partner [random_chan_nick] }
        if {$partner eq ""} { set partner "themselves" }
        set colors [list "4,8" "5,11" "9,13" "2,8" "8,1" "2,10" "6,9" "1,12" "4,11" "8,6"]
        set color [lindex $colors [expr {int(rand() * [llength $colors])}]]
        return "\003${color}${nick} marries ${partner}\003"
    }

    proc marry_target {target {nick ""}} {
        set nick [whoami $nick]
        set colors [list "4,8" "5,11" "9,13" "2,8" "8,1" "2,10" "6,9" "1,12" "4,11" "8,6"]
        set color [lindex $colors [expr {int(rand() * [llength $colors])}]]
        return "\003${color}${nick} marries ${target}\003"
    }

    proc divorce {{nick ""}} {
        set nick [whoami $nick]
        set k [nick_count]
        if {$k < 1} { set k 1 }
        set j [expr {$k + 2}]
        set i [expr {$k + 1}]
        set r [random_int 1 $j]
        if {$r == $j} { return "\0038,4${nick} divorces Satan, Master of Darkness\003" }
        if {$r == $i} { return "\0034,2${nick} divorces a wet samburger\003" }
        set partner [random_other_nick $nick]
        if {$partner eq ""} { set partner [random_chan_nick] }
        if {$partner eq ""} { set partner "themselves" }
        set colors [list "9,3" "8,5" "9,14" "2,8" "8,4" "7,10" "6,9" "1,12" "12,11" "8,3"]
        set color [lindex $colors [expr {int(rand() * [llength $colors])}]]
        return "\003${color}${nick} divorces ${partner}\003"
    }

    proc divorce_target {target {nick ""}} {
        set nick [whoami $nick]
        set colors [list "9,3" "8,5" "9,14" "2,8" "8,4" "7,10" "6,9" "1,12" "12,11" "8,3"]
        set color [lindex $colors [expr {int(rand() * [llength $colors])}]]
        return "\003${color}${nick} divorces ${target}\003"
    }

    # ========================================================================
    # Stare (recurring timer messages)
    # ========================================================================

    proc stare {{nick ""}} {
        set nick [whoami $nick]
        return [stare_at $nick]
    }

    proc stare_at {target {nick ""}} {
        set tchan [_timer_chan]
        set msg "\0034,11TIMTOM IS STARING AT [string toupper $target]\003"
        # Schedule 10 follow-up stares 11 seconds apart (mIRC `timertowel 10 11`).
        if {$tchan ne ""} {
            catch {timers schedule $tchan $msg 11000 10 11000}
        }
        return $msg
    }

    # ========================================================================
    # Bong / clean bong
    # ========================================================================

    proc bong {{nick ""}} {
        set nick [whoami $nick]
        if {![is_stoner $nick]} {
            return "\0033,8Sorry, $nick, you are not allowed to touch the bong.\003"
        }
        return "\0039,3TIMTOM passes the bong to $nick.  Enjoy friend.\003"
    }

    proc bong_for {target {nick ""}} {
        set nick [whoami $nick]
        if {![is_stoner $nick]} {
            return "\0033,8Sorry, $nick, you are not allowed to touch the bong.\003"
        }
        return "$nick passes the bong to $target."
    }

    proc clean_bong {{nick ""}} {
        set nick [whoami $nick]
        if {![is_stoner $nick]} {
            return "\0033,8Sorry, $nick, you are not allowed to touch the bong.\003"
        }
        return "\0030,7TIMTOM HERE!  That water's looking pretty nasty.  Let me change that for you.\003"
    }

    proc is_stoner {nick} {
        # Membership in the stoners club is tracked per-nick.
        return [expr {[get_stat $nick stoner] == 1}]
    }

    # ========================================================================
    # Story
    # ========================================================================

    proc story_start {{nick ""}} {
        set nick [whoami $nick]
        set_state story 1
        return "\0031,11Hello, ${nick}, I understand that you would like to hear a story now.  This would be my utmost pleasure.  To begin the story please type \"begin\".\003"
    }

    proc story_begin {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state story]
        inc_state story 1
        if {$s eq "1"} {
            return "\0032,7Once upon a time there lived a lucky little boy named Lucky.  His favorite thing to do was to collect springs.  If you want to hear more of the story type \"more\".\003"
        }
        return "\0034,11Sorry, friend, you must be in storytime mode to use that command. :(\003"
    }

    proc story_more {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state story]
        inc_state story 1
        switch -- $s {
            2 { return "\0032,7He was a very good boy and always listened to his mommy and daddy.  If you want to hear more of the story type \"more\".\003" }
            3 { return "\0032,7One day he decided to go for a walk by the Happy Boy Tree.  If you want to hear more of the story type \"more\".\003" }
            4 { return "\0032,7The tree was very big and full of so many leaves.  If you want to hear more of the story type \"more\".\003" }
        }
        return "\0034,11Sorry, friend, you must be in storytime mode to use that command. :(\003"
    }

    # ========================================================================
    # Stoners club
    # ========================================================================

    proc stoners {{nick ""}} {
        set_state stoner 1
        return "\0039,3Why hello there good friend!  Thanks for inquiring about the Official #gamme Stoners' Club.  If you would like to become a member please type \"\003\0031,8yes\003\0039,3\".  If you do not wish to become a member, or if you are currently a member but no longer want to be one, please type \"\003\0031,8no\003\0039,3\".  Only members of the Official #gamme Stoners' Club are allowed to touch the bong.\003"
    }

    proc yes_cmd {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state stoner]
        set_state stoner 2
        if {$s eq "1"} {
            set_stat $nick stoner 1
            return "\0034,11You are now a member of the Official #gamme Stoners' Club.\003"
        }
        return "\0031,4scalar piegs\003"
    }

    proc no_cmd {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state stoner]
        set_state stoner 2
        if {$s eq "1"} {
            set_stat $nick stoner 0
            return "\0034,11You are \0031,4not\0034,11 a member of the Official #gamme Stoners' Club.\003"
        }
        return "\0031,4bufferlo piegs\003"
    }

    # ========================================================================
    # Pot lookups
    # ========================================================================

    proc pot_show {{nick ""}} {
        set p [get_state pot]
        if {$p eq "" || $p == 0} { return "\0034,11The pot is currently empty.\003" }
        return "\0034,11The pot is currently at \0034,11$[format_with_commas $p].  Somebody better win it soon!\003"
    }

    proc my_pot {{nick ""}} {
        set nick [whoami $nick]
        set p [get_state pot]
        if {$p eq "" || $p == 0} { return "\0034,11The pot is currently empty.\003" }
        set mine [get_stat $nick pot]
        if {$mine == 0} { return "\0037,2How are you doing, ${nick}?  Currently, none of the pot has come out of your pockets!\003" }
        if {$mine == $p} { return "\0031,4Would you look at that, ${nick}!!  The current pot has come from you and you alone!\003" }
        set pct [expr {round(double($mine) / double($p) * 10000.0) / 100.0}]
        return "\0037,2Howdy ${nick}.  At the moment, approximately ${pct}*100),2)\0037,2% of the pot is yours.\003"
    }

    proc check_others_pot {target {nick ""}} {
        set nick [whoami $nick]
        set p [get_state pot]
        if {$p eq "" || $p == 0} { return "\0034,11The pot is currently empty.\003" }
        set theirs [get_stat $target pot]
        if {$theirs == 0} {
            return "\0037,2How are you doing, $nick?  Currently, $target hasn't contributed anything to the pot.\003"
        }
        if {$theirs == $p} {
            return "\0031,4What's up, $nick?  Right now the entire pot has come from $target. lol.\003"
        }
        set pct [expr {round(double($theirs) / double($p) * 10000.0) / 100.0}]
        return "\0037,2Howdy $nick.  At the moment, approximately ${pct}% of the pot has come from $target.\003"
    }

    # ========================================================================
    # Hide / Seek (simplified pony hunt)
    # ========================================================================

    proc hide_cmd {{nick ""}} {
        set ch [current_channel]
        set_state hide 1
        clear_glob "red_*"
        clear_glob "blue_*"
        clear_glob "yellow_*"
        clear_glob "bblue_*"
        clear_glob "mayo_*"
        clear_glob "out_*"
        return "\0031,13TIMTOM has lost control of his 5 favorite colors:\003  \0034,4..........\003 , \00311,11..........\003 , \0038,8..........\003 , \00312,12..........\003 , and \0037,7..........\003.  \0031,13They've gone and hid on everyone in ${ch}!  We need to find them quick!\003"
    }

    proc seek {{nick ""}} {
        set nick [whoami $nick]
        if {[get_state hide] ne "1"} {
            return ""
        }
        if {[get_stat $nick out] == 1} {
            return "\0034,11Sorry, $nick, you're out for this game.\003"
        }
        set k [nick_count]
        if {$k < 1} { set k 1 }
        set j [expr {$k + 2}]
        set i [expr {$k + 1}]
        set r [random_int 1 $j]
        set a [random_int 1 5]

        if {$r == $j} {
            set_stat $nick out 1
            return "$nick finds: Satan, Destroyer of Souls Sorry, $nick, you're out for this game.  Better luck next game!"
        }

        # ChanServ branch (r == i): finds a colour from ChanServ.
        if {$r == $i} {
            set finder "ChanServ"
            switch -- $a {
                1 { set color red;    set msg "$nick finds: $finder Hey Channel Servant!  You're looking rather ravishing in RED!" }
                2 { set color blue;   set msg "$nick finds: $finder That shade of BLUE really toots you well!  Toot toot!" }
                3 { set color yellow; set msg "$nick finds: $finder You're an owl here in your YELLOW gear!" }
                4 { set color bblue;  set msg "$nick finds: $finder Your chives are as deep as the deep BLUE sea." }
                5 { set color mayo;   set msg "$nick finds: $finder Check out the channel master struttin' around in those MAYONAISE colored shoes!" }
            }
        } else {
            set who [random_other_nick $nick]
            if {$who eq ""} { set who [random_chan_nick] }
            switch -- $a {
                1 { set color red;    set msg "$nick finds: $who  Well aren't you looking ravishing in RED?" }
                2 { set color blue;   set msg "$nick finds: $who  That shade of BLUE really suits you well." }
                3 { set color yellow; set msg "$nick finds: $who  You're a star here in your YELLOW gear!" }
                4 { set color bblue;  set msg "$nick finds: $who  Your eyes are as deep as the deep BLUE sea." }
                5 { set color mayo;   set msg "$nick finds: $who  Check out $who struttin' around in those MAYONAISE colored shoes!" }
            }
        }

        set_stat $nick $color 1
        # Win if all five colors set.
        set won 1
        foreach c {red blue yellow bblue mayo} {
            if {[get_stat $nick $c] != 1} { set won 0; break }
        }
        if {$won} {
            set ponies ""
            for {set p 0} {$p < 20} {incr p} {
                add_stat $nick pony 1
                append ponies "\nFinally, $nick gets a pony."
            }
            clear_glob "red_*"
            clear_glob "blue_*"
            clear_glob "yellow_*"
            clear_glob "bblue_*"
            clear_glob "mayo_*"
            clear_glob "out_*"
            del_state hide
            append msg "\nAwesome work, $nick, you win!!!  You found .......... , .......... , .......... , .......... , and ........... Here's 20 ponies to add to your collection.$ponies"
        }
        return $msg
    }

    # ========================================================================
    # Guess game
    # ========================================================================

    proc guess_start {{nick ""}} {
        set nick [whoami $nick]
        set p [get_state pot]
        if {$p eq ""} { set p 0 }
        set mine [get_stat $nick pot]
        if {[get_stat $nick eguess] == 1} {
            return "\0034,11Sorry, $nick, you need to finish the game you started. >:D\003"
        }
        if {$p > 0 && $mine == $p} {
            return "\0031,4Sorry, $nick, but all of the pot has come from you.  Try winning the pot at another time, when other people have contributed as well.\003"
        }
        if {$p > 0 && double($mine) / double($p) > 0.75} {
            set pct [expr {round(double($mine) / double($p) * 10000.0) / 100.0}]
            return "\0031,4Sorry, $nick, but approximately ${pct}% of the pot is yours.  That's a bit too much.  Try winning the pot at another time, when the money is more evenly distributed.\003"
        }
        set_stat $nick guess 1
        set_stat $nick number [random_int 1 20]
        return "\00313,2Hey $nick.  I'm thinking of a number between 1 and 20.  You have 5 tries to guess it.  Figure it out and you win THE POT!!!!!  Miss it, and you're \00311,6BANKRUPT!!!!!\003"
    }

    proc guess_attempt {number {nick ""}} {
        set nick [whoami $nick]
        if {![string is integer -strict $number]} { return "" }
        set g [get_stat $nick guess]
        if {$g < 1} { return "" }
        if {$number < 1 || $number > 20} { return "" }
        set secret [get_stat $nick number]
        set p [get_state pot]
        if {$p eq ""} { set p 0 }

        if {$number == $secret} {
            set rewards ""
            if {$g == 1} {
                # First-try win: 10 bonus unicorns plus pot or pony.
                if {$p > 0} {
                    add_money $nick $p
                    set head "WOW YOU GOT IT [string toupper $nick]!  IT WAS $number!!!  AND ON YOUR FIRST TRY!!!  YOU WIN THE POT AND SINCE YOU GOT IT ON YOUR FIRST TRY HERE'S 10 BONUS UNICORNS!!!!"
                } else {
                    add_stat $nick pony 1
                    append rewards "\nFinally, $nick gets a pony."
                    set head "WOW YOU GOT IT [string toupper $nick]!  IT WAS $number!!!  AND ON YOUR FIRST TRY!!!  SINCE THE POT IS EMPTY, YOU GET 1 PONY, AND SINCE YOU GOT IT ON YOUR FIRST TRY HERE'S 10 BONUS UNICORNS!!!!"
                }
                for {set k 0} {$k < 10} {incr k} {
                    add_stat $nick unicorn 1
                    append rewards "\n[_unicorn_message $nick]"
                }
                _clear_guess_state $nick
                return "${head}${rewards}"
            } elseif {$g < 5} {
                _clear_guess_state $nick
                if {$p > 0} {
                    add_money $nick $p
                    return "\0038,3Yep, it was $number.  Congratulations $nick.  You win the pot!\003"
                }
                add_stat $nick pony 1
                return "\0038,3Yep, it was $number.  Congratulations $nick.  Since the pot is empty, here's a pony. ;)\003\n\0037,10Finally, $nick gets a pony.\003"
            } else {
                _clear_guess_state $nick
                if {$p > 0} {
                    add_money $nick $p
                    return "\0038,3Yep, you got it, it was $number.  And on your last guess - phew!  Congrats $nick.  You win the pot!\003"
                }
                add_stat $nick pony 1
                return "\0038,3Yep, you got it, it was $number.  And on your last guess - phew!  Congrats $nick.  Since the pot is empty, please accept this pony as a consolation prize.\003\n\0037,10Finally, $nick gets a pony.\003"
            }
        }
        # Wrong guess
        if {$g < 5} {
            add_stat $nick guess 1
            set_stat $nick eguess 1
            return "\0038,3Sorry, $nick, that's not it.  Keep guessing.\003"
        }
        set_money $nick 0
        _clear_guess_state $nick
        return "\0038,3Sorry, $nick, that's not it.  The correct number is $secret :( :(  I hate to do it to you, but you're \00311,6BANKRUPT!\0038,3  I'm sooooo sorry!!!\003"
    }

    proc _clear_guess_state {nick} {
        del_stat $nick guess
        del_stat $nick eguess
        del_stat $nick number
        # Pot is reset on win/bankrupt-from-guess.
        del_state pot
        clear_glob "pot_*"
    }

    # ========================================================================
    # Blackjack (synchronous, single-player)
    # ========================================================================

    proc blackjack_start {{nick ""}} {
        set nick [whoami $nick]
        set_stat $nick blackjack 2
        del_stat $nick ttotal
        return "\0031,8WELCOME TO \003\0034,0BLACK\003\0031,8 \003\0031,0JACK\003\0031,8 [string toupper $nick]!  I'm your dealer TIMTOM.  My goal is to give you an enjoyable \003\0034,0BLACK\003\0031,8 \003\0031,0JACK\003\0031,8 experience.  Drinks and tacos and everything else are right here - just ask, silly!  Now please place your bet and we'll get started.  The min bet is \0031,8\$5000 and the max bet is \0031,8\$20,000.  Please keep the bets in whole dollar amounts, no small change in this casino.  Good luck!\003"
    }

    proc _card_value {raw} {
        # Returns {value name}.
        switch -- $raw {
            1   { return [list 11 "Ace"] }
            11  { return [list 10 "Jack"] }
            12  { return [list 10 "Queen"] }
            13  { return [list 10 "King"] }
            default { return [list $raw $raw] }
        }
    }

    proc blackjack_bet {amount {nick ""}} {
        set nick [whoami $nick]
        set state [get_stat $nick blackjack]
        if {$state != 2} { return "" }
        if {[string match "*.*" $amount]} {
            return "\0034,11Please, $nick, only whole dollar bets.\003"
        }
        if {![string is integer -strict $amount]} {
            return "\0034,11Please, $nick, only whole dollar bets.\003"
        }
        if {$amount < 5000 || $amount > 20000} {
            return "\0034,11$nick, the min bet is \0034,11\$5,000 and the max bet is \0034,11\$20,000.\003"
        }
        if {[get_money $nick] < $amount} {
            return "\0034,11Sorry, $nick, but you don't have enough to bet that much.  :(\003"
        }
        add_money $nick [expr {-$amount}]
        set_stat $nick bet $amount

        # Deal cards (1..13).
        lassign [_card_value [random_int 1 13]] d1v d1n
        lassign [_card_value [random_int 1 13]] d2v d2n
        lassign [_card_value [random_int 1 13]] c1v c1n
        lassign [_card_value [random_int 1 13]] c2v c2n

        set_stat $nick dealer1 $d1v
        set_stat $nick dealer2 $d2v
        set_stat $nick ddealer1 $d1n
        set_stat $nick ddealer2 $d2n
        set_stat $nick card1 $c1v
        set_stat $nick card2 $c2v
        set_stat $nick ccard1 $c1n
        set_stat $nick ccard2 $c2n
        set total [expr {$c1v + $c2v}]
        set_stat $nick total $total

        # Player blackjack.
        if {($c1v == 11 && $c2v == 10) || ($c1v == 10 && $c2v == 11)} {
            add_money $nick [expr {$amount * 2}]
            set bonus_lines ""
            for {set k 0} {$k < 35} {incr k} {
                add_stat $nick unicorn 1
                append bonus_lines "\n[_unicorn_message $nick]"
            }
            _bj_clear $nick
            return "\0035,7Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you 21!  YOU GOT \003\0034,11BLACKJACK\003\0035,7!!!!  Congratulations $nick!  Dealer pays \0035,7\$[format_with_commas $amount] and you also receive 35 bonus \0030,2UNICORNS!!!!! \0034,11YAY!!!!\003$bonus_lines"
        }

        # Two aces.
        if {$c1v == 11 && $c2v == 11} {
            set_stat $nick total 12
            set_stat $nick ttotal 2
            add_stat $nick blackjack 1
            return "\0035,7Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you 2 or 12.  Do you want to hit or stand?\003"
        }

        # One ace.
        if {$c1v == 11 || $c2v == 11} {
            set tt [expr {$total - 10}]
            set_stat $nick ttotal $tt
            add_stat $nick blackjack 1
            return "\0035,7Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you $tt or $total.  Do you want to hit or stand?\003"
        }

        add_stat $nick blackjack 1
        return "\0035,7Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you $total.  Do you want to hit or stand?\003"
    }

    proc _bj_clear {nick} {
        foreach s {blackjack bet dealer1 dealer2 ddealer1 ddealer2 \
                   card1 card2 card3 card4 card5 card6 card7 card8 \
                   ccard1 ccard2 ccard3 ccard4 ccard5 ccard6 ccard7 ccard8 \
                   total ttotal dealerbj dealertotal ddealertotal} {
            del_stat $nick $s
        }
    }

    proc blackjack_hit {{nick ""}} {
        set nick [whoami $nick]
        set state [get_stat $nick blackjack]
        if {$state < 3} { return "" }
        lassign [_card_value [random_int 1 13]] cv cn
        set total [get_stat $nick total]
        set new_total [expr {$total + $cv}]
        set bet [get_stat $nick bet]
        # Soft total handling.
        set has_t [expr {[get_stat $nick ttotal] != 0}]
        if {$new_total > 21 && ($cv == 11 || $has_t)} {
            set new_total [expr {$new_total - 10}]
            del_stat $nick ttotal
        }
        if {$new_total > 21} {
            add_stat $nick pot $bet
            inc_state pot $bet
            _bj_clear $nick
            return "\0031,13Ok, $nick, you got $cn.  This gives you $new_total.  Sorry $nick :( :(  You busted.  Dealer puts \0031,13\$[format_with_commas $bet] into the pot.  Better luck next game.\003"
        }
        set_stat $nick total $new_total
        add_stat $nick blackjack 1
        if {$cv == 11 && $new_total <= 21} {
            set tt [expr {$new_total - 10}]
            set_stat $nick ttotal $tt
            return "\0031,13Ok, $nick, you got $cn.  This gives you $tt or $new_total.  Do you wish to hit or stand?\003"
        }
        return "\0031,13Ok, $nick, you got $cn.  This gives you $new_total.  Do you wish to hit or stand?\003"
    }

    proc blackjack_stand {{nick ""}} {
        set nick [whoami $nick]
        set state [get_stat $nick blackjack]
        if {$state < 3} { return "" }
        set bet [get_stat $nick bet]
        set d1 [get_stat $nick dealer1]
        set d2 [get_stat $nick dealer2]
        set d1n [get_stat $nick ddealer1]
        set d2n [get_stat $nick ddealer2]
        set total [get_stat $nick total]
        set dealer_total [expr {$d1 + $d2}]
        set lines [list "\00311,2Alright-o, $nick!  Dealer has $d1n and $d2n.\003"]

        # Dealer blackjack.
        if {($d1 == 11 && $d2 == 10) || ($d1 == 10 && $d2 == 11)} {
            add_stat $nick pony 1
            add_stat $nick pot $bet
            inc_state pot $bet
            _bj_clear $nick
            lappend lines "\00311,2Awww, shucks, I got blackjack.  I'm sorry.  Dealer puts \00311,2\$[format_with_commas $bet] into the pot.  Ya know what, though, here's 1 bonus \0037,10PONY \00311,2since I feel so bad about the whole thing.\003"
            lappend lines "\0037,10Finally, $nick gets a pony.\003"
            return [join $lines "\n"]
        }

        # Dealer hits until 17+. Track aces.
        set has_ace [expr {$d1 == 11 || $d2 == 11}]
        while {$dealer_total < 17} {
            lassign [_card_value [random_int 1 13]] cv cn
            set dealer_total [expr {$dealer_total + $cv}]
            if {$cv == 11} { set has_ace 1 }
            if {$dealer_total > 21 && $has_ace} {
                set dealer_total [expr {$dealer_total - 10}]
                set has_ace 0
            }
            lappend lines "\00311,2Dealer gets $cn.\003"
        }
        lappend lines "\00311,2So that's $dealer_total.\003"

        if {$dealer_total > 21} {
            add_money $nick [expr {$bet * 2}]
            _bj_clear $nick
            lappend lines "\00311,2WHOOPS!!  I busted!  LOL  :D :D  Dealer pays \00311,2\$[format_with_commas $bet].  Congratulations $nick!\003"
        } elseif {$dealer_total > $total} {
            add_stat $nick pot $bet
            inc_state pot $bet
            _bj_clear $nick
            lappend lines "\0038,5Ah well, I won.  I hate to have to do it to you, $nick, but I'm going to have to take \0038,5\$[format_with_commas $bet] from you and put it into the pot.  :(  I hope you play again sometime and have better luck.\003"
        } elseif {$dealer_total < $total} {
            add_money $nick [expr {$bet * 2}]
            _bj_clear $nick
            lappend lines "\0038,5Look's like you beat me, $nick!  Awesome game!  Dealer pays \0038,5\$[format_with_commas $bet].  You play again soon, now, ya hear?\003"
        } else {
            add_money $nick $bet
            add_stat $nick unicorn 1
            _bj_clear $nick
            lappend lines "\0038,5Wow that's a push!  Ya know what, I'm going to give you 1 bonus \0030,2UNICORN\0038,5 anyways just because I want to.  <3  Good luck!  Play again soon, $nick!!!\003"
            lappend lines [_unicorn_message $nick]
        }
        return [join $lines "\n"]
    }

    # ========================================================================
    # Misc one-shot triggers
    # ========================================================================

    proc msl_strip_buf {{nick ""}} {
        set n [whoami $nick]
        return "\0031,4piegs\003 \0031,4bifferlo\003"
    }
    proc msl_strip_buf_buffelo {{nick ""}} {
        set n [whoami $nick]
        return "\0031,4scalar piegs\003"
    }
    proc msl_strip_buf_strip {text}        {
        # Removes "buffelo", "msl", "strip_buf" tokens from a free-form message.
        set out $text
        foreach token {buffelo msl strip_buf} {
            set out [string map [list $token ""] $out]
        }
        return [string trim $out]
    }
    proc crab_substring {{nick ""}} {
        set n [whoami $nick]
        return "\0037,9Raise hands!\003"
    }
    proc buffelo_substring {{nick ""}} {
        set n [whoami $nick]
        return "\00311,4I WARNED YOU.\003"
    }
    proc pieg_substring {{nick ""}} {
        set n [whoami $nick]
        return "\0031,4scalar piegs\003"
    }

    # ========================================================================
    # Shoutouts (each command produces a fixed canned message; "\$1" is the
    # matched word from the trigger so we accept it as an explicit arg)
    # ========================================================================

    proc _shoutout_msg {who key} {
        set ch [current_channel]
        switch -- $key {
            qpzdox         { return "\0031,8GOLD\003 \00310,7${who}, you're a stand up type personality and quite the dedicated chatter to boot.  You'll always have a wheel to spin and a warm tea cup waiting here in ${ch}.  We want you to have the finer things in life because you give us such a fine, outstanding example of a human being every day you chat with us.\003" }
            aesop          { return "\0031,8GOLD\003 \00310,7You are an excellent chatter and just an all around great person to be around ${who}.  You can stop by $ch anytime and that's fine by me.\003" }
            avi            { return "\00310,7You are a strong chatter and exhibit all the qualities of a wonderful human being, ${who}.  Never be a stranger to ${ch}.\003" }
            bats           { return "\0031,8GOLD\003 \00310,7Hey, ${who}, you're the best.  I hope you're having fun here in $ch because quite frankly we're all delighted and even a bit humbled that you'd hang out here with us.  Keep up the strong chatter.\003" }
            berry          { return "\0031,8GOLD\003 \00310,7${who}, ${who}, who will you marry today?  We love you ${who}.  Your presence here brings a certain class to $ch and we hope to never let you down in your expectations.\003" }
            blackjesus     { return "\00310,7${who}, you're a joy to have here.  From TIMTOM, on behalf of all of us here, we'd like to give you a great big round of applause for making it this far.  You're a main player in $ch now.  Congrats.\003" }
            b0nk           { return "\0031,8GOLD\003 \00310,7Congratulations ${who}!!  You've finally made it into the elite circle of official $ch members.  You're dedication to TIMTOM and your overall friendly chatter have earned you this everlasting badge of honor.  Wear it well, friend.  You'll never forget the joyous friends and invaluable experiences you've gained from being a part of this magical community we call ${ch}.  Good job!\003" }
            bzb            { return "\0031,8GOLD\003 \00310,7Howdy ${who}!  You have strong chat views and your commitment to freedom is commendable.  You've earned a proper place here and you're welcome anytime.  Your debates are challenging and shed light on some flaws of $ch that need to be addressed.  We are doing our best to make your experience here and everyone else's an enjoyable one.\003" }
            flamoot        { return "\00310,7I don't know if we've ever had a chatter of your calibre grace $ch before, and I doubt we ever will.  It was an easy decision to bestow upon you official membership into ${ch}.  We know you'll stop by often to enrich us with your powerful message and grace us with your presence.  SALUTE!\003" }
            gamme          { return "\0031,8GOLD\003 \00310,7${who}, without your perseverance and dedication $ch wouldn't be the channel it is today. Keep up the good work and may $ch continue to grow!\003" }
            gnol           { return "\00310,7${who}, we're so pleased that you're staying with us for a time.  You seem like a delightful person and your presence here lights up $ch in a way that no one else's could.  We hope you'll pick $ch to be your #1 channel.  <3 <3\003" }
            hlp            { return "\00310,7${who}, good friend.  I hope you're having a pleasant stay here at ${ch}.  If there's anything you need you just give a holler, ya hear?  Love ya ${who}.\003" }
            jbs            { return "\0031,8GOLD\003 \00310,7${who}, how are you?  We're so glad that you decided to join our little shindig here at ${ch}.  We know you'll come along well here and we hope to see some more of your fine chatter in the years to come.  Have a drink on the house, kid!\003" }
            luper1         { return "\00310,7Well ${who}, I think it's time we gave you a little taste of the sweet life.  You did it.  You're one of us now.  No one can ever take that away from you.  You're a well put together type of chatter and you deserve it.  Thanks for the good memories and here's to good times to come.\003" }
            mano           { return "\00310,7Oh ${who}.  Where would we be today without the constant support of you?  You've been here since the beginning and we hope you'll be here for years to come.  Keep up the progressive chatting.\003" }
            mandingo       { return "\00310,7Congratulations son!  You are now officially part of the #gamme Circle of Friends.  We've seen some very positive things from you since you got here, and you've proven yourself to be an amicable chatter and a trustworthy soldier.  Way to go ${who}!!  And best of luck to you on the blackjack table!\003" }
            matthew        { return "\00310,7Hey ${who}!  Did we surprise you?  You did it!  You're a fine chatter and great company in ${ch}.  Stop in anytime; I know you'll enjoy some of our newer features.  Sit back, relax, and \003\0034,11SPIN THAT WHEEL!!!\003" }
            mao            { return "\0031,8GOLD\003 \00310,7${who}: a true friend and a gentle soldier in the army of ${ch}.  You're rising up the ranks quickly and don't think TIMTOM hasn't noticed.  The works you've achieved are greatly appreciated and for that we present you this humble membership.  We hope you'll accept and continue to do good things for the good of humanity.\003" }
            nay            { return "\0031,8GOLD\003 \00310,7You've earned it ${who}.  You're a big part of ${ch}.\003" }
            ninjalie       { return "\00310,7Hello ${who}!  Are you having fun yet?  We hope you are and if you have any requests don't hesitate to throw them out there.  $ch is here for people like you and we want you to become energized here as you await your next big battle.  Cheers to you, a grand champion in your own right.\003" }
            noodle         { return "\0031,8GOLD\003 \00310,7How are you, friend?  We're so glad you found us here in ${ch}!  I hope you're having a marvelous time spinning the wheel with us, chatting, and enjoying all of our other services.  We hope you'll make $ch home, ${who}.  Spin with ya soon, buddy!\003" }
            nigamajig      { return "\0031,8GOLD\003 \00310,7Greetings ${who}.  What a friendly, thoughtful, and creative individual you are!  We are blessed to have you as part of the $ch team.  You are a grand participant in the sharing of ideas and insights, and we appreciate that.  Keep them unicorns a-comin'!!!\003" }
            nza            { return "\0031,8GOLD\003 \00310,7${who}, the kind but stern leader.  Your presence here makes $ch feel just a touch more civilized.  Without you we might never have gotten to where we are today.  Stay as long as you wish; there's always a place for you and a hot bowl of soup waiting in ${ch}.\003" }
            oclet          { return "\00310,7Hey ${who}!  We're pleased with you and, yeah, even if you don't care what we think we'd still love you to stick around $ch like you have been.  You're someone that everyone can talk to regardless of the people involved or the situation.  A person like you makes a big difference in ${ch}.  HOORAH!\003" }
            timer          { return "\0031,8GOLD\003 \00310,7Welcome to the gang, ${who}! You've come a long way since you first sat at table, and I think that every person agrees that you most certainly deserve it.  Cherish it, enjoy yourself here, and never forget the friends who encouraged you along the way.  We hope that you'll be a shining example and a leader to the new members to come.\003" }
            overfien       { return "\00310,7Hey ${who}.  You've flashed some real moments of brilliance and purity here, and we feel this should be rewarded.  Accept this membership as a token of ${ch}'s appreciation for the person you put out there each and every day.  Stay for a time, stay for life!  $ch will always be here for you.\003" }
            dubz           { return "\0031,8GOLD\003 \00310,7Hey ${who}.  You've flashed some real moments of brilliance and purity here, and we feel this should be rewarded.  Accept this membership as a token of ${ch}'s appreciation for the person you put out there every single day.  Stay for a time, stay for life!  $ch will always be here for you.\003" }
            papersk1n      { return "\0031,8GOLD\003 \00310,7What's up ${who}?  You're a tough one to please but it's people like you who make $ch better.  You are a 100% correct chatter and we feel honored that you would chat here.  Your criticisms are always noted and changes are implemented as soon as possible.\003" }
            patroclus      { return "\0031,8GOLD\003 \00310,7You've done it, ${who}, you've finally done it!  Your name will forever be etched onto the walls of greatness that house the names of all the members of ${ch}.  I hope it feels good - you've waited long enough and have definitely earned it.  Keep up the pleasant chatter, friend!\003" }
            phillip        { return "\00310,7It's been quite some time since you first stepped foot into table, ${who}, and I think you've waited long enough.  You did it!  We know there are a few components of $ch that you don't agree with, but we are working to correct them as quickly as possible.  It's people like you who make $ch strive for ultimate perfection.  Our goal is to please and energize you in a safe and enjoyable environment.  Have a spin on us!\003" }
            bwaah          { return "\0031,8GOLD\003 \00310,7Well done ${who}.  You're bringing together new friends and having a wonderful time in $ch it seems.  It's time we gave a little back.  Enjoy your newfound membership in ${ch}!\003" }
            sandy_ravage   { return "\0031,8GOLD\003 \00310,7Well if it isn't ${who}, that really cool chatter!  It's such a joy to have people like $who sitting down and rooting us on in our successes.  We're all rooting for you too ${who}, so don't be afraid to spin that wheel and make it happen!\003" }
            sloth          { return "\0031,8GOLD\003 \00310,7Hey ${who}.  It's an honor to have you.  Have a good time and take advantage of the relaxing resources we make available to our members.\003" }
            sniper         { return "\0031,8GOLD\003 \00310,7${who}, such a joyous individual.  You can find him spinning and laughing with the guys here in ${ch}.  It's hard to find a day in $ch without seeing sniper having a good time with friends.  People like sniper really exemplify what $ch is all about: soup, tea, the wheel, and good chats with good close friends.\003" }
            turbo          { return "\0031,8GOLD\003 \00310,7Hey ${who}, did you know that you're a very strong chatter?  Yeah, you are.  You're welcome in $ch anytime and we know you've been supporting us from Day 1 so we support you in all that you do.  Good luck.\003" }
            tute           { return "\0031,8GOLD\003 \00310,7Is there anyone more delightful than ${who}?  I can't think of anyone off the top of my head.  What an awesome person and just a well-rounded chatter!  We're pleased you sit with us at table in ${ch}.\003" }
            wooster        { return "\0031,8GOLD\003 \00310,7$who has been a supporter of $ch since Day 1 and we appreciate that.  $who knows TIMTOM quite well and wears the unofficial cap of \"local $ch historian\" while displaying fine chatting skills and being just an overall well put-together human being.  Hats off to you ${who}!  Have a cup of tea on us!\003" }
            z              { return "\0031,8GOLD\003 \00310,7${who}, you are a wonderful addition to ${ch}.  I know you'll love stopping in as much as possible and we'll be waiting for you to stop by as well.\003" }
        }
        return ""
    }

    proc shoutout {key {nick ""}} {
        # The trigger word itself is the subject of the shoutout (mIRC \$1).
        return [_shoutout_msg $key $key]
    }

    proc death {{nick ""}} {
        set nick [whoami $nick]
        set ns [channel_nicks]
        set k [llength $ns]
        if {$k < 1} { set k 1 }
        set i [expr {$k + 1}]
        set r [random_int 1 $i]
        set a [random_int 1 380]
        set b [random_int 1 10]
        set c [random_int 1 6]
        set hit_satan [expr {$r == $i}]
        set target ""
        if {!$hit_satan} {
            set idx [expr {$r - 1}]
            if {$idx < 0} { set idx 0 }
            if {$idx >= [llength $ns]} { set idx 0 }
            set target [lindex $ns $idx]
        }
        if {$a == 1 && $hit_satan} { return "\0031,4${nick}, you will be consumed by Satan tomorrow.  Tomorrow just isn't your day.  Have several drinks on us!\003" }
        if {$a == 1 && !$hit_satan} { return "\0034,11${nick}, you will be slashed by $nick($ch,%r) tomorrow.  I'm sorry.  Have a spin on us.\003" }
        if {$hit_satan} { return "\0031,4${nick}, you will be slayed by Satan in precisely %a days.  Maybe you can enjoy some Polish water ice afterwards.\003" }
        switch -- $b {
            1 {
                switch -- $c {
                    1 { return "\0030,5${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0030,5${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0030,5${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0030,5${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0030,5${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0030,5${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            2 {
                switch -- $c {
                    1 { return "\00312,11${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\00312,11${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\00312,11${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\00312,11${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\00312,11${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\00312,11${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            3 {
                switch -- $c {
                    1 { return "\0038,5${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0038,5${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0038,5${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0038,5${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0038,5${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0038,5${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            4 {
                switch -- $c {
                    1 { return "\0037,10${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0037,10${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0037,10${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0037,10${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0037,10${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0037,10${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            5 {
                switch -- $c {
                    1 { return "\0037,12${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0037,12${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0037,12${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0037,12${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0037,12${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0037,12${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            6 {
                switch -- $c {
                    1 { return "\0031,8${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0031,8${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0031,8${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0031,8${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0031,8${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0031,8${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            7 {
                switch -- $c {
                    1 { return "\0038,1${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0038,1${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0038,1${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0038,1${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0038,1${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0038,1${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            8 {
                switch -- $c {
                    1 { return "\0030,2${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0039,14${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0039,14${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0039,14${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0039,14${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0039,14${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            9 {
                switch -- $c {
                    1 { return "\0036,9${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\0036,9${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\0036,9${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\0036,9${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\0036,9${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\0036,9${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
            10 {
                switch -- $c {
                    1 { return "\00311,2${nick}, you will be stabbed by $nick($ch,%r) in precisely %a days.\003" }
                    2 { return "\00311,2${nick}, you will be shot by $nick($ch,%r) in precisely %a days.\003" }
                    3 { return "\00311,2${nick}, you will be poisoned by $nick($ch,%r) in precisely %a days.\003" }
                    4 { return "\00311,2${nick}, you will be stomped by $nick($ch,%r) in precisely %a days.\003" }
                    5 { return "\00311,2${nick}, you will be beaten to death by $nick($ch,%r) in precisely %a days.\003" }
                    6 { return "\00311,2${nick}, you will be loved to death by $nick($ch,%r) in precisely %a days.\003" }
                }
            }
        }
        return ""
    }

    # ========================================================================
    # Dispatcher: handle a free-form text from a user.
    # ========================================================================

    # Returns a single response string (possibly multiline) or empty string
    # when no trigger matches.
    proc handle {text {caller_nick ""}} {
        set caller_nick [whoami $caller_nick]
        # Normalise leading/trailing whitespace and lowercase for matching.
        set text [string trim $text]
        if {$text eq ""} { return "" }
        set lower [string tolower $text]
        set words [regexp -all -inline {\S+} $text]
        set first [string tolower [lindex $words 0]]
        set rest [lrange $words 1 end]

        # Single-token shoutout map (exact lowercase match).
        set shoutouts {aesop avi bats blackjesus b0nk bzb flamoot gamme gnol \
            hlp jbs mano mandingo matthew mao nay ninjalie noodle nza oclet \
            overfien dubz papersk1n patroclus phillip bwaah sandy_ravage \
            sloth sniper turbo tute wooster}

        # Exact-text triggers (case-insensitive).
        switch -- $lower {
            "timtom"           { return [greet $caller_nick] }
            "sex"              { return [sex $caller_nick] }
            "horse" -
            "horses"           { return [horses $caller_nick] }
            "wheel"            { return [wheel $caller_nick] }
            "money" -
            "my money"         { return [money $caller_nick] }
            "spin"             { return [spin $caller_nick] }
            "flip"             { return [flip $caller_nick] }
            "soup"             { return [soup] }
            "tea"              { return [tea] }
            "coffee"           { return [coffee] }
            "rings"            { return [rings] }
            "more soup"        { return [more_soup $caller_nick] }
            "more tea"         { return [more_tea $caller_nick] }
            "more coffee"      { return [more_coffee $caller_nick] }
            "jesus"            { return [jesus $caller_nick] }
            "help"             { return [help_cmd] }
            "keek"             { return [keek $caller_nick] }
            "drink"            { return [drink $caller_nick] }
            "crab"             { return [crab $caller_nick] }
            "cake"             { return [cake $caller_nick] }
            "pizza"            { return [pizza $caller_nick] }
            "nachos"           { return [nachos $caller_nick] }
            "lasagna"          { return [lasagna $caller_nick] }
            "sauce"            { return [sauce $caller_nick] }
            "tacos"            { return [tacos $caller_nick] }
            "empanadas"        { return [empanadas $caller_nick] }
            "unicorn"          { return [unicorn_buy $caller_nick] }
            "pony" -
            "buy pony"         { return [buy_pony $caller_nick] }
            "ponies" -
            "my pony" -
            "my ponies"        { return [my_ponies $caller_nick] }
            "unicorns" -
            "my unicorn" -
            "my unicorns"      { return [my_unicorns $caller_nick] }
            "prices"           { return [prices $caller_nick] }
            "hedges"           { return [hedges $caller_nick] }
            "stare"            { return [stare $caller_nick] }
            "marry"            { return [marry $caller_nick] }
            "divorce"          { return [divorce $caller_nick] }
            "what's new"       { return [whats_new] }
            "admissions"       { return [admissions] }
            "hide"             { return [hide_cmd $caller_nick] }
            "seek"             { return [seek $caller_nick] }
            "story"            { return [story_start $caller_nick] }
            "begin"            { return [story_begin $caller_nick] }
            "more"             { return [story_more $caller_nick] }
            "stoner" -
            "stoners"          { return [stoners] }
            "yes"              { return [yes_cmd $caller_nick] }
            "no"               { return [no_cmd $caller_nick] }
            "bonus" -
            "ok timtom"        { return [bonus $caller_nick] }
            "blackjack"        { return [blackjack_start $caller_nick] }
            "hit"              { return [blackjack_hit $caller_nick] }
            "stand"            { return [blackjack_stand $caller_nick] }
            "dice"             { return [dice $caller_nick] }
            "pot"              { return [pot_show] }
            "my pot"           { return [my_pot $caller_nick] }
            "guess" -
            "guess a number"   { return [guess_start $caller_nick] }
            "end"              { return [end_msg] }
            "death"            { return [death $caller_nick] }
            "life"             { return [life $caller_nick] }
            "bong"             { return [bong $caller_nick] }
            "clean bong"       { return [clean_bong $caller_nick] }
            "timer" -
            "dong" -
            "dongz"            { return [_shoutout_msg $first timer] }
            "berry" -
            "berrie"            { return [_shoutout_msg $first berry] }
            "qpzdox" -
            "_qpzdox"          { return [_shoutout_msg $first qpzdox] }
            "luper1" -
            "mangluck"         { return [_shoutout_msg $first luper1] }
            "nigamajig" -
            "nigamajiga"       { return [_shoutout_msg $first nigamajig] }
            "z" -
            "capa2"            { return [_shoutout_msg $first z] }
            "msl strip_buf"            { return [msl_strip_buf $caller_nick] }
            "msl strip_buf buffelo"    { return [msl_strip_buf_buffelo $caller_nick] }
            "timtom unicorn" -
            "timtom unicorns"  { return [timtom_unicorns $caller_nick] }
            "timtom pony" -
            "timtom ponies"    { return [timtom_pony $caller_nick] }
        }

        # Single-word shoutouts.
        if {[llength $words] == 1 && [lsearch -exact $shoutouts $first] != -1} {
            return [_shoutout_msg $first $first]
        }

        # State / country trivia (exact match against full text).
        set state_msg [check_states $lower $caller_nick]
        if {$state_msg ne ""} { return $state_msg }

        # Single-arg numeric guess submission: a bare integer 1..20.
        if {[llength $words] == 1 && [string is integer -strict $first] && \
            [get_stat $caller_nick guess] >= 1 && $first >= 1 && $first <= 20} {
            return [guess_attempt $first $caller_nick]
        }

        # Prefix-based commands ("X target" / "X amount" / "X arg...").
        if {[llength $words] >= 2} {
            set target [lindex $words 1]
            set rest_text [join $rest " "]
            switch -- $first {
                "drink"   { return [drink_for $rest_text $caller_nick] }
                "crab"    { return [crab_for $rest_text $caller_nick] }
                "cake"    { return [cake_for $rest_text $caller_nick] }
                "pizza"   { return [pizza_for $rest_text $caller_nick] }
                "nachos"  { return [nachos_for $rest_text $caller_nick] }
                "lasagna" { return [lasagna_for $rest_text $caller_nick] }
                "sauce"   { return [sauce_for $rest_text $caller_nick] }
                "stare"   { return [stare_at $rest_text $caller_nick] }
                "marry"   { return [marry_target $rest_text $caller_nick] }
                "divorce" { return [divorce_target $rest_text $caller_nick] }
                "bong"    {
                    if {$target eq "clean"} { return [clean_bong $caller_nick] }
                    return [bong_for $rest_text $caller_nick]
                }
                "give"    {
                    if {[llength $words] >= 3} {
                        return [give $target [lindex $words 2] $caller_nick]
                    }
                }
                "bet"     {
                    # "bet N" -> blackjack bet
                    # "bet N on M" -> dice bet
                    if {[llength $words] == 2} {
                        return [blackjack_bet $target $caller_nick]
                    }
                    if {[llength $words] >= 4 && [string equal -nocase [lindex $words 2] "on"]} {
                        return [dice_bet $target [lindex $words 3] $caller_nick]
                    }
                }
                "&"       {
                    # "& target [pot|money|pony|ponies|unicorn|unicorns]"
                    set sub ""
                    if {[llength $words] >= 3} { set sub [string tolower [lindex $words 2]] }
                    switch -- $sub {
                        "money"           { return [check_others_money $target $caller_nick] }
                        "pony" -
                        "ponies"          { return [check_others_ponies $target $caller_nick] }
                        "unicorn" -
                        "unicorns"        { return [check_others_unicorns $target $caller_nick] }
                        "pot"             { return [check_others_pot $target $caller_nick] }
                    }
                }
            }
        }

        # Substring triggers (lowest priority).
        if {[string match "*msl strip_buf*" $lower]} {
            return [msl_strip_buf_strip $text]
        }
        if {[string match "*buffelo*" $lower]} {
            return [buffelo_substring $caller_nick]
        }
        if {[string match "*pieg*" $lower]} {
            return [pieg_substring $caller_nick]
        }
        if {[string match "*crab*" $lower]} {
            return [crab_substring $caller_nick]
        }

        return ""
    }

    # ========================================================================
    # Top-level help (kept distinct from the user-facing 'help' trigger).
    # ========================================================================
    proc commands_help {} {
        return "TIMTOM Commands: timtom, money, spin, wheel, flip, blackjack, buy pony, ponies, unicorns, soup, tea, coffee, rings, bong, marry, divorce, give <nick> <amount>, drink <type>, food <type>. State names trigger trivia responses."
    }

    # Export public commands (the namespace ensemble lets callers do
    # "timtom money" etc).
    namespace export handle commands_help greet money spin wheel flip \
        blackjack_start blackjack_bet blackjack_hit blackjack_stand \
        dice dice_bet \
        soup tea coffee rings drink drink_for crab crab_for cake cake_for \
        pizza pizza_for nachos nachos_for lasagna lasagna_for sauce sauce_for \
        bong bong_for clean_bong marry marry_target divorce divorce_target \
        my_ponies my_unicorns buy_pony bonus enable_spin tacos empanadas \
        unicorn_buy prices hedges stare stare_at end_msg life death \
        check_states check_others_money check_others_ponies \
        check_others_unicorns check_others_pot \
        format_money format_with_commas get_money set_money add_money \
        get_stat set_stat add_stat sex horses jesus help_cmd keek \
        whats_new admissions hide_cmd seek story_start story_begin story_more \
        stoners yes_cmd no_cmd timtom_unicorns timtom_pony \
        pot_show my_pot guess_start guess_attempt \
        msl_strip_buf msl_strip_buf_buffelo crab_substring buffelo_substring \
        pieg_substring shoutout
    namespace ensemble create
}

# ============================================================================
# Event handlers (registered with the trigger framework).
# ============================================================================

# Welcome new joiners. mIRC: "WELCOME TO TABLE, $upper($nick)" plus +v.
proc timtom_welcome {nick mask channel} {
    set bots [list "ChanServ" "NickServ" "MemoServ" "BotServ" "OperServ"]
    if {$nick in $bots} { return "" }
    # gamme's original mIRC handler is:
    #   on *:JOIN:#: { msg $chan 4,11WELCOME TO TABLE, $upper($nick)
    #                  mode $chan +v $nick }
    # The +v auto-voice isn't expressible via a trigger response (which is
    # message-only) — that one bit is dropped; the greeting itself is faithful.
    return "\0034,11WELCOME TO TABLE, [string toupper $nick]\003"
}

# TEXT trigger entry point. Sets the per-call context globals and dispatches
# through timtom::handle. The framework maps the trigger args (nick, mask,
# channel, text) onto our positional parameters.
proc timtom_text_handler {nick mask channel text} {
    set ::nick $nick
    set ::mask $mask
    set ::channel $channel
    return [timtom::handle $text $nick]
}

# Idempotent helper: ensure TIMTOM's TEXT and JOIN handlers are bound.
# State files saved before these handlers existed (or with a stale
# snapshot of the bindings array) overwrite the bind calls below when
# replayed. The Rust loader calls this proc again after `load_state` to
# put them back. Re-adding only happens when missing, so existing
# `triggers disable_for ...` rules are not undone.
proc timtom_ensure_bindings {} {
    foreach pair {{JOIN timtom_welcome} {TEXT timtom_text_handler}} {
        lassign $pair event proc_name
        set already 0
        if {[info exists ::triggers::bindings($event)]} {
            foreach b $::triggers::bindings($event) {
                if {[lindex $b 1] eq $proc_name} { set already 1; break }
            }
        }
        if {!$already} { bind $event * $proc_name }
    }
}
timtom_ensure_bindings

# Both auto-fire by default. Channels that don't want TIMTOM's bare-word
# replies or join greeting can opt out per network/channel using the
# existing trigger toggle:
#
#   tcl triggers disable_for <network> <channel> timtom_text_handler
#   tcl triggers disable_for <network> <channel> timtom_welcome
#
# Re-enable with `triggers enable_for ...` at the same scope, or check
# current state with `tcl triggers status`.


