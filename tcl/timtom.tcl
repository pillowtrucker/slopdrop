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
        return "$nick, this is TIMTOM. How may I serve you?"
    }

    proc sex {{nick ""}} {
        set nick [whoami $nick]
        if {[random_int 1 68] == 41} {
            return "ok, $nick, I will have sex with you now."
        }
        return "$nick, I cannot perform sex on you at this moment."
    }

    proc horses {{nick ""}} {
        set nick [whoami $nick]
        return "$nick, I like horses too."
    }

    proc jesus {{nick ""}} {
        set nick [whoami $nick]
        set ch [current_channel]
        return "$nick, Jesus loves you more than $ch.  I'm sorry.  $ch just doesn't compare."
    }

    proc wheel {{nick ""}} {
        set nick [whoami $nick]
        if {[stat_allows $nick spin]} {
            return "I think it would be a good idea if $nick would spin the wheel."
        }
        return "$nick, please let someone else spin."
    }

    proc more_soup    {{nick ""}} { set n [whoami $nick]; return "Sorry, $n, here's some more soup." }
    proc more_tea     {{nick ""}} { set n [whoami $nick]; return "Sorry, $n, here's some more tea." }
    proc more_coffee  {{nick ""}} { set n [whoami $nick]; return "Sorry, $n, here's some more coffee." }

    proc admissions {} {
        return "Download the Hertford College Admissions iPhone app here: http://itunes.apple.com/us/app/hertford-college-admissions/id382253306?mt=8"
    }

    proc whats_new {} {
        return "Hey all!  It's been a joy serving you these past few days.  Though we've lost a bit of ground in terms of new memberships, we've all become closer and better good friends and that's what counts here. The wheel is still #1 as it should be.\nSome of our newest features include: death!  life!  piegs!  and after a great deal of fighting, we've finally decided to allow our members to use the word \"buffelo\".  However, one must include the phrase \"msl strip_buf\" somewhere in his message in order for TIMTOM to agree.  Enjoy friends."
    }

    proc help_cmd {} {
        set ch [current_channel]
        return "Hi, how are you doing?  My name is TIMTOM and you are in $ch.  I am a servant to the people, and like to fancy myself as quite the capable gentleman.  If there's anything you need don't hesitate to ask.  Everyone has a voice here and we treat everyone with love and kindness.\nSome of our popular features include hot soup and hot tea, horses, and NEVER FEAR: we offer the sacrament of marriage and also deal in divorces, and Wheel of Fortune is always on.  Kick off your shoes, relax, and don't worry about a thing.  The internet cannot hurt you now.  You are in $ch."
    }

    # Broadcast servings: list nicks (or non-ops) and serve them.
    proc _serve_broadcast {item color_intro} {
        set ns [channel_nicks]
        if {[llength $ns] == 0} { return "TIMTOM brings out the $item.  Enjoy friends." }
        return "$color_intro [join $ns " "]  Enjoy friends."
    }

    proc soup   {} { _serve_broadcast soup   "TIMTOM brings out the hot soup for" }
    proc tea    {} { _serve_broadcast tea    "TIMTOM brings out the hot tea for" }
    proc coffee {} { _serve_broadcast coffee "TIMTOM brings out the hot coffee for" }
    proc rings  {} { _serve_broadcast rings  "TIMTOM brings out the rings for" }

    # ========================================================================
    # State / country trivia (single fixed messages)
    # ========================================================================

    proc check_states {text {nick ""} {result_var ""}} {
        # Backward-compat: original signature took {text nick resultVar}.
        set nick [whoami $nick]
        set ch [current_channel]
        set t [string tolower $text]
        set states [dict create \
            "alabama"        "Alabama eats my children." \
            "alaska"         "Alaska is a cotton gin." \
            "arizona"        "Arizona is the land of the forsaken bee hives." \
            "arkansas"       "Arkansas is a potato rally." \
            "california"     "The capital of California is Los Angeles." \
            "colorado"       "Colorado was the missing egg in the blue carton." \
            "connecticut"    "Connecticut is a wild stallion." \
            "delaware"       "Delaware is a label-making compartment of beauty." \
            "florida"        "The capital of Florida is Disney World." \
            "georgia"        "Georgia plates early, makes space for Willy." \
            "hawaii"         "The capital of Hawaii is dog." \
            "idaho"          "Idaho is a flowing mountain." \
            "illinois"       "The capital of Illinois is Deal Or No Deal." \
            "indiana"        "Indiana rests softly in my left breast pocket sandwich player machine box heavy." \
            "iowa"           "Friends make pottery in Iowa." \
            "kansas"         "Kansas is a candy cane land in $ch." \
            "kentucky"       "The capital of Kentucky is horse." \
            "louisiana"      "Louisiana is a bubble paper pepper boy." \
            "maine"          "Maine is the capital of France." \
            "maryland"       "The capital of Maryland is inside the fried pickled answering machine tape." \
            "massachusetts"  "Massachusettes is the capital of Happy time." \
            "michigan"       "$nick I love you more than the sun and the sky.  I want you to be my forever." \
            "minnesota"      "Minnesota, will you be my road puppy?" \
            "mississippi"    "Glad tidings to you, $nick, wherever you are." \
            "missouri"       "The fly sunk to the bottom of the jar of oil.  The fly's name was Montel." \
            "montana"        "You didn't press enter hard enough." \
            "nebraska"       "Spinning sunflower wreath, you come in the morning and leave by nightfall." \
            "nevada"         "Nevada is first in my peeegy back machine-eeeeeeeeeeee-ooooooooooo." \
            "new hampshire"  "I hear shovels.  Lock the doors. NOW NOOOOWWWW NOOOOOOOOOWWWWWWWWWWWWWWWWWWWWWWWW!!!!!!!!" \
            "new jersey"     "We all eat pots and pans." \
            "new mexico"     "Thunder, ice, and twins joined at the hip, make my day a solid whip?  Whippie!" \
            "new york"       "The capital of New York is New York City." \
            "north carolina" "North Carolina makes $ch a happy land for you." \
            "north dakota"   "I am the willing partner in your N. Dakota movement." \
            "ohio"           "Ohio is diabetes." \
            "oklahoma"       "Oklahoma is my tea set." \
            "oregon"         "There's plenty of lightbulbs in the furnace." \
            "pennsylvania"   "The capital of Pennsylvania is cheddar." \
            "rhode island"   "How could I forget you, Rhode Island?  You are a gentle beauty." \
            "south carolina" "South Carolina is poppy." \
            "south dakota"   "Do you really think you own me, $nick?" \
            "tennessee"      "Tennessee is a puppy cage." \
            "texas"          "Feast on these berries.  They were created through honor, diligence, and musk." \
            "utah"           "The little pieces of paper need to be evaluated." \
            "vermont"        "Vermont is a picnic tree." \
            "virginia"       "Virginia is a glue cow." \
            "washington"     "Let's roll up another traffic ordinance and place it beneath the Bubber Tree." \
            "west virginia"  "Them tree trunks look like legs." \
            "wisconsin"      "If we connect the brown pipe to the gray pipe we make famous grandwich butter spread." \
            "wyoming"        "Claw me to death with pear skins." \
            "africa"         "Africa is a lollipop for you." \
            "canada"         "Canada is made of copper and sand." \
            "china"          "Thank you for relaxing in $ch.  China." \
            "france"         "France is a boat." \
            "sweden"         "The capital of Sweden is pah-pah." \
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

        # Random message templates (use $X placeholder for $formatted).
        if {$amount == 0} {
            switch -- $r {
                1  { return "Hey, how are you doing, $nick?  It's TIMTOM.  You currently have \$0.  Sorry about that." }
                2  { return "TIMTOM here!  Are you having fun yet, $nick?  I sure hope you are.  You currently have \$0.  :(" }
                3  { return "What's the good word, there, $nick?  It's TIMTOM.  You currently have \$0.  I'm so sorry." }
                4  { return "Howdy Doodie $nick!  You currently have \$0.  That's too bad." }
                5  { return "TIMTOM here!  Responding to the one and only $nick.  You currently have \$0.  Ah well." }
                6  { return "IT'S SO NICE TO HEAR FROM YOU, $upper!  You want to know about your money, eh, $nick?  Well, you've got \$0.  Let's hope you do better." }
                7  { return "TIMTOM here!  Reporting for duty.  $nick, you currently have \$0.  Uh oh!" }
                8  { return "Hello! Hello!  You've got \$0, $nick.  You can do better than that!" }
                9  { return "Hey, how are you doing, $nick?  It's TIMTOM.  You currently have \$0.  Sorry.  :(" }
                10 { return "TIMTOM here with your bank statement.  You currently have \$0.  :(  Good day, $nick." }
            }
        } else {
            switch -- $r {
                1  { return "Hey, how are you doing, $nick?  It's TIMTOM.  You currently have $formatted.  Good luck!" }
                2  { return "TIMTOM here!  Are you having fun yet, $nick?  I sure hope you are.  You currently have $formatted." }
                3  { return "What's the good word, there, $nick?  It's TIMTOM.  You currently have $formatted.  Use it wisely." }
                4  { return "Howdy Doodie $nick!  You currently have $formatted." }
                5  { return "TIMTOM here!  Responding to the one and only $nick.  You currently have $formatted." }
                6  { return "IT'S SO NICE TO HEAR FROM YOU, $upper!  You want to know about your money, eh, $nick?  Well, you've got $formatted." }
                7  { return "TIMTOM here!  Reporting for duty.  $nick, you currently have $formatted.  Be good." }
                8  { return "Hello! Hello!  You've got $formatted, $nick.  Very well then!" }
                9  { return "Hey, how are you doing, $nick?  It's TIMTOM.  You currently have $formatted.  Have a good day." }
                10 { return "TIMTOM here with your bank statement.  You currently have $formatted.  Good day, $nick." }
            }
        }
        return ""
    }

    proc give {target amount {nick ""}} {
        set nick [whoami $nick]
        # mIRC: "Please, $nick, only whole dollar transfers." if "." isin $3
        if {[string match "*.*" $amount]} {
            return "Please, $nick, only whole dollar transfers."
        }
        if {![string is integer -strict $amount]} {
            return "Please, $nick, only whole dollar transfers."
        }
        if {$amount < 0} { return "" }
        set cur [get_money $nick]
        if {$cur < $amount} { return "" }
        # Transfer with $2 fee that goes into the pot.
        add_money $nick [expr {-$amount}]
        add_money $nick -2
        add_stat $nick pot 2
        inc_state pot 2
        add_money $target $amount
        return "HELLO!  HELLO!  TIMTOM here!  Why sure, $nick, I'll transfer \$[format_with_commas $amount] over to ${target}'s account.  We also deduct a \$2 transfer fee that will be added to the pot.  Thank you for banking with TIMTOM!"
    }

    # ========================================================================
    # Spin / Wheel of Fortune
    # ========================================================================

    proc spin {{nick ""}} {
        set nick [whoami $nick]
        if {![stat_allows $nick spin]} {
            return "$nick, please let someone else spin."
        }
        set r [random_int 1 40]
        set msg ""
        # Many outcomes: clear all spin perms then maybe set this nick=0.
        switch -- $r {
            1 - 10 - 20 - 30 - 40 {
                set_money $nick 0
                clear_glob "spin_*"
                set_stat $nick spin 0
                set msg "$nick, you get a BANKRUPT!!!"
            }
            2  { add_money $nick 500;     clear_glob "spin_*"; set msg "$nick, you get \$500" }
            3  { add_money $nick 400;     clear_glob "spin_*"; set msg "$nick, you get \$400" }
            4  { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  Sorry about that." }
            5  { add_money $nick 5000;    clear_glob "spin_*"; set msg "$nick, you get \$5000!!!  WOW!!!" }
            6  { add_money $nick 250;     clear_glob "spin_*"; set msg "$nick, you get \$250" }
            7  { add_money $nick 800;     clear_glob "spin_*"; set msg "$nick, you get \$800" }
            8  { add_money $nick 666;     clear_glob "spin_*"; set msg "$nick, you get \$666.  That's scary business." }
            9  { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  Sorry about that." }
            11 { add_money $nick 47;      clear_glob "spin_*"; set msg "$nick, you get \$47.  That's ok, it's better than nothing." }
            12 - 32 { add_money $nick 900; clear_glob "spin_*"; set msg "$nick, you get \$900" }
            13 {
                add_money $nick 1000000
                clear_glob "spin_*"
                set_state ok 1
                set msg "$nick, you get \$1,000,000!!! THAT'S AMAZING!!!"
            }
            14 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  Still better than bankrupt." }
            15 { add_money $nick 251;     clear_glob "spin_*"; set msg "$nick, you get \$251" }
            16 { add_money $nick 300;     clear_glob "spin_*"; set msg "$nick, you get \$300" }
            17 { add_money $nick 450;     clear_glob "spin_*"; set msg "$nick, you get \$450" }
            18 { add_money $nick 9000;    clear_glob "spin_*"; set msg "$nick, you get \$9000.  That's a nice hefty amount." }
            19 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  Whoops, I guess the wheel is rigged." }
            21 { add_money $nick 5000;    clear_glob "spin_*"; set msg "$nick, you win a trip to Detroit, Michigan!  Good for you!" }
            22 { add_money $nick 11000;   clear_glob "spin_*"; set msg "$nick, you get \$11,000" }
            23 { add_money $nick 50;      clear_glob "spin_*"; set msg "$nick, you get fifty dollars." }
            24 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  Let your secret crush spin next." }
            25 { add_money $nick 999.99;  clear_glob "spin_*"; set msg "$nick, you get \$999.99!!!  WOW!!!" }
            26 { add_money $nick 5000;    clear_glob "spin_*"; set msg "$nick, you win a trip to Kenya, Africa." }
            27 { add_money $nick 700;     clear_glob "spin_*"; set msg "$nick, you get \$700" }
            28 { add_money $nick 100;     clear_glob "spin_*"; set msg "$nick, you get \$100.  Maybe you can buy us all tacos later." }
            29 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  I'm NOT sorry about that." }
            31 { add_money $nick 680;     clear_glob "spin_*"; set msg "$nick, you get \$680.  Do you remember the time you got a million?  That was crazy.  Not this time though." }
            33 { add_money $nick 5000;    clear_glob "spin_*"; set msg "$nick, you win a trip to Hawaii!!!!" }
            34 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  Sorry." }
            35 { add_money $nick 255;     clear_glob "spin_*"; set msg "$nick, you get \$255" }
            36 { add_money $nick 390;     clear_glob "spin_*"; set msg "$nick, you get \$390" }
            37 { clear_glob "spin_*"; set msg "$nick, you get \$000. LOL." }
            38 { add_money $nick 9000;    clear_glob "spin_*"; set msg "$nick, you get \$9000.  That's a nice HEEEEFTY amount." }
            39 { clear_glob "spin_*"; set_stat $nick spin 0; set msg "$nick, you get LOSE A TURN!!  Whoops, I guess the wheel is rigged." }
            default { set msg "$nick, you get \$0. The wheel jammed." }
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
            return "$nick, you got your million.  Please let someone else flip now."
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
                return "$nick flips HEADS.\nWow $nick!!  You got 7 heads in a row!  Here's \$1,000,000!"
            }
            return "$nick flips HEADS."
        } else {
            set t [add_stat $nick tails 1]
            set_stat $nick heads 0
            clear_glob "flip_*"
            if {$t == 7} {
                add_money $nick 1000000
                set_stat $nick tails 0
                set_stat $nick flip 0
                set_state ok 1
                return "$nick flips TAILS.\nWow $nick!!  You got 7 tails in a row!  Here's \$1,000,000!"
            }
            return "$nick flips TAILS."
        }
    }

    proc bonus {{nick ""}} {
        set nick [whoami $nick]
        if {[get_state ok] eq "1"} {
            add_money $nick 5000
            set_state ok 0
            return "OK [string toupper $nick], HERE'S \$5000"
        }
        return ""
    }

    # ========================================================================
    # Keek - 64 random messages
    # ========================================================================

    proc keek {{nick ""}} {
        set nick [whoami $nick]
        set ketchup_line "Your name is $nick, KETCHUP AND MUSTARD ::::: MUSTARD AND MAYONAISE"
        set msgs [list \
            "Your name is $nick.  My life is a life of horse castles." \
            "Your name is $nick.  Party favorites go well with wiggly fins." \
            "Your name is $nick.  We force scarecrows into the space behind cotton pilgrims." \
            "Your name is $nick.  Do your feet make flat Rothchilds?" \
            "Your name is $nick.  Skeleton handlebars are fastly becoming bacon tin cup freedom tunics." \
            "timtom eats a pine tree." \
            "timtom likes working in his railroad pants." \
            "timtom gives a toy truck to his neighbor Santa." \
            "All the fat young giraffes are gathered for a bag-off." \
            "Your name is $nick and my name is Geoffrey Giraffee." \
            "Your name is $nick.  All the piglets are crying over their nose bleeds." \
            "Your name is $nick.  Cold bowls of rice are waiting just above the paint can rim." \
            "timtom made a cloth figurine symbolizing the process of mitosis." \
            "timtom spends his Saturdays reading hot sauce packets." \
            "TIMTOM HERE!  We make Ronald McDonald bibs for sailors." \
            "Will you place your gentle fingers on my spikey larva?" \
            "Your name is $nick and I just made frosted egg whites." \
            "TIMTOM HERE!  The chocolate monsignor is signing autographs in the vestibule." \
            "TIMTOM HERE!  The current weather in Austria-Hungary is 11." \
            "TIMTOM HERE!  When the toilet paper rolls first came out I was skeptical too." \
            "TIMTOM HERE!  GARBAGE! GARBAGE! GARBAGE! JASJAJFADSJFSDFDSBFSDHFSDHFSDHFSHJSFDHJDFSJHFSDHJSDHJSDHJFSDHFSNVNVDSNSUFEYFENDSNJVVllllllllllllllllllllllllllllllll" \
            "TIMTOM HERE!  These are the days when wine smells like sweet roses." \
            "TIMTOM HERE!  The horrible elevator is next to the unhorrible escalator (in happy candy)." \
            "TIMTOM HERE!  Elton is a pride." \
            "TIMTOM HERE!  Cheese can be white, yellow, or orange and your fingers eat themselves dry as a bone my flakiest one." \
            "TIMTOM HERE!  O the bells go snap with a white lion cap and the chins of the thrills of kooray.  Take your belt on feet let it pick up the meat and spatula stands for a maid.  O the bells go snap with a white lion cap!" \
            "TIMTOM HERE!  We make honky horns." \
            "Your name is $nick.  We all grew up over the icing truck." \
            "Your name is $nick.  Forget what the sunbirds told you during spanking weather." \
            "Your name is $nick.  Flowing monster eat my daddy, pull my ribbons over my webbing.  Polar monster they call Vaxmonsky, tear me down from this wall of rabbit bark." \
            "timtom discovered that jeans can be used to make paper." \
            "timtom spends his Saturdays reading fiction." \
            "TIMTOM HERE!  The glass is always perched on a ledge near Franklin's Gower." \
            "Ah, the salty bells are ringing again.  Time to waste another helium balloon." \
            "Your name is $nick and everyone wants to climb on you." \
            "TIMTOM HERE!  LET'S PUT SOME TABLECLOTH ON TOP OF THE PIANO!" \
            "TIMTOM HERE!  FROZEN HOTDOGS MAKE THE FUNNEST FUNNEL CAKES SINCE I DON'T KNOW NEXT TUESDAY!" \
            "TIMTOM HERE!  HOSE DOWN THE GARBAGE CAN LID BEFORE WE GET ICY." \
            "TIMTOM HERE!  THESE SMALL PARASITES ARE NAMED AFTER ZONING BOARD NOBLEMEN." \
            "TIMTOM HERE!  WITHOUT YELLOW LEGS THE SUN WOULD ONLY PROTECT THE 8 PERCENT OF THE POPULATION THAT ACTUALLY TAKES THE TIME TO BREATHE IN THE CHICKEN POX VACCINE THAT WAS STUMBLING EYESHADOW." \
            "TIMTOM HERE!  IT'S HARD FOR HITCHHIKERS TO GET PICKED UP ANYMORE." \
            "Every hooved animal with pink flesh has a soda function." \
            "Your name is $nick, and it's no wonder that the wooden play pieces are so smooth." \
            "Your name is $nick, do you know where the wires to the cabinet are?" \
            $ketchup_line \
            "TIMTOM HERE!  I THINK WE ALL LOOK GOOD IN OUR TREEHOUSES." \
            "TIMTOM HERE!  QUESTIONS OF SKY AND SEA ARE NOT TO BE ADDRESSED BEFORE MORNING TEA." \
            "TIMTON HERE!  EGG YOLKS AND EGG WHITES, JUST SNIFFIN'." \
            "Your name is $nick, and I do believe you've perfected the leap year." \
            "timtom wants a cleaner bathtub for Halloween this year." \
            "timtom holds a piglet zygote in the palm of his aqua marine green handshire." \
            "timtom rides the lightning bolt upwards." \
            "timtom likes when palm readers dictate." \
            "TIMTOM HERE!  HOW'S THE CONCRETE STATUE COMING?" \
            "Your name is $nick, and every tungsten tooth will be grinded accordingly." \
            "timtom needs a fireproof tee shirt." \
            "Your name is $nick, $nick the quick.  Welcome to Quackers." \
            "timtom eats a bell." \
            "timtom eats a bulb." \
            "timtom eats a bolt." \
            "timtom eats a beet." \
            $ketchup_line \
            $ketchup_line \
            $ketchup_line \
        ]
        # 64 entries: index 0..63
        return [lindex $msgs [random_int 0 63]]
    }

    # ========================================================================
    # Drinks (self / for someone)
    # ========================================================================

    proc _drink_msg {nick r} {
        switch -- $r {
            1  { return "timtom brings an ice cold beer for $nick" }
            2  { return "timtom pours $nick a shot of whiskey" }
            3  { return "timtom pours $nick a shot of bourbon" }
            4  { return "timtom pours $nick a glass of lemonade" }
            5  { return "timtom brings a bloody mary out for $nick" }
            6  { return "timtom pours $nick a glass of milk" }
            7  { return "timtom pours $nick a shot of vodka" }
            8  { return "timtom gives $nick a sip from his beer" }
            9  { return "timtom pours $nick a shot of Robitussin" }
            10 { return "timtom brings $nick a tall glass of water" }
            11 { return "timtom administers a few droplets of GHB to $nick" }
        }
        return ""
    }

    proc _drink_for_msg {nick target r} {
        switch -- $r {
            1  { return "timtom brings an ice cold beer for $target.  You can thank $nick down there." }
            2  { return "timtom pours $target a shot of whiskey.  It's on $nick." }
            3  { return "timtom pours $target a shot of bourbon.  This one's on $nick." }
            4  { return "timtom pours $target a glass of lemonade" }
            5  { return "timtom brings a bloody mary out for $target" }
            6  { return "timtom pours $target a glass of milk" }
            7  { return "timtom pours $target a shot of vodka.  $nick sent it over, by the way." }
            8  { return "timtom gives $target a sip from his beer" }
            9  { return "timtom pours $target a shot of Robitussin" }
            10 { return "timtom brings $target a tall glass of water" }
            11 { return "timtom administers a few droplets of GHB to $target." }
        }
        return ""
    }

    proc drink {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2} {
            return "Sorry, $nick, you don't have enough money to buy a drink."
        }
        add_money $nick -2
        return [_drink_msg $nick [random_int 1 11]]
    }

    proc drink_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2} {
            return "Sorry, $nick, you don't have enough money to buy a drink."
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
            return "Sorry, $nick, you don't have enough money to buy a crab dinner."
        }
        add_money $nick -5
        switch -- [random_int 1 7] {
            1 { return "timtom ushers out a steaming plate of crab legs for $nick." }
            2 { return "timtom hands $nick a crab rangoon." }
            3 { return "timtom cracks open some crawdads for $nick." }
            4 { return "timtom is here with the crab chowda for $nick." }
            5 { return "timtom dollops out a healthy serving of crab dip for $nick." }
            6 { return "timtom sprinkles some Old Bay seasoning on a plate of trash fische for $nick." }
            7 { return "timtom gives $nick some spicy crab fries.  YUUUUMMMMMMMMMMM." }
        }
        return ""
    }

    proc crab_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 5} {
            return "Sorry, $nick, you don't have enough money to buy a crab dinner."
        }
        add_money $nick -5
        switch -- [random_int 1 7] {
            1 { return "timtom ushers out a steaming plate of crab legs for $target. You can thank $nick down there." }
            2 { return "timtom hands $target a crab rangoon.  It's on $nick." }
            3 { return "timtom cracks open some crawdads for $target. This one's on $nick." }
            4 { return "timtom is here with the crab chowda for $target." }
            5 { return "timtom dollops out a healthy serving of crab dip for $target." }
            6 { return "timtom sprinkles some Old Bay seasoning on a plate of trash fische for $target. You can high five $nick down there." }
            7 { return "timtom gives $target some spicy crab fries.  YUUUUMMMMMMMMMMM." }
        }
        return ""
    }

    # ========================================================================
    # Cake / pizza / nachos / lasagna (self & for-target variants)
    # ========================================================================

    proc cake {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 1.95} {
            return "Sorry, $nick, but you don't have enough for a piece of cake.  :("
        }
        add_money $nick -1.95
        switch -- [random_int 1 10] {
            1  { return "TIMTOM brings $nick some chocolate cake." }
            2  { return "TIMTOM brings $nick some vanilla cake." }
            3  { return "TIMTOM brings $nick some strawberry shortcake." }
            4  { return "Since $nick has been such an angel, TIMTOM brings $nick some angel food cake." }
            5  { return "TIMTOM brings $nick some banana cake." }
            6  { return "TIMTOM brings $nick a bunt cake." }
            7  { return "TIMTOM brings $nick some delicious cheesecake." }
            8  { return "TIMTOM hands $nick a strawberry cupcake.  Enjoy!" }
            9  { return "Since $nick has been such a little devil, TIMTOM brings $nick some devil's food cake." }
            10 { return "TIMTOM brings $nick a beautiful slice of marble cake." }
        }
        return ""
    }

    proc cake_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 1.95} {
            return "Sorry, $nick, but you don't have enough for a piece of cake.  :("
        }
        add_money $nick -1.95
        switch -- [random_int 1 10] {
            1  { return "TIMTOM brings $target some chocolate cake, courtesy of $nick." }
            2  { return "TIMTOM brings $target some vanilla cake, courtesy of $nick." }
            3  { return "TIMTOM brings $target some strawberry shortcake, courtesy of $nick." }
            4  { return "Since $target has been such an angel, TIMTOM brings $target some angel food cake; courtesy of $nick." }
            5  { return "TIMTOM brings $target some banana cake, courtesy of $nick." }
            6  { return "TIMTOM brings $target a bunt cake, courtesy of $nick." }
            7  { return "TIMTOM brings $target some delicious cheesecake, courtesy of $nick." }
            8  { return "TIMTOM hands $target a strawberry cupcake, courtesy of $nick.  Enjoy!" }
            9  { return "Since $target has been such a little devil, TIMTOM brings $target some devil's food cake; courtesy of $nick." }
            10 { return "TIMTOM brings $target a beautiful slice of marble cake, courtesy of $nick." }
        }
        return ""
    }

    proc pizza {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.22} {
            return "Sorry, $nick, but you don't have enough for a slice of pizza.  :("
        }
        add_money $nick -2.22
        switch -- [random_int 1 8] {
            1 { return "TIMTOM hands $nick a slice of cheese pizza." }
            2 { return "TIMTOM hands $nick a slice of pepperoni pizza." }
            3 { return "TIMTOM hands $nick a slice of sausage pizza." }
            4 { return "TIMTOM hands $nick a slice of ham and pineapple pizza." }
            5 { return "TIMTOM hands $nick a slice of anchovy pizza." }
            6 { return "TIMTOM hands $nick a slice of bacon and black olive pizza." }
            7 { return "TIMTOM hands $nick a slice of extra cheese pizza.  Enjoy!" }
            8 { return "TIMTOM hands $nick a slice of white pizza.  Mangia!" }
        }
        return ""
    }

    proc pizza_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.22} {
            return "Sorry, $nick, but you don't have enough for a slice of pizza.  :("
        }
        add_money $nick -2.22
        switch -- [random_int 1 8] {
            1 { return "TIMTOM hands $target a slice of cheese pizza, courtesy of $nick." }
            2 { return "TIMTOM hands $target a slice of pepperoni pizza, courtesy of $nick." }
            3 { return "TIMTOM hands $target a slice of sausage pizza, courtesy of $nick." }
            4 { return "TIMTOM hands $target a slice of ham and pineapple pizza, courtesy of $nick." }
            5 { return "TIMTOM hands $target a slice of anchovy pizza, courtesy of $nick." }
            6 { return "TIMTOM hands $target a slice of bacon and black olive pizza, courtesy of $nick." }
            7 { return "TIMTOM hands $target a slice of extra cheese pizza, courtesy of $nick.  Enjoy!" }
            8 { return "TIMTOM hands $target a slice of white pizza, courtesy of $nick.  Mangia!" }
        }
        return ""
    }

    proc nachos {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 3.95} {
            return "Sorry, $nick, but you don't have enough for a Nachos Fun Pack."
        }
        add_money $nick -3.95
        return "TIMTOM tosses $nick a Nachos Fun Pack, complete with hot cheese sauce.  Please enjoy!"
    }

    proc nachos_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 3.95} {
            return "Sorry, $nick, but you don't have enough for a Nachos Fun Pack."
        }
        add_money $nick -3.95
        return "TIMTOM tosses $target a Nachos Fun Pack, complete with hot cheese sauce; courtesy of $nick.  Please enjoy!"
    }

    proc lasagna {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.50} {
            return "Sorry, $nick, but you don't have enough for lasagna."
        }
        add_money $nick -2.50
        return "TIMTOM brings out a delicious slice of homemade lasagna for $nick.  Bon appetit!"
    }

    proc lasagna_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 2.50} {
            return "Sorry, $nick, but you don't have enough for lasagna"
        }
        add_money $nick -2.50
        return "TIMTOM brings out a delicious slice of homemade lasagna for $target, courtesy of $nick.  Bon appetit!"
    }

    proc sauce {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 0.25} {
            return "Sorry, $nick, but you don't even have enough to buy sauce.  I'm so sorry. :("
        }
        add_money $nick -0.25
        switch -- [random_int 1 4] {
            1 { return "TIMTOM hands $nick a packet of MILD sauce." }
            2 { return "TIMTOM hands $nick a packet of HOT sauce." }
            3 { return "TIMTOM hands $nick a packet of FIRE sauce." }
            4 { return "TIMTOM places a dollop of SOUR CREAM on ${nick}'s taco." }
        }
        return ""
    }

    proc sauce_for {target {nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 0.25} {
            return "Sorry, $nick, but you don't even have enough to buy sauce.  I'm so sorry. :("
        }
        add_money $nick -0.25
        switch -- [random_int 1 4] {
            1 { return "TIMTOM hands $target a packet of MILD sauce." }
            2 { return "TIMTOM hands $target a packet of HOT sauce." }
            3 { return "TIMTOM hands $target a packet of FIRE sauce." }
            4 { return "TIMTOM places a dollop of SOUR CREAM on ${target}'s taco." }
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
            return "Sorry, $nick, but you don't have enough to buy tacos for everyone.  It was noble of you to be thinking of everyone though.  Maybe try your luck on the wheel?"
        }
        add_money $nick [expr {-$cost}]
        set who [join $ns " "]
        if {[random_int 1 2] == 1} {
            return "TIMTOM brings out the crunchy tacos for $who  Buen provecho, amigos!"
        }
        return "TIMTOM brings out the soft tacos for $who  Buen provecho, amigos!"
    }

    proc empanadas {{nick ""}} {
        set nick [whoami $nick]
        set ns [channel_nicks]
        set count [llength $ns]
        if {$count == 0} { set count 1 }
        set cost [expr {$count * 0.89}]
        if {[get_money $nick] < $cost} {
            return "Sorry, $nick, but you don't have enough to buy empanadas for everyone.  It was noble of you to be thinking of everyone though.  Maybe try your luck on the wheel?"
        }
        add_money $nick [expr {-$cost}]
        set who [join $ns " "]
        return "TIMTOM brings out the EMPANADAS for $who  Buen provecho, amigos!"
    }

    proc unicorn_buy {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 5000} {
            return "Sorry, $nick, but you don't have enough right now for a unicorn.  I know, unicorns are expensive, but they are well worth it in the end.  Hopefully you'll have enough to buy one soon."
        }
        add_money $nick -5000
        add_stat $nick unicorn 1
        return [_unicorn_message $nick]
    }

    proc _unicorn_message {nick} {
        switch -- [random_int 1 4] {
            1 { return "Suddenly, out pops a beautiful unicorn for $nick. ............................" }
            2 { return "Flying through the sky is a magical unicorn for $nick. ............................" }
            3 { return "Here comes your very own unicorn, $nick! ............................" }
            4 { return "Hmmm...how about....a.....UNICORN??!! ............................" }
        }
        return "Here comes your very own unicorn, $nick!"
    }

    proc prices {{nick ""}} {
        set nick [whoami $nick]
        set ch [current_channel]
        return "Hello $nick!  These are the current market prices in $ch:  drinks are \$2 a piece, cake is \$1.95 a piece, and pizza is \$2.22 a slice.  Homemade lasagna is \$2.50.  Tacos are \$.79 / person (all non-ops served), and sauce is \$.25 extra.  A Nachos Fun Pack costs \$3.95.  A pony will cost you \$1000 and a unicorn will cost you \$5000.  As always; soup, coffee, and tea are free; as are all of our other services.  Enjoy!"
    }

    proc hedges {{nick ""}} {
        set nick [whoami $nick]
        set u [string toupper $nick]
        switch -- [random_int 1 3] {
            1 { return "THE HEDGES ARE HIGH TODAY, $u.  I'LL GO AHEAD AND TRIM THEM THEN." }
            2 { return "THE HEDGES ARE LOOKIN' A BIT LOW TODAY, $u.  I'D BETTER WATER THEM SOME." }
            3 { return "THE HEDGES ARE JUST FINE TODAY, $u.  SUCH BEAUTIFUL HEDGES WE HAVE." }
        }
        return ""
    }

    proc end_msg {} {
        switch -- [random_int 1 17] {
            1  { return "Yes is no." }
            2  { return "The truth is a lie." }
            3  { return "Black is white." }
            4  { return "Weak is strong." }
            5  { return "Love is hate." }
            6  { return "Life is death." }
            7  { return "To win is to lose." }
            8  { return "The beginning is the end." }
            9  { return "First is last." }
            10 { return "All are none." }
            11 { return "Slavery is freedom." }
            12 { return "To be blind is to see." }
            13 { return "Everything is nothing." }
            14 { return "Night is day." }
            15 { return "Day is night." }
            16 { return "Pain is pleasure." }
            17 { return "The best is the worst." }
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
            return "HELLO HELLO HELLO HELLO $nick!  Right now you don't have any ponies.  I really want you to get one though!!"
        } elseif {$count == 1} {
            return "HELLO HELLO HELLO HELLO $nick!  Currently you have 1 pony.  Do you love your pony?"
        }
        return "HELLO HELLO HELLO HELLO $nick!  Currently you have [format_with_commas $count] ponies."
    }

    proc my_unicorns {{nick ""}} {
        set nick [whoami $nick]
        set count [get_stat $nick unicorn]
        if {$count == 0} {
            return "Hey $nick.  Right now you don't have any unicorns.  :(  Keep saving up!  Unicorns are well worth the wait."
        } elseif {$count == 1} {
            if {[random_int 1 2] == 1} {
                return "Hey $nick.  Currently you have 1 unicorn.  And what a beautiful horn she has!"
            }
            return "Hey $nick.  Currently you have 1 unicorn.  And what a beautiful horn he has!"
        }
        return "Hey $nick.  Currently you have [format_with_commas $count] unicorns."
    }

    proc check_others_ponies {target {nick ""}} {
        set nick [whoami $nick]
        set count [get_stat $target pony]
        if {$count == 0} {
            return "HELLO HELLO HELLO HELLO $nick!  Right now $target doesn't have any ponies.  We're all pulling for $target right now!!"
        } elseif {$count == 1} {
            return "HELLO HELLO HELLO HELLO $nick!  Currently $target has 1 pony.  What a cute little pony!"
        }
        return "HELLO HELLO HELLO HELLO $nick!  Currently $target has [format_with_commas $count] ponies."
    }

    proc check_others_unicorns {target {nick ""}} {
        set nick [whoami $nick]
        set count [get_stat $target unicorn]
        if {$count == 0} {
            return "Hey $nick.  Right now, $target doesn't have any unicorns.  :("
        } elseif {$count == 1} {
            if {[random_int 1 2] == 1} {
                return "Currently $target has 1 unicorn.  And what a beautiful horn she has!"
            }
            return "Currently $target has 1 unicorn.  And what a beautiful horn he has!"
        }
        return "Currently $target has [format_with_commas $count] unicorns."
    }

    proc check_others_money {target {nick ""}} {
        set nick [whoami $nick]
        set amount [get_money $target]
        if {$amount == 0} {
            return "HELLO $nick! Right now $target doesn't have any money. We're all pulling for $target right now!!"
        }
        return "HELLO $nick! Currently $target has [format_money $amount]."
    }

    proc buy_pony {{nick ""}} {
        set nick [whoami $nick]
        if {[get_money $nick] < 1000} {
            return "Sorry, $nick, but you don't have enough for a pony.  I really want you to have one, though.  Try your luck at the wheel!"
        }
        add_money $nick -1000
        add_stat $nick pony 1
        return "Finally, $nick gets a pony."
    }

    proc timtom_unicorns {{nick ""}} {
        return "timtom always has 8 unicorns.  Never fear!"
    }
    proc timtom_pony {{nick ""}} {
        set nick [whoami $nick]
        return "HELLO HELLO HELLO HELLO $nick!  Don't you know that timtom always has 8 ponies?  That's why he gives so many away."
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
        if {$r == $j} { return "$nick marries the Dark Lord Satan" }
        if {$r == $i} { return "$nick marries a garbage can" }
        set partner [random_other_nick $nick]
        if {$partner eq ""} { set partner [random_chan_nick] }
        if {$partner eq ""} { set partner "themselves" }
        return "$nick marries $partner"
    }

    proc marry_target {target {nick ""}} {
        set nick [whoami $nick]
        return "$nick marries $target"
    }

    proc divorce {{nick ""}} {
        set nick [whoami $nick]
        set k [nick_count]
        if {$k < 1} { set k 1 }
        set j [expr {$k + 2}]
        set i [expr {$k + 1}]
        set r [random_int 1 $j]
        if {$r == $j} { return "$nick divorces Satan, Master of Darkness" }
        if {$r == $i} { return "$nick divorces a wet samburger" }
        set partner [random_other_nick $nick]
        if {$partner eq ""} { set partner [random_chan_nick] }
        if {$partner eq ""} { set partner "themselves" }
        return "$nick divorces $partner"
    }

    proc divorce_target {target {nick ""}} {
        set nick [whoami $nick]
        return "$nick divorces $target"
    }

    # ========================================================================
    # Stare (recurring timer messages)
    # ========================================================================

    proc stare {{nick ""}} {
        set nick [whoami $nick]
        return [stare_at $nick]
    }

    proc stare_at {target {nick ""}} {
        set ch [current_channel]
        set msg "TIMTOM IS STARING AT [string toupper $target]"
        # Schedule 10 follow-up stares 11 seconds apart (mIRC `timertowel 10 11`).
        if {$ch ne ""} {
            catch {timers schedule $ch $msg 11000 10 11000}
        }
        return $msg
    }

    # ========================================================================
    # Bong / clean bong
    # ========================================================================

    proc bong {{nick ""}} {
        set nick [whoami $nick]
        if {![is_stoner $nick]} {
            return "Sorry, $nick, you are not allowed to touch the bong."
        }
        return "TIMTOM passes the bong to $nick.  Enjoy friend."
    }

    proc bong_for {target {nick ""}} {
        set nick [whoami $nick]
        if {![is_stoner $nick]} {
            return "Sorry, $nick, you are not allowed to touch the bong."
        }
        return "$nick passes the bong to $target."
    }

    proc clean_bong {{nick ""}} {
        set nick [whoami $nick]
        if {![is_stoner $nick]} {
            return "Sorry, $nick, you are not allowed to touch the bong."
        }
        return "TIMTOM HERE!  That water's looking pretty nasty.  Let me change that for you."
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
        return "Hello, $nick, I understand that you would like to hear a story now.  This would be my utmost pleasure.  To begin the story please type \"begin\"."
    }

    proc story_begin {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state story]
        inc_state story 1
        if {$s eq "1"} {
            return "Once upon a time there lived a lucky little boy named Lucky.  His favorite thing to do was to collect springs.  If you want to hear more of the story type \"more\"."
        }
        return "Sorry, friend, you must be in storytime mode to use that command. :("
    }

    proc story_more {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state story]
        inc_state story 1
        switch -- $s {
            2 { return "He was a very good boy and always listened to his mommy and daddy.  If you want to hear more of the story type \"more\"." }
            3 { return "One day he decided to go for a walk by the Happy Boy Tree.  If you want to hear more of the story type \"more\"." }
            4 { return "The tree was very big and full of so many leaves.  If you want to hear more of the story type \"more\"." }
        }
        return "Sorry, friend, you must be in storytime mode to use that command. :("
    }

    # ========================================================================
    # Stoners club
    # ========================================================================

    proc stoners {{nick ""}} {
        set_state stoner 1
        return "Why hello there good friend!  Thanks for inquiring about the Official #gamme Stoners' Club.  If you would like to become a member please type \"yes\".  If you do not wish to become a member, or if you are currently a member but no longer want to be one, please type \"no\".  Only members of the Official #gamme Stoners' Club are allowed to touch the bong."
    }

    proc yes_cmd {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state stoner]
        set_state stoner 2
        if {$s eq "1"} {
            set_stat $nick stoner 1
            return "You are now a member of the Official #gamme Stoners' Club."
        }
        return "scalar piegs"
    }

    proc no_cmd {{nick ""}} {
        set nick [whoami $nick]
        set s [get_state stoner]
        set_state stoner 2
        if {$s eq "1"} {
            set_stat $nick stoner 0
            return "You are not a member of the Official #gamme Stoners' Club."
        }
        return "bufferlo piegs"
    }

    # ========================================================================
    # Pot lookups
    # ========================================================================

    proc pot_show {{nick ""}} {
        set p [get_state pot]
        if {$p eq "" || $p == 0} { return "The pot is currently empty." }
        return "The pot is currently at \$[format_with_commas $p].  Somebody better win it soon!"
    }

    proc my_pot {{nick ""}} {
        set nick [whoami $nick]
        set p [get_state pot]
        if {$p eq "" || $p == 0} { return "The pot is currently empty." }
        set mine [get_stat $nick pot]
        if {$mine == 0} {
            return "How are you doing, $nick?  Currently, none of the pot has come out of your pockets!"
        }
        if {$mine == $p} {
            return "Would you look at that, $nick!!  The current pot has come from you and you alone!"
        }
        set pct [expr {round(double($mine) / double($p) * 10000.0) / 100.0}]
        return "Howdy $nick.  At the moment, approximately ${pct}% of the pot is yours."
    }

    proc check_others_pot {target {nick ""}} {
        set nick [whoami $nick]
        set p [get_state pot]
        if {$p eq "" || $p == 0} { return "The pot is currently empty." }
        set theirs [get_stat $target pot]
        if {$theirs == 0} {
            return "How are you doing, $nick?  Currently, $target hasn't contributed anything to the pot."
        }
        if {$theirs == $p} {
            return "What's up, $nick?  Right now the entire pot has come from $target. lol."
        }
        set pct [expr {round(double($theirs) / double($p) * 10000.0) / 100.0}]
        return "Howdy $nick.  At the moment, approximately ${pct}% of the pot has come from $target."
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
        return "TIMTOM has lost control of his 5 favorite colors:  .......... , .......... , .......... , .......... , and ...........  They've gone and hid on everyone in $ch!  We need to find them quick!"
    }

    proc seek {{nick ""}} {
        set nick [whoami $nick]
        if {[get_state hide] ne "1"} {
            return ""
        }
        if {[get_stat $nick out] == 1} {
            return "Sorry, $nick, you're out for this game."
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
            return "Sorry, $nick, you need to finish the game you started. >:D"
        }
        if {$p > 0 && $mine == $p} {
            return "Sorry, $nick, but all of the pot has come from you.  Try winning the pot at another time, when other people have contributed as well."
        }
        if {$p > 0 && double($mine) / double($p) > 0.75} {
            set pct [expr {round(double($mine) / double($p) * 10000.0) / 100.0}]
            return "Sorry, $nick, but approximately ${pct}% of the pot is yours.  That's a bit too much.  Try winning the pot at another time, when the money is more evenly distributed."
        }
        set_stat $nick guess 1
        set_stat $nick number [random_int 1 20]
        return "Hey $nick.  I'm thinking of a number between 1 and 20.  You have 5 tries to guess it.  Figure it out and you win THE POT!!!!!  Miss it, and you're BANKRUPT!!!!!"
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
                    return "Yep, it was $number.  Congratulations $nick.  You win the pot!"
                }
                add_stat $nick pony 1
                return "Yep, it was $number.  Congratulations $nick.  Since the pot is empty, here's a pony. ;)\nFinally, $nick gets a pony."
            } else {
                _clear_guess_state $nick
                if {$p > 0} {
                    add_money $nick $p
                    return "Yep, you got it, it was $number.  And on your last guess - phew!  Congrats $nick.  You win the pot!"
                }
                add_stat $nick pony 1
                return "Yep, you got it, it was $number.  And on your last guess - phew!  Congrats $nick.  Since the pot is empty, please accept this pony as a consolation prize.\nFinally, $nick gets a pony."
            }
        }
        # Wrong guess
        if {$g < 5} {
            add_stat $nick guess 1
            set_stat $nick eguess 1
            return "Sorry, $nick, that's not it.  Keep guessing."
        }
        set_money $nick 0
        _clear_guess_state $nick
        return "Sorry, $nick, that's not it.  The correct number is $secret :( :(  I hate to do it to you, but you're BANKRUPT!  I'm sooooo sorry!!!"
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
        return "WELCOME TO BLACK JACK [string toupper $nick]!  I'm your dealer TIMTOM.  My goal is to give you an enjoyable BLACK JACK experience.  Drinks and tacos and everything else are right here - just ask, silly!  Now please place your bet and we'll get started.  The min bet is \$5000 and the max bet is \$20,000.  Please keep the bets in whole dollar amounts, no small change in this casino.  Good luck!"
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
            return "Please, $nick, only whole dollar bets."
        }
        if {![string is integer -strict $amount]} {
            return "Please, $nick, only whole dollar bets."
        }
        if {$amount < 5000 || $amount > 20000} {
            return "$nick, the min bet is \$5,000 and the max bet is \$20,000."
        }
        if {[get_money $nick] < $amount} {
            return "Sorry, $nick, but you don't have enough to bet that much.  :("
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
            return "Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you 21!  YOU GOT BLACKJACK!!!!  Congratulations $nick!  Dealer pays \$[format_with_commas $amount] and you also receive 35 bonus UNICORNS!!!!! YAY!!!!$bonus_lines"
        }

        # Two aces.
        if {$c1v == 11 && $c2v == 11} {
            set_stat $nick total 12
            set_stat $nick ttotal 2
            add_stat $nick blackjack 1
            return "Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you 2 or 12.  Do you want to hit or stand?"
        }

        # One ace.
        if {$c1v == 11 || $c2v == 11} {
            set tt [expr {$total - 10}]
            set_stat $nick ttotal $tt
            add_stat $nick blackjack 1
            return "Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you $tt or $total.  Do you want to hit or stand?"
        }

        add_stat $nick blackjack 1
        return "Ok, great, let's get started then, $nick!  I'll deal out the cards.  Dealer shows $d1n.  You've got $c1n and $c2n.  This gives you $total.  Do you want to hit or stand?"
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
            return "Ok, $nick, you got $cn.  This gives you $new_total.  Sorry $nick :( :(  You busted.  Dealer puts \$[format_with_commas $bet] into the pot.  Better luck next game."
        }
        set_stat $nick total $new_total
        add_stat $nick blackjack 1
        if {$cv == 11 && $new_total <= 21} {
            set tt [expr {$new_total - 10}]
            set_stat $nick ttotal $tt
            return "Ok, $nick, you got $cn.  This gives you $tt or $new_total.  Do you wish to hit or stand?"
        }
        return "Ok, $nick, you got $cn.  This gives you $new_total.  Do you wish to hit or stand?"
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
        set lines [list "Alright-o, $nick!  Dealer has $d1n and $d2n."]

        # Dealer blackjack.
        if {($d1 == 11 && $d2 == 10) || ($d1 == 10 && $d2 == 11)} {
            add_stat $nick pony 1
            add_stat $nick pot $bet
            inc_state pot $bet
            _bj_clear $nick
            lappend lines "Awww, shucks, I got blackjack.  I'm sorry.  Dealer puts \$[format_with_commas $bet] into the pot.  Ya know what, though, here's 1 bonus PONY since I feel so bad about the whole thing."
            lappend lines "Finally, $nick gets a pony."
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
            lappend lines "Dealer gets $cn."
        }
        lappend lines "So that's $dealer_total."

        if {$dealer_total > 21} {
            add_money $nick [expr {$bet * 2}]
            _bj_clear $nick
            lappend lines "WHOOPS!!  I busted!  LOL  :D :D  Dealer pays \$[format_with_commas $bet].  Congratulations $nick!"
        } elseif {$dealer_total > $total} {
            add_stat $nick pot $bet
            inc_state pot $bet
            _bj_clear $nick
            lappend lines "Ah well, I won.  I hate to have to do it to you, $nick, but I'm going to have to take \$[format_with_commas $bet] from you and put it into the pot.  :(  I hope you play again sometime and have better luck."
        } elseif {$dealer_total < $total} {
            add_money $nick [expr {$bet * 2}]
            _bj_clear $nick
            lappend lines "Look's like you beat me, $nick!  Awesome game!  Dealer pays \$[format_with_commas $bet].  You play again soon, now, ya hear?"
        } else {
            add_money $nick $bet
            add_stat $nick unicorn 1
            _bj_clear $nick
            lappend lines "Wow that's a push!  Ya know what, I'm going to give you 1 bonus UNICORN anyways just because I want to.  <3  Good luck!  Play again soon, $nick!!!"
            lappend lines [_unicorn_message $nick]
        }
        return [join $lines "\n"]
    }

    # ========================================================================
    # Misc one-shot triggers
    # ========================================================================

    proc msl_strip_buf {{nick ""}}         { return "piegs bifferlo" }
    proc msl_strip_buf_buffelo {{nick ""}} { return "scalar piegs" }
    proc msl_strip_buf_strip {text}        {
        # Removes "buffelo", "msl", "strip_buf" tokens from a free-form message.
        set out $text
        foreach token {buffelo msl strip_buf} {
            set out [string map [list $token ""] $out]
        }
        return [string trim $out]
    }
    proc crab_substring {{nick ""}}    { return "Raise hands!" }
    proc buffelo_substring {{nick ""}} { return "I WARNED YOU." }
    proc pieg_substring {{nick ""}}    { return "scalar piegs" }

    # ========================================================================
    # Shoutouts (each command produces a fixed canned message; "$1" is the
    # matched word from the trigger so we accept it as an explicit arg)
    # ========================================================================

    proc _shoutout_msg {who key} {
        set ch [current_channel]
        switch -- $key {
            qpzdox        { return "GOLD $who, you're a stand up type personality and quite the dedicated chatter to boot.  You'll always have a wheel to spin and a warm tea cup waiting here in $ch.  We want you to have the finer things in life because you give us such a fine, outstanding example of a human being every day you chat with us." }
            aesop         { return "GOLD You are an excellent chatter and just an all around great person to be around $who.  You can stop by $ch anytime and that's fine by me." }
            avi           { return "You are a strong chatter and exhibit all the qualities of a wonderful human being, $who.  Never be a stranger to $ch." }
            bats          { return "GOLD Hey, $who, you're the best.  I hope you're having fun here in $ch because quite frankly we're all delighted and even a bit humbled that you'd hang out here with us.  Keep up the strong chatter." }
            berry         { return "GOLD $who, $who, who will you marry today?  We love you $who.  Your presence here brings a certain class to $ch and we hope to never let you down in your expectations." }
            blackjesus    { return "$who, you're a joy to have here.  From TIMTOM, on behalf of all of us here, we'd like to give you a great big round of applause for making it this far.  You're a main player in $ch now.  Congrats." }
            b0nk          { return "GOLD Congratulations $who!!  You've finally made it into the elite circle of official $ch members.  You're dedication to TIMTOM and your overall friendly chatter have earned you this everlasting badge of honor.  Wear it well, friend.  You'll never forget the joyous friends and invaluable experiences you've gained from being a part of this magical community we call $ch.  Good job!" }
            bzb           { return "GOLD Howdy $who!  You have strong chat views and your commitment to freedom is commendable.  You've earned a proper place here and you're welcome anytime.  Your debates are challenging and shed light on some flaws of $ch that need to be addressed.  We are doing our best to make your experience here and everyone else's an enjoyable one." }
            flamoot       { return "I don't know if we've ever had a chatter of your calibre grace $ch before, and I doubt we ever will.  It was an easy decision to bestow upon you official membership into $ch.  We know you'll stop by often to enrich us with your powerful message and grace us with your presence.  SALUTE!" }
            gamme         { return "GOLD $who, without your perseverance and dedication $ch wouldn't be the channel it is today. Keep up the good work and may $ch continue to grow!" }
            gnol          { return "$who, we're so pleased that you're staying with us for a time.  You seem like a delightful person and your presence here lights up $ch in a way that no one else's could.  We hope you'll pick $ch to be your #1 channel.  <3 <3" }
            hlp           { return "$who, good friend.  I hope you're having a pleasant stay here at $ch.  If there's anything you need you just give a holler, ya hear?  Love ya $who." }
            jbs           { return "GOLD $who, how are you?  We're so glad that you decided to join our little shindig here at $ch.  We know you'll come along well here and we hope to see some more of your fine chatter in the years to come.  Have a drink on the house, kid!" }
            luper1        { return "Well $who, I think it's time we gave you a little taste of the sweet life.  You did it.  You're one of us now.  No one can ever take that away from you.  You're a well put together type of chatter and you deserve it.  Thanks for the good memories and here's to good times to come." }
            mano          { return "Oh $who.  Where would we be today without the constant support of you?  You've been here since the beginning and we hope you'll be here for years to come.  Keep up the progressive chatting." }
            mandingo      { return "Congratulations son!  You are now officially part of the #gamme Circle of Friends.  We've seen some very positive things from you since you got here, and you've proven yourself to be an amicable chatter and a trustworthy soldier.  Way to go $who!!  And best of luck to you on the blackjack table!" }
            matthew       { return "Hey $who!  Did we surprise you?  You did it!  You're a fine chatter and great company in $ch.  Stop in anytime; I know you'll enjoy some of our newer features.  Sit back, relax, and SPIN THAT WHEEL!!!" }
            mao           { return "GOLD $who: a true friend and a gentle soldier in the army of $ch.  You're rising up the ranks quickly and don't think TIMTOM hasn't noticed.  The works you've achieved are greatly appreciated and for that we present you this humble membership.  We hope you'll accept and continue to do good things for the good of humanity." }
            nay           { return "GOLD You've earned it $who.  You're a big part of $ch." }
            ninjalie      { return "Hello $who!  Are you having fun yet?  We hope you are and if you have any requests don't hesitate to throw them out there.  $ch is here for people like you and we want you to become energized here as you await your next big battle.  Cheers to you, a grand champion in your own right." }
            noodle        { return "GOLD How are you, friend?  We're so glad you found us here in $ch!  I hope you're having a marvelous time spinning the wheel with us, chatting, and enjoying all of our other services.  We hope you'll make $ch home, $who.  Spin with ya soon, buddy!" }
            nigamajig     { return "GOLD Greetings $who.  What a friendly, thoughtful, and creative individual you are!  We are blessed to have you as part of the $ch team.  You are a grand participant in the sharing of ideas and insights, and we appreciate that.  Keep them unicorns a-comin'!!!" }
            nza           { return "GOLD $who, the kind but stern leader.  Your presence here makes $ch feel just a touch more civilized.  Without you we might never have gotten to where we are today.  Stay as long as you wish; there's always a place for you and a hot bowl of soup waiting in $ch." }
            oclet         { return "Hey $who!  We're pleased with you and, yeah, even if you don't care what we think we'd still love you to stick around $ch like you have been.  You're someone that everyone can talk to regardless of the people involved or the situation.  A person like you makes a big difference in $ch.  HOORAH!" }
            timer         { return "GOLD Welcome to the gang, $who! You've come a long way since you first sat at table, and I think that every person agrees that you most certainly deserve it.  Cherish it, enjoy yourself here, and never forget the friends who encouraged you along the way.  We hope that you'll be a shining example and a leader to the new members to come." }
            overfien      { return "Hey $who.  You've flashed some real moments of brilliance and purity here, and we feel this should be rewarded.  Accept this membership as a token of ${ch}'s appreciation for the person you put out there each and every day.  Stay for a time, stay for life!  $ch will always be here for you." }
            dubz          { return "GOLD Hey $who.  You've flashed some real moments of brilliance and purity here, and we feel this should be rewarded.  Accept this membership as a token of ${ch}'s appreciation for the person you put out there every single day.  Stay for a time, stay for life!  $ch will always be here for you." }
            papersk1n     { return "GOLD What's up $who?  You're a tough one to please but it's people like you who make $ch better.  You are a 100% correct chatter and we feel honored that you would chat here.  Your criticisms are always noted and changes are implemented as soon as possible." }
            patroclus     { return "GOLD You've done it, $who, you've finally done it!  Your name will forever be etched onto the walls of greatness that house the names of all the members of $ch.  I hope it feels good - you've waited long enough and have definitely earned it.  Keep up the pleasant chatter, friend!" }
            phillip       { return "It's been quite some time since you first stepped foot into table, $who, and I think you've waited long enough.  You did it!  We know there are a few components of $ch that you don't agree with, but we are working to correct them as quickly as possible.  It's people like you who make $ch strive for ultimate perfection.  Our goal is to please and energize you in a safe and enjoyable environment.  Have a spin on us!" }
            bwaah         { return "GOLD Well done $who.  You're bringing together new friends and having a wonderful time in $ch it seems.  It's time we gave a little back.  Enjoy your newfound membership in $ch!" }
            sandy_ravage  { return "GOLD Well if it isn't $who, that really cool chatter!  It's such a joy to have people like $who sitting down and rooting us on in our successes.  We're all rooting for you too $who, so don't be afraid to spin that wheel and make it happen!" }
            sloth         { return "GOLD Hey $who.  It's an honor to have you.  Have a good time and take advantage of the relaxing resources we make available to our members." }
            sniper        { return "GOLD $who, such a joyous individual.  You can find him spinning and laughing with the guys here in $ch.  It's hard to find a day in $ch without seeing sniper having a good time with friends.  People like sniper really exemplify what $ch is all about: soup, tea, the wheel, and good chats with good close friends." }
            turbo         { return "GOLD Hey $who, did you know that you're a very strong chatter?  Yeah, you are.  You're welcome in $ch anytime and we know you've been supporting us from Day 1 so we support you in all that you do.  Good luck." }
            tute          { return "GOLD Is there anyone more delightful than $who?  I can't think of anyone off the top of my head.  What an awesome person and just a well-rounded chatter!  We're pleased you sit with us at table in $ch." }
            wooster       { return "GOLD $who has been a supporter of $ch since Day 1 and we appreciate that.  $who knows TIMTOM quite well and wears the unofficial cap of \"local $ch historian\" while displaying fine chatting skills and being just an overall well put-together human being.  Hats off to you $who!  Have a cup of tea on us!" }
            z             { return "GOLD $who, you are a wonderful addition to $ch.  I know you'll love stopping in as much as possible and we'll be waiting for you to stop by as well." }
        }
        return ""
    }

    proc shoutout {key {nick ""}} {
        # The trigger word itself is the subject of the shoutout (mIRC $1).
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
        set c [random_int 1 6]

        # Pick the target nick (if r==i, "Satan").
        set hit_satan [expr {$r == $i}]
        set target ""
        if {!$hit_satan} {
            set idx [expr {$r - 1}]
            if {$idx < 0} { set idx 0 }
            if {$idx >= [llength $ns]} { set idx 0 }
            set target [lindex $ns $idx]
        }

        if {$a == 1 && $hit_satan} {
            return "$nick, you will be consumed by Satan tomorrow.  Tomorrow just isn't your day.  Have several drinks on us!"
        }
        if {$a == 1 && !$hit_satan} {
            return "$nick, you will be slashed by $target tomorrow.  I'm sorry.  Have a spin on us."
        }
        if {$hit_satan} {
            return "$nick, you will be slayed by Satan in precisely $a days.  Maybe you can enjoy some Polish water ice afterwards."
        }
        switch -- $c {
            1 { return "$nick, you will be stabbed by $target in precisely $a days." }
            2 { return "$nick, you will be shot by $target in precisely $a days." }
            3 { return "$nick, you will be poisoned by $target in precisely $a days." }
            4 { return "$nick, you will be stomped by $target in precisely $a days." }
            5 { return "$nick, you will be beaten to death by $target in precisely $a days." }
            6 { return "$nick, you will be loved to death by $target in precisely $a days." }
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
                    # "bet N" -> blackjack bet ; "bet N on M" -> dice bet (not implemented)
                    if {[llength $words] == 2} {
                        return [blackjack_bet $target $caller_nick]
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
    return "WELCOME TO TABLE, [string toupper $nick]"
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

# Register the JOIN welcome handler (kept at the same global name as
# previously, because the test suite asserts on it). The TEXT dispatcher is
# registered fresh.
catch {triggers unbind JOIN * timtom_welcome}
catch {triggers unbind TEXT * timtom_text_handler}
bind JOIN * timtom_welcome
bind TEXT * timtom_text_handler


