// SPDX-License-Identifier: Apache-2.0
//! Expressions (KerML 8.2.5.8): operator expressions (8.2.5.8.1), primary expressions (8.2.5.8.2),
//! base expressions (8.2.5.8.3) and literals (8.2.5.8.4).
//!
//! The productions of 8.2.5.8.1 are ambiguous on their own; Table 6 and its Note 2 give the
//! precedence and associativity, which a Pratt parser implements: each binary operator has a
//! binding power, all are left-associative except `^` and `**`.

use super::names::{body, qualified_name, qualified_name_or_chain};
use super::{at_qualified_name, is_name, Context};
use crate::kinds::SyntaxKind as K;
use crate::parser::{CompletedMarker, Parser};

/// The binding power of the unary operators, above every binary operator (Table 6).
const UNARY: u8 = 30;

/// What a binary operator takes on its right.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Right {
    /// An expression.
    Expression,
    /// A type reference: the classification and metaclassification operators.
    Type,
}

/// The binary operator at the current token: its left and right binding powers, the kind of
/// node it makes, what it takes on its right, and how many tokens it is (`@@` is two `@`
/// tokens, ADR 0015). Table 6, lowest first.
fn infix(p: &Parser) -> Option<(u8, u8, K, Right, usize)> {
    let (level, kind, right) = match p.current() {
        K::QuestionQuestion => (1, K::BinaryExpression, Right::Expression),
        K::ImpliesKw => (2, K::BinaryExpression, Right::Expression),
        K::Pipe | K::OrKw => (3, K::BinaryExpression, Right::Expression),
        K::XorKw => (4, K::BinaryExpression, Right::Expression),
        K::Amp | K::AndKw => (5, K::BinaryExpression, Right::Expression),
        K::EqEq | K::NotEq | K::EqEqEq | K::NotEqEq => (6, K::BinaryExpression, Right::Expression),
        K::IstypeKw | K::HastypeKw | K::At | K::AsKw | K::MetaKw => {
            (7, K::ClassificationExpression, Right::Type)
        }
        K::Lt | K::Gt | K::LtEq | K::GtEq => (8, K::BinaryExpression, Right::Expression),
        K::DotDot => (9, K::BinaryExpression, Right::Expression),
        K::Plus | K::Minus => (10, K::BinaryExpression, Right::Expression),
        K::Star | K::Slash | K::Percent => (11, K::BinaryExpression, Right::Expression),
        // Right-associative (Note 2): the right operand may have the same operator again.
        K::Caret | K::StarStar => return Some((24, 24, K::BinaryExpression, Right::Expression, 1)),
        _ => return None,
    };
    let tokens = if p.at(K::At) && p.nth_at(1, K::At) {
        2
    } else {
        1
    };
    Some((level * 2, level * 2 + 1, kind, right, tokens))
}

/// Whether an expression can start at the current token.
pub(super) fn at_expression(p: &Parser) -> bool {
    at_qualified_name(p, 0)
        || matches!(
            p.current(),
            K::Plus
                | K::Minus
                | K::Tilde
                | K::NotKw
                | K::AllKw
                | K::IfKw
                | K::IstypeKw
                | K::HastypeKw
                | K::At
                | K::AsKw
                | K::NullKw
                | K::TrueKw
                | K::FalseKw
                | K::StringValue
                | K::DecimalValue
                | K::ExponentialValue
                | K::Star
                | K::LParen
                | K::LBrace
        )
        // `.5`: a real literal (8.2.5.8.4).
        || (p.at(K::Dot) && matches!(p.peek(1), K::DecimalValue | K::ExponentialValue))
}

/// OwnedExpression (KerML 8.2.5.8.1), or an error if there is none.
pub(super) fn expression(p: &mut Parser) -> Option<CompletedMarker> {
    expression_bp(p, 0)
}

fn expression_bp(p: &mut Parser, min: u8) -> Option<CompletedMarker> {
    let mut lhs = prefix(p)?;
    while let Some((left, right, kind, operand, tokens)) = infix(p) {
        if left < min {
            break;
        }
        let m = lhs.precede(p);
        for _ in 0..tokens {
            p.bump();
        }
        match operand {
            // TypeReference: `[QualifiedName]` (8.2.5.8.1).
            Right::Type => {
                qualified_name(p, "KerML 8.2.5.8.1");
            }
            Right::Expression => {
                expression_bp(p, right);
            }
        }
        lhs = m.complete(p, kind);
    }
    Some(lhs)
}

/// The unary, extent, conditional and argument-less classification expressions, or a primary
/// expression (KerML 8.2.5.8.1).
fn prefix(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        // UnaryOperatorExpression: `( '+' | '-' | '~' | 'not' ) Argument`.
        K::Plus | K::Minus | K::Tilde | K::NotKw => {
            let m = p.start();
            p.bump();
            expression_bp(p, UNARY);
            Some(m.complete(p, K::UnaryExpression))
        }
        // ExtentExpression: `'all' TypeReferenceMember`.
        K::AllKw => {
            let m = p.start();
            p.bump();
            qualified_name(p, "KerML 8.2.5.8.1");
            Some(m.complete(p, K::ExtentExpression))
        }
        // ConditionalExpression: `'if' Argument '?' ArgumentExpression 'else' ArgumentExpression`.
        K::IfKw => {
            let m = p.start();
            p.bump();
            expression(p);
            p.expect(K::Question, "syntax::expected-token", "KerML 8.2.5.8.1");
            expression(p);
            p.expect(K::ElseKw, "syntax::expected-token", "KerML 8.2.5.8.1");
            expression(p);
            Some(m.complete(p, K::ConditionalExpression))
        }
        // ClassificationExpression without its argument: `istype T`, `hastype T`, `@T`, `as T`.
        K::IstypeKw | K::HastypeKw | K::At | K::AsKw => {
            let m = p.start();
            p.bump();
            qualified_name(p, "KerML 8.2.5.8.1");
            Some(m.complete(p, K::ClassificationExpression))
        }
        _ => primary(p),
    }
}

/// PrimaryExpression (KerML 8.2.5.8.2): a base expression and its postfix operations.
fn primary(p: &mut Parser) -> Option<CompletedMarker> {
    let mut lhs = base(p)?;
    loop {
        lhs = match p.current() {
            // CollectExpression: `PrimaryArgument '.' BodyArgument`.
            K::Dot if p.nth_at(1, K::LBrace) => {
                let m = lhs.precede(p);
                p.bump();
                body_expression(p);
                m.complete(p, K::CollectExpression)
            }
            // MetadataAccessExpression: `ElementReferenceMember '.' 'metadata'` (8.2.5.8.3).
            K::Dot if p.nth_at(1, K::MetadataKw) => {
                let m = lhs.precede(p);
                p.bump();
                p.bump();
                m.complete(p, K::MetadataAccessExpression)
            }
            // FeatureChainExpression: `NonFeatureChainPrimaryArgument '.' FeatureChainMember`.
            K::Dot if at_qualified_name(p, 1) => {
                let m = lhs.precede(p);
                p.bump();
                qualified_name_or_chain(p, "KerML 8.2.5.8.2");
                m.complete(p, K::FeatureChainExpression)
            }
            // SelectExpression: `PrimaryArgument '.?' BodyArgument`.
            K::DotQuestion => {
                let m = lhs.precede(p);
                p.bump();
                if p.at(K::LBrace) {
                    body_expression(p);
                } else {
                    p.error(
                        "syntax::expected-token",
                        "expected `{`",
                        Some("KerML 8.2.5.8.2"),
                    );
                }
                m.complete(p, K::SelectExpression)
            }
            // FunctionOperationExpression: `PrimaryArgument '->' InvocationTypeMember
            // ( BodyArgument | FunctionReferenceArgument | ArgumentList )`.
            K::Arrow => {
                let m = lhs.precede(p);
                p.bump();
                qualified_name_or_chain(p, "KerML 8.2.5.8.2");
                if p.at(K::LBrace) {
                    body_expression(p);
                } else if p.at(K::LParen) {
                    argument_list(p);
                } else if at_qualified_name(p, 0) {
                    qualified_name(p, "KerML 8.2.5.8.2");
                } else {
                    p.error(
                        "syntax::expected-token",
                        "expected `(`, `{` or a function",
                        Some("KerML 8.2.5.8.2"),
                    );
                }
                m.complete(p, K::FunctionOperationExpression)
            }
            // IndexExpression: `PrimaryArgument '#' '(' SequenceExpressionList ')'`.
            K::Hash if p.nth_at(1, K::LParen) => {
                let m = lhs.precede(p);
                p.bump();
                p.bump();
                expression_list(p, K::RParen);
                m.complete(p, K::IndexExpression)
            }
            // BracketExpression: `PrimaryArgument '[' SequenceExpressionList ']'`.
            K::LBracket => {
                let m = lhs.precede(p);
                p.bump();
                expression_list(p, K::RBracket);
                m.complete(p, K::BracketExpression)
            }
            _ => return Some(lhs),
        };
    }
}

/// `SequenceExpressionList close`: `OwnedExpression ( ',' OwnedExpression )* ','?`.
fn expression_list(p: &mut Parser, close: K) {
    expression(p);
    while p.eat(K::Comma) {
        if p.at(close) {
            break;
        }
        expression(p);
    }
    p.expect(close, "syntax::expected-token", "KerML 8.2.5.8.2");
}

/// The number of tokens of the qualified name at the `n`-th token, or 0.
fn qualified_name_len(p: &Parser, n: usize) -> usize {
    let mut i = n;
    if p.peek(i) == K::Dollar {
        i += 2;
    }
    if !is_name(p.peek(i)) {
        return 0;
    }
    i += 1;
    while p.peek(i) == K::ColonColon && is_name(p.peek(i + 1)) {
        i += 2;
    }
    i - n
}

/// Whether the qualified name or feature chain at the current token is followed by `(`: an
/// invocation rather than a feature reference.
fn at_invocation(p: &Parser) -> bool {
    let mut n = qualified_name_len(p, 0);
    while n > 0 && p.peek(n) == K::Dot && qualified_name_len(p, n + 1) > 0 {
        n += 1 + qualified_name_len(p, n + 1);
    }
    n > 0 && p.peek(n) == K::LParen
}

/// BaseExpression (KerML 8.2.5.8.3), or an error.
fn base(p: &mut Parser) -> Option<CompletedMarker> {
    let m = p.start();
    let kind = match p.current() {
        // NullExpression: `'null' | '(' ')'`.
        K::NullKw => {
            p.bump();
            K::NullExpression
        }
        K::LParen if p.nth_at(1, K::RParen) => {
            p.bump();
            p.bump();
            K::NullExpression
        }
        // SequenceExpression: `'(' SequenceExpressionList ')'` (8.2.5.8.2).
        K::LParen => {
            p.bump();
            expression_list(p, K::RParen);
            K::SequenceExpression
        }
        // BodyExpression: `'{' FunctionBodyPart '}'`.
        K::LBrace => {
            m.abandon(p);
            return Some(body_expression(p));
        }
        // ConstructorExpression: `'new' InstantiatedTypeMember ArgumentList` (ADR 0015).
        K::BasicName if p.nth_is_new(0) && at_qualified_name(p, 1) => {
            p.bump();
            qualified_name_or_chain(p, "KerML 8.2.5.8.3");
            if p.at(K::LParen) {
                argument_list(p);
            } else {
                p.error(
                    "syntax::expected-token",
                    "expected `(`",
                    Some("KerML 8.2.5.8.3"),
                );
            }
            K::ConstructorExpression
        }
        // InvocationExpression: `InstantiatedTypeMember ArgumentList`.
        _ if at_qualified_name(p, 0) && at_invocation(p) => {
            qualified_name_or_chain(p, "KerML 8.2.5.8.3");
            argument_list(p);
            K::InvocationExpression
        }
        // FeatureReferenceExpression: `[QualifiedName]`.
        _ if at_qualified_name(p, 0) => {
            qualified_name(p, "KerML 8.2.5.8.3");
            K::FeatureReferenceExpression
        }
        _ => match literal(p) {
            Some(kind) => kind,
            None => {
                m.abandon(p);
                p.error(
                    "syntax::expected-expression",
                    "expected an expression",
                    Some("KerML 8.2.5.8"),
                );
                return None;
            }
        },
    };
    Some(m.complete(p, kind))
}

/// LiteralExpression (KerML 8.2.5.8.4): bump it and return its kind, or `None`.
pub(super) fn literal(p: &mut Parser) -> Option<K> {
    let real_after = |p: &Parser, n| matches!(p.nth(n), K::DecimalValue | K::ExponentialValue);
    let kind = match p.current() {
        K::TrueKw | K::FalseKw => K::LiteralBoolean,
        K::StringValue => K::LiteralString,
        K::Star => K::LiteralInfinity,
        // RealValue: `DECIMAL_VALUE? '.' ( DECIMAL_VALUE | EXPONENTIAL_VALUE ) | EXPONENTIAL_VALUE`.
        K::ExponentialValue => K::LiteralReal,
        K::DecimalValue if p.nth_at(1, K::Dot) && real_after(p, 2) => {
            p.bump();
            p.bump();
            K::LiteralReal
        }
        K::Dot if real_after(p, 1) => {
            p.bump();
            K::LiteralReal
        }
        K::DecimalValue => K::LiteralInteger,
        _ => return None,
    };
    p.bump();
    Some(kind)
}

/// ArgumentList (KerML 8.2.5.8.3): `'(' ( PositionalArgumentList | NamedArgumentList )? ')'`. A
/// named argument is `[QualifiedName] '=' OwnedExpression`.
fn argument_list(p: &mut Parser) {
    let m = p.start();
    p.bump(); // `(`
    if !p.at(K::RParen) {
        loop {
            let n = qualified_name_len(p, 0);
            if n > 0 && p.nth_at(n, K::Eq) {
                let a = p.start();
                qualified_name(p, "KerML 8.2.5.8.3");
                p.bump(); // `=`
                expression(p);
                a.complete(p, K::NamedArgument);
            } else {
                expression(p);
            }
            if !p.eat(K::Comma) {
                break;
            }
        }
    }
    p.expect(K::RParen, "syntax::expected-token", "KerML 8.2.5.8.3");
    m.complete(p, K::ArgumentList);
}

/// BodyExpression (KerML 8.2.5.8.3): `'{' FunctionBodyPart '}'`, at a `{`.
fn body_expression(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    body(p, Context::Function, "KerML 8.2.5.8.3");
    m.complete(p, K::BodyExpression)
}
