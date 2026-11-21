// SPDX-License-Identifier: Apache-2.0
//! The Kernel layer (KerML 8.2.5), apart from expressions (`expressions.rs`): packages, prefix
//! metadata and metadata features, multiplicities, feature values, element filters, the
//! classifier-like definitions, and the kernel features (steps, expressions, connectors, flows).

use super::core::{eat_typed_by, feature_declaration, feature_specialization_part};
use super::expressions::{expression, literal};
use super::names::{body, identification, owned, qualified_name, qualified_name_or_chain};
use super::root::{annotation, misplaced_prefixes};
use super::{at_qualified_name, is_name, Context};
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

/// ElementFilterMember (KerML 8.2.5.13): `MemberPrefix 'filter' OwnedExpression ';'`, in a
/// package body.
pub(super) fn element_filter(p: &mut Parser, m: Marker, context: Context) {
    misplaced_prefixes(p, "KerML 8.2.5.13");
    if context != Context::Package {
        p.error(
            "syntax::misplaced-member",
            "only a package can have an element filter",
            Some("KerML 8.2.5.13"),
        );
    }
    p.bump(); // `filter`
    expression(p);
    p.expect(K::Semicolon, "syntax::expected-token", "KerML 8.2.5.13");
    m.complete(p, K::ElementFilterMember);
}

/// FilterPackageMember (KerML 8.2.3.4.2): `'[' OwnedExpression ']'`, after an import declaration.
pub(super) fn filter_package_member(p: &mut Parser) {
    let m = p.start();
    p.bump(); // `[`
    expression(p);
    p.expect(K::RBracket, "syntax::expected-token", "KerML 8.2.3.4.2");
    m.complete(p, K::FilterPackageMember);
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

/// MetadataFeature (KerML 8.2.5.12): `PrefixMetadataMember* ( '@' | 'metadata' )
/// MetadataFeatureDeclaration ( 'about' Annotation ( ',' Annotation )* )? MetadataBody`, where
/// MetadataFeatureDeclaration is `( Identification ( ':' | 'typed' 'by' ) )? OwnedFeatureTyping`.
pub(super) fn metadata_feature(p: &mut Parser, m: Marker) {
    prefix_metadata_list(p);
    misplaced_prefixes(p, "KerML 8.2.5.12");
    p.bump(); // `@` or `metadata`
    let declared =
        p.at(K::Lt) || (is_name(p.current()) && (p.nth_at(1, K::Colon) || p.nth_at(1, K::TypedKw)));
    if declared {
        identification(p);
        eat_typed_by(p);
    }
    owned(p, K::OwnedFeatureTyping, true, "KerML 8.2.5.12");
    if p.eat(K::AboutKw) {
        annotation(p);
        while p.eat(K::Comma) {
            annotation(p);
        }
    }
    body(p, Context::Metadata, "KerML 8.2.5.12");
    m.complete(p, K::MetadataFeature);
}

/// MetadataBodyFeature (KerML 8.2.5.12): `'feature'? ( ':>>' | 'redefines' )? OwnedRedefinition
/// FeatureSpecializationPart? ValuePart? MetadataBody`. A feature in a metadata body always
/// redefines one.
pub(super) fn metadata_body_feature(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.5.12");
    p.eat(K::FeatureKw);
    if p.at(K::ColonGtGt) || p.at(K::RedefinesKw) {
        p.bump();
    }
    owned(p, K::OwnedRedefinition, true, "KerML 8.2.5.12");
    feature_specialization_part(p);
    feature_value(p);
    body(p, Context::Metadata, "KerML 8.2.5.12");
    m.complete(p, K::Feature);
}

/// FeatureValue (KerML 8.2.5.10): `( '=' | ':=' | 'default' ( '=' | ':=' )? ) OwnedExpression`.
pub(super) fn feature_value(p: &mut Parser) {
    if !(p.at(K::Eq) || p.at(K::ColonEq) || p.at(K::DefaultKw)) {
        return;
    }
    let m = p.start();
    if p.eat(K::DefaultKw) {
        if p.at(K::Eq) || p.at(K::ColonEq) {
            p.bump();
        }
    } else {
        p.bump();
    }
    expression(p);
    m.complete(p, K::FeatureValue);
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

/// MultiplicityExpressionMember (KerML 8.2.5.11): a LiteralExpression (8.2.5.8.4) or a
/// FeatureReferenceExpression (8.2.5.8.3), and nothing else.
fn bound(p: &mut Parser) {
    let m = p.start();
    let kind = if at_qualified_name(p, 0) {
        qualified_name(p, "KerML 8.2.5.8.3");
        K::FeatureReferenceExpression
    } else if let Some(kind) = literal(p) {
        kind
    } else {
        m.abandon(p);
        p.error(
            "syntax::expected-bound",
            "expected a multiplicity bound: a number, `*` or a feature",
            Some("KerML 8.2.5.11"),
        );
        return;
    };
    m.complete(p, kind);
    // A `;` or `{` right after the bound: the `]` is missing, which `multiplicity_bounds` reports.
    let missing_bracket = p.at(K::Semicolon) || p.at(K::LBrace);
    if !p.at(K::DotDot) && !p.at(K::RBracket) && !p.at_eof() && !missing_bracket {
        // A longer expression, up to the `..` or `]`: the grammar allows only a literal or a
        // feature reference.
        let e = p.start();
        p.error(
            "syntax::expected-bound",
            "a multiplicity bound is a literal or a feature reference, not an expression",
            Some("KerML 8.2.5.11"),
        );
        skip_balanced(p, |p| {
            p.at(K::DotDot) || p.at(K::RBracket) || p.at(K::Semicolon) || p.at(K::LBrace)
        });
        e.complete(p, K::Error);
    }
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

/// The classifier-like definitions by keyword: `TypePrefix keyword ClassifierDeclaration body`.
/// The node kind, the body (a function body for functions and predicates), and the clause.
pub(super) fn definition(keyword: K) -> Option<(K, Context, &'static str)> {
    Some(match keyword {
        K::ClassifierKw => (K::Classifier, Context::Type, "KerML 8.2.4.2.1"),
        K::DatatypeKw => (K::DataType, Context::Type, "KerML 8.2.5.1"),
        K::ClassKw => (K::Class, Context::Type, "KerML 8.2.5.2"),
        K::StructKw => (K::Structure, Context::Type, "KerML 8.2.5.3"),
        K::AssocKw => (K::Association, Context::Type, "KerML 8.2.5.4"),
        K::BehaviorKw => (K::Behavior, Context::Type, "KerML 8.2.5.6.1"),
        K::FunctionKw => (K::Function, Context::Function, "KerML 8.2.5.7.1"),
        K::PredicateKw => (K::Predicate, Context::Function, "KerML 8.2.5.7.3"),
        K::InteractionKw => (K::Interaction, Context::Type, "KerML 8.2.5.9.1"),
        K::MetaclassKw => (K::Metaclass, Context::Type, "KerML 8.2.5.12"),
        _ => return None,
    })
}

/// The keywords of the kernel features, which follow a FeaturePrefix (KerML 8.2.5.5–8.2.5.9).
pub(super) const KERNEL_FEATURES: &[K] = &[
    K::StepKw,
    K::ExprKw,
    K::BoolKw,
    K::InvKw,
    K::ConnectorKw,
    K::BindingKw,
    K::SuccessionKw,
    K::FlowKw,
];

/// A kernel feature, at its keyword after the FeaturePrefix, which `m` already holds.
pub(super) fn kernel_feature(p: &mut Parser, m: Marker) {
    // The FeatureDeclaration is optional in each of these, as for `feature` (ADR 0015).
    let kind = match p.current() {
        // Step (8.2.5.6.2): `'step' FeatureDeclaration ValuePart? TypeBody`.
        K::StepKw => {
            p.bump();
            feature_declaration(p, false);
            feature_value(p);
            body(p, Context::Type, "KerML 8.2.5.6.2");
            K::Step
        }
        // Expression (8.2.5.7.2), BooleanExpression and Invariant (8.2.5.7.4):
        // `keyword FeatureDeclaration ValuePart? FunctionBody`, `inv` with `true` or `false`.
        K::ExprKw | K::BoolKw | K::InvKw => {
            let kind = match p.current() {
                K::ExprKw => K::Expression,
                K::BoolKw => K::BooleanExpression,
                _ => K::Invariant,
            };
            let invariant = p.at(K::InvKw);
            p.bump();
            if invariant && (p.at(K::TrueKw) || p.at(K::FalseKw)) {
                p.bump();
            }
            feature_declaration(p, false);
            feature_value(p);
            body(p, Context::Function, "KerML 8.2.5.7");
            kind
        }
        K::ConnectorKw => {
            p.bump();
            connector_declaration(p);
            body(p, Context::Type, "KerML 8.2.5.5.1");
            K::Connector
        }
        K::BindingKw => {
            p.bump();
            ends_declaration(p, K::OfKw, K::Eq, "KerML 8.2.5.5.2");
            body(p, Context::Type, "KerML 8.2.5.5.2");
            K::BindingConnector
        }
        K::SuccessionKw if p.nth_at(1, K::FlowKw) => {
            p.bump();
            p.bump();
            flow_declaration(p);
            body(p, Context::Type, "KerML 8.2.5.9.2");
            K::SuccessionFlow
        }
        K::SuccessionKw => {
            p.bump();
            ends_declaration(p, K::FirstKw, K::ThenKw, "KerML 8.2.5.5.3");
            body(p, Context::Type, "KerML 8.2.5.5.3");
            K::Succession
        }
        _ => {
            p.bump(); // `flow`
            flow_declaration(p);
            body(p, Context::Type, "KerML 8.2.5.9.2");
            K::Flow
        }
    };
    m.complete(p, kind);
}

/// The first of `targets` among the tokens ahead, outside brackets, before the declaration ends
/// at `;`, `{` or `}`: it decides between the forms of a connector, binding, succession or flow.
/// The end counts inside brackets too, since none of them is valid there: an unclosed `[` must
/// not make the scan run on to the end of the file.
fn first_ahead(p: &Parser, targets: &[K]) -> Option<K> {
    let mut depth = 0u32;
    let mut n = 0;
    loop {
        let k = p.peek(n);
        if k == K::Eof || matches!(k, K::Semicolon | K::LBrace | K::RBrace) {
            return None;
        }
        if depth == 0 && targets.contains(&k) {
            return Some(k);
        }
        match k {
            K::LParen | K::LBracket => depth += 1,
            K::RParen | K::RBracket => depth = depth.saturating_sub(1),
            _ => {}
        }
        n += 1;
    }
}

/// ConnectorDeclaration (KerML 8.2.5.5.1), or `FeatureDeclaration? ValuePart?`:
///
/// ```text
/// BinaryConnectorDeclaration = ( FeatureDeclaration? 'from' | 'all' 'from'? )?
///                              ConnectorEndMember 'to' ConnectorEndMember
/// NaryConnectorDeclaration   = FeatureDeclaration? '(' ConnectorEndMember ','
///                              ConnectorEndMember ( ',' ConnectorEndMember )* ')'
/// ```
fn connector_declaration(p: &mut Parser) {
    let clause = "KerML 8.2.5.5.1";
    match first_ahead(
        p,
        &[
            K::FromKw,
            K::ToKw,
            K::LParen,
            K::Eq,
            K::ColonEq,
            K::DefaultKw,
        ],
    ) {
        Some(K::FromKw) => {
            if p.at(K::AllKw) && p.nth_at(1, K::FromKw) {
                p.bump();
            } else {
                feature_declaration(p, false);
            }
            keyword(p, K::FromKw, clause);
            connector_end(p);
            if keyword(p, K::ToKw, clause) {
                connector_end(p);
            }
        }
        Some(K::ToKw) => {
            p.eat(K::AllKw);
            connector_end(p);
            keyword(p, K::ToKw, clause);
            connector_end(p);
        }
        Some(K::LParen) => {
            feature_declaration(p, false);
            keyword(p, K::LParen, clause);
            connector_end(p);
            p.expect(K::Comma, "syntax::expected-token", clause);
            connector_end(p);
            while p.eat(K::Comma) {
                connector_end(p);
            }
            p.expect(K::RParen, "syntax::expected-token", clause);
        }
        _ => {
            feature_declaration(p, false);
            feature_value(p);
        }
    }
}

/// BindingConnectorDeclaration (KerML 8.2.5.5.2) and SuccessionDeclaration (8.2.5.5.3), which
/// have the same shape with `of … =` and `first … then`:
///
/// ```text
///   FeatureDeclaration ( opener ConnectorEndMember separator ConnectorEndMember )?
/// | 'all'? ( opener? ConnectorEndMember separator ConnectorEndMember )?
/// ```
fn ends_declaration(p: &mut Parser, opener: K, separator: K, clause: &'static str) {
    match first_ahead(p, &[opener, separator]) {
        Some(k) if k == opener => {
            if p.at(K::AllKw) && p.nth_at(1, opener) {
                p.bump();
            } else {
                feature_declaration(p, false);
            }
            keyword(p, opener, clause);
            connector_end(p);
            if keyword(p, separator, clause) {
                connector_end(p);
            }
        }
        Some(_) => {
            p.eat(K::AllKw);
            connector_end(p);
            keyword(p, separator, clause);
            connector_end(p);
        }
        None => feature_declaration(p, false),
    }
}

/// The keyword that `first_ahead` found, or that separates two ends: bump it. Any tokens before
/// it are reported, once, and kept in an `Error` node, so that the keyword is never taken for
/// what precedes it. Stops without it at `;`, `{` or `}`; returns whether it was found.
fn keyword(p: &mut Parser, kind: K, clause: &'static str) -> bool {
    if p.eat(kind) {
        return true;
    }
    p.error(
        "syntax::expected-token",
        format!("expected {}", kind.description()),
        Some(clause),
    );
    if p.at_eof() || p.at(K::Semicolon) || p.at(K::LBrace) || p.at(K::RBrace) {
        return false;
    }
    let m = p.start();
    while !(p.at_eof() || p.at(kind) || p.at(K::Semicolon) || p.at(K::LBrace) || p.at(K::RBrace)) {
        p.bump();
    }
    m.complete(p, K::Error);
    p.eat(kind)
}

/// ConnectorEnd (KerML 8.2.5.5.1): `OwnedCrossMultiplicity? ( NAME REFERENCES )?
/// OwnedReferenceSubsetting`, where the cross multiplicity is an OwnedMultiplicity.
fn connector_end(p: &mut Parser) {
    let m = p.start();
    if p.at(K::LBracket) {
        multiplicity_bounds(p);
    }
    if is_name(p.current()) && (p.nth_at(1, K::ColonColonGt) || p.nth_at(1, K::ReferencesKw)) {
        p.bump();
        p.bump();
    }
    owned(p, K::OwnedReferenceSubsetting, true, "KerML 8.2.5.5.1");
    m.complete(p, K::ConnectorEnd);
}

/// FlowDeclaration (KerML 8.2.5.9.2):
///
/// ```text
///   FeatureDeclaration ValuePart? ( 'of' PayloadFeatureMember )?
///   ( 'from' FlowEndMember 'to' FlowEndMember )?
/// | 'all'? FlowEndMember 'to' FlowEndMember
/// ```
fn flow_declaration(p: &mut Parser) {
    let clause = "KerML 8.2.5.9.2";
    if first_ahead(
        p,
        &[K::FromKw, K::OfKw, K::ToKw, K::Eq, K::ColonEq, K::DefaultKw],
    ) == Some(K::ToKw)
    {
        p.eat(K::AllKw);
        flow_end(p);
        keyword(p, K::ToKw, clause);
        flow_end(p);
        return;
    }
    feature_declaration(p, false);
    feature_value(p);
    if p.eat(K::OfKw) {
        payload_feature(p);
    }
    if p.eat(K::FromKw) {
        flow_end(p);
        if keyword(p, K::ToKw, clause) {
            flow_end(p);
        }
    }
}

/// FlowEnd (KerML 8.2.5.9.2): `( OwnedReferenceSubsetting '.' )? FlowFeatureMember`, written as
/// one qualified name or feature chain whose last segment is the flow feature.
fn flow_end(p: &mut Parser) {
    if !at_qualified_name(p, 0) {
        qualified_name(p, "KerML 8.2.5.9.2"); // reports the error
        return;
    }
    let m = p.start();
    qualified_name_or_chain(p, "KerML 8.2.5.9.2");
    m.complete(p, K::FlowEnd);
}

/// PayloadFeature (KerML 8.2.5.9.2):
///
/// ```text
///   Identification PayloadFeatureSpecializationPart ValuePart?
/// | Identification ValuePart
/// | OwnedFeatureTyping OwnedMultiplicity? | OwnedMultiplicity OwnedFeatureTyping?
/// ```
///
/// A name followed by a specialization or a value is the payload's identification; otherwise it
/// is the payload's type.
fn payload_feature(p: &mut Parser) {
    let m = p.start();
    let identified = p.at(K::Lt)
        || (is_name(p.current())
            && matches!(
                p.nth(1),
                K::Colon
                    | K::TypedKw
                    | K::ColonGt
                    | K::SubsetsKw
                    | K::ColonColonGt
                    | K::ReferencesKw
                    | K::FatArrow
                    | K::CrossesKw
                    | K::ColonGtGt
                    | K::RedefinesKw
                    | K::Eq
                    | K::ColonEq
                    | K::DefaultKw
            ));
    if identified {
        identification(p);
        feature_specialization_part(p);
        feature_value(p);
    } else if p.at(K::LBracket) {
        multiplicity_bounds(p);
        if at_qualified_name(p, 0) {
            owned(p, K::OwnedFeatureTyping, true, "KerML 8.2.5.9.2");
        }
    } else {
        owned(p, K::OwnedFeatureTyping, true, "KerML 8.2.5.9.2");
        if p.at(K::LBracket) {
            multiplicity_bounds(p);
        }
    }
    m.complete(p, K::PayloadFeature);
}
