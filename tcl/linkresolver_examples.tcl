# Example Custom Link Resolvers
# These demonstrate how to create application-specific resolvers

namespace eval ::linkresolver {

    # YouTube Video Resolver
    # Extracts video title and metadata from YouTube links
    proc youtube_resolver {url nick channel} {
        variable max_title_length

        # Check cache first
        set cached [get_cached $url]
        if {$cached ne ""} {
            return $cached
        }

        # Extract video ID from various YouTube URL formats
        set video_id ""

        # youtu.be format: https://youtu.be/VIDEO_ID
        if {[regexp {youtu\.be/([a-zA-Z0-9_-]+)} $url -> vid]} {
            set video_id $vid
        }

        # youtube.com format: https://www.youtube.com/watch?v=VIDEO_ID
        if {[regexp {[?&]v=([a-zA-Z0-9_-]+)} $url -> vid]} {
            set video_id $vid
        }

        if {$video_id eq ""} {
            return ""
        }

        # Fetch the page
        if {[catch {http get $url} content]} {
            return ""
        }

        # Extract title
        set title ""
        if {[regexp -nocase {<title>([^<]+)</title>} $content -> raw_title]} {
            # YouTube titles often end with " - YouTube"
            set title [regsub { - YouTube$} $raw_title ""]
            set title [decode_html_entities $title]
            set title [string trim $title]
        }

        # Try to extract duration and view count from page
        set duration ""
        set views ""

        # Look for duration in meta tags or JSON
        if {[regexp {"lengthSeconds":"(\d+)"} $content -> seconds]} {
            set mins [expr {$seconds / 60}]
            set secs [expr {$seconds % 60}]
            set duration [format "%d:%02d" $mins $secs]
        }

        # Look for view count
        if {[regexp {"viewCount":"(\d+)"} $content -> view_count]} {
            set views [format_number $view_count]
        }

        # Build response
        if {$title ne ""} {
            set result "▶ YouTube: $title"
            if {$duration ne ""} {
                append result " \[$duration\]"
            }
            if {$views ne ""} {
                append result " ($views views)"
            }

            # Truncate if too long
            if {[string length $result] > $max_title_length} {
                set result "[string range $result 0 [expr {$max_title_length - 4}]]..."
            }

            set_cached $url $result
            return $result
        }

        return ""
    }

    # Bluesky Post Resolver
    # Resolves Bluesky posts to show author and content
    proc bluesky_resolver {url nick channel} {
        variable max_title_length

        # Check cache first
        set cached [get_cached $url]
        if {$cached ne ""} {
            return $cached
        }

        # Fetch the page
        if {[catch {http get $url} content]} {
            return ""
        }

        # Extract author and post content from meta tags
        set author ""
        set post_text ""

        # Try to get author from meta tags
        if {[regexp -nocase {<meta property="og:title" content="([^"]+)"} $content -> meta_title]} {
            set author [decode_html_entities $meta_title]
        }

        # Try to get post content from meta description
        if {[regexp -nocase {<meta property="og:description" content="([^"]+)"} $content -> meta_desc]} {
            set post_text [decode_html_entities $meta_desc]
        }

        # Alternative: extract from JSON-LD or page structure
        if {$post_text eq "" && [regexp {"text":"([^"]+)"} $content -> text]} {
            set post_text [decode_html_entities $text]
        }

        # Build response
        if {$author ne "" && $post_text ne ""} {
            # Clean up author (often includes "on Bluesky")
            set author [regsub { on Bluesky.*$} $author ""]

            set result "🦋 Bluesky - $author: $post_text"

            # Truncate if too long
            if {[string length $result] > $max_title_length} {
                set result "[string range $result 0 [expr {$max_title_length - 4}]]..."
            }

            set_cached $url $result
            return $result
        }

        return ""
    }

    # Twitter/X Resolver
    # Resolves tweets to show author and content
    proc twitter_resolver {url nick channel} {
        variable max_title_length

        # Check cache first
        set cached [get_cached $url]
        if {$cached ne ""} {
            return $cached
        }

        # Fetch the page (note: Twitter may block bot requests)
        if {[catch {http get $url} content]} {
            return ""
        }

        # Extract from meta tags
        set author ""
        set tweet_text ""

        if {[regexp -nocase {<meta property="og:title" content="([^"]+)"} $content -> meta_title]} {
            set author [decode_html_entities $meta_title]
        }

        if {[regexp -nocase {<meta property="og:description" content="([^"]+)"} $content -> meta_desc]} {
            set tweet_text [decode_html_entities $meta_desc]
        }

        if {$author ne "" && $tweet_text ne ""} {
            set result "🐦 Twitter - $author: $tweet_text"

            if {[string length $result] > $max_title_length} {
                set result "[string range $result 0 [expr {$max_title_length - 4}]]..."
            }

            set_cached $url $result
            return $result
        }

        return ""
    }

    # Reddit Resolver
    # Resolves Reddit posts to show subreddit, title, and score
    proc reddit_resolver {url nick channel} {
        variable max_title_length

        # Check cache first
        set cached [get_cached $url]
        if {$cached ne ""} {
            return $cached
        }

        # Reddit has a JSON API - append .json to URL
        set json_url "${url}.json"

        if {[catch {http get $json_url} content]} {
            # Fallback to HTML parsing
            if {[catch {http get $url} content]} {
                return ""
            }

            # Try HTML extraction
            if {[regexp -nocase {<title>([^<]+)</title>} $content -> title]} {
                set title [decode_html_entities $title]
                # Reddit titles often end with " : subreddit"
                if {[regexp {^(.+) : ([a-zA-Z0-9_]+)$} $title -> post_title subreddit]} {
                    set result "🔴 r/$subreddit: $post_title"

                    if {[string length $result] > $max_title_length} {
                        set result "[string range $result 0 [expr {$max_title_length - 4}]]..."
                    }

                    set_cached $url $result
                    return $result
                }
            }
            return ""
        }

        # Parse JSON (basic extraction without full JSON parser)
        # Look for common fields
        set title ""
        set subreddit ""
        set score ""

        if {[regexp {"title":\s*"([^"]+)"} $content -> post_title]} {
            set title [decode_html_entities $post_title]
        }

        if {[regexp {"subreddit":\s*"([^"]+)"} $content -> sub]} {
            set subreddit $sub
        }

        if {[regexp {"score":\s*(\d+)} $content -> points]} {
            set score [format_number $points]
        }

        if {$title ne "" && $subreddit ne ""} {
            set result "🔴 r/$subreddit: $title"
            if {$score ne ""} {
                append result " ($score points)"
            }

            if {[string length $result] > $max_title_length} {
                set result "[string range $result 0 [expr {$max_title_length - 4}]]..."
            }

            set_cached $url $result
            return $result
        }

        return ""
    }

    # GitHub Resolver
    # Resolves GitHub repos and issues/PRs
    proc github_resolver {url nick channel} {
        variable max_title_length

        # Check cache first
        set cached [get_cached $url]
        if {$cached ne ""} {
            return $cached
        }

        if {[catch {http get $url} content]} {
            return ""
        }

        # Extract title and type
        set title ""
        set type "GitHub"

        if {[regexp -nocase {<title>([^<]+)</title>} $content -> page_title]} {
            set title [decode_html_entities $page_title]
            # Clean up GitHub's title format
            set title [regsub { · GitHub$} $title ""]
            set title [string trim $title]
        }

        # Detect type from URL
        if {[regexp {/issues/(\d+)} $url -> issue_num]} {
            set type "Issue #$issue_num"
        } elseif {[regexp {/pull/(\d+)} $url -> pr_num]} {
            set type "PR #$pr_num"
        } elseif {[regexp {github\.com/([^/]+/[^/]+)/?$} $url -> repo]} {
            set type "Repo"
        }

        if {$title ne ""} {
            set result "🐙 GitHub $type: $title"

            if {[string length $result] > $max_title_length} {
                set result "[string range $result 0 [expr {$max_title_length - 4}]]..."
            }

            set_cached $url $result
            return $result
        }

        return ""
    }

    # Helper: Format large numbers with commas/abbreviations
    proc format_number {num} {
        if {$num >= 1000000} {
            return "[expr {$num / 1000000}]M"
        } elseif {$num >= 1000} {
            return "[expr {$num / 1000}]K"
        }
        return $num
    }
}

# Auto-register example resolvers when this file is loaded
# Users can customize which resolvers to enable

# Built-in resolvers (fourth param = 1 means builtin, won't be persisted)
# Uncomment the ones you want to use:
::linkresolver::register {youtube\.com/watch|youtu\.be/} ::linkresolver::youtube_resolver 10 1
::linkresolver::register {bsky\.app/profile/.*/(post|feed)} ::linkresolver::bluesky_resolver 10 1
# ::linkresolver::register {(twitter\.com|x\.com)/.*/(status|statuses)/} ::linkresolver::twitter_resolver 10 1
# ::linkresolver::register {reddit\.com/r/[^/]+/comments/} ::linkresolver::reddit_resolver 10 1
# ::linkresolver::register {github\.com/[^/]+/[^/]+} ::linkresolver::github_resolver 10 1

# Enable auto-resolution by default (comment out if you want manual control)
::linkresolver::enable
