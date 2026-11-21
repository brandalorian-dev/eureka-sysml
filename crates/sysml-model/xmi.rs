// SPDX-License-Identifier: Apache-2.0
//! A minimal XML reader for `build.rs`: enough to read the normative `KerML.xmi` (elements,
//! attributes, entity references), and nothing more. It is not part of the crate's API.

/// An XML element: its tag, its attributes, and its child elements. Text is not kept.
pub struct Node {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn children<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.tag == tag)
    }

    /// Every element of the tree, depth first.
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Node>) {
        for c in &self.children {
            out.push(c);
            c.walk(out);
        }
    }
}

/// Parse a document into its root element. Panics on malformed input, which in a build script is
/// a build failure with the reason.
pub fn parse(text: &str) -> Node {
    let mut stack: Vec<Node> = vec![Node {
        tag: String::new(),
        attrs: Vec::new(),
        children: Vec::new(),
    }];
    let b = text.as_bytes();
    let mut i = 0;
    while let Some(lt) = text[i..].find('<').map(|p| p + i) {
        if text[lt..].starts_with("<?") {
            i = text[lt..].find("?>").expect("unterminated <?") + lt + 2;
        } else if text[lt..].starts_with("<!--") {
            i = text[lt..].find("-->").expect("unterminated comment") + lt + 3;
        } else if text[lt..].starts_with("</") {
            i = text[lt..].find('>').expect("unterminated end tag") + lt + 1;
            let node = stack.pop().expect("unbalanced end tag");
            stack
                .last_mut()
                .expect("unbalanced end tag")
                .children
                .push(node);
        } else {
            // A start tag: the name, then attributes up to `>` or `/>`.
            let mut j = lt + 1;
            while !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
                j += 1;
            }
            let mut node = Node {
                tag: text[lt + 1..j].to_owned(),
                attrs: Vec::new(),
                children: Vec::new(),
            };
            loop {
                while b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if b[j] == b'/' {
                    stack.last_mut().unwrap().children.push(node);
                    i = j + 2;
                    break;
                }
                if b[j] == b'>' {
                    stack.push(node);
                    i = j + 1;
                    break;
                }
                let eq = text[j..].find('=').expect("attribute without =") + j;
                let name = text[j..eq].trim().to_owned();
                let quote = b[eq + 1];
                let end = text[eq + 2..]
                    .find(quote as char)
                    .expect("unterminated value")
                    + eq
                    + 2;
                node.attrs.push((name, unescape(&text[eq + 2..end])));
                j = end + 1;
            }
        }
    }
    let mut root = stack.pop().expect("empty document");
    assert!(stack.is_empty(), "unclosed elements");
    root.children.pop().expect("no root element")
}

/// Replace the entity and character references of an attribute value.
fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::new();
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let semi = rest[amp..].find(';').expect("unterminated entity") + amp;
        let entity = &rest[amp + 1..semi];
        match entity {
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16)
                } else {
                    entity.trim_start_matches('#').parse()
                };
                out.push(char::from_u32(code.expect("bad character reference")).unwrap());
            }
        }
        rest = &rest[semi + 1..];
    }
    out.push_str(rest);
    out
}
