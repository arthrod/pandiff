use super::*;
use pretty_assertions::assert_eq;

#[test]
fn parse_and_serialize_round_trip() {
    let dom = parse("<p>hi <em>there</em></p>");
    assert_eq!(
        serialize_document(&dom),
        "<html><head></head><body><p>hi <em>there</em></p></body></html>"
    );
}

#[test]
fn inner_and_outer_html() {
    let dom = parse("<p id=\"x\">a<b>c</b></p>");
    let p = get_elements_by_tag_name(&dom.document, "p")[0].clone();
    assert_eq!(inner_html(&p), "a<b>c</b>");
    assert_eq!(outer_html(&p), "<p id=\"x\">a<b>c</b></p>");
}

#[test]
fn outer_html_replace_swaps_node() {
    let dom = parse("<p><del>gone</del> stay</p>");
    let del = get_elements_by_tag_name(&dom.document, "del")[0].clone();
    outer_html_replace(&del, "<span class=\"del\">gone</span>");
    let p = get_elements_by_tag_name(&dom.document, "p")[0].clone();
    assert_eq!(inner_html(&p), "<span class=\"del\">gone</span> stay");
}

#[test]
fn attributes_get_set_remove() {
    let dom = parse("<img src=\"a.png\" style=\"x\">");
    let img = get_elements_by_tag_name(&dom.document, "img")[0].clone();
    assert!(has_attr(&img, "style"));
    remove_attr(&img, "style");
    assert!(!has_attr(&img, "style"));
    set_attr(&img, "alt", "cap");
    assert_eq!(get_attr(&img, "alt").as_deref(), Some("cap"));
}

#[test]
fn text_content_reads_and_sets() {
    let dom = parse("<p>foo<b>bar</b></p>");
    let p = get_elements_by_tag_name(&dom.document, "p")[0].clone();
    assert_eq!(text_content(&p), "foobar");
    set_text_content(&p, "new");
    assert_eq!(inner_html(&p), "new");
}

#[test]
fn clone_tree_is_independent() {
    let dom = parse("<p><del>x</del><ins>y</ins></p>");
    let p = get_elements_by_tag_name(&dom.document, "p")[0].clone();
    let clone = clone_tree(&p);
    // Remove <del> from the clone; original keeps it.
    let del = get_elements_by_tag_name(&clone, "del")[0].clone();
    remove_node(&del);
    assert_eq!(inner_html(&clone), "<ins>y</ins>");
    assert_eq!(inner_html(&p), "<del>x</del><ins>y</ins>");
}

#[test]
fn selectors_classes_and_ancestors() {
    let dom =
        parse("<p><span class=\"math inline\">a</span><span class=\"math display\">b</span></p>");
    assert_eq!(
        elements_by_tag_and_classes(&dom.document, "span", &["math", "inline"]).len(),
        1
    );
    assert_eq!(
        elements_by_tag_and_classes(&dom.document, "span", &["math"]).len(),
        2
    );
}

#[test]
fn li_p_only_child_selector() {
    let dom = parse("<ul><li><p>only</p></li><li><p>a</p><p>b</p></li></ul>");
    let matches = li_p_only_child(&dom.document);
    assert_eq!(matches.len(), 1);
    assert_eq!(text_content(&matches[0]), "only");
}

#[test]
fn ancestor_tag_within_boundary() {
    let dom = parse("<figure><ins><img src=\"a\"></ins><img src=\"b\"></figure>");
    let figure = get_elements_by_tag_name(&dom.document, "figure")[0].clone();
    let imgs = get_elements_by_tag_name(&figure, "img");
    assert!(has_ancestor_tag(&imgs[0], "ins", &figure));
    assert!(!has_ancestor_tag(&imgs[1], "ins", &figure));
}

#[test]
fn next_sibling_returns_following_node() {
    let dom = parse("<p><span>a</span><span>b</span></p>");
    let spans = get_elements_by_tag_name(&dom.document, "span");
    let next = next_sibling(&spans[0]).unwrap();
    assert!(Rc::ptr_eq(&next, &spans[1]));
    assert!(next_sibling(&spans[1]).is_none());
}
