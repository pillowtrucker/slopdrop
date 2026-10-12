# Image procs (PLAN-slopdrop-art.md in veles-agent, the wrap-up round).
#
# The old bot sent these to magick.buttes.org, which is gone. On Bluesky
# veles now draws them: `magick_scheme` records its program
# (`bsky::scheme`) and veles runs it over the thread's pictures
# (veles-slopdrop's scheme.rs), so the base library's image procs
# (mirror_left, image-say, magick-concat, macify2, …) draw again. Each
# call returns a handle, `magick:N`, that a later program names
# (`(image magick:N)`), and veles draws only the programs no other one
# names. Only pictures the thread shows can be used: `bsky_image` gives
# the URL of the post above's (`mirror_left [bsky_image]`).
#
# The base library (stolen-treasure.tcl) loads after this file and
# rebinds these names to the dead service, so slopdrop calls
# `::magick_dsl::revive` once the state is loaded (tcl_wrapper.rs), and
# this file calls it when it is reloaded. A name still bound to a dead
# version, or to an older copy of these, gets the one here; a tracked
# proc of the same name (somebody's own) is left alone.

namespace eval ::magick_dsl {
    # name params body: what `revive` binds. Each body carries the mark
    # `magick_dsl` so a reload knows its own.
    variable procs {
        magick_scheme args {
            # magick_dsl: a program in the base library's image language
            tailcall ::bsky::scheme [join $args]
        }
        magick_id {url} {
            # magick_dsl: a picture is named by its URL or handle as is
            return $url
        }
        magick {cmd source args} {
            # magick_dsl: the old bot's `magick CMD SOURCE ARG …`
            tailcall ::magick_dsl::call $cmd $source {*}$args
        }
        scheme_escape {str} {
            # magick_dsl
            string map {\" \\\" \\ \\\\} $str
        }
        scheme_image_or_sexpr {sexpr} {
            # magick_dsl
            if {[regexp {^( |\n|\r)*\(} $sexpr]} { return $sexpr }
            return "(image \"[scheme_escape $sexpr]\")"
        }
    }

    # Is this proc body one to replace: the dead service's, the old
    # placeholder's, or an older copy of ours?
    proc _replaceable {body} {
        foreach mark {magick_dsl buttes.org magick_uncached {not configured}} {
            if {[string first $mark $body] >= 0} { return 1 }
        }
        return 0
    }

    proc revive {} {
        variable procs
        foreach {name params body} $procs {
            if {[llength [info procs ::$name]] && ![_replaceable [info body ::$name]]} {
                continue
            }
            ::slopdrop::_original_proc ::$name $params $body
        }
    }

    proc _num {cmd v} {
        if {![string is double -strict $v]} {
            error "magick $cmd: \"$v\" is not a number"
        }
        return $v
    }

    # The old bot's `magick CMD SOURCE ARG …` as a program.
    proc call {cmd source args} {
        set img [scheme_image_or_sexpr $source]
        switch -- $cmd {
            noop - flip - flop - grayscale - invert - sharpen {
                if {[llength $args]} { error "magick $cmd: takes no arguments" }
                set prog "($cmd $img)"
            }
            resize - resize_to_fit - crop - rotate - scale - blur {
                set nums [lmap a $args { _num $cmd $a }]
                set prog "($cmd $img [join $nums])"
            }
            border {
                lassign $args px colour
                set prog "(border $img [_num border $px]"
                if {$colour ne ""} { append prog " \"[scheme_escape $colour]\"" }
                append prog ")"
            }
            composite {
                if {[llength $args] != 3} { error "magick composite: SOURCE TOP X Y" }
                lassign $args top x y
                set prog "(composite $img [scheme_image_or_sexpr $top] [_num composite $x] [_num composite $y])"
            }
            annotate {
                if {[llength $args] < 5} { error "magick annotate: SOURCE SIZE X Y COLOUR TEXT" }
                lassign $args size x y colour
                set text [join [lrange $args 4 end]]
                set prog "(annotate $img [_num annotate $size] [_num annotate $x] [_num annotate $y] \"[scheme_escape $colour]\" \"[scheme_escape $text]\")"
            }
            default {
                error "magick $cmd: not drawn here (resize resize_to_fit crop rotate scale blur border flip flop grayscale invert sharpen composite annotate noop)"
            }
        }
        tailcall magick_scheme $prog
    }
}

::magick_dsl::revive

# Common composite operations - all need external setup
proc magick-scale-composite-bottom-left {url1 url2 {scale 0.5}} {
    error "magick-scale-composite-bottom-left: ImageMagick integration not configured."
}

proc magick-scale-composite-top-left {url1 url2 {scale 0.5}} {
    error "magick-scale-composite-top-left: ImageMagick integration not configured."
}

proc magick-scale-composite-bottom-right {url1 url2 {scale 0.5}} {
    error "magick-scale-composite-bottom-right: ImageMagick integration not configured."
}

proc magick-scale-composite-top-right {url1 url2 {scale 0.5}} {
    error "magick-scale-composite-top-right: ImageMagick integration not configured."
}

# Helper function used by magick procs
proc scale-and-composite-gen {url1 url2 scale pos1 pos2} {
    error "scale-and-composite-gen: ImageMagick integration not configured."
}

# Glasses meme overlay
proc magick_glasses {url} {
    error "magick_glasses: ImageMagick integration not configured."
}

# Rotation
proc magick_rotate {url degrees} {
    error "magick_rotate: ImageMagick integration not configured."
}
