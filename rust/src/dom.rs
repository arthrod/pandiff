//! A thin mutable-DOM layer over `markup5ever_rcdom`, providing the subset of
//! jsdom operations used by `postprocess`: parsing, serialization,
//! inner/outer-HTML get & set, attribute access, tree manipulation, deep
//! cloning, and the handful of selectors the algorithm needs.

use html5ever::serialize::{SerializeOpts, TraversalScope};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{ns, parse_document, parse_fragment, LocalName, ParseOpts, QualName};
use markup5ever::Attribute;
use markup5ever_rcdom::{Handle, Node, NodeData, RcDom, SerializableHandle};
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Parsing & serialization
// ---------------------------------------------------------------------------

/// Parse a full HTML document (jsdom wraps fragments in `<html><head><body>`).
pub fn parse(html: &str) -> RcDom {
    parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap()
}

/// Serialize an entire document the way jsdom `serialize()` does: the document
/// node's children, i.e. `<html>…</html>` (no doctype unless present).
pub fn serialize_document(dom: &RcDom) -> String {
    let mut out = Vec::new();
    let ser: SerializableHandle = dom.document.clone().into();
    html5ever::serialize(&mut out, &ser, SerializeOpts::default()).unwrap();
    String::from_utf8(out).unwrap()
}

/// `outerHTML`: serialize the node including itself.
pub fn outer_html(h: &Handle) -> String {
    let mut out = Vec::new();
    let ser: SerializableHandle = h.clone().into();
    html5ever::serialize(
        &mut out,
        &ser,
        SerializeOpts {
            traversal_scope: TraversalScope::IncludeNode,
            ..Default::default()
        },
    )
    .unwrap();
    String::from_utf8(out).unwrap()
}

/// `innerHTML`: serialize the node's children.
pub fn inner_html(h: &Handle) -> String {
    let mut s = String::new();
    for c in h.children.borrow().iter() {
        s.push_str(&outer_html(c));
    }
    s
}

fn parse_fragment_nodes(html: &str, context_tag: &str) -> Vec<Handle> {
    let ctx = QualName::new(None, ns!(html), LocalName::from(context_tag));
    let frag = parse_fragment(RcDom::default(), ParseOpts::default(), ctx, vec![], false)
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap();
    let wrapper = frag.document.children.borrow()[0].clone();
    let nodes = wrapper.children.borrow().clone();
    for n in &nodes {
        n.parent.set(None);
    }
    wrapper.children.borrow_mut().clear();
    nodes
}

/// `innerHTML = html`: replace children with the parsed fragment.
pub fn set_inner_html(h: &Handle, html: &str) {
    let ctx = tag_name(h).unwrap_or_else(|| "body".to_string());
    let nodes = parse_fragment_nodes(html, &ctx);
    // Detach existing children.
    for c in h.children.borrow().iter() {
        c.parent.set(None);
    }
    h.children.borrow_mut().clear();
    for n in nodes {
        n.parent.set(Some(Rc::downgrade(h)));
        h.children.borrow_mut().push(n);
    }
}

/// `outerHTML = html`: replace this node in its parent with the parsed fragment.
pub fn outer_html_replace(h: &Handle, html: &str) {
    let p = match parent(h) {
        Some(p) => p,
        None => return,
    };
    let ctx = tag_name(&p).unwrap_or_else(|| "body".to_string());
    let nodes = parse_fragment_nodes(html, &ctx);
    let mut children = p.children.borrow_mut();
    if let Some(pos) = children.iter().position(|c| Rc::ptr_eq(c, h)) {
        children.remove(pos);
        for (i, n) in nodes.into_iter().enumerate() {
            n.parent.set(Some(Rc::downgrade(&p)));
            children.insert(pos + i, n);
        }
    }
    h.parent.set(None);
}

// ---------------------------------------------------------------------------
// Node creation
// ---------------------------------------------------------------------------

pub fn create_element(name: &str) -> Handle {
    Node::new(NodeData::Element {
        name: QualName::new(None, ns!(html), LocalName::from(name)),
        attrs: RefCell::new(Vec::new()),
        template_contents: RefCell::new(None),
        mathml_annotation_xml_integration_point: false,
    })
}

pub fn create_text(text: &str) -> Handle {
    Node::new(NodeData::Text {
        contents: RefCell::new(StrTendril::from(text)),
    })
}

// ---------------------------------------------------------------------------
// Tree navigation & mutation
// ---------------------------------------------------------------------------

pub fn parent(h: &Handle) -> Option<Handle> {
    let weak = h.parent.take();
    let result = weak.as_ref().and_then(|w| w.upgrade());
    h.parent.set(weak);
    result
}

/// Snapshot of child nodes (any type).
pub fn children(h: &Handle) -> Vec<Handle> {
    h.children.borrow().clone()
}

/// jsdom `nextSibling` (next node of any type, or `None`).
pub fn next_sibling(h: &Handle) -> Option<Handle> {
    let p = parent(h)?;
    let children = p.children.borrow();
    let pos = children.iter().position(|c| Rc::ptr_eq(c, h))?;
    children.get(pos + 1).cloned()
}

pub fn append_child(parent: &Handle, child: &Handle) {
    detach(child);
    child.parent.set(Some(Rc::downgrade(parent)));
    parent.children.borrow_mut().push(child.clone());
}

/// Detach a node from its current parent (if any).
pub fn detach(h: &Handle) {
    if let Some(p) = parent(h) {
        let mut ch = p.children.borrow_mut();
        if let Some(pos) = ch.iter().position(|c| Rc::ptr_eq(c, h)) {
            ch.remove(pos);
        }
    }
    h.parent.set(None);
}

/// jsdom `removeChild` / `node.parentNode.removeChild(node)`.
pub fn remove_node(h: &Handle) {
    detach(h);
}

/// Deep clone (jsdom `cloneNode(true)`).
pub fn clone_tree(h: &Handle) -> Handle {
    let data = clone_node_data(&h.data);
    let new = Node::new(data);
    for c in h.children.borrow().iter() {
        let cc = clone_tree(c);
        cc.parent.set(Some(Rc::downgrade(&new)));
        new.children.borrow_mut().push(cc);
    }
    new
}

fn clone_node_data(data: &NodeData) -> NodeData {
    match data {
        NodeData::Document => NodeData::Document,
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => NodeData::Doctype {
            name: name.clone(),
            public_id: public_id.clone(),
            system_id: system_id.clone(),
        },
        NodeData::Text { contents } => NodeData::Text {
            contents: RefCell::new(contents.borrow().clone()),
        },
        NodeData::Comment { contents } => NodeData::Comment {
            contents: contents.clone(),
        },
        NodeData::Element {
            name,
            attrs,
            mathml_annotation_xml_integration_point,
            ..
        } => NodeData::Element {
            name: name.clone(),
            attrs: RefCell::new(attrs.borrow().clone()),
            template_contents: RefCell::new(None),
            mathml_annotation_xml_integration_point: *mathml_annotation_xml_integration_point,
        },
        NodeData::ProcessingInstruction { target, contents } => NodeData::ProcessingInstruction {
            target: target.clone(),
            contents: contents.clone(),
        },
    }
}

// ---------------------------------------------------------------------------
// Element / attribute helpers
// ---------------------------------------------------------------------------

pub fn is_element(h: &Handle) -> bool {
    matches!(h.data, NodeData::Element { .. })
}

/// Lowercase tag name (HTML element names are already lowercased by the parser).
pub fn tag_name(h: &Handle) -> Option<String> {
    match &h.data {
        NodeData::Element { name, .. } => Some(name.local.to_string()),
        _ => None,
    }
}

pub fn get_attr(h: &Handle, name: &str) -> Option<String> {
    if let NodeData::Element { attrs, .. } = &h.data {
        let local = LocalName::from(name);
        for a in attrs.borrow().iter() {
            if a.name.local == local {
                return Some(a.value.to_string());
            }
        }
    }
    None
}

pub fn has_attr(h: &Handle, name: &str) -> bool {
    get_attr(h, name).is_some()
}

pub fn set_attr(h: &Handle, name: &str, value: &str) {
    if let NodeData::Element { attrs, .. } = &h.data {
        let local = LocalName::from(name);
        let mut attrs = attrs.borrow_mut();
        for a in attrs.iter_mut() {
            if a.name.local == local {
                a.value = StrTendril::from(value);
                return;
            }
        }
        attrs.push(Attribute {
            name: QualName::new(None, ns!(), local),
            value: StrTendril::from(value),
        });
    }
}

pub fn remove_attr(h: &Handle, name: &str) {
    if let NodeData::Element { attrs, .. } = &h.data {
        let local = LocalName::from(name);
        attrs.borrow_mut().retain(|a| a.name.local != local);
    }
}

/// jsdom `className`: the `class` attribute value, or "" if absent.
pub fn class_name(h: &Handle) -> String {
    get_attr(h, "class").unwrap_or_default()
}

fn class_list(h: &Handle) -> Vec<String> {
    class_name(h)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Text content
// ---------------------------------------------------------------------------

/// jsdom `textContent`: concatenation of all descendant text nodes.
pub fn text_content(h: &Handle) -> String {
    let mut s = String::new();
    collect_text(h, &mut s);
    s
}

fn collect_text(h: &Handle, out: &mut String) {
    match &h.data {
        NodeData::Text { contents } => out.push_str(&contents.borrow()),
        _ => {
            for c in h.children.borrow().iter() {
                collect_text(c, out);
            }
        }
    }
}

/// jsdom `textContent = value`: replace all children with a single text node
/// (or none, if value is empty).
pub fn set_text_content(h: &Handle, value: &str) {
    for c in h.children.borrow().iter() {
        c.parent.set(None);
    }
    h.children.borrow_mut().clear();
    if !value.is_empty() {
        let t = create_text(value);
        t.parent.set(Some(Rc::downgrade(h)));
        h.children.borrow_mut().push(t);
    }
}

// ---------------------------------------------------------------------------
// Selectors
// ---------------------------------------------------------------------------

/// Preorder list of descendant elements (excluding `root`).
pub fn descendants(root: &Handle) -> Vec<Handle> {
    let mut out = Vec::new();
    fn walk(node: &Handle, out: &mut Vec<Handle>) {
        for c in node.children.borrow().iter() {
            if is_element(c) {
                out.push(c.clone());
            }
            walk(c, out);
        }
    }
    walk(root, &mut out);
    out
}

/// jsdom `getElementsByTagName(name)` on `root` (descendants in document order).
pub fn get_elements_by_tag_name(root: &Handle, name: &str) -> Vec<Handle> {
    descendants(root)
        .into_iter()
        .filter(|e| tag_name(e).as_deref() == Some(name))
        .collect()
}

/// Elements matching `tag` whose class list contains all of `classes`.
pub fn elements_by_tag_and_classes(root: &Handle, tag: &str, classes: &[&str]) -> Vec<Handle> {
    descendants(root)
        .into_iter()
        .filter(|e| {
            if tag_name(e).as_deref() != Some(tag) {
                return false;
            }
            let list = class_list(e);
            classes.iter().all(|c| list.iter().any(|x| x == c))
        })
        .collect()
}

/// First descendant element (document order) whose tag is one of `tags`.
pub fn query_first_of_tags(root: &Handle, tags: &[&str]) -> Option<Handle> {
    descendants(root).into_iter().find(|e| {
        tag_name(e)
            .map(|t| tags.contains(&t.as_str()))
            .unwrap_or(false)
    })
}

/// Does `node` have an ancestor with the given tag, searching up to but not
/// including `boundary`?
pub fn has_ancestor_tag(node: &Handle, tag: &str, boundary: &Handle) -> bool {
    let mut cur = parent(node);
    while let Some(p) = cur {
        if Rc::ptr_eq(&p, boundary) {
            return false;
        }
        if tag_name(&p).as_deref() == Some(tag) {
            return true;
        }
        cur = parent(&p);
    }
    false
}

/// Number of element children of `h`.
pub fn element_child_count(h: &Handle) -> usize {
    h.children.borrow().iter().filter(|c| is_element(c)).count()
}

/// `li > p:only-child`: `p` elements that are the sole element child of an `li`.
pub fn li_p_only_child(root: &Handle) -> Vec<Handle> {
    descendants(root)
        .into_iter()
        .filter(|e| {
            if tag_name(e).as_deref() != Some("p") {
                return false;
            }
            parent(e).is_some_and(|p| {
                tag_name(&p).as_deref() == Some("li") && element_child_count(&p) == 1
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
