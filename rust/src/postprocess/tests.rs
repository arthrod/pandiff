//! Oracle tests for `postprocess`. Expected values come from running the exact
//! TypeScript `postprocess` (compiled `lib/index.js`) under jsdom.

use super::postprocess;
use pretty_assertions::assert_eq;

/// (input, expected) pairs captured from the reference implementation.
const VECTORS: &[(&str, &str)] = &[
    (
        "<p>a <ins data-operation-index=\"1\">b </ins>c</p>",
        "<html><head></head><body><p>a <span class=\"ins\">b </span>c</p></body></html>",
    ),
    (
        "<p>a <del data-operation-index=\"1\">b</del><ins data-operation-index=\"1\">d</ins> c</p>",
        "<html><head></head><body><p>a <span class=\"sub\"><span class=\"del\">b</span><span class=\"ins\">d</span></span> c</p></body></html>",
    ),
    (
        "<p><span class=\"math inline\">x</span> and <span class=\"math inline\"><del>a</del><ins>b</ins></span></p>",
        "<html><head></head><body><p>\\(x\\) and <span class=\"sub\"><span class=\"del\">\\(a\\)</span><span class=\"ins\">\\(b\\)</span></span></p></body></html>",
    ),
    (
        "<ul><li><p>only para</p></li></ul>",
        "<html><head></head><body><ul><li>only para</li></ul></body></html>",
    ),
    (
        "<p><em><del>x</del><ins>y</ins></em></p>",
        "<html><head></head><body><p><em><span class=\"sub\"><span class=\"del\">x</span><span class=\"ins\">y</span></span></em></p></body></html>",
    ),
    (
        "<p><del>foo</del><ins>bar</ins></p>",
        "<html><head></head><body><p><span class=\"del\">foo</span></p><p><span class=\"ins\">bar</span></p></body></html>",
    ),
    (
        "<figure><del><img src=\"minus.png\" alt=\"image\"></del><ins><img src=\"plus.png\" alt=\"image\"></ins><figcaption>image</figcaption></figure>",
        "<html><head></head><body><span class=\"del\"><img src=\"minus.png\" alt=\"image\"></span><p></p><span class=\"ins\"><img src=\"plus.png\" alt=\"image\"></span></body></html>",
    ),
    (
        "<pre><code>line1\n<del>old</del><ins>new</ins>\nline3</code></pre>",
        "<html><head></head><body><pre class=\"diff\"> line1\n-old\n+new\n line3</pre></body></html>",
    ),
    (
        "<p><img src=\"a.png\" alt=\"cap\" title=\"cap\" style=\"x\"></p>",
        "<html><head></head><body><p><img src=\"a.png\" alt=\"cap\"></p></body></html>",
    ),
];

#[test]
fn matches_jsdom_postprocess() {
    for (input, expected) in VECTORS {
        assert_eq!(&postprocess(input), expected, "input={input:?}");
    }
}
