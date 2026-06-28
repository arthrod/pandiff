//! CriticMarkup regexes and the `critic*` conversion helpers.

use fancy_regex::Regex as FancyRegex;
use once_cell::sync::Lazy;
use regex::Regex;

// --- CriticMarkup source patterns -----------------------------------------

static CRITIC_DEL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)\{--(.*?)--\}").unwrap());
static CRITIC_INS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)\{\+\+(.*?)\+\+\}").unwrap());
// Substitution needs negative lookahead, unsupported by `regex` — use fancy-regex.
static CRITIC_SUB: Lazy<FancyRegex> = Lazy::new(|| {
    FancyRegex::new(r"\{~~((?:[^~]|(?:~(?!>)))+)~>((?:[^~]|(?:~(?!~\})))+)~~\}").unwrap()
});

// --- span/div patterns produced by the markdown writer --------------------

static SPAN_DEL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)<span class="del">(.*?)</span>"#).unwrap());
static SPAN_INS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)<span class="ins">(.*?)</span>"#).unwrap());
static SPAN_SUB: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?s)<span class="sub"><span class="del">(.*?)</span><span class="ins">(.*?)</span></span>"#,
    )
    .unwrap()
});
static DIV_DEL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)<div class="del">\s*(.*?)\s*</div>"#).unwrap());
static DIV_INS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)<div class="ins">\s*(.*?)\s*</div>"#).unwrap());

/// Apply the three CriticMarkup replacements (delete, insert, substitute) with
/// the given replacement templates, in the same order as the TypeScript code.
fn apply_critic(text: &str, del: &str, ins: &str, sub: &str) -> String {
    let s = CRITIC_DEL.replace_all(text, del).into_owned();
    let s = CRITIC_INS.replace_all(&s, ins).into_owned();
    CRITIC_SUB.replace_all(&s, sub).into_owned()
}

/// `{--a--}` → `<del>a</del>`, etc.
pub fn critic_html(text: &str) -> String {
    apply_critic(
        text,
        "<del>${1}</del>",
        "<ins>${1}</ins>",
        "<del>${1}</del><ins>${2}</ins>",
    )
}

/// LaTeX coloured-markup form.
pub fn critic_latex(text: &str) -> String {
    apply_critic(
        text,
        r"<span>\color{Maroon}~~<span>${1}</span>~~</span>",
        r"<span>\color{OliveGreen}${1}</span>",
        r"<span>\color{RedOrange}~~<span>${1}</span>~~<span>${2}</span></span>",
    )
}

/// Word Track-Changes form.
pub fn critic_track_changes(text: &str) -> String {
    apply_critic(
        text,
        r#"<span class="deletion">${1}</span>"#,
        r#"<span class="insertion">${1}</span>"#,
        r#"<span class="deletion">${1}</span><span class="insertion">${2}</span>"#,
    )
}

/// Reject all changes: keep deletions, drop insertions, keep the old half of
/// substitutions.
pub fn critic_reject(text: &str) -> String {
    apply_critic(text, "${1}", "", "${1}")
}

/// Accept all changes: drop deletions, keep insertions, keep the new half of
/// substitutions.
pub fn critic_accept(text: &str) -> String {
    apply_critic(text, "", "${1}", "${2}")
}

/// Convert the `<span class="...">` / `<div class="...">` markup emitted by the
/// markdown writer back into CriticMarkup. Substitutions must be handled before
/// plain del/ins because they nest those spans.
pub fn spans_to_critic(text: &str) -> String {
    let s = SPAN_SUB.replace_all(text, "{~~${1}~>${2}~~}").into_owned();
    let s = SPAN_DEL.replace_all(&s, "{--${1}--}").into_owned();
    let s = SPAN_INS.replace_all(&s, "{++${1}++}").into_owned();
    let s = DIV_DEL.replace_all(&s, "{--${1}--}").into_owned();
    DIV_INS.replace_all(&s, "{++${1}++}").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn html_conversion() {
        assert_eq!(critic_html("{--a--}"), "<del>a</del>");
        assert_eq!(critic_html("{++b++}"), "<ins>b</ins>");
        assert_eq!(critic_html("{~~a~>b~~}"), "<del>a</del><ins>b</ins>");
    }

    #[test]
    fn reject_and_accept_are_complementary() {
        let t = "x {--del--} y {++ins++} z {~~old~>new~~} w";
        assert_eq!(critic_reject(t), "x del y  z old w");
        assert_eq!(critic_accept(t), "x  y ins z new w");
    }

    #[test]
    fn track_changes_conversion() {
        assert_eq!(
            critic_track_changes("{~~a~>b~~}"),
            r#"<span class="deletion">a</span><span class="insertion">b</span>"#
        );
    }

    #[test]
    fn latex_conversion() {
        assert_eq!(critic_latex("{++x++}"), r"<span>\color{OliveGreen}x</span>");
    }

    #[test]
    fn sub_handles_tilde_and_gt_inside_content() {
        // content containing a tilde not followed by '>' stays inside group 1
        assert_eq!(critic_accept("{~~a~b~>c~~}"), "c");
        assert_eq!(critic_reject("{~~a~b~>c~~}"), "a~b");
    }

    #[test]
    fn spans_round_trip_to_critic() {
        assert_eq!(spans_to_critic(r#"<span class="del">x</span>"#), "{--x--}");
        assert_eq!(spans_to_critic(r#"<span class="ins">y</span>"#), "{++y++}");
        assert_eq!(
            spans_to_critic(
                r#"<span class="sub"><span class="del">a</span><span class="ins">b</span></span>"#
            ),
            "{~~a~>b~~}"
        );
    }

    #[test]
    fn multiline_del_matches() {
        assert_eq!(critic_accept("{--line1\nline2--}"), "");
    }

    // --- Additional boundary & regression tests ----------------------------

    #[test]
    fn empty_string_is_unchanged() {
        assert_eq!(critic_html(""), "");
        assert_eq!(critic_latex(""), "");
        assert_eq!(critic_track_changes(""), "");
        assert_eq!(critic_reject(""), "");
        assert_eq!(critic_accept(""), "");
        assert_eq!(spans_to_critic(""), "");
    }

    #[test]
    fn text_with_no_markers_passes_through_unchanged() {
        let plain = "Hello, world! No markup here.";
        assert_eq!(critic_html(plain), plain);
        assert_eq!(critic_reject(plain), plain);
        assert_eq!(critic_accept(plain), plain);
        assert_eq!(spans_to_critic(plain), plain);
    }

    #[test]
    fn multiple_del_markers_in_same_string() {
        assert_eq!(
            critic_html("{--a--} and {--b--}"),
            "<del>a</del> and <del>b</del>"
        );
        assert_eq!(critic_accept("{--a--} and {--b--}"), " and ");
        assert_eq!(critic_reject("{--a--} and {--b--}"), "a and b");
    }

    #[test]
    fn multiple_ins_markers_in_same_string() {
        assert_eq!(
            critic_html("{++x++} and {++y++}"),
            "<ins>x</ins> and <ins>y</ins>"
        );
        assert_eq!(critic_accept("{++x++} and {++y++}"), "x and y");
        assert_eq!(critic_reject("{++x++} and {++y++}"), " and ");
    }

    #[test]
    fn multiple_sub_markers_in_same_string() {
        assert_eq!(
            critic_html("{~~a~>b~~} mid {~~c~>d~~}"),
            "<del>a</del><ins>b</ins> mid <del>c</del><ins>d</ins>"
        );
    }

    #[test]
    fn mixed_del_ins_sub_in_same_string() {
        let t = "{--del--} {++ins++} {~~old~>new~~}";
        assert_eq!(
            critic_html(t),
            "<del>del</del> <ins>ins</ins> <del>old</del><ins>new</ins>"
        );
        assert_eq!(critic_reject(t), "del  old");
        assert_eq!(critic_accept(t), " ins new");
    }

    #[test]
    fn spans_to_critic_converts_div_del() {
        assert_eq!(
            spans_to_critic(r#"<div class="del">paragraph text</div>"#),
            "{--paragraph text--}"
        );
    }

    #[test]
    fn spans_to_critic_converts_div_ins() {
        assert_eq!(
            spans_to_critic(r#"<div class="ins">paragraph text</div>"#),
            "{++paragraph text++}"
        );
    }

    #[test]
    fn spans_to_critic_div_strips_surrounding_whitespace() {
        // The DIV_DEL regex includes `\s*` around the capture group
        assert_eq!(
            spans_to_critic("<div class=\"del\">\n  hello\n</div>"),
            "{--hello--}"
        );
    }

    #[test]
    fn spans_to_critic_sub_handled_before_plain_del_ins() {
        // If sub were not handled first, the inner del/ins spans would be
        // converted independently, producing malformed CriticMarkup.
        let input =
            r#"<span class="sub"><span class="del">old</span><span class="ins">new</span></span>"#;
        assert_eq!(spans_to_critic(input), "{~~old~>new~~}");
    }

    #[test]
    fn spans_to_critic_multiple_spans() {
        let input = r#"<span class="del">a</span> and <span class="ins">b</span>"#;
        assert_eq!(spans_to_critic(input), "{--a--} and {++b++}");
    }

    #[test]
    fn critic_html_with_content_containing_special_chars() {
        // Angle brackets and ampersands inside a deletion should pass through
        assert_eq!(critic_html("{--<em>x</em>--}"), "<del><em>x</em></del>");
    }

    #[test]
    fn critic_sub_with_gt_inside_new_half() {
        // `>` inside the replacement half should not terminate the pattern early
        assert_eq!(critic_accept("{~~old~>a>b~~}"), "a>b");
        assert_eq!(critic_reject("{~~old~>a>b~~}"), "old");
    }

    #[test]
    fn critic_latex_del_produces_strikethrough() {
        let out = critic_latex("{--removed--}");
        assert!(out.contains("\\color{Maroon}"), "expected Maroon color");
        assert!(out.contains("removed"), "expected original text");
    }

    #[test]
    fn critic_latex_sub_produces_both_halves() {
        let out = critic_latex("{~~old~>new~~}");
        assert!(out.contains("RedOrange"), "expected RedOrange color");
        assert!(out.contains("old"), "expected old text");
        assert!(out.contains("new"), "expected new text");
    }

    #[test]
    fn critic_track_changes_del_uses_deletion_class() {
        let out = critic_track_changes("{--removed--}");
        assert_eq!(out, r#"<span class="deletion">removed</span>"#);
    }

    #[test]
    fn critic_track_changes_ins_uses_insertion_class() {
        let out = critic_track_changes("{++added++}");
        assert_eq!(out, r#"<span class="insertion">added</span>"#);
    }
}
