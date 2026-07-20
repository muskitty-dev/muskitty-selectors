//! `Element` trait implementation for `muskitty_dom::Node`.
//!
//! Gated behind the `dom` feature (`#[cfg(feature = "dom")]`). Provides
//! a newtype wrapper [`DomElement`] around `Rc<RefCell<Node>>` that
//! satisfies the orphan rule while implementing the [`Element`] trait.
//!
//! # Usage
//!
//! ```ignore
//! # #[cfg(feature = "dom")]
//! # {
//! use muskitty_dom::Node;
//! use muskitty_selectors::matching::{matches, DomElement};
//! use muskitty_selectors::parser::parse_a_selector;
//!
//! let doc = Node::new_document();
//! let div = Node::new_element_html("div", vec![], &doc);
//! let sel = parse_a_selector("div").unwrap();
//! assert!(matches(&sel, &DomElement::new(div)));
//! # }
//! ```

use crate::matching::Element;
use muskitty_dom::{Node, NodeType};
use std::cell::RefCell;
use std::rc::Rc;

/// Newtype wrapper implementing [`Element`] for `Rc<RefCell<Node>>`.
///
/// The orphan rule prevents us from implementing `Element` directly on
/// `Rc<RefCell<Node>>` (both types are foreign). This thin zero-cost
/// wrapper resolves that — it is just a `#[repr(transparent)]` tuple
/// struct around the DOM node handle.
#[derive(Clone, Debug)]
pub struct DomElement(Rc<RefCell<Node>>);

impl DomElement {
    /// Wrap a DOM node handle. The caller is responsible for ensuring
    /// that `node` is an Element node; non-Element accessors return
    /// empty defaults rather than panicking.
    pub fn new(node: Rc<RefCell<Node>>) -> Self {
        DomElement(node)
    }

    /// Return a reference to the inner `Rc<RefCell<Node>>`.
    pub fn inner(&self) -> &Rc<RefCell<Node>> {
        &self.0
    }

    /// Consume the wrapper and return the inner `Rc<RefCell<Node>>`.
    pub fn into_inner(self) -> Rc<RefCell<Node>> {
        self.0
    }

    fn borrow_node(&self) -> std::cell::Ref<'_, Node> {
        self.0.borrow()
    }
}

/// Walk `node.previous_sibling()` chain skipping non-Element siblings.
fn previous_sibling_element(node: &Rc<RefCell<Node>>) -> Option<Rc<RefCell<Node>>> {
    let mut cur = node.borrow().previous_sibling();
    while let Some(sibling) = cur {
        if sibling.borrow().node_type == NodeType::Element {
            return Some(sibling);
        }
        cur = sibling.borrow().previous_sibling();
    }
    None
}

/// Walk `node.next_sibling()` chain skipping non-Element siblings.
fn next_sibling_element(node: &Rc<RefCell<Node>>) -> Option<Rc<RefCell<Node>>> {
    let mut cur = node.borrow().next_sibling();
    while let Some(sibling) = cur {
        if sibling.borrow().node_type == NodeType::Element {
            return Some(sibling);
        }
        cur = sibling.borrow().next_sibling();
    }
    None
}

impl Element for DomElement {
    fn local_name(&self) -> String {
        self.borrow_node()
            .kind
            .as_element()
            .map(|e| e.local_name.clone())
            .unwrap_or_default()
    }

    fn namespace_uri(&self) -> Option<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.namespace_uri.clone())
    }

    fn id(&self) -> Option<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute("id").map(String::from))
    }

    fn classes(&self) -> Vec<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute("class"))
            .map(|class_attr| {
                class_attr
                    .split_ascii_whitespace()
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_attribute(&self, name: &str) -> Option<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute(name).map(String::from))
    }

    fn parent_element(&self) -> Option<Self> {
        self.borrow_node().parent_element().map(DomElement)
    }

    fn previous_sibling_element(&self) -> Option<Self> {
        previous_sibling_element(&self.0).map(DomElement)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        next_sibling_element(&self.0).map(DomElement)
    }

    fn child_elements(&self) -> Vec<Self> {
        self.borrow_node()
            .child_nodes()
            .iter()
            .filter(|c| c.borrow().node_type == NodeType::Element)
            .cloned()
            .map(DomElement)
            .collect()
    }

    fn is_empty(&self) -> bool {
        for child in self.borrow_node().child_nodes() {
            let child_borrow = child.borrow();
            match child_borrow.node_type {
                NodeType::Element => return false,
                NodeType::Text => {
                    if let Some(t) = child_borrow.kind.as_text() {
                        if !t.data.chars().all(|c| c.is_ascii_whitespace()) {
                            return false;
                        }
                    }
                }
                _ => {}
            }
        }
        true
    }

    fn index_among_siblings(&self) -> usize {
        let mut idx = 1;
        let mut cur = self.previous_sibling_element();
        while let Some(prev) = cur {
            idx += 1;
            cur = prev.previous_sibling_element();
        }
        idx
    }

    fn count_among_siblings(&self) -> usize {
        self.index_among_siblings() + {
            let mut count = 0;
            let mut cur = self.next_sibling_element();
            while let Some(next) = cur {
                count += 1;
                cur = next.next_sibling_element();
            }
            count
        }
    }

    fn index_among_type(&self) -> usize {
        let my_name = self.local_name();
        let mut idx = 1;
        let mut cur = self.previous_sibling_element();
        while let Some(prev) = cur {
            if prev.local_name().eq_ignore_ascii_case(&my_name) {
                idx += 1;
            }
            cur = prev.previous_sibling_element();
        }
        idx
    }

    fn count_among_type(&self) -> usize {
        let my_name = self.local_name();
        let mut count = 1;
        let mut cur = self.previous_sibling_element();
        while let Some(prev) = cur {
            if prev.local_name().eq_ignore_ascii_case(&my_name) {
                count += 1;
            }
            cur = prev.previous_sibling_element();
        }
        let mut cur = self.next_sibling_element();
        while let Some(next) = cur {
            if next.local_name().eq_ignore_ascii_case(&my_name) {
                count += 1;
            }
            cur = next.next_sibling_element();
        }
        count
    }
}
