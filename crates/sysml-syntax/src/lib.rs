// SPDX-License-Identifier: Apache-2.0
//! Lossless, error-tolerant lexer and parser for KerML and SysML (PROJECT_PLAN §4.2).
//!
//! The design is ADR 0011: a hand-written lexer ([`lex`]), a hand-written event-based
//! recursive-descent parser with error recovery, and a `rowan` lossless syntax tree ([`parse`]).
//! The text is always reproduced byte for byte by the tree, and malformed input never fails: it
//! produces [`Diagnostic`]s (ADR 0012) and `Error` nodes.

mod diagnostic;
mod grammar;
mod kinds;
mod lexer;
mod parser;

pub use diagnostic::{Diagnostic, Related, Severity};
pub use kinds::{Language, SyntaxKind, Sysml, KERML_RESERVED, SYSML_RESERVED};
pub use lexer::{lex, Lexed, Token};
pub use rowan::{TextRange, TextSize};

/// A node of a syntax tree.
pub type SyntaxNode = rowan::SyntaxNode<Sysml>;
/// A token of a syntax tree.
pub type SyntaxToken = rowan::SyntaxToken<Sysml>;
/// A node or a token.
pub type SyntaxElement = rowan::SyntaxElement<Sysml>;

/// The result of parsing a text: a lossless tree and the diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Parse {
    green: rowan::GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// The root of the syntax tree, a `RootNamespace` node.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// The lexical and syntax diagnostics, sorted by position.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// Parse `text` as one root namespace of `language`.
pub fn parse(language: Language, text: &str) -> Parse {
    let lexed = lex(language, text);
    let mut p = parser::Parser::new(&lexed.tokens);
    grammar::root_namespace(&mut p);
    let (green, diagnostics) = parser::build(text, &lexed.tokens, p.finish(), lexed.diagnostics);
    Parse { green, diagnostics }
}
