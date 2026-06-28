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

#[test]
fn tag_name_on_text_node_is_none() {
    let t = create_text("hi");
    assert_eq!(tag_name(&t), None);
    assert!(!is_element(&t));
}

#[test]
fn get_attr_missing_returns_none() {
    let dom = parse("<p>x</p>");
    let p = get_elements_by_tag_name(&dom.document, "p")[0].clone();
    assert_eq!(get_attr(&p, "class"), None);
    // get_attr on a non-element is also None.
    let t = create_text("hi");
    assert_eq!(get_attr(&t, "class"), None);
}

#[test]
fn set_attr_adds_then_updates() {
    let el = create_element("p");
    set_attr(&el, "id", "a"); // adds new attribute
    assert_eq!(get_attr(&el, "id").as_deref(), Some("a"));
    set_attr(&el, "id", "b"); // updates existing
    assert_eq!(get_attr(&el, "id").as_deref(), Some("b"));
    remove_attr(&el, "id");
    assert!(!has_attr(&el, "id"));
}

#[test]
fn outer_html_replace_on_parentless_node_is_noop() {
    let el = create_element("p");
    // No parent: should return without panicking and leave the node intact.
    outer_html_replace(&el, "<span>x</span>");
    assert_eq!(tag_name(&el).as_deref(), Some("p"));
}

#[test]
fn outer_html_replace_when_not_in_parent_children() {
    // Inconsistent state: parent pointer set but node absent from parent's
    // children. Exercises the "position not found" branch.
    let parent_el = create_element("div");
    let child = create_element("p");
    child.parent.set(Some(Rc::downgrade(&parent_el)));
    outer_html_replace(&child, "<span>x</span>");
    assert!(parent_el.children.borrow().is_empty());
}

#[test]
fn set_and_remove_attr_on_non_element_are_noops() {
    let t = create_text("hi");
    set_attr(&t, "id", "x"); // no-op on a text node
    remove_attr(&t, "id"); // no-op on a text node
    assert_eq!(get_attr(&t, "id"), None);
}

#[test]
fn has_ancestor_tag_returns_false_when_boundary_not_an_ancestor() {
    let dom = parse("<div><p><span>x</span></p></div><section>sib</section>");
    let span = get_elements_by_tag_name(&dom.document, "span")[0].clone();
    let section = get_elements_by_tag_name(&dom.document, "section")[0].clone();
    // `section` is not an ancestor of `span`, so the walk never hits the
    // boundary and exhausts the parent chain (reaching the document root),
    // returning false for a tag that isn't present.
    assert!(!has_ancestor_tag(&span, "ul", &section));
}

#[test]
fn clone_tree_clones_comment_and_doctype_and_document() {
    // Comment node within a document.
    let dom = parse("<!DOCTYPE html><p>a<!--c--></p>");
    let doc_clone = clone_tree(&dom.document); // Document + Doctype arms
    assert!(serialize_doc_like(&doc_clone).contains("<!--c-->"));
    assert!(serialize_doc_like(&doc_clone).contains("<!DOCTYPE html>"));
}

// Helper: serialize a cloned document node (children-only).
fn serialize_doc_like(h: &Handle) -> String {
    let mut s = String::new();
    for c in h.children.borrow().iter() {
        s.push_str(&outer_html(c));
    }
    s
}

#[test]
fn clone_tree_clones_processing_instruction() {
    use html5ever::tendril::StrTendril;
    use markup5ever_rcdom::{Node, NodeData};
    let pi = Node::new(NodeData::ProcessingInstruction {
        target: StrTendril::from("xml"),
        contents: StrTendril::from("data"),
    });
    let clone = clone_tree(&pi);
    match &clone.data {
        NodeData::ProcessingInstruction { target, contents } => {
            assert_eq!(target.as_ref(), "xml");
            assert_eq!(contents.as_ref(), "data");
        }
        _ => panic!("expected PI"),
    }
}

#[test]
fn has_ancestor_tag_walks_to_root_and_returns_false() {
    let dom = parse("<div><p><span>x</span></p></div>");
    let span = get_elements_by_tag_name(&dom.document, "span")[0].clone();
    // No <ul> ancestor anywhere up to the document root.
    assert!(!has_ancestor_tag(&span, "ul", &dom.document));
    // But <div> is an ancestor.
    assert!(has_ancestor_tag(&span, "div", &dom.document));
}
