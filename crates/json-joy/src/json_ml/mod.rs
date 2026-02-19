//! json-ml — JsonML (JSON Markup Language) types, HTML serializer, and walker.
//!
//! Mirrors `packages/json-joy/src/json-ml/`.
//!
//! JsonML represents HTML/XML trees as nested JSON arrays:
//! `[tag, attrs, ...children]` where attrs is `null` or a key→value map.

// ── Types ──────────────────────────────────────────────────────────────────

/// A single tag name.  Numeric tags are serialized as their string form.
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    /// Named tag (e.g. `"div"`, `"span"`)
    Named(String),
    /// Empty string — represents a *fragment* (no wrapper element)
    Fragment,
    /// Numeric tag (converted to string when rendering HTML)
    Numeric(i64),
}

impl Tag {
    pub fn is_fragment(&self) -> bool {
        matches!(self, Tag::Fragment)
    }

    pub fn as_str_repr(&self) -> Option<String> {
        match self {
            Tag::Named(s) => Some(s.clone()),
            Tag::Fragment => None,
            Tag::Numeric(n) => Some(n.to_string()),
        }
    }
}

/// A node in the JsonML tree: either a text leaf or an element.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonMlNode {
    /// Text content leaf
    Text(String),
    /// Element: `[tag, attrs, ...children]`
    Element(JsonMlElement),
}

/// A JsonML element: tag, optional attributes, and children.
///
/// Attributes use an ordered `Vec<(key, value)>` to preserve insertion order,
/// matching upstream JS `Record<string, unknown>` insertion-order semantics.
/// Attribute values are pre-stringified (matching the upstream `attrs[key] + ''`
/// coercion that happens on serialization).
#[derive(Debug, Clone, PartialEq)]
pub struct JsonMlElement {
    pub tag: Tag,
    pub attrs: Option<Vec<(String, String)>>,
    pub children: Vec<JsonMlNode>,
}

// ── HTML serializer ────────────────────────────────────────────────────────

/// Escape text content per upstream regex `/[\u00A0-\u9999<>&]/gim`.
///
/// Escapes `<`, `>`, `&`, and characters in the range U+00A0–U+9999.
/// Characters outside this range (including emoji / higher codepoints) are
/// left as-is, matching upstream behaviour.
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        if ch == '<' || ch == '>' || ch == '&' || (code >= 0x00A0 && code <= 0x9999) {
            out.push_str(&format!("&#{};", code));
        } else {
            out.push(ch);
        }
    }
    out
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

/// Serialize a [`JsonMlNode`] to an HTML string.
///
/// - `tab`: indentation string (e.g. `"  "`); use `""` for compact output.
/// - `indent`: current indentation prefix (used in recursion).
pub fn to_html(node: &JsonMlNode, tab: &str, indent: &str) -> String {
    match node {
        JsonMlNode::Text(s) => format!("{}{}", indent, escape_text(s)),
        JsonMlNode::Element(el) => element_to_html(el, tab, indent),
    }
}

fn element_to_html(el: &JsonMlElement, tab: &str, indent: &str) -> String {
    let is_fragment = el.tag.is_fragment();
    let children_indent = if is_fragment {
        indent.to_owned()
    } else {
        format!("{}{}", indent, tab)
    };
    let do_indent = !tab.is_empty();

    // Check if all children are text
    let text_only_children = el.children.iter().all(|c| matches!(c, JsonMlNode::Text(_)));

    let children_str = if text_only_children {
        el.children
            .iter()
            .map(|c| match c {
                JsonMlNode::Text(s) => escape_text(s),
                _ => unreachable!(),
            })
            .collect::<String>()
    } else {
        let mut s = String::new();
        for (i, child) in el.children.iter().enumerate() {
            if do_indent && (!is_fragment || i > 0) {
                s.push('\n');
            }
            s.push_str(&to_html(child, tab, &children_indent));
        }
        s
    };

    if is_fragment {
        return children_str;
    }

    let tag_str = el.tag.as_str_repr().unwrap_or_default();

    // Emit attributes in insertion order (matches upstream `for...in` over
    // JS objects which preserve insertion order for string keys).
    let mut attr_str = String::new();
    if let Some(attrs) = &el.attrs {
        for (k, v) in attrs {
            attr_str.push(' ');
            attr_str.push_str(k);
            attr_str.push_str("=\"");
            attr_str.push_str(&escape_attr(v));
            attr_str.push('"');
        }
    }

    let html_head = format!("<{}{}", tag_str, attr_str);
    if !children_str.is_empty() {
        let closing_indent = if do_indent && !text_only_children {
            format!("\n{}", indent)
        } else {
            String::new()
        };
        format!(
            "{}{}>{}{}{}{}{}",
            indent, html_head, children_str, closing_indent, "</", tag_str, ">"
        )
    } else {
        format!("{}{} />", indent, html_head)
    }
}

// ── Walker ─────────────────────────────────────────────────────────────────

/// An iterator that walks a [`JsonMlNode`] tree depth-first (pre-order).
pub struct JsonMlWalker {
    stack: Vec<JsonMlNode>,
}

impl JsonMlWalker {
    pub fn new(root: JsonMlNode) -> Self {
        Self { stack: vec![root] }
    }
}

impl Iterator for JsonMlWalker {
    type Item = JsonMlNode;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        if let JsonMlNode::Element(ref el) = node {
            // Push children in reverse so the first child is popped first
            for child in el.children.iter().rev() {
                self.stack.push(child.clone());
            }
        }
        Some(node)
    }
}

/// Walk a [`JsonMlNode`] tree, yielding each node depth-first.
pub fn walk(node: JsonMlNode) -> JsonMlWalker {
    JsonMlWalker::new(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> JsonMlNode {
        JsonMlNode::Text(s.to_owned())
    }

    fn el(
        tag: &str,
        attrs: Option<Vec<(String, String)>>,
        children: Vec<JsonMlNode>,
    ) -> JsonMlNode {
        JsonMlNode::Element(JsonMlElement {
            tag: if tag.is_empty() {
                Tag::Fragment
            } else {
                Tag::Named(tag.to_owned())
            },
            attrs,
            children,
        })
    }

    fn attrs(pairs: &[(&str, &str)]) -> Option<Vec<(String, String)>> {
        Some(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn text_node_to_html() {
        let node = text("Hello & world");
        assert_eq!(to_html(&node, "", ""), "Hello &#38; world");
    }

    #[test]
    fn self_closing_element() {
        let node = el("br", None, vec![]);
        assert_eq!(to_html(&node, "", ""), "<br />");
    }

    #[test]
    fn element_with_text_child() {
        let node = el("div", None, vec![text("hello")]);
        assert_eq!(to_html(&node, "", ""), "<div>hello</div>");
    }

    #[test]
    fn element_with_attrs() {
        let node = el("span", attrs(&[("class", "foo")]), vec![text("hi")]);
        assert_eq!(to_html(&node, "", ""), r#"<span class="foo">hi</span>"#);
    }

    #[test]
    fn attrs_preserve_insertion_order() {
        // The second attribute must appear second (no sorting)
        let node = el("div", attrs(&[("z", "1"), ("a", "2")]), vec![]);
        assert_eq!(to_html(&node, "", ""), r#"<div z="1" a="2" />"#);
    }

    #[test]
    fn fragment_renders_children_only() {
        let node = el("", None, vec![text("a"), text("b")]);
        assert_eq!(to_html(&node, "", ""), "ab");
    }

    #[test]
    fn mixed_element_and_text_children() {
        // ['div', null, ['b', null, 'bold'], ' text']
        let node = el(
            "div",
            None,
            vec![el("b", None, vec![text("bold")]), text(" text")],
        );
        assert_eq!(to_html(&node, "", ""), "<div><b>bold</b> text</div>");
    }

    #[test]
    fn escape_text_upper_bound() {
        // U+00A0 (non-breaking space) should be escaped
        assert_eq!(escape_text("\u{00A0}"), "&#160;");
        // U+9999 should be escaped
        assert_eq!(escape_text("\u{9999}"), "&#39321;");
        // U+10000 (above 0x9999) should NOT be escaped — matches upstream
        assert_eq!(escape_text("\u{10000}"), "\u{10000}");
    }

    #[test]
    fn walker_visits_all_nodes() {
        let tree = el(
            "div",
            None,
            vec![text("a"), el("span", None, vec![text("b")])],
        );
        let visited: Vec<_> = walk(tree).collect();
        assert_eq!(visited.len(), 4); // div, "a", span, "b"
    }

    #[test]
    fn walker_order_is_preorder() {
        let tree = el("div", None, vec![text("first"), text("second")]);
        let nodes: Vec<_> = walk(tree).collect();
        assert!(matches!(&nodes[0], JsonMlNode::Element(_)));
        assert_eq!(nodes[1], text("first"));
        assert_eq!(nodes[2], text("second"));
    }

    #[test]
    fn escape_text_special_chars() {
        assert_eq!(escape_text("<div>"), "&#60;div&#62;");
        assert_eq!(escape_text("&"), "&#38;");
    }

    #[test]
    fn escape_attr_special_chars() {
        assert_eq!(escape_attr("foo&bar"), "foo&amp;bar");
        assert_eq!(escape_attr("say \"hi\""), "say &quot;hi&quot;");
        assert_eq!(escape_attr("<script>"), "&lt;script>");
    }

    #[test]
    fn numeric_tag() {
        let node = JsonMlNode::Element(JsonMlElement {
            tag: Tag::Numeric(1),
            attrs: None,
            children: vec![text("heading")],
        });
        assert_eq!(to_html(&node, "", ""), "<1>heading</1>");
    }

    #[test]
    fn indented_output() {
        let tree = el("div", None, vec![el("span", None, vec![text("inner")])]);
        let result = to_html(&tree, "  ", "");
        assert!(result.contains('\n'));
        assert!(result.starts_with("<div>"));
    }
}
