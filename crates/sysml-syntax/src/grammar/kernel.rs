// SPDX-License-Identifier: Apache-2.0
//! The Kernel layer (KerML 8.2.5). Step 1a.3 has the parts that the Root and Core layers need
//! first: packages (8.2.5.13), multiplicities with literal and feature-reference bounds (8.2.5.11)
//! and prefix metadata (8.2.5.12). Every other Kernel construct is reported once as
//! `syntax::not-yet-supported` and skipped as a whole, so that the rest of the file still parses;
//! step 1a.4 replaces the skipping with the productions.

use super::names::{body, identification, owned, qualified_name, qualified_name_or_chain};
use super::root::misplaced_prefixes;
use super::{at_qualified_name, Context};
use crate::kinds::SyntaxKind as K;
use crate::parser::{Marker, Parser};

/// Package (KerML 8.2.5.13): `PrefixMetadataMember* 'package' Identification PackageBody`, and
/// LibraryPackage: `'standard'? 'library' PrefixMetadataMember* 'package' Identification
/// PackageBody`. The grammar's `( isStandard ?= 'standard' )` has no `?`, but KerML 7.4.14 shows
/// `library package` without `standard` (ADR 0014).
pub(super) fn package(p: &mut Parser, m: Marker) {
    let before = prefix_metadata_list(p);
    let kind = if p.at(K::StandardKw) || p.at(K::LibraryKw) {
        if before {
            p.error(
                "syntax::misplaced-prefix",
                "the prefix metadata of a library package comes after `library`",
                Some("KerML 8.2.5.13"),
            );
        }
        p.eat(K::StandardKw);
        p.expect(K::LibraryKw, "syntax::expected-token", "KerML 8.2.5.13");
        prefix_metadata_list(p);
        K::LibraryPackage
    } else {
        K::Package
    };
    misplaced_prefixes(p, "KerML 8.2.5.13");
    p.expect(K::PackageKw, "syntax::expected-token", "KerML 8.2.5.13");
    identification(p);
    body(p, Context::Package, "KerML 8.2.5.13");
    m.complete(p, kind);
}

/// PrefixMetadataMember and PrefixMetadataAnnotation (KerML 8.2.5.12): `'#' PrefixMetadataFeature`,
/// where PrefixMetadataFeature is an OwnedFeatureTyping, `[QualifiedName] | FeatureChain`.
pub(super) fn prefix_metadata(p: &mut Parser) {
    let m = p.start();
    p.bump(); // `#`
    qualified_name_or_chain(p, "KerML 8.2.5.12");
    m.complete(p, K::PrefixMetadata);
}

/// `PrefixMetadataMember*`. Returns whether there was any.
pub(super) fn prefix_metadata_list(p: &mut Parser) -> bool {
    let mut any = false;
    while p.at(K::Hash) {
        prefix_metadata(p);
        any = true;
    }
    any
}

/// Multiplicity (KerML 8.2.5.11): MultiplicitySubset, `'multiplicity' Identification Subsets
/// TypeBody`, or MultiplicityRange, `'multiplicity' Identification MultiplicityBounds TypeBody`.
pub(super) fn multiplicity(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.5.11");
    p.bump(); // `multiplicity`
    identification(p);
    let kind = if p.at(K::ColonGt) || p.at(K::SubsetsKw) {
        // Subsets (8.2.4.3.1): `SUBSETS OwnedSubsetting`.
        p.bump();
        owned(p, K::OwnedSubsetting, true, "KerML 8.2.5.11");
        K::MultiplicitySubset
    } else {
        if p.at(K::LBracket) {
            multiplicity_bounds(p);
        } else {
            p.error(
                "syntax::expected-token",
                "expected `[` or `subsets`",
                Some("KerML 8.2.5.11"),
            );
        }
        K::MultiplicityRange
    };
    body(p, Context::Type, "KerML 8.2.5.11");
    m.complete(p, kind);
}

/// MultiplicityBounds (KerML 8.2.5.11): `'[' ( MultiplicityExpressionMember '..' )?
/// MultiplicityExpressionMember ']'`. Also OwnedMultiplicity, whose range has only the bounds.
pub(super) fn multiplicity_bounds(p: &mut Parser) {
    let m = p.start();
    p.bump(); // `[`
    bound(p);
    if p.eat(K::DotDot) {
        bound(p);
    }
    p.expect(K::RBracket, "syntax::expected-token", "KerML 8.2.5.11");
    m.complete(p, K::MultiplicityBounds);
}

/// MultiplicityExpressionMember (KerML 8.2.5.11): a LiteralExpression or a
/// FeatureReferenceExpression. Literals are KerML 8.2.5.8.4; the feature reference is a
/// qualified name (8.2.5.8.3).
fn bound(p: &mut Parser) {
    let m = p.start();
    let real_after = |p: &Parser, n| matches!(p.nth(n), K::DecimalValue | K::ExponentialValue);
    let kind = match p.current() {
        K::TrueKw | K::FalseKw => {
            p.bump();
            K::LiteralBoolean
        }
        K::StringValue => {
            p.bump();
            K::LiteralString
        }
        K::Star => {
            p.bump();
            K::LiteralInfinity
        }
        // RealValue: `DECIMAL_VALUE? '.' ( DECIMAL_VALUE | EXPONENTIAL_VALUE ) | EXPONENTIAL_VALUE`.
        K::ExponentialValue => {
            p.bump();
            K::LiteralReal
        }
        K::DecimalValue if p.nth_at(1, K::Dot) && real_after(p, 2) => {
            p.bump();
            p.bump();
            p.bump();
            K::LiteralReal
        }
        K::Dot if real_after(p, 1) => {
            p.bump();
            p.bump();
            K::LiteralReal
        }
        K::DecimalValue => {
            p.bump();
            K::LiteralInteger
        }
        _ if at_qualified_name(p, 0) => {
            qualified_name(p, "KerML 8.2.5.8.3");
            K::FeatureReferenceExpression
        }
        _ => {
            m.abandon(p);
            p.error(
                "syntax::expected-bound",
                "expected a multiplicity bound: a number, `*` or a feature",
                Some("KerML 8.2.5.11"),
            );
            return;
        }
    };
    m.complete(p, kind);
    if !p.at(K::DotDot) && !p.at(K::RBracket) && !p.at_eof() {
        // A longer expression (8.2.5.8), up to the `..` or `]`.
        let e = p.start();
        p.error(
            "syntax::not-yet-supported",
            "expressions are not parsed yet, other than literals and feature references",
            Some("KerML 8.2.5.8"),
        );
        skip_balanced(p, |p| p.at(K::DotDot) || p.at(K::RBracket));
        e.complete(p, K::Error);
    }
}

/// ValuePart (KerML 8.2.5.10): `= …`, `:= …` or `default …`, an expression (step 1a.4). Skipped
/// up to the `;` or `{` of the feature's body.
pub(super) fn value_part(p: &mut Parser) {
    if !(p.at(K::Eq) || p.at(K::ColonEq) || p.at(K::DefaultKw)) {
        return;
    }
    let m = p.start();
    p.error(
        "syntax::not-yet-supported",
        "feature values are not parsed yet",
        Some("KerML 8.2.5.10"),
    );
    // The body is the first `{` at the top level that does not start an expression: an
    // expression body comes right after an operator, a `(`, a `,` or `-> name`.
    let mut before = [K::Eof; 2];
    loop {
        let k = p.current();
        let starts_expression = matches!(
            before[1],
            K::Eq | K::ColonEq | K::DefaultKw | K::LParen | K::Comma | K::Arrow
        ) || before[0] == K::Arrow;
        if p.at_eof()
            || k == K::Semicolon
            || k == K::RBrace
            || (k == K::LBrace && !starts_expression)
        {
            break;
        }
        if matches!(k, K::LParen | K::LBracket | K::LBrace) {
            skip_group(p);
            before = [before[1], k];
        } else {
            p.bump();
            before = [before[1], k];
        }
    }
    m.complete(p, K::Error);
}

/// FilterPackageMember (KerML 8.2.3.4.2) and other `[ … ]` expressions (step 1a.4).
pub(super) fn not_yet_bracketed(p: &mut Parser, what: &str, clause: &'static str) {
    let m = p.start();
    p.error(
        "syntax::not-yet-supported",
        format!("{what} not parsed yet"),
        Some(clause),
    );
    skip_group(p);
    m.complete(p, K::Error);
}

/// Bump a bracketed group, `( … )`, `[ … ]` or `{ … }`, with the groups nested in it.
fn skip_group(p: &mut Parser) {
    let mut depth = 0u32;
    while !p.at_eof() {
        match p.current() {
            K::LParen | K::LBracket | K::LBrace => depth += 1,
            K::RParen | K::RBracket | K::RBrace => depth = depth.saturating_sub(1),
            _ => {}
        }
        p.bump();
        if depth == 0 {
            return;
        }
    }
}

/// Bump tokens and whole groups until `stop` at the top level, or a closing bracket that is not
/// ours.
fn skip_balanced(p: &mut Parser, stop: impl Fn(&Parser) -> bool) {
    while !p.at_eof() && !stop(p) {
        match p.current() {
            K::LParen | K::LBracket | K::LBrace => skip_group(p),
            K::RParen | K::RBracket | K::RBrace => return,
            _ => p.bump(),
        }
    }
}

/// The Kernel constructs that step 1a.3 does not parse yet, by the keyword that starts them:
/// what they are, and their clause.
const NOT_YET: &[(K, &str, &str)] = &[
    (K::DatatypeKw, "data types are", "KerML 8.2.5.1"),
    (K::ClassKw, "classes are", "KerML 8.2.5.2"),
    (K::StructKw, "structures are", "KerML 8.2.5.3"),
    (K::AssocKw, "associations are", "KerML 8.2.5.4"),
    (K::ConnectorKw, "connectors are", "KerML 8.2.5.5"),
    (K::BindingKw, "binding connectors are", "KerML 8.2.5.5"),
    (K::SuccessionKw, "successions are", "KerML 8.2.5.5"),
    (K::BehaviorKw, "behaviors are", "KerML 8.2.5.6"),
    (K::StepKw, "steps are", "KerML 8.2.5.6"),
    (K::FunctionKw, "functions are", "KerML 8.2.5.7"),
    (K::ExprKw, "expressions are", "KerML 8.2.5.7"),
    (K::PredicateKw, "predicates are", "KerML 8.2.5.7"),
    (K::BoolKw, "Boolean expressions are", "KerML 8.2.5.7"),
    (K::InvKw, "invariants are", "KerML 8.2.5.7"),
    (K::ReturnKw, "result parameters are", "KerML 8.2.5.7"),
    (K::InteractionKw, "interactions are", "KerML 8.2.5.9"),
    (K::FlowKw, "flows are", "KerML 8.2.5.9"),
    (K::MetaclassKw, "metaclasses are", "KerML 8.2.5.12"),
    (K::MetadataKw, "metadata features are", "KerML 8.2.5.12"),
    (K::At, "metadata features are", "KerML 8.2.5.12"),
    (K::FilterKw, "element filters are", "KerML 8.2.5.13"),
];

/// Whether a member starting with `kind` is a Kernel construct that is not parsed yet.
pub(super) fn is_not_yet(kind: K) -> bool {
    NOT_YET.iter().any(|(k, ..)| *k == kind)
}

/// A member that is a Kernel construct not parsed yet (`NOT_YET`): one diagnostic, and the whole
/// member, up to its `;` or the `}` of its body, in an `Error` node.
pub(super) fn not_yet(p: &mut Parser, m: Marker, kind: K) {
    let (_, what, clause) = NOT_YET
        .iter()
        .find(|(k, ..)| *k == kind)
        .copied()
        .unwrap_or((kind, "these elements are", "KerML 8.2.5"));
    p.error(
        "syntax::not-yet-supported",
        format!("{what} not parsed yet"),
        Some(clause),
    );
    while !p.at_eof() && !p.at(K::RBrace) {
        if p.at(K::LBrace) {
            skip_group(p);
            break;
        }
        let semicolon = p.at(K::Semicolon);
        if matches!(p.current(), K::LParen | K::LBracket) {
            skip_group(p);
        } else {
            p.bump();
        }
        if semicolon {
            break;
        }
    }
    m.complete(p, K::Error);
}
