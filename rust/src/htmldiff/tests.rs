//! Oracle tests for the htmldiff port. Expected values were produced by the
//! reference `node-htmldiff` v0.9.4 implementation.

use super::*;
use pretty_assertions::assert_eq;

/// (before, after, expected) vectors captured from node-htmldiff.
const VECTORS: &[(&str, &str, &str)] = &[
    (
        "<p>this is some text</p>",
        "<p>this is some more text</p>",
        "<p>this is some <ins data-operation-index=\"1\">more </ins>text</p>",
    ),
    (
        "<p>this is some text</p>",
        "<p>this is some text</p>",
        "<p>this is some text</p>",
    ),
    (
        "foo bar baz",
        "Foo bar baz",
        "<del data-operation-index=\"0\">foo</del><ins data-operation-index=\"0\">Foo</ins> bar baz",
    ),
    (
        "<p>foo</p>",
        "<p>bar</p>",
        "<p><del data-operation-index=\"1\">foo</del><ins data-operation-index=\"1\">bar</ins></p>",
    ),
    (
        "<p>a b c</p>",
        "<p>a d c</p>",
        "<p>a <del data-operation-index=\"1\">b</del><ins data-operation-index=\"1\">d</ins> c</p>",
    ),
    (
        "<p>The quick brown fox</p>",
        "<p>The slow brown fox</p>",
        "<p>The <del data-operation-index=\"1\">quick</del><ins data-operation-index=\"1\">slow</ins> brown fox</p>",
    ),
    (
        "<ul><li>one</li><li>two</li></ul>",
        "<ul><li>one</li><li>three</li></ul>",
        "<ul><li>one</li><li><del data-operation-index=\"1\">two</del><ins data-operation-index=\"1\">three</ins></li></ul>",
    ),
    (
        "<p>delete me</p><p>keep</p>",
        "<p>keep</p>",
        "<p data-diff-node=\"del\" data-operation-index=\"0\"><del data-operation-index=\"0\">delete me</del></p><p>keep</p>",
    ),
    (
        "<p>keep</p>",
        "<p>insert me</p><p>keep</p>",
        "<p data-diff-node=\"ins\" data-operation-index=\"0\"><ins data-operation-index=\"0\">insert me</ins></p><p>keep</p>",
    ),
    (
        "<em>italic</em> text",
        "<em>bold</em> text",
        "<em><del data-operation-index=\"1\">italic</del><ins data-operation-index=\"1\">bold</ins></em> text",
    ),
    (
        "<a href=\"x\">link</a>",
        "<a href=\"y\">link</a>",
        "<del data-operation-index=\"0\"><a href=\"x\">link</a></del><ins data-operation-index=\"0\"><a href=\"y\">link</a></ins>",
    ),
    (
        "<img src=\"a.png\">",
        "<img src=\"b.png\">",
        "<del data-operation-index=\"0\"><img src=\"a.png\"></del><ins data-operation-index=\"0\"><img src=\"b.png\"></ins>",
    ),
    (
        "said &ldquo;foo bar&rdquo;",
        "said &ldquo;Foo bar&rdquo;",
        "said &ldquo;<del data-operation-index=\"1\">foo</del><ins data-operation-index=\"1\">Foo</ins> bar&rdquo;",
    ),
    (
        "<p>one two three four</p>",
        "<p>one four</p>",
        "<p>one <del data-operation-index=\"1\">two three </del>four</p>",
    ),
    (
        "<code>x = 1</code>",
        "<code>x = 2</code>",
        "<code>x = <del data-operation-index=\"1\">1</del><ins data-operation-index=\"1\">2</ins></code>",
    ),
    (
        "<p>nested <strong>bold word</strong> here</p>",
        "<p>nested <strong>bold change</strong> here</p>",
        "<p>nested <strong>bold <del data-operation-index=\"1\">word</del><ins data-operation-index=\"1\">change</ins></strong> here</p>",
    ),
    (
        "",
        "said something",
        "<ins data-operation-index=\"0\">said something</ins>",
    ),
    (
        "removed entirely",
        "",
        "<del data-operation-index=\"0\">removed entirely</del>",
    ),
    (
        "<p>multi</p>\n<p>line</p>",
        "<p>multi</p>\n<p>changed</p>",
        "<p>multi</p>\n<p><del data-operation-index=\"1\">line</del><ins data-operation-index=\"1\">changed</ins></p>",
    ),
];

#[test]
fn matches_node_htmldiff_vectors() {
    for (before, after, expected) in VECTORS {
        let got = htmldiff(before, after);
        assert_eq!(&got, expected, "before={before:?} after={after:?}");
    }
}

#[test]
fn identical_input_returns_unchanged() {
    assert_eq!(htmldiff("<p>same</p>", "<p>same</p>"), "<p>same</p>");
}

#[test]
fn tokenizer_splits_words_tags_and_whitespace() {
    let atomic = Regex::new(DEFAULT_ATOMIC).unwrap();
    let toks = html_to_tokens("<p>hi there</p>", &atomic);
    let strings: Vec<&str> = toks.iter().map(|t| t.string.as_str()).collect();
    assert_eq!(strings, vec!["<p>", "hi", " ", "there", "</p>"]);
}

#[test]
fn key_collapses_text_whitespace() {
    assert_eq!(key_for_token("hi   there"), "hi there");
}

#[test]
fn key_for_anchor_uses_href_only() {
    assert_eq!(
        key_for_token("<a href=\"x\" class=\"c\">"),
        "<a href=\"x\"></a>"
    );
}

/// Edge-case vectors (atomic tags, comments, keys, void tags, consecutive
/// replacements) captured from node-htmldiff.
const EDGE_VECTORS: &[(&str, &str, &str)] = &[
    (
        "<object data=\"a.swf\">x</object>",
        "<object data=\"b.swf\">x</object>",
        "<del data-operation-index=\"0\"><object data=\"a.swf\">x</object></del><ins data-operation-index=\"0\"><object data=\"b.swf\">x</object></ins>",
    ),
    (
        "<svg data-uuid=\"0123456789012345678901234567890123\"><circle/></svg> a",
        "<svg data-uuid=\"9999999999999999999999999999999999\"><circle/></svg> b",
        "<del data-operation-index=\"0\"><svg data-uuid=\"0123456789012345678901234567890123\"><circle/></svg> a</del><ins data-operation-index=\"0\"><svg data-uuid=\"9999999999999999999999999999999999\"><circle/></svg> b</ins>",
    ),
    (
        "<iframe src=\"a.html\"></iframe> x",
        "<iframe src=\"b.html\"></iframe> y",
        "<del data-operation-index=\"0\"><iframe src=\"a.html\"></iframe> x</del><ins data-operation-index=\"0\"><iframe src=\"b.html\"></iframe> y</ins>",
    ),
    (
        "a <!-- note --> b",
        "a <!-- note --> c",
        "a  <del data-operation-index=\"1\">b</del><ins data-operation-index=\"1\">c</ins>",
    ),
    (
        "the quick brown fox",
        "a slow red cat",
        "<del data-operation-index=\"0\">the quick brown fox</del><ins data-operation-index=\"0\">a slow red cat</ins>",
    ),
    (
        "a<br/>b",
        "a<br/>c",
        "a<br/><del data-operation-index=\"1\">b</del><ins data-operation-index=\"1\">c</ins>",
    ),
    (
        "<math><mi>x</mi></math> a",
        "<math><mi>y</mi></math> b",
        "<del data-operation-index=\"0\"><math><mi>x</mi></math> a</del><ins data-operation-index=\"0\"><math><mi>y</mi></math> b</ins>",
    ),
    // Completely disjoint content (no common token, not even whitespace) →
    // find_best_match returns None for the segment.
    (
        "abc",
        "xyz",
        "<del data-operation-index=\"0\">abc</del><ins data-operation-index=\"0\">xyz</ins>",
    ),
];

#[test]
fn tokenizer_handles_consecutive_whitespace() {
    let atomic = Regex::new(DEFAULT_ATOMIC).unwrap();
    let toks = html_to_tokens("a  b", &atomic);
    let strings: Vec<&str> = toks.iter().map(|t| t.string.as_str()).collect();
    assert_eq!(strings, vec!["a", "  ", "b"]);
}

#[test]
fn void_tag_inside_deleted_region_is_wrapped() {
    // Deleting the whole paragraph puts the void <br/> inside a del wrapper,
    // exercising the void-tag branch of build_notes.
    let got = htmldiff("<p>a<br/>b</p>", "");
    assert!(got.contains("<del"), "got: {got}");
    assert!(got.contains("<br/>"), "got: {got}");
}

#[test]
fn matches_node_htmldiff_edge_vectors() {
    for (before, after, expected) in EDGE_VECTORS {
        assert_eq!(&htmldiff(before, after), expected, "before={before:?}");
    }
}

#[test]
fn opts_class_name_data_prefix_and_atomic_tags() {
    assert_eq!(
        htmldiff_opts("<p>foo</p>", "<p>bar</p>", Some("diff-class"), "", None),
        "<p><del data-operation-index=\"1\" class=\"diff-class\">foo</del><ins data-operation-index=\"1\" class=\"diff-class\">bar</ins></p>"
    );
    assert_eq!(
        htmldiff_opts("<p>foo</p>", "<p>bar</p>", None, "pfx", None),
        "<p><del data-pfx-operation-index=\"1\">foo</del><ins data-pfx-operation-index=\"1\">bar</ins></p>"
    );
    // With <b> atomic, the whole element shares key "<b>" and is treated equal.
    assert_eq!(
        htmldiff_opts("<b>foo bar</b>", "<b>foo baz</b>", None, "", Some("b")),
        "<b>foo baz</b>"
    );
}

#[test]
fn object_key_extracts_data_attribute() {
    assert_eq!(
        key_for_token("<object data=\"a.swf\" id=\"y\">"),
        "<object src=\"a.swf\"></object>"
    );
}

#[test]
fn iframe_key_extracts_src() {
    assert_eq!(
        key_for_token("<iframe src=\"a.html\" frameborder=\"0\">"),
        "<iframe src=\"a.html\"></iframe>"
    );
}

#[test]
fn empty_token_key_is_empty() {
    assert_eq!(key_for_token(""), "");
}

#[test]
fn end_of_atomic_tag_short_word_is_false() {
    assert!(!end_of_atomic_tag("<a", "iframe"));
    assert!(end_of_atomic_tag("foo</a", "a"));
}

#[test]
fn compare_matches_covers_all_orderings() {
    // m2 entirely before m1 → -1
    let m1 = Match::new(5, 5, 2, 0, 0);
    let before = Match::new(0, 0, 2, 0, 0);
    assert_eq!(compare_matches(&m1, &before), -1);
    // m2 entirely after m1 → 1
    let after = Match::new(10, 10, 2, 0, 0);
    assert_eq!(compare_matches(&m1, &after), 1);
    // overlapping / criss-cross → 0
    let cross = Match::new(6, 0, 2, 0, 0);
    assert_eq!(compare_matches(&m1, &cross), 0);
}

#[test]
fn void_token_is_wrappable_and_not_tag() {
    let atomic = Regex::new(DEFAULT_ATOMIC).unwrap();
    assert!(is_wrappable("<br/>", &atomic));
    assert!(is_void_tag("<br/>"));
    assert!(is_tag("text").is_none());
}

#[test]
fn merge_consecutive_replaces_combines_adjacent_replace_ops() {
    let ops = vec![
        Operation {
            action: Action::Replace,
            start_in_before: 0,
            end_in_before: 0,
            start_in_after: 0,
            end_in_after: 0,
        },
        Operation {
            action: Action::Replace,
            start_in_before: 1,
            end_in_before: 2,
            start_in_after: 1,
            end_in_after: 3,
        },
        Operation {
            action: Action::Equal,
            start_in_before: 3,
            end_in_before: 3,
            start_in_after: 4,
            end_in_after: 4,
        },
    ];
    let merged = merge_consecutive_replaces(ops);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].action, Action::Replace);
    assert_eq!(merged[0].end_in_before, 2);
    assert_eq!(merged[0].end_in_after, 3);
}

#[test]
fn bst_rejects_overlapping_matches() {
    let mut bst = MatchBst::default();
    bst.add(Match::new(0, 0, 2, 0, 0)); // before 0..1, after 0..1
    bst.add(Match::new(5, 5, 2, 0, 0)); // disjoint, goes right
    bst.add(Match::new(1, 1, 1, 0, 0)); // overlaps root → compare == 0 → dropped
    let v = bst.into_vec();
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].start_in_before, 0);
    assert_eq!(v[1].start_in_before, 5);
}

#[test]
fn data_prefix_applies_to_inner_closed_tags() {
    // The whole paragraph content is replaced, so the deleted segment contains
    // an <em>…</em> closed within it → the inner tag gets data-diff-node markers
    // carrying the custom data prefix.
    let got = htmldiff_opts("<p>a <em>b</em> c</p>", "<p>x</p>", None, "pfx", None);
    assert!(got.contains("data-diff-node=\"del\""), "got: {got}");
    assert!(got.contains("data-pfx-operation-index"), "got: {got}");
}
