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
}
