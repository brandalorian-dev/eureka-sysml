// SPDX-License-Identifier: Apache-2.0
//! The event-based parser framework (ADR 0011).
//!
//! Productions call [`Parser`] methods, which record start, token, finish and error events over
//! the significant tokens; the parser never sees trivia. [`build`] then replays the events
//! against all the tokens, attaching trivia, to make a lossless `rowan` green tree. A marker can
//! also be wrapped in a new parent after the fact ([`CompletedMarker::precede`]), which the
//! left-associative binary expressions need.
//!
//! The parser always terminates, and the tree is always lossless, even if a production stops
//! making progress: after [`FUEL`] lookaheads without consuming a token the parser sees the end of
//! the input, and [`build`] puts any tokens left over into an `Error` node.

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

/// Lookaheads allowed between two consumed tokens before the parser treats itself as stuck.
pub(crate) const FUEL: u32 = 256;

pub(crate) struct Parser {
    kinds: Vec<SyntaxKind>,
    pos: usize,
    events: Vec<Event>,
    fuel: std::cell::Cell<u32>,
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
            fuel: std::cell::Cell::new(FUEL),
        }
    }

    /// The kind of the `n`-th significant token ahead, or `Eof`. A production that looks ahead
    /// [`FUEL`] times without consuming a token sees `Eof`, so that it terminates.
    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        let fuel = self.fuel.get();
        if fuel == 0 {
            return SyntaxKind::Eof;
        }
        self.fuel.set(fuel - 1);
        self.kinds
            .get(self.pos + n)
            .copied()
            .unwrap_or(SyntaxKind::Eof)
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    /// Whether the `n`-th significant token ahead is `kind`.
    pub(crate) fn nth_at(&self, n: usize, kind: SyntaxKind) -> bool {
        self.nth(n) == kind
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    pub(crate) fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.current())
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.current() == SyntaxKind::Eof
    }

    /// Add the current token to the node being built.
    pub(crate) fn bump(&mut self) {
        assert!(self.pos < self.kinds.len(), "bump at the end of the input");
        self.events.push(Event::Token);
        self.pos += 1;
        self.fuel.set(FUEL);
    }

    /// Add the current token if it is `kind`.
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        let at = self.at(kind);
        if at {
            self.bump();
        }
        at
    }

    /// Add the current token if it is `kind`, or report that it was expected.
    pub(crate) fn expect(
        &mut self,
        kind: SyntaxKind,
        code: &'static str,
        clause: &'static str,
    ) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error(
            code,
            format!("expected {}", kind.description()),
            Some(clause),
        );
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
    /// node, so that one error never hides the next. Returns whether it consumed a token.
    ///
    /// At a token in `recovery` it consumes nothing: that token belongs to the enclosing
    /// production (for example the `}` that closes a body), which must consume it. A caller that
    /// loops must therefore also stop at its recovery tokens.
    #[allow(dead_code)] // members recover in the grammar; expression lists will use it (step 1a.4)
    pub(crate) fn err_recover(
        &mut self,
        code: &'static str,
        message: &str,
        clause: Option<&'static str>,
        recovery: &[SyntaxKind],
    ) -> bool {
        self.error(code, message, clause);
        if self.at_eof() || self.at_any(recovery) {
            return false;
        }
        let m = self.start();
        while !self.at_eof() && !self.at_any(recovery) {
            self.bump();
        }
        m.complete(self, SyntaxKind::Error);
        true
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

    /// Drop the node. Its start stays in the event list as a tombstone, so that no other event
    /// takes its position: a `precede` link may still point at it.
    pub(crate) fn abandon(self, _p: &mut Parser) {}
}

/// A completed node, which can still be wrapped in a parent.
pub(crate) struct CompletedMarker {
    pos: u32,
}

impl CompletedMarker {
    /// Start a node that will become the parent of this one.
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
    let mut last_error: Option<TextRange> = None;
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
                    next = match events
                        .get_mut(at)
                        .map(|e| std::mem::replace(e, Event::Tombstone))
                    {
                        Some(Event::Start {
                            kind,
                            forward_parent,
                        }) => {
                            parents.push(kind);
                            forward_parent
                        }
                        // The parent was abandoned after `precede`: the chain ends here.
                        _ => None,
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
                    if b.index < tokens.len() {
                        // The grammar stopped before the end: keep the rest, so the tree stays lossless.
                        diagnostics.push(Diagnostic::error(
                            "syntax::parser-stuck",
                            b.next_significant_range(),
                            "the parser stopped before the end of the input",
                        ));
                        b.builder
                            .start_node(rowan::SyntaxKind(SyntaxKind::Error as u16));
                        while b.index < tokens.len() {
                            b.token();
                        }
                        b.builder.finish_node();
                    }
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
                // One syntax error per position: the first is the cause, a second one at the same
                // token (from the enclosing production's recovery) only repeats it.
                let range = b.next_significant_range();
                if last_error != Some(range) {
                    let mut d = Diagnostic::error(code, range, message);
                    d.clause = clause;
                    diagnostics.push(d);
                    last_error = Some(range);
                }
            }
            Event::Tombstone => {}
        }
    }
    diagnostics.sort_by(|a, b| (a.range.start(), a.code).cmp(&(b.range.start(), b.code)));
    // A production that reports the same error repeatedly (for example while stuck) reports it once.
    diagnostics.dedup_by(|a, b| a.code == b.code && a.range == b.range);
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
    fn precede_then_abandon_leaves_the_node_unwrapped() {
        // An expression production wraps the left operand, finds no operator, and gives up.
        let (root, _) = tree("a", |p| {
            let r = p.start();
            let lhs = p.start();
            p.bump();
            let lhs = lhs.complete(p, SyntaxKind::Error);
            let parent = lhs.precede(p);
            parent.abandon(p);
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(shape(&root), "RootNamespace(Error)");
        assert_eq!(root.text().to_string(), "a");
    }

    #[test]
    fn precede_then_abandon_after_more_events() {
        // The abandoned parent is no longer the last event, so it becomes a tombstone.
        let (root, _) = tree("a b", |p| {
            let r = p.start();
            let lhs = p.start();
            p.bump();
            let lhs = lhs.complete(p, SyntaxKind::Error);
            let parent = lhs.precede(p);
            p.bump();
            parent.abandon(p);
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(shape(&root), "RootNamespace(Error)");
        assert_eq!(root.text().to_string(), "a b");
    }

    #[test]
    fn a_stuck_production_terminates_and_stays_lossless() {
        // A caller bug: looping until the end while the current token is in the recovery set.
        // The parser must neither hang nor lose text.
        let (root, diags) = tree("a } b", |p| {
            let r = p.start();
            while !p.at_eof() {
                p.err_recover(
                    "syntax::expected-member",
                    "expected a member",
                    None,
                    &[SyntaxKind::RBrace],
                );
            }
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(root.text().to_string(), "a } b", "lossless");
        assert!(
            diags.iter().any(|d| d.code == "syntax::parser-stuck"),
            "{diags:?}"
        );
        let mut seen = std::collections::HashSet::new();
        assert!(
            diags.iter().all(|d| seen.insert((d.code, d.range))),
            "duplicate diagnostics are dropped"
        );
    }

    #[test]
    fn err_recover_reports_whether_it_consumed() {
        let (_, _) = tree("} x", |p| {
            let r = p.start();
            assert!(!p.err_recover(
                "syntax::expected-member",
                "expected a member",
                None,
                &[SyntaxKind::RBrace]
            ));
            p.bump(); // the caller consumes the recovery token
            assert!(p.err_recover("syntax::expected-member", "expected a member", None, &[]));
            r.complete(p, SyntaxKind::RootNamespace);
        });
    }

    #[test]
    fn expected_messages_describe_kinds_without_fixed_text() {
        let (_, diags) = tree(";", |p| {
            let r = p.start();
            p.expect(
                SyntaxKind::BasicName,
                "syntax::expected-name",
                "KerML 8.2.2.3",
            );
            p.bump();
            r.complete(p, SyntaxKind::RootNamespace);
        });
        assert_eq!(diags[0].message, "expected a name");
        assert_eq!(SyntaxKind::StringValue.description(), "a string");
        assert_eq!(SyntaxKind::Semicolon.description(), "`;`");
        assert_eq!(SyntaxKind::PackageKw.description(), "`package`");
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
