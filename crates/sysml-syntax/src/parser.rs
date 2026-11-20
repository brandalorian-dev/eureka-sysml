// SPDX-License-Identifier: Apache-2.0
//! The event-based parser framework (ADR 0011).
//!
//! Productions call [`Parser`] methods, which record start, token, finish and error events over
//! the significant tokens; the parser never sees trivia. [`build`] then replays the events
//! against all the tokens, attaching trivia, to make a lossless `rowan` green tree. A marker can
//! also be wrapped in a new parent after the fact ([`CompletedMarker::precede`]), which the
//! left-associative binary expressions need.

use crate::kinds::SyntaxKind;
use crate::lexer::Token;
use crate::Diagnostic;
use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

#[derive(Debug)]
pub(crate) enum Event {
    /// The start of a node. `forward_parent` is the distance to an event that starts its parent,
    /// set by `precede`.
    Start {
        kind: SyntaxKind,
        forward_parent: Option<u32>,
    },
    Finish,
    Token,
    Error {
        code: &'static str,
        message: String,
        clause: Option<&'static str>,
    },
    /// An abandoned start.
    Tombstone,
}

pub(crate) struct Parser {
    kinds: Vec<SyntaxKind>,
    pos: usize,
    events: Vec<Event>,
}

impl Parser {
    pub(crate) fn new(tokens: &[Token]) -> Parser {
        Parser {
            kinds: tokens
                .iter()
                .map(|t| t.kind)
                .filter(|k| !k.is_trivia())
                .collect(),
            pos: 0,
            events: Vec::new(),
        }
    }

    /// The kind of the `n`-th significant token ahead, or `Eof`.
    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        self.kinds
            .get(self.pos + n)
            .copied()
            .unwrap_or(SyntaxKind::Eof)
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    #[allow(dead_code)] // used by the KerML productions from step 1a.3
    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    pub(crate) fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.current())
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.pos >= self.kinds.len()
    }

    /// Add the current token to the node being built.
    pub(crate) fn bump(&mut self) {
        assert!(!self.at_eof(), "bump at the end of the input");
        self.events.push(Event::Token);
        self.pos += 1;
    }

    /// Add the current token if it is `kind`.
    #[allow(dead_code)] // used by the KerML productions from step 1a.3
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        let at = self.at(kind);
        if at {
            self.bump();
        }
        at
    }

    /// Add the current token if it is `kind`, or report that it was expected.
    #[allow(dead_code)] // used by the KerML productions from step 1a.3
    pub(crate) fn expect(
        &mut self,
        kind: SyntaxKind,
        code: &'static str,
        clause: &'static str,
    ) -> bool {
        if self.eat(kind) {
            return true;
        }
        let what = kind
            .text()
            .map_or_else(|| format!("{kind:?}"), |t| format!("`{t}`"));
        self.error(code, format!("expected {what}"), Some(clause));
        false
    }

    /// Report an error at the current token.
    pub(crate) fn error(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        clause: Option<&'static str>,
    ) {
        self.events.push(Event::Error {
            code,
            message: message.into(),
            clause,
        });
    }

    /// Report an error, and wrap the tokens up to one in `recovery` (or the end) in an `Error`
    /// node, so that one error never hides the next.
    pub(crate) fn err_recover(
        &mut self,
        code: &'static str,
        message: &str,
        clause: Option<&'static str>,
        recovery: &[SyntaxKind],
    ) {
        self.error(code, message, clause);
        if self.at_eof() || self.at_any(recovery) {
            return;
        }
        let m = self.start();
        while !self.at_eof() && !self.at_any(recovery) {
            self.bump();
        }
        m.complete(self, SyntaxKind::Error);
    }

    /// Start a node.
    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len() as u32;
        self.events.push(Event::Tombstone);
        Marker { pos }
    }

    pub(crate) fn finish(self) -> Vec<Event> {
        self.events
    }
}

/// A started node; it must be completed or abandoned.
#[must_use = "a node must be completed or abandoned"]
pub(crate) struct Marker {
    pos: u32,
}

impl Marker {
    pub(crate) fn complete(self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
        p.events[self.pos as usize] = Event::Start {
            kind,
            forward_parent: None,
        };
        p.events.push(Event::Finish);
        CompletedMarker { pos: self.pos }
    }

    #[allow(dead_code)] // used by the KerML productions from step 1a.3
    pub(crate) fn abandon(self, p: &mut Parser) {
        if self.pos as usize == p.events.len() - 1 {
            p.events.pop();
        }
    }
}

/// A completed node, which can still be wrapped in a parent.
#[allow(dead_code)] // `pos` is read by `precede`, used from step 1a.4
pub(crate) struct CompletedMarker {
    pos: u32,
}

impl CompletedMarker {
    /// Start a node that will become the parent of this one.
    #[allow(dead_code)] // used by the KerML productions from step 1a.3
    pub(crate) fn precede(self, p: &mut Parser) -> Marker {
        let parent = p.start();
        if let Event::Start { forward_parent, .. } = &mut p.events[self.pos as usize] {
            *forward_parent = Some(parent.pos - self.pos);
        }
        parent
    }
}

/// Replay `events` against `tokens` into a lossless green tree. Trivia between significant tokens
/// goes into the innermost open node; trivia before a nested node goes into its parent.
pub(crate) fn build(
    text: &str,
    tokens: &[Token],
    mut events: Vec<Event>,
    mut diagnostics: Vec<Diagnostic>,
) -> (GreenNode, Vec<Diagnostic>) {
    let mut b = TreeBuilder {
        text,
        tokens,
        index: 0,
        offset: 0,
        builder: GreenNodeBuilder::new(),
    };
    let mut depth = 0usize;
    let mut parents = Vec::new();
    for i in 0..events.len() {
        match std::mem::replace(&mut events[i], Event::Tombstone) {
            Event::Start {
                kind,
                forward_parent,
            } => {
                // Collect this node and the parents `precede` attached to it, outermost first.
                parents.push(kind);
                let mut at = i;
                let mut next = forward_parent;
                while let Some(distance) = next {
                    at += distance as usize;
                    next = match std::mem::replace(&mut events[at], Event::Tombstone) {
                        Event::Start {
                            kind,
                            forward_parent,
                        } => {
                            parents.push(kind);
                            forward_parent
                        }
                        _ => unreachable!("a forward parent is a start event"),
                    };
                }
                for kind in parents.drain(..).rev() {
                    if depth > 0 {
                        b.trivia();
                    }
                    b.builder.start_node(rowan::SyntaxKind(kind as u16));
                    depth += 1;
                }
            }
            Event::Finish => {
                if depth == 1 {
                    b.trivia(); // the root keeps the trailing trivia
                }
                b.builder.finish_node();
                depth -= 1;
            }
            Event::Token => {
                b.trivia();
                b.token();
            }
            Event::Error {
                code,
                message,
                clause,
            } => {
                let mut d = Diagnostic::error(code, b.next_significant_range(), message);
                d.clause = clause;
                diagnostics.push(d);
            }
            Event::Tombstone => {}
        }
    }
    diagnostics.sort_by_key(|d| d.range.start());
    (b.builder.finish(), diagnostics)
}

struct TreeBuilder<'a> {
    text: &'a str,
    tokens: &'a [Token],
    index: usize,
    offset: usize,
    builder: GreenNodeBuilder<'static>,
}

impl TreeBuilder<'_> {
    fn token(&mut self) {
        let t = self.tokens[self.index];
        let end = self.offset + t.len as usize;
        self.builder.token(
            rowan::SyntaxKind(t.kind as u16),
            &self.text[self.offset..end],
        );
        self.offset = end;
        self.index += 1;
    }

    fn trivia(&mut self) {
        while self
            .tokens
            .get(self.index)
            .is_some_and(|t| t.kind.is_trivia())
        {
            self.token();
        }
    }

    /// The range of the next significant token, or an empty range at the end.
    fn next_significant_range(&self) -> TextRange {
        let mut offset = self.offset;
        for t in &self.tokens[self.index..] {
            if !t.kind.is_trivia() {
                return TextRange::at(TextSize::from(offset as u32), TextSize::from(t.len));
            }
            offset += t.len as usize;
        }
        TextRange::empty(TextSize::from(self.text.len() as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::Language;
    use crate::lexer::lex;
    use crate::SyntaxNode;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn tree(text: &str, grammar: impl Fn(&mut Parser)) -> (SyntaxNode, Vec<Diagnostic>) {
        let lexed = lex(Language::KerML, text);
        let mut p = Parser::new(&lexed.tokens);
        grammar(&mut p);
        let (green, diags) = build(text, &lexed.tokens, p.finish(), lexed.diagnostics);
        (SyntaxNode::new_root(green), diags)
    }

    fn shape(node: &SyntaxNode) -> String {
        let kids: Vec<String> = node.children().map(|c| shape(&c)).collect();
        if kids.is_empty() {
            format!("{:?}", node.kind())
        } else {
            format!("{:?}({})", node.kind(), kids.join(" "))
        }
    }

    #[test]
    fn trivia_before_a_nested_node_belongs_to_the_parent() {
        let (root, _) = tree("  a /* c */ b ", |p| {
            let r = p.start();
            let inner = p.start();
            p.bump();
            p.bump();
            inner.complete(p, SyntaxKind::Error);
            p.bump();
            r.complete(p, SyntaxKind::RootNamespace);
        });
        let inner = root.first_child().unwrap();
        assert_eq!(
            inner.text().to_string(),
            "a /* c */",
            "leading trivia stays outside"
        );
        assert_eq!(
            root.text().to_string(),
            "  a /* c */ b ",
            "lossless, trailing trivia in the root"
        );
    }

    #[test]
    fn precede_wraps_a_completed_node() {
        // `a + b`: parse `a`, then wrap it in a new node once `+` is seen.
        let (root, _) = tree("a + b", |p| {
            let r = p.start();
            let lhs = p.start();
            p.bump();
            let lhs = lhs.complete(p, SyntaxKind::Error);
            let bin = lhs.precede(p);
            p.bump();
            let rhs = p.start();
            p.bump();
            rhs.complete(p, SyntaxKind::Error);
            bin.complete(p, SyntaxKind::RootNamespace);
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(shape(&root), "RootNamespace(RootNamespace(Error Error))");
        assert_eq!(root.text().to_string(), "a + b");
    }

    #[test]
    fn abandoned_markers_leave_no_node() {
        let (root, _) = tree("a", |p| {
            let r = p.start();
            let m = p.start();
            m.abandon(p);
            p.bump();
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(shape(&root), "RootNamespace");
        assert_eq!(root.text().to_string(), "a");
    }

    #[test]
    fn expected_tokens_are_reported_at_the_next_token() {
        let (_, diags) = tree("a ;", |p| {
            let r = p.start();
            p.bump();
            p.expect(
                SyntaxKind::LBrace,
                "syntax::expected-brace",
                "KerML 8.2.3.4.1",
            );
            p.bump();
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "expected `{`");
        assert_eq!(u32::from(diags[0].range.start()), 2);
        assert_eq!(diags[0].clause, Some("KerML 8.2.3.4.1"));
    }

    #[test]
    fn recovery_stops_at_a_recovery_token() {
        let (root, diags) = tree("x y ; z", |p| {
            let r = p.start();
            p.err_recover(
                "syntax::expected-member",
                "expected a member",
                None,
                &[SyntaxKind::Semicolon],
            );
            p.bump(); // `;`
            p.err_recover("syntax::expected-member", "expected a member", None, &[]);
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(shape(&root), "RootNamespace(Error Error)");
        let errors: Vec<String> = root.children().map(|c| c.text().to_string()).collect();
        assert_eq!(errors, ["x y", "z"]);
        assert_eq!(diags.len(), 2);
    }
}
