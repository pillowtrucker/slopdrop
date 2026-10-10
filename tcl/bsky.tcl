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

    # quote uri — quote a post (an at-uri or a bsky.app link). One quote or
    # card per reply.
    proc quote {uri} {
        if {!$::bsky::active} { return $uri }
        _record kind quote uri $uri
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
        reset spec profile getpost getthread search feed followers
}
