# Bluesky (atproto) helpers — for code evaluated from a Bluesky post.
#
# When veles evaluates a /tcl line that came from a Bluesky post, slopdrop
# sets, before the code runs:
#
#   $::bsky::active   1 (0 everywhere else: IRC, the web page, timers)
#   $::bsky::post     the post that asked, a dict:
#                       uri cid text did handle created langs facets embed reply
#                     facets   a list of dicts {start end type value}: UTF-8
#                              BYTE offsets into text, type mention|link|tag,
#                              value the DID, URL or tag
#                     embed    a dict {type images {{url alt} …} uri title
#                              description record} (absent keys are absent)
#                     reply    a dict {root parent} of at-uris, {} for a top post
#   $::bsky::thread   the posts above it, oldest first, each the same dict
#   $::bsky::event    for a trigger handler only (bind LIKE …): a dict
#                       kind did handle uri cid subject text
#
# and it empties $::bsky::out, which the procs below fill. After the code
# runs, slopdrop hands the out list back to veles, which turns it into the
# reply's rich text (facets) and embeds. veles is the authority on what a
# reply may carry: a mention of someone outside the thread posts as plain
# text, and an image from a public /tcl is refused.
#
# Pictures (PLAN-slopdrop-art.md): `bsky::art` attaches a drawing as an
# image, and a reply whose whole output is a drawing becomes one without
# it. `bsky_quote [bsky_get_parent] {{t} fedoragoonsay $t}` quotes the
# post above and answers with the fedora goon saying its words.
# `bsky_caption [bsky_get_parent] TOP BOTTOM` writes over the parent's
# picture (`bsky::magick` has the other ops; the owner's only, for now).
#
# Every output proc RETURNS the visible text and RECORDS what it means, so
# they compose like any string:
#
#   /tcl return "see [bsky::link docs https://example.com] [bsky::tag tcl]"
#
# Off Bluesky ($::bsky::active 0) the same procs return text that reads
# right on IRC (`docs <https://example.com>`, `#tcl`), so a stored proc can
# use them on both.
#
# These live in ::bsky, a namespace, so slopdrop's state tracking (global
# procs and vars only) never commits them: they are stock, reloaded from
# this file, and an eval cannot break them for the next one.

namespace eval ::bsky {
    variable active 0
    variable post {}
    variable thread {}
    variable event {}
    variable out {}

    # ── reading a post ───────────────────────────────────────────────

    # The post a reader proc reads: the one given, or the one that asked.
    proc _post {p} {
        if {$p eq ""} { return $::bsky::post }
        return $p
    }

    # A dict field, or the default when the key is absent.
    proc _get {d key {default ""}} {
        if {[dict exists $d $key]} { return [dict get $d $key] }
        return $default
    }

    # text ?post? — the post's text.
    proc text {{p ""}} {
        return [_get [_post $p] text]
    }

    # author ?post? — the author's handle, or the DID when no handle is known.
    proc author {{p ""}} {
        set p [_post $p]
        set h [_get $p handle]
        if {$h ne ""} { return $h }
        return [_get $p did]
    }

    # byteslice text start end — the characters between two UTF-8 BYTE
    # offsets (end exclusive), which is how a facet indexes its text.
    proc byteslice {text start end} {
        set bytes [encoding convertto utf-8 $text]
        return [encoding convertfrom utf-8 [string range $bytes $start [expr {$end - 1}]]]
    }

    # facet_text facet ?post? — the visible text one facet covers.
    proc facet_text {facet {p ""}} {
        set p [_post $p]
        return [byteslice [_get $p text] [dict get $facet start] [dict get $facet end]]
    }

    # facets ?type? ?post? — the post's facets, optionally only one type
    # (mention, link or tag).
    proc facets {{type ""} {p ""}} {
        set out {}
        foreach f [_get [_post $p] facets] {
            if {$type eq "" || [dict get $f type] eq $type} { lappend out $f }
        }
        return $out
    }

    # links ?post? — every URL the post links.
    proc links {{p ""}} {
        set out {}
        foreach f [facets link $p] { lappend out [dict get $f value] }
        return $out
    }

    # tags ?post? — every hashtag, without the #.
    proc tags {{p ""}} {
        set out {}
        foreach f [facets tag $p] { lappend out [dict get $f value] }
        return $out
    }

    # mentions ?post? — every mention as a pair {text did}: the @handle as
    # written, and the DID it points at.
    proc mentions {{p ""}} {
        set p [_post $p]
        set out {}
        foreach f [facets mention $p] {
            lappend out [list [facet_text $f $p] [dict get $f value]]
        }
        return $out
    }

    # strip_facets ?types? ?post? — the text with the spans of the given
    # facet types cut out (default: every facet), whitespace tidied. Cuts
    # run from the end so earlier offsets stay true.
    proc strip_facets {{types {mention link tag}} {p ""}} {
        set p [_post $p]
        set bytes [encoding convertto utf-8 [_get $p text]]
        set spans {}
        foreach f [_get $p facets] {
            if {[dict get $f type] in $types} {
                lappend spans [list [dict get $f start] [dict get $f end]]
            }
        }
        foreach s [lsort -integer -index 0 -decreasing $spans] {
            lassign $s a b
            set bytes [string replace $bytes $a [expr {$b - 1}]]
        }
        set text [encoding convertfrom utf-8 $bytes]
        return [string trim [regsub -all {[ \t]{2,}} $text " "]]
    }

    # images ?post? — the post's images as pairs {url alt}.
    proc images {{p ""}} {
        set e [_get [_post $p] embed]
        return [_get $e images]
    }

    # is_reply ?post? — 1 when the post answers another one.
    proc is_reply {{p ""}} {
        return [expr {[_get [_post $p] reply] ne ""}]
    }

    # ── writing the reply ────────────────────────────────────────────

    proc _record {args} {
        lappend ::bsky::out [dict create {*}$args]
    }

    # link text ?url? — a link with its own text; with one argument the
    # text IS the url.
    proc link {text {url ""}} {
        if {$url eq ""} { set url $text }
        if {!$::bsky::active} {
            if {$url eq $text} { return $url }
            return "$text <$url>"
        }
        _record kind link text $text uri $url
        return $text
    }

    # tag name — a hashtag (# optional), shown as #name.
    proc tag {name} {
        set name [string trimleft $name #]
        set shown "#$name"
        if {$::bsky::active} { _record kind tag text $shown tag $name }
        return $shown
    }

    # mention who — a mention by handle or DID (@ optional), shown as @who.
    # veles links it only for the caller and the thread's participants;
    # anyone else stays plain text, so a proc cannot notify strangers.
    proc mention {who} {
        set who [string trimleft $who @]
        set shown "@$who"
        if {$::bsky::active} { _record kind mention text $shown handle $who }
        return $shown
    }

    # Is this a post dict (has a uri), rather than a uri string?
    proc _is_post {v} {
        expr {![catch {dict size $v}] && [dict exists $v uri]}
    }

    # quote post — quote a post: a post dict (`bsky::parent`,
    # `$::bsky::thread`), an at-uri or a bsky.app link. One quote or card
    # per reply. veles lets anyone but the owner quote only a post of the
    # thread or their own. Off Bluesky: the uri, or `<author> text` for a
    # post dict.
    proc quote {what} {
        if {[_is_post $what]} {
            if {!$::bsky::active} { return "<[author $what]> [text $what]" }
            _record kind quote uri [dict get $what uri]
            return ""
        }
        if {!$::bsky::active} { return $what }
        _record kind quote uri $what
        return ""
    }

    # card url ?title? ?description? — a link card. veles fills a missing
    # title from the page.
    proc card {url {title ""} {description ""}} {
        if {!$::bsky::active} { return $url }
        _record kind card uri $url title $title description $description
        return ""
    }

    # image url ?alt? — attach an image. Owner only (tcladmin): a public
    # reply cannot carry pictures from arbitrary URLs. An empty alt is
    # written by veles' vision model.
    proc image {url {alt ""}} {
        if {!$::bsky::active} { return $url }
        _record kind image uri $url alt $alt
        return ""
    }

    # art text ?-alt text? ?-theme dark|light? — attach TEXT drawn as a
    # picture (PLAN-slopdrop-art.md). IRC art — mIRC colour, ANSI, block,
    # box and braille drawings — does not survive a Bluesky post, so veles
    # draws it in the IRC font and attaches the image. Returns "" on
    # Bluesky; off it, the text itself, so a stored proc draws on IRC as it
    # always did. Up to 4 pictures per reply. A reply whose whole output is
    # a drawing becomes a picture without this; `art` is for a drawing
    # beside text, or one longer than the output's line cap.
    proc art {text args} {
        set alt ""
        set theme ""
        foreach {k v} $args {
            switch -- $k {
                -alt { set alt $v }
                -theme { set theme $v }
                default { error "bsky::art: unknown option \"$k\" (-alt, -theme)" }
            }
        }
        if {!$::bsky::active} { return $text }
        _record kind art text $text alt $alt theme $theme
        return ""
    }

    # magick source ?-alt text? op ?op …? — an image macro
    # (PLAN-slopdrop-art.md P6): a picture from the thread, changed by a
    # chain of ops, attached to the reply. SOURCE is a post dict (its
    # first picture; `bsky_get_parent` gives one), the at-uri of a post in
    # the thread, or the URL of a picture the thread shows. Each OP is a
    # list:
    #   {resize W H}    0 for one side keeps the shape
    #   {scale PCT}     {rotate 90|180|270}   {flip}  {flop}
    #   {crop X Y W H}  {border PX ?COLOUR?}  {blur SIGMA}  {sharpen}
    #   {grayscale}     {invert}              {caption TOP ?BOTTOM?}
    # At most 10 ops. veles fetches only pictures the thread shows, and
    # for now only the owner's macros: anyone else's is dropped with a
    # note. Returns ""; off Bluesky, the source picture's URL.
    proc magick {source args} {
        set alt ""
        if {[lindex $args 0] eq "-alt"} {
            set alt [lindex $args 1]
            set args [lrange $args 2 end]
        }
        if {!$::bsky::active} {
            if {[catch {_image_of $source} url]} { return "" }
            return $url
        }
        set url [_image_of $source]
        if {[llength $args] == 0} { error "bsky::magick: no ops (resize, caption, …)" }
        if {[llength $args] > 10} { error "bsky::magick: at most 10 ops" }
        set enc {}
        foreach op $args {
            set name [lindex $op 0]
            set n [expr {[llength $op] - 1}]
            switch -- $name {
                resize  { set ok [expr {$n == 2}] }
                crop    { set ok [expr {$n == 4}] }
                scale - rotate - blur { set ok [expr {$n == 1}] }
                border - caption { set ok [expr {$n == 1 || $n == 2}] }
                flip - flop - sharpen - grayscale - invert { set ok [expr {$n == 0}] }
                default {
                    error "bsky::magick: unknown op \"$name\" (resize scale rotate flip flop crop border blur sharpen grayscale invert caption)"
                }
            }
            if {!$ok} { error "bsky::magick: wrong number of arguments for $name: \"$op\"" }
            lappend enc [join $op \x1f]
        }
        _record kind magick src $url ops [join $enc \x1e] alt $alt
        return ""
    }

    # caption source top ?bottom? — the classic image macro: a picture of
    # the thread with TOP and BOTTOM written over it (bsky::magick).
    proc caption {source top {bottom ""}} {
        if {$bottom eq ""} { return [magick $source [list caption $top]] }
        return [magick $source [list caption $top $bottom]]
    }

    # The URL of a picture: a post dict's first one, a thread post's by its
    # at-uri, or a URL as given.
    proc _image_of {source} {
        if {[_is_post $source]} {
            set p $source
        } elseif {[string match at://* $source]} {
            set p [_find $source]
            if {$p eq ""} { error "bsky::magick: no such post: $source" }
        } else {
            return $source
        }
        set imgs [images $p]
        if {[llength $imgs] == 0} { error "bsky::magick: that post has no picture" }
        return [lindex $imgs 0 0]
    }

    # label val ?val …? — self-label the reply: nudity, sexual, porn or
    # graphic-media, a content warning readers opt into. veles keeps only
    # those values, and adds its own when its image check sees what one
    # names.
    proc label {args} {
        if {$::bsky::active} {
            foreach v $args { _record kind label val $v }
        }
        return ""
    }

    # parent ?post? — the post the given one (default: the one that
    # asked) answers, as a post dict; {} for a top-level post. From the
    # thread when it is there, else one AppView read. Off Bluesky: the
    # channel's previous line as a post dict (text, handle = the nick).
    proc parent {{p ""}} {
        if {!$::bsky::active && $p eq ""} { return [_previous_line] }
        set r [_get [_post $p] reply]
        if {$r eq ""} { return {} }
        return [_find [_get $r parent]]
    }

    # root ?post? — the thread's first post, as a post dict (a top-level
    # post is its own root).
    proc root {{p ""}} {
        if {!$::bsky::active && $p eq ""} { return [_previous_line] }
        set p [_post $p]
        set r [_get $p reply]
        if {$r eq ""} { return $p }
        return [_find [_get $r root]]
    }

    # The post with this at-uri: from the thread, else an AppView read.
    proc _find {uri} {
        if {$uri eq ""} { return {} }
        set bare [lindex [split $uri #] 0]
        foreach t $::bsky::thread {
            if {[lindex [split [_get $t uri] #] 0] eq $bare} { return $t }
        }
        if {[catch {getpost $bare} view] || $view eq ""} { return {} }
        return [_from_view $view]
    }

    # An AppView postView in the post-dict shape (no facets: the
    # AppView's shape for them differs, and a reply reads the text).
    proc _from_view {v} {
        set rec [_get $v record]
        set a [_get $v author]
        set reply {}
        set rr [_get $rec reply]
        if {$rr ne ""} {
            set reply [dict create root [_get [_get $rr root] uri] \
                parent [_get [_get $rr parent] uri]]
        }
        return [dict create uri [_get $v uri] cid [_get $v cid] \
            text [_get $rec text] did [_get $a did] handle [_get $a handle] \
            created [_get $rec createdAt] langs [_get $rec langs] \
            facets {} embed {} reply $reply]
    }

    # Off Bluesky: the channel's last line by someone other than the
    # caller (the caller's own line may be the command itself), as a post
    # dict.
    proc _previous_line {} {
        set chan [expr {[info exists ::channel] ? $::channel : ""}]
        set me [expr {[info exists ::nick] ? $::nick : ""}]
        if {![info exists ::slopdrop_log_lines($chan)]} { return {} }
        foreach e [lreverse $::slopdrop_log_lines($chan)] {
            if {![string equal -nocase [lindex $e 1] $me]} {
                return [dict create uri "" text [lindex $e 3] \
                    handle [lindex $e 1] did "" reply {}]
            }
        }
        return {}
    }

    # quotewith post ?lambda? ?arg …? — quote a post and answer it in one
    # line. The lambda (an `apply` lambda: {params body}) gets the post's
    # text, then any further args; its result is the reply. A result of
    # more than one line, or with IRC colour, is a drawing and goes out as
    # a picture (`bsky::art`); one plain line is the post's text. Off
    # Bluesky: the lambda's result, as IRC shows it.
    #
    #   /tcl bsky_quote [bsky_get_parent] {{t} fedoragoonsay $t}
    #   /tcl bsky_quote [bsky_get_parent] {{t} lmiot $t}
    proc quotewith {p {lambda ""} args} {
        if {$p eq ""} {
            error "bsky_quote: no post to quote (a top-level post has no parent)"
        }
        if {$::bsky::active} { quote $p }
        if {$lambda eq ""} { return "" }
        set out [apply $lambda [text $p] {*}$args]
        if {$::bsky::active && [_drawing $out]} { return [art $out] }
        return $out
    }

    # More than one line, or mIRC/ANSI formatting: a drawing.
    proc _drawing {s} {
        expr {[string first "\n" [string trim $s "\n"]] >= 0
              || [regexp {[\x02\x03\x04\x0f\x16\x1b\x1d\x1e\x1f]} $s]}
    }

    # lang code ?code …? — the reply's languages (BCP-47: en, de, pt-BR).
    proc lang {args} {
        if {$::bsky::active} {
            foreach c $args { _record kind lang code $c }
        }
        return ""
    }

    # newpost — end this post of the reply here; the rest continues in the
    # next post of the thread. A line break off Bluesky. (Not `break`: a
    # proc of that name would capture the loop keyword inside ::bsky.)
    proc newpost {} {
        if {$::bsky::active} { return "\f" }
        return "\n"
    }

    # reset — forget everything recorded so far in this evaluation.
    proc reset {} {
        set ::bsky::out {}
        return ""
    }

    # spec — what has been recorded so far (for debugging a proc).
    proc spec {} {
        return $::bsky::out
    }

    # ── reading Bluesky (the bot's AppView session) ──────────────────
    #
    # bsky::query is native: veles hands each Bluesky evaluation a short-
    # lived capability that Tcl cannot read, good for a few reads. Off
    # Bluesky, or past the per-evaluation limit, it fails with the reason.
    # Each answer is the AppView's JSON as nested dicts and lists.

    # profile who — app.bsky.actor.getProfile.
    proc profile {who} {
        return [query app.bsky.actor.getProfile actor [string trimleft $who @]]
    }

    # getpost uri — one post (app.bsky.feed.getPosts), {} when it is gone.
    proc getpost {uri} {
        set r [query app.bsky.feed.getPosts uris $uri]
        return [lindex [_get $r posts] 0]
    }

    # getthread uri ?depth? — app.bsky.feed.getPostThread.
    proc getthread {uri {depth 6}} {
        return [query app.bsky.feed.getPostThread uri $uri depth $depth]
    }

    # search q ?limit? — app.bsky.feed.searchPosts, the posts list.
    proc search {q {limit 10}} {
        return [_get [query app.bsky.feed.searchPosts q $q limit $limit] posts]
    }

    # feed who ?limit? — an author's recent posts (app.bsky.feed.getAuthorFeed).
    proc feed {who {limit 10}} {
        set r [query app.bsky.feed.getAuthorFeed actor [string trimleft $who @] limit $limit]
        return [_get $r feed]
    }

    # followers who ?limit? — app.bsky.graph.getFollowers, the followers list.
    proc followers {who {limit 50}} {
        set r [query app.bsky.graph.getFollowers actor [string trimleft $who @] limit $limit]
        return [_get $r followers]
    }

    namespace export text author byteslice facet_text facets links tags mentions \
        strip_facets images is_reply link tag mention quote card image lang newpost \
        reset spec profile getpost getthread search feed followers \
        art label parent root quotewith magick caption
}

# The USER's spellings (2026-10-12). Global procs made at load with the
# unwrapped `proc`, so state tracking never commits them (it records what
# an evaluation defines); a tracked proc of the same name still overrides
# one, as it would any stock proc.
#
#   bsky_get_parent ?post?            the post this one answers
#   bsky_get_root ?post?              the thread's first post
#   bsky_quote post ?lambda? ?arg …?  quote it and answer (bsky::quotewith)
#   bsky_art text ?-alt a? ?-theme t? attach a drawing as a picture
#   bsky_magick source op ?op …?      a picture of the thread, changed
#   bsky_caption source top ?bottom?  … with words written over it
::slopdrop::_original_proc ::bsky_get_parent {{p ""}} { tailcall ::bsky::parent $p }
::slopdrop::_original_proc ::bsky_get_root {{p ""}} { tailcall ::bsky::root $p }
::slopdrop::_original_proc ::bsky_quote {p args} { tailcall ::bsky::quotewith $p {*}$args }
::slopdrop::_original_proc ::bsky_art {text args} { tailcall ::bsky::art $text {*}$args }
::slopdrop::_original_proc ::bsky_magick {source args} { tailcall ::bsky::magick $source {*}$args }
::slopdrop::_original_proc ::bsky_caption {source args} { tailcall ::bsky::caption $source {*}$args }
