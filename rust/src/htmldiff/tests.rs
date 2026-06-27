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
