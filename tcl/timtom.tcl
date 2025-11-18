# TIMTOM IRC Bot - TCL Port
# Original mIRC script by gamme (2011-2017)
# Ported to TCL framework under timtom:: namespace
# Licensed under GNU GPL v3

namespace eval timtom {
    # Configuration
    variable bucket "timtom"

    # =========================================================================
    # Helper Functions
    # =========================================================================

    # Format money with commas for large amounts
    proc format_money {amount} {
        if {$amount == 0} {
            return "\$0"
        }
        # Handle decimals
        if {[string match "*.*" $amount]} {
            set parts [split $amount "."]
            set whole [lindex $parts 0]
            set decimal [lindex $parts 1]
            # Pad decimal to 2 digits
            if {[string length $decimal] == 1} {
                append decimal "0"
            }
            set formatted [format_with_commas $whole]
            return "\$$formatted.$decimal"
        } else {
            return "\$[format_with_commas $amount]"
        }
    }

    proc format_with_commas {num} {
        set num [expr {int($num)}]
        set str [format "%d" $num]
        set len [string length $str]
        if {$len <= 3} {
            return $str
        }
        set result ""
        set count 0
        for {set i [expr {$len - 1}]} {$i >= 0} {incr i -1} {
            if {$count > 0 && $count % 3 == 0} {
                set result ",$result"
            }
            set result "[string index $str $i]$result"
            incr count
        }
        return $result
    }

    # Get user's money
    proc get_money {nick} {
        variable bucket
        set key "money_[string tolower $nick]"
        if {[cache exists $bucket $key]} {
            return [cache get $bucket $key]
        }
        return 0
    }

    # Set user's money
    proc set_money {nick amount} {
        variable bucket
        set key "money_[string tolower $nick]"
        cache put $bucket $key $amount
    }

    # Add to user's money
    proc add_money {nick amount} {
        set current [get_money $nick]
        set new [expr {$current + $amount}]
        set_money $nick $new
        return $new
    }

    # Get user stat (ponies, unicorns, etc)
    proc get_stat {nick stat} {
        variable bucket
        set key "${stat}_[string tolower $nick]"
        if {[cache exists $bucket $key]} {
            return [cache get $bucket $key]
        }
        return 0
    }

    # Set user stat
    proc set_stat {nick stat value} {
        variable bucket
        set key "${stat}_[string tolower $nick]"
        cache put $bucket $key $value
    }

    # Add to user stat
    proc add_stat {nick stat amount} {
        set current [get_stat $nick $stat]
        set new [expr {$current + $amount}]
        set_stat $nick $stat $new
        return $new
    }

    # Get global state variable
    proc get_state {key} {
        variable bucket
        if {[cache exists $bucket $key]} {
            return [cache get $bucket $key]
        }
        return ""
    }

    # Set global state variable
    proc set_state {key value} {
        variable bucket
        cache put $bucket $key $value
    }

    # Random choice from list
    proc random_choice {args} {
        lindex $args [expr {int(rand() * [llength $args])}]
    }

    # =========================================================================
    # Main Command Dispatcher
    # =========================================================================

    proc handle {text} {
        set nick $::nick
        set text_lower [string tolower $text]
        set words [split $text]
        set first_word [string tolower [lindex $words 0]]

        # Simple trigger matching
        switch -glob -- $text_lower {
            "timtom" {
                return [greet $nick]
            }
            "sex" {
                return [sex $nick]
            }
            "horse" - "horses" {
                return [horses $nick]
            }
            "wheel" {
                return [wheel $nick]
            }
            "money" - "my money" {
                return [money $nick]
            }
            "spin" {
                return [spin $nick]
            }
            "soup" {
                return [serve_all "soup"]
            }
            "tea" {
                return [serve_all "tea"]
            }
            "coffee" {
                return [serve_all "coffee"]
            }
            "more soup" {
                return "Sorry, $nick, here's some more soup."
            }
            "more tea" {
                return "Sorry, $nick, here's some more tea."
            }
            "more coffee" {
                return "Sorry, $nick, here's some more coffee."
            }
            "jesus" {
                return [jesus $nick]
            }
            "rings" {
                return [rings]
            }
            "flip" {
                return [flip $nick]
            }
            "bonus" - "ok timtom" {
                return [bonus $nick]
            }
            "blackjack" {
                return [blackjack_start $nick]
            }
            "hit" {
                return [blackjack_hit $nick]
            }
            "stand" {
                return [blackjack_stand $nick]
            }
            "pony" - "ponies" - "my pony" - "my ponies" {
                return [my_ponies $nick]
            }
            "unicorn" - "unicorns" - "my unicorn" - "my unicorns" {
                return [my_unicorns $nick]
            }
            "buy pony" {
                return [buy_pony $nick]
            }
            "stare" {
                return [stare $nick]
            }
            default {
                # Check for state triggers
                if {[check_states $text_lower $nick result]} {
                    return $result
                }
                # Check for drink orders
                if {[string match "drink *" $text_lower]} {
                    return [drink $nick [lrange $words 1 end]]
                }
                # Check for food orders
                if {[string match "food *" $text_lower]} {
                    return [food $nick [string range $text_lower 5 end]]
                }
                # Check for bong commands
                if {[string match "bong*" $text_lower]} {
                    return [bong $nick $words]
                }
                # Check for bet command
                if {$first_word eq "bet"} {
                    return [blackjack_bet [lindex $words 1]]
                }
                # Check for marry/divorce
                if {$first_word eq "marry"} {
                    return [marry]
                }
                if {$first_word eq "divorce"} {
                    return [divorce]
                }
                # Check for person lookup (& nick)
                if {$first_word eq "&" && [llength $words] >= 2} {
                    set target [lindex $words 1]
                    set subcommand [string tolower [lindex $words 2]]
                    if {$subcommand eq "money"} {
                        return [check_others_money $nick $target]
                    } elseif {$subcommand eq "pony" || $subcommand eq "ponies"} {
                        return [check_others_ponies $nick $target]
                    } elseif {$subcommand eq "unicorn" || $subcommand eq "unicorns"} {
                        return [check_others_unicorns $nick $target]
                    }
                }
                # Check for give money
                if {$first_word eq "give" && [llength $words] >= 3} {
                    return [give_money $nick [lindex $words 1] [lindex $words 2]]
                }
                return ""
            }
        }
    }

    # =========================================================================
    # Core Commands
    # =========================================================================

    proc greet {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        return "$nick, this is TIMTOM. How may I serve you?"
    }

    proc sex {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set r [expr {int(rand() * 68) + 1}]
        if {$r == 41} {
            return "ok, $nick, I will have sex with you now."
        } else {
            return "$nick, I cannot perform sex on you at this moment."
        }
    }

    proc horses {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        return "$nick, I like horses too."
    }

    proc jesus {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        return "$nick, Jesus loves you more than $::channel. I'm sorry. $::channel just doesn't compare."
    }

    proc wheel {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set can_spin [get_stat $nick "spin"]
        if {$can_spin != 0} {
            return "I think it would be a good idea if $nick would spin the wheel."
        } else {
            return "$nick, please let someone else spin."
        }
    }

    proc stare {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set stares [list \
            "TIMTOM stares at $nick." \
            "TIMTOM stares deeply into $nick's eyes." \
            "TIMTOM gives $nick an uncomfortable stare." \
            "TIMTOM locks eyes with $nick and doesn't blink." \
            "TIMTOM gazes intensely at $nick." \
        ]
        return [lindex $stares [expr {int(rand() * [llength $stares])}]]
    }

    # =========================================================================
    # Money System
    # =========================================================================

    proc money {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set amount [get_money $nick]
        set formatted [format_money $amount]

        set responses [list \
            "Hey, how are you doing, $nick? It's TIMTOM. You currently have $formatted." \
            "TIMTOM here! Are you having fun yet, $nick? I sure hope you are. You currently have $formatted." \
            "What's the good word, there, $nick? It's TIMTOM. You currently have $formatted." \
            "Howdy Doodie $nick! You currently have $formatted." \
            "TIMTOM here! Responding to the one and only $nick. You currently have $formatted." \
            "IT'S SO NICE TO HEAR FROM YOU, [string toupper $nick]! You want to know about your money, eh? Well, you've got $formatted." \
            "TIMTOM here! Reporting for duty. $nick, you currently have $formatted." \
            "Hello! Hello! You've got $formatted, $nick." \
            "Hey, how are you doing, $nick? It's TIMTOM. You currently have $formatted. Have a good day." \
            "TIMTOM here with your bank statement. You currently have $formatted. Good day, $nick." \
        ]

        set r [expr {int(rand() * [llength $responses])}]
        set response [lindex $responses $r]

        # Add suffix based on amount
        if {$amount == 0} {
            set suffixes [list "Sorry about that." ":(," "I'm so sorry." "That's too bad." "Ah well." "Let's hope you do better." "Uh oh!" "You can do better than that!"]
            append response " [lindex $suffixes [expr {int(rand() * [llength $suffixes])}]]"
        } else {
            set suffixes [list "Good luck!" "" "Use it wisely." "" "" "" "Be good." "Very well then!" "" ""]
            append response " [lindex $suffixes [expr {int(rand() * [llength $suffixes])}]]"
        }

        return $response
    }

    proc check_others_money {nick target} {
        set amount [get_money $target]
        set formatted [format_money $amount]
        if {$amount == 0} {
            return "HELLO $nick! Right now $target doesn't have any money. We're all pulling for $target right now!!"
        } else {
            return "HELLO $nick! Currently $target has $formatted."
        }
    }

    proc give_money {nick target amount} {
        if {![string is double $amount] || $amount <= 0} {
            return "That's not a valid amount, $nick."
        }
        set current [get_money $nick]
        if {$current < $amount} {
            return "Sorry $nick, you don't have enough money to give."
        }
        # Apply 5% fee
        set fee [expr {$amount * 0.05}]
        set received [expr {$amount - $fee}]
        add_money $nick [expr {-$amount}]
        add_money $target $received
        return "$nick gives [format_money $received] to $target (after 5% fee)."
    }

    # =========================================================================
    # Wheel of Fortune (Spin)
    # =========================================================================

    proc spin {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set can_spin [get_stat $nick "spin"]
        if {$can_spin == 0} {
            return "$nick, please let someone else spin."
        }

        set r [expr {int(rand() * 40) + 1}]
        set result ""

        # Wheel outcomes
        switch $r {
            1 - 10 - 20 - 30 - 40 {
                # BANKRUPT
                set_money $nick 0
                set_stat $nick "spin" 0
                set result "$nick, you get a BANKRUPT!!!"
            }
            4 - 9 - 14 - 19 - 24 - 29 - 34 - 39 {
                # LOSE A TURN
                set_stat $nick "spin" 0
                set messages [list \
                    "$nick, you get LOSE A TURN!! Sorry about that." \
                    "$nick, you get LOSE A TURN!! Still better than bankrupt." \
                    "$nick, you get LOSE A TURN!! Whoops, I guess the wheel is rigged." \
                    "$nick, you get LOSE A TURN!! Let your secret crush spin next." \
                    "$nick, you get LOSE A TURN!! I'm NOT sorry about that." \
                ]
                set result [lindex $messages [expr {int(rand() * [llength $messages])}]]
            }
            2 {
                add_money $nick 500
                set_stat $nick "spin" 0
                set result "$nick, you get \$500"
            }
            3 {
                add_money $nick 400
                set_stat $nick "spin" 0
                set result "$nick, you get \$400"
            }
            5 {
                add_money $nick 5000
                set_stat $nick "spin" 0
                set result "$nick, you get \$5000!!! WOW!!!"
            }
            6 {
                add_money $nick 250
                set_stat $nick "spin" 0
                set result "$nick, you get \$250"
            }
            7 {
                add_money $nick 800
                set_stat $nick "spin" 0
                set result "$nick, you get \$800"
            }
            8 {
                add_money $nick 666
                set_stat $nick "spin" 0
                set result "$nick, you get \$666. That's scary business."
            }
            11 {
                add_money $nick 47
                set_stat $nick "spin" 0
                set result "$nick, you get \$47. That's ok, it's better than nothing."
            }
            12 - 32 {
                add_money $nick 900
                set_stat $nick "spin" 0
                set result "$nick, you get \$900"
            }
            13 {
                add_money $nick 1000000
                set_stat $nick "spin" 0
                set_state "ok" 1
                set result "$nick, you get \$1,000,000!!! THAT'S AMAZING!!!"
            }
            15 {
                add_money $nick 251
                set_stat $nick "spin" 0
                set result "$nick, you get \$251"
            }
            16 {
                add_money $nick 300
                set_stat $nick "spin" 0
                set result "$nick, you get \$300"
            }
            17 {
                add_money $nick 450
                set_stat $nick "spin" 0
                set result "$nick, you get \$450"
            }
            18 - 38 {
                add_money $nick 9000
                set_stat $nick "spin" 0
                set result "$nick, you get \$9,000. That's a nice hefty amount."
            }
            21 {
                add_money $nick 5000
                set_stat $nick "spin" 0
                set result "$nick, you win a trip to Detroit, Michigan! Good for you!"
            }
            22 {
                add_money $nick 11000
                set_stat $nick "spin" 0
                set result "$nick, you get \$11,000"
            }
            23 {
                add_money $nick 50
                set_stat $nick "spin" 0
                set result "$nick, you get fifty dollars."
            }
            25 {
                add_money $nick 999.99
                set_stat $nick "spin" 0
                set result "$nick, you get \$999.99!!! WOW!!!"
            }
            26 {
                add_money $nick 5000
                set_stat $nick "spin" 0
                set result "$nick, you win a trip to Kenya, Africa."
            }
            27 {
                add_money $nick 700
                set_stat $nick "spin" 0
                set result "$nick, you get \$700"
            }
            28 {
                add_money $nick 100
                set_stat $nick "spin" 0
                set result "$nick, you get \$100. Maybe you can buy us all tacos later."
            }
            31 {
                add_money $nick 680
                set_stat $nick "spin" 0
                set result "$nick, you get \$680. Do you remember the time you got a million? That was crazy. Not this time though."
            }
            33 {
                add_money $nick 5000
                set_stat $nick "spin" 0
                set result "$nick, you win a trip to Hawaii!!!!"
            }
            35 {
                add_money $nick 255
                set_stat $nick "spin" 0
                set result "$nick, you get \$255"
            }
            36 {
                add_money $nick 390
                set_stat $nick "spin" 0
                set result "$nick, you get \$390"
            }
            37 {
                set_stat $nick "spin" 0
                set result "$nick, you get \$000. LOL."
            }
        }

        # Reset everyone's spin ability
        return $result
    }

    # Enable spinning for a user (called when wheel is mentioned)
    proc enable_spin {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set_stat $nick "spin" 1
    }

    # =========================================================================
    # Food and Drink Commands
    # =========================================================================

    proc serve_all {item} {
        set nicks [names]
        set nick_list [join $nicks " "]
        switch $item {
            "soup" {
                return "TIMTOM brings out the hot soup for $nick_list. Enjoy friends."
            }
            "tea" {
                return "TIMTOM brings out the hot tea for $nick_list. Enjoy friends."
            }
            "coffee" {
                return "TIMTOM brings out the hot coffee for $nick_list. Enjoy friends."
            }
        }
    }

    proc drink {nick args} {
        set drink_type [string tolower [join $args]]
        set drinks [dict create \
            "water" "TIMTOM serves water to $nick. Enjoy!" \
            "juice" "TIMTOM serves orange juice to $nick. Enjoy!" \
            "orange juice" "TIMTOM serves orange juice to $nick. Enjoy!" \
            "lemonade" "TIMTOM serves lemonade to $nick. Enjoy!" \
            "milk" "TIMTOM serves cold milk to $nick. Enjoy!" \
            "soda" "TIMTOM serves soda to $nick. Enjoy!" \
            "beer" "TIMTOM serves a cold beer to $nick. Enjoy!" \
            "wine" "TIMTOM serves fine wine to $nick. Enjoy!" \
            "cocktail" "TIMTOM serves a fancy cocktail to $nick. Enjoy!" \
            "whiskey" "TIMTOM serves whiskey to $nick. Enjoy!" \
            "vodka" "TIMTOM serves vodka to $nick. Enjoy!" \
        ]
        if {[dict exists $drinks $drink_type]} {
            return [dict get $drinks $drink_type]
        }
        return "TIMTOM serves $drink_type to $nick. Enjoy!"
    }

    proc food {nick food_type} {
        set foods [dict create \
            "pizza" "TIMTOM serves hot pizza to $nick. Enjoy!" \
            "crab" "TIMTOM serves delicious crab to $nick. Enjoy!" \
            "nachos" "TIMTOM serves cheesy nachos to $nick. Enjoy!" \
            "lasagna" "TIMTOM serves fresh lasagna to $nick. Enjoy!" \
            "tacos" "TIMTOM serves tasty tacos to $nick. Enjoy!" \
            "burger" "TIMTOM serves a juicy burger to $nick. Enjoy!" \
        ]
        if {[dict exists $foods $food_type]} {
            return [dict get $foods $food_type]
        }
        return "TIMTOM serves $food_type to $nick. Enjoy!"
    }

    proc rings {} {
        set nicks [names]
        set nick_list [join $nicks " "]
        return "TIMTOM brings out the onion rings for $nick_list. Enjoy friends."
    }

    # =========================================================================
    # Interactive Features
    # =========================================================================

    proc bong {nick words} {
        # Check if user is allowed (simplified - everyone allowed for now)
        if {[llength $words] == 1} {
            # Just "bong" - pass to self
            set colors [list "red" "blue" "green" "yellow" "purple" "orange" "pink" "cyan" "magenta" "gold"]
            set color [lindex $colors [expr {int(rand() * [llength $colors])}]]
            return "TIMTOM passes the $color bong to $nick. Enjoy friend."
        } elseif {[string tolower [lindex $words 1]] eq "clean"} {
            return "TIMTOM HERE! That water's looking pretty nasty. Let me change that for you."
        } else {
            # Pass to someone else
            set target [lrange $words 1 end]
            set colors [list "red" "blue" "green" "yellow" "purple" "orange" "pink" "cyan" "magenta" "gold"]
            set color [lindex $colors [expr {int(rand() * [llength $colors])}]]
            return "$nick passes the $color bong to $target."
        }
    }

    proc flip {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set allowed [get_stat $nick "flip_allowed"]
        if {$allowed eq "0"} {
            return "$nick, you got your million. Please let someone else flip now."
        }

        set r [expr {int(rand() * 2)}]
        if {$r == 0} {
            # Heads
            set streak [add_stat $nick "heads" 1]
            set_stat $nick "tails" 0
            if {$streak >= 7} {
                add_money $nick 1000000
                set_stat $nick "heads" 0
                set_stat $nick "flip_allowed" 0
                set_state "ok" 1
                return "$nick flips HEADS.\nWow $nick!! You got 7 heads in a row! Here's \$1,000,000!"
            }
            return "$nick flips HEADS."
        } else {
            # Tails
            set streak [add_stat $nick "tails" 1]
            set_stat $nick "heads" 0
            if {$streak >= 7} {
                add_money $nick 1000000
                set_stat $nick "tails" 0
                set_stat $nick "flip_allowed" 0
                set_state "ok" 1
                return "$nick flips TAILS.\nWow $nick!! You got 7 tails in a row! Here's \$1,000,000!"
            }
            return "$nick flips TAILS."
        }
    }

    proc bonus {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set ok [get_state "ok"]
        if {$ok eq "1"} {
            add_money $nick 5000
            set_state "ok" 0
            return "OK [string toupper $nick], HERE'S \$5000"
        }
        return ""
    }

    proc marry {} {
        set nicks [names]
        if {[llength $nicks] < 2} {
            return "Not enough people in the channel for a marriage!"
        }
        set person1 [lindex $nicks [expr {int(rand() * [llength $nicks])}]]
        set person2 [lindex $nicks [expr {int(rand() * [llength $nicks])}]]
        while {$person2 eq $person1} {
            set person2 [lindex $nicks [expr {int(rand() * [llength $nicks])}]]
        }
        return "I now pronounce $person1 and $person2 married! Congratulations!"
    }

    proc divorce {} {
        set nicks [names]
        if {[llength $nicks] < 2} {
            return "Not enough people in the channel for a divorce!"
        }
        set person1 [lindex $nicks [expr {int(rand() * [llength $nicks])}]]
        set person2 [lindex $nicks [expr {int(rand() * [llength $nicks])}]]
        while {$person2 eq $person1} {
            set person2 [lindex $nicks [expr {int(rand() * [llength $nicks])}]]
        }
        return "$person1 and $person2 are now divorced! Sorry to hear that."
    }

    # =========================================================================
    # Pony and Unicorn System
    # =========================================================================

    proc my_ponies {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set count [get_stat $nick "pony"]
        if {$count == 0} {
            return "Hey $nick! You don't have any ponies yet. Type 'buy pony' to get one for \$1000!"
        } elseif {$count == 1} {
            return "Hey $nick! You have 1 pony. What a cute little pony!"
        } else {
            return "Hey $nick! You have [format_with_commas $count] ponies."
        }
    }

    proc my_unicorns {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set count [get_stat $nick "unicorn"]
        if {$count == 0} {
            return "Hey $nick! You don't have any unicorns yet. Win them in special events!"
        } elseif {$count == 1} {
            return "Hey $nick! You have 1 magical unicorn!"
        } else {
            return "Hey $nick! You have [format_with_commas $count] unicorns."
        }
    }

    proc check_others_ponies {nick target} {
        set count [get_stat $target "pony"]
        if {$count == 0} {
            return "HELLO $nick! Right now $target doesn't have any ponies. We're all pulling for $target right now!!"
        } elseif {$count == 1} {
            return "HELLO $nick! Currently $target has 1 pony. What a cute little pony!"
        } else {
            return "HELLO $nick! Currently $target has [format_with_commas $count] ponies."
        }
    }

    proc check_others_unicorns {nick target} {
        set count [get_stat $target "unicorn"]
        if {$count == 0} {
            return "HELLO $nick! Right now $target doesn't have any unicorns."
        } elseif {$count == 1} {
            return "HELLO $nick! Currently $target has 1 unicorn!"
        } else {
            return "HELLO $nick! Currently $target has [format_with_commas $count] unicorns."
        }
    }

    proc buy_pony {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set cost 1000
        set current [get_money $nick]
        if {$current < $cost} {
            return "Sorry $nick, you need \$1000 to buy a pony. You only have [format_money $current]."
        }
        add_money $nick [expr {-$cost}]
        set count [add_stat $nick "pony" 1]
        return "Congratulations $nick! You bought a pony! You now have $count ponies."
    }

    # =========================================================================
    # Blackjack Game
    # =========================================================================

    proc blackjack_start {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set_stat $nick "blackjack" 2
        return "WELCOME TO BLACKJACK [string toupper $nick]! I'm your dealer TIMTOM. My goal is to give you an enjoyable BLACKJACK experience. Drinks and tacos and everything else are right here - just ask, silly! Now please place your bet and we'll get started. The min bet is \$5000 and the max bet is \$20,000. Please keep the bets in whole dollar amounts. Good luck!"
    }

    proc blackjack_bet {amount {nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set state [get_stat $nick "blackjack"]
        if {$state != 2} {
            return ""
        }

        if {![string is integer $amount]} {
            return "$nick, please only whole dollar bets."
        }

        if {$amount < 5000 || $amount > 20000} {
            return "$nick, the min bet is \$5,000 and the max bet is \$20,000."
        }

        set current [get_money $nick]
        if {$current < $amount} {
            return "Sorry, $nick, but you don't have enough to bet that much. :("
        }

        # Deduct bet
        add_money $nick [expr {-$amount}]
        set_stat $nick "bet" $amount

        # Deal cards
        set dealer1 [expr {int(rand() * 13) + 1}]
        set dealer2 [expr {int(rand() * 13) + 1}]
        set card1 [expr {int(rand() * 13) + 1}]
        set card2 [expr {int(rand() * 13) + 1}]

        # Convert face cards
        lassign [card_value $dealer1] dealer1_val dealer1_name
        lassign [card_value $dealer2] dealer2_val dealer2_name
        lassign [card_value $card1] card1_val card1_name
        lassign [card_value $card2] card2_val card2_name

        set_stat $nick "dealer1" $dealer1_val
        set_stat $nick "dealer2" $dealer2_val
        set_stat $nick "dealer1_name" $dealer1_name
        set_stat $nick "dealer2_name" $dealer2_name
        set_stat $nick "card1" $card1_val
        set_stat $nick "card2" $card2_val
        set_stat $nick "card1_name" $card1_name
        set_stat $nick "card2_name" $card2_name

        set total [expr {$card1_val + $card2_val}]
        set_stat $nick "total" $total

        # Check for blackjack
        if {($card1_val == 11 && $card2_val == 10) || ($card1_val == 10 && $card2_val == 11)} {
            # Player blackjack!
            add_money $nick [expr {$amount * 2}]
            add_stat $nick "unicorn" 35
            set_stat $nick "blackjack" 0
            return "Ok, great, let's get started then, $nick! I'll deal out the cards. Dealer shows $dealer1_name. You've got $card1_name and $card2_name. This gives you 21! YOU GOT BLACKJACK!!!! Congratulations $nick! Dealer pays [format_money $amount] and you also receive 35 bonus UNICORNS!!!!! YAY!!!!"
        }

        set_stat $nick "blackjack" 3

        # Handle soft totals (ace)
        if {$card1_val == 11 && $card2_val == 11} {
            set_stat $nick "total" 12
            set_stat $nick "soft" 2
            return "Ok, great, let's get started then, $nick! I'll deal out the cards. Dealer shows $dealer1_name. You've got $card1_name and $card2_name. This gives you 2 or 12. Do you want to hit or stand?"
        } elseif {$card1_val == 11 || $card2_val == 11} {
            set soft_total [expr {$total - 10}]
            set_stat $nick "soft" $soft_total
            return "Ok, great, let's get started then, $nick! I'll deal out the cards. Dealer shows $dealer1_name. You've got $card1_name and $card2_name. This gives you $soft_total or $total. Do you want to hit or stand?"
        }

        return "Ok, great, let's get started then, $nick! I'll deal out the cards. Dealer shows $dealer1_name. You've got $card1_name and $card2_name. This gives you $total. Do you want to hit or stand?"
    }

    proc card_value {card} {
        switch $card {
            1 { return [list 11 "Ace"] }
            11 { return [list 10 "Jack"] }
            12 { return [list 10 "Queen"] }
            13 { return [list 10 "King"] }
            default { return [list $card $card] }
        }
    }

    proc blackjack_hit {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set state [get_stat $nick "blackjack"]
        if {$state < 3} {
            return ""
        }

        set total [get_stat $nick "total"]
        set bet [get_stat $nick "bet"]

        # Draw new card
        set card [expr {int(rand() * 13) + 1}]
        lassign [card_value $card] card_val card_name

        set new_total [expr {$total + $card_val}]

        # Handle bust
        if {$new_total > 21} {
            # Check for soft total
            if {$card_val == 11 && $new_total <= 31} {
                set new_total [expr {$new_total - 10}]
            } elseif {[get_stat $nick "soft"] ne "" && $new_total <= 31} {
                set new_total [expr {$new_total - 10}]
                set_stat $nick "soft" ""
            }
        }

        if {$new_total > 21} {
            # Bust
            set_stat $nick "blackjack" 0
            add_stat $nick "pot" $bet
            return "Ok, $nick, you got $card_name. This gives you $new_total. Sorry $nick :( :( You busted. Dealer puts [format_money $bet] into the pot. Better luck next game."
        }

        set_stat $nick "total" $new_total
        set_stat $nick "blackjack" [expr {$state + 1}]

        return "Ok, $nick, you got $card_name. This gives you $new_total. Do you wish to hit or stand?"
    }

    proc blackjack_stand {{nick ""}} {
        if {$nick eq ""} { set nick $::nick }
        set state [get_stat $nick "blackjack"]
        if {$state < 3} {
            return ""
        }

        set player_total [get_stat $nick "total"]
        set bet [get_stat $nick "bet"]
        set dealer1 [get_stat $nick "dealer1"]
        set dealer2 [get_stat $nick "dealer2"]
        set dealer1_name [get_stat $nick "dealer1_name"]
        set dealer2_name [get_stat $nick "dealer2_name"]

        set dealer_total [expr {$dealer1 + $dealer2}]
        set result "Alright-o, $nick! Dealer has $dealer1_name and $dealer2_name. "

        # Dealer draws until 17+
        while {$dealer_total < 17} {
            set card [expr {int(rand() * 13) + 1}]
            lassign [card_value $card] card_val card_name
            set dealer_total [expr {$dealer_total + $card_val}]

            # Handle aces
            if {$dealer_total > 21 && $card_val == 11} {
                set dealer_total [expr {$dealer_total - 10}]
            }

            append result "Dealer gets $card_name. "
        }

        append result "So that's $dealer_total. "

        # Determine winner
        if {$dealer_total > 21} {
            # Dealer bust
            add_money $nick [expr {$bet * 2}]
            set_stat $nick "blackjack" 0
            append result "WHOOPS!! I busted! LOL :D :D Dealer pays [format_money $bet]. Congratulations $nick!"
        } elseif {$dealer_total > $player_total} {
            # Dealer wins
            add_stat $nick "pot" $bet
            set_stat $nick "blackjack" 0
            append result "I win! Dealer puts [format_money $bet] into the pot. Better luck next time, $nick."
        } elseif {$dealer_total < $player_total} {
            # Player wins
            add_money $nick [expr {$bet * 2}]
            set_stat $nick "blackjack" 0
            append result "Look's like you beat me, $nick! Awesome game! Dealer pays [format_money $bet]. Congratulations!"
        } else {
            # Push
            add_money $nick $bet
            set_stat $nick "blackjack" 0
            append result "It's a push! Your bet of [format_money $bet] is returned."
        }

        return $result
    }

    # =========================================================================
    # State/Geography Trivia
    # =========================================================================

    proc check_states {text nick resultVar} {
        upvar $resultVar result

        # US States
        set states [dict create \
            "alabama" "Alabama eats my children." \
            "alaska" "Alaska is a cotton gin." \
            "arizona" "Arizona is the land of the forsaken bee hives." \
            "arkansas" "Arkansas is a potato rally." \
            "california" "The capital of California is Los Angeles." \
            "colorado" "Colorado was the missing egg in the blue carton." \
            "connecticut" "Connecticut is a wild stallion." \
            "delaware" "Delaware is a label-making compartment of beauty." \
            "florida" "The capital of Florida is Disney World." \
            "georgia" "Georgia plates early, makes space for Willy." \
            "hawaii" "The capital of Hawaii is dog." \
            "idaho" "Idaho is a flowing mountain." \
            "illinois" "The capital of Illinois is Deal Or No Deal." \
            "indiana" "Indiana rests softly in my left breast pocket sandwich player machine box heavy." \
            "iowa" "Friends make pottery in Iowa." \
            "kansas" "Kansas is a candy cane land in $::channel." \
            "kentucky" "The capital of Kentucky is horse." \
            "louisiana" "Louisiana is a bubble paper pepper boy." \
            "maine" "Maine is the capital of France." \
            "maryland" "The capital of Maryland is inside the fried pickled answering machine tape." \
            "massachusetts" "Massachusettes is the capital of Happy time." \
            "michigan" "$nick I love you more than the sun and the sky. I want you to be my forever." \
            "minnesota" "Minnesota, will you be my road puppy?" \
            "mississippi" "Glad tidings to you, $nick, wherever you are." \
            "missouri" "The fly sunk to the bottom of the jar of oil. The fly's name was Montel." \
            "montana" "You didn't press enter hard enough." \
            "nebraska" "Spinning sunflower wreath, you come in the morning and leave by nightfall." \
            "nevada" "Nevada is first in my peeegy back machine-eeeeeeeeeeee-ooooooooooo." \
            "new hampshire" "I hear shovels. Lock the doors. NOW NOOOOWWWW NOOOOOOOOOWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW!!!!!!!!" \
            "new jersey" "We all eat pots and pans." \
            "new mexico" "Thunder, ice, and twins joined at the hip, make my day a solid whip? Whippie!" \
            "new york" "The capital of New York is New York City." \
            "north carolina" "North Carolina makes $::channel a happy land for you." \
            "north dakota" "I am the willing partner in your N. Dakota movement." \
            "ohio" "Ohio is diabetes." \
            "oklahoma" "Oklahoma is my tea set." \
            "oregon" "There's plenty of lightbulbs in the furnace." \
            "pennsylvania" "The capital of Pennsylvania is cheddar." \
            "rhode island" "How could I forget you, Rhode Island? You are a gentle beauty." \
            "south carolina" "South Carolina is poppy." \
            "south dakota" "Do you really think you own me, $nick?" \
            "tennessee" "Tennessee is a puppy cage." \
            "texas" "Feast on these berries. They were created through honor, diligence, and musk." \
            "utah" "The little pieces of paper need to be evaluated." \
            "vermont" "Vermont is a picnic tree." \
            "virginia" "Virginia is a glue cow." \
            "washington" "Let's roll up another traffic ordinance and place it beneath the Bubber Tree." \
            "west virginia" "Them tree trunks look like legs." \
            "wisconsin" "If we connect the brown pipe to the gray pipe we make famous grandwich butter spread." \
            "wyoming" "Claw me to death with pear skins." \
        ]

        # Countries
        set countries [dict create \
            "africa" "Africa is a lollipop for you." \
            "canada" "Canada is made of copper and sand." \
            "china" "Thank you for relaxing in $::channel. China." \
            "france" "France is a boat." \
            "sweden" "The capital of Sweden is pah-pah." \
        ]

        # Check states
        if {[dict exists $states $text]} {
            set result [dict get $states $text]
            return 1
        }

        # Check countries
        if {[dict exists $countries $text]} {
            set result [dict get $countries $text]
            return 1
        }

        return 0
    }

    # =========================================================================
    # Help Command
    # =========================================================================

    proc help {} {
        return "TIMTOM Commands: timtom, money, spin, wheel, flip, blackjack, buy pony, ponies, unicorns, soup, tea, coffee, rings, bong, marry, divorce, give <nick> <amount>, drink <type>, food <type>. State names trigger trivia responses."
    }

    # Export procs and create ensemble
    namespace export handle help greet money spin wheel flip blackjack_start \
        blackjack_bet blackjack_hit blackjack_stand serve_all drink food \
        bong marry divorce my_ponies my_unicorns buy_pony bonus enable_spin \
        format_money get_money set_money add_money get_stat set_stat add_stat \
        stare sex horses jesus
    namespace ensemble create
}

# Initialize the timtom bucket if it doesn't exist
if {![catch {cache keys timtom}]} {
    # Bucket exists
} else {
    # Initialize with empty values
}
