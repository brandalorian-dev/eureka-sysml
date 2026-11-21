// SPDX-License-Identifier: Apache-2.0
//! The Core layer (KerML 8.2.4): types, classifiers, features and their relationships.

use super::names::{
    body, identification, owned, owned_list, qualified_name, qualified_name_or_chain,
};
use super::root::misplaced_prefixes;
use super::{is_name, kernel, Context};
use crate::kinds::SyntaxKind as K;
use crate::parser::{Marker, Parser};

/// Bump a keyword-or-symbol terminal (KerML 8.2.2.7), such as SPECIALIZES (`:>` or
/// `specializes`), or report that it was expected.
fn expect_either(p: &mut Parser, symbol: K, keyword: K, clause: &'static str) -> bool {
    if p.at(symbol) || p.at(keyword) {
        p.bump();
        return true;
    }
    p.error(
        "syntax::expected-token",
        format!(
            "expected {} or {}",
            symbol.description(),
            keyword.description()
        ),
        Some(clause),
    );
    false
}

/// TYPED_BY (KerML 8.2.2.7): `':' | 'typed' 'by'`.
pub(super) fn eat_typed_by(p: &mut Parser) -> bool {
    if p.eat(K::Colon) {
        return true;
    }
    if p.eat(K::TypedKw) {
        p.expect(K::ByKw, "syntax::expected-token", "KerML 8.2.2.7");
        return true;
    }
    false
}

/// TypePrefix (KerML 8.2.4.1.1): `'abstract'? PrefixMetadataMember*`.
fn type_prefix(p: &mut Parser) {
    p.eat(K::AbstractKw);
    kernel::prefix_metadata_list(p);
}

/// Type (KerML 8.2.4.1.1): `TypePrefix 'type' TypeDeclaration TypeBody`, where TypeDeclaration is
/// `'all'? Identification OwnedMultiplicity? ( SpecializationPart | ConjugationPart )+
/// TypeRelationshipPart*`. The grammar requires at least one specialization or conjugation part.
pub(super) fn type_(p: &mut Parser, m: Marker) {
    type_prefix(p);
    misplaced_prefixes(p, "KerML 8.2.4.1.1");
    p.bump(); // `type`
    p.eat(K::AllKw);
    identification(p);
    if p.at(K::LBracket) {
        kernel::multiplicity_bounds(p);
    }
    let mut parts = false;
    loop {
        if p.at(K::ColonGt) || p.at(K::SpecializesKw) {
            // SpecializationPart (8.2.4.1.1).
            p.bump();
            owned_list(p, K::OwnedSpecialization, true, "KerML 8.2.4.1.2");
        } else if p.at(K::Tilde) || p.at(K::ConjugatesKw) {
            // ConjugationPart (8.2.4.1.1).
            p.bump();
            owned(p, K::OwnedConjugation, true, "KerML 8.2.4.1.3");
        } else {
            break;
        }
        parts = true;
    }
    if !parts {
        p.error(
            "syntax::expected-specialization",
            "expected `specializes` or `conjugates`: a type declaration has at least one",
            Some("KerML 8.2.4.1.1"),
        );
    }
    relationship_parts(p, false);
    body(p, Context::Type, "KerML 8.2.4.1.1");
    m.complete(p, K::Type);
}

/// Classifier (KerML 8.2.4.2.1): `TypePrefix 'classifier' ClassifierDeclaration TypeBody`, where
/// ClassifierDeclaration is `'all'? Identification OwnedMultiplicity?
/// ( SuperclassingPart | ConjugationPart )? TypeRelationshipPart*`. The Kernel definitions have
/// the same shape with their own keyword (`datatype`, `class`, `assoc struct`, `function` …),
/// and functions and predicates have a FunctionBody (KerML 8.2.5.1–8.2.5.12).
pub(super) fn classifier(p: &mut Parser, m: Marker, keyword: K) {
    let (mut kind, body_context, clause) =
        kernel::definition(keyword).unwrap_or((K::Classifier, Context::Type, "KerML 8.2.4.2.1"));
    type_prefix(p);
    misplaced_prefixes(p, clause);
    p.bump(); // the keyword
    if kind == K::Association && p.eat(K::StructKw) {
        kind = K::AssociationStructure;
    }
    p.eat(K::AllKw);
    identification(p);
    if p.at(K::LBracket) {
        kernel::multiplicity_bounds(p);
    }
    if p.at(K::ColonGt) || p.at(K::SpecializesKw) {
        // SuperclassingPart (8.2.4.2.1).
        p.bump();
        owned_list(p, K::OwnedSubclassification, false, "KerML 8.2.4.2.2");
    } else if p.at(K::Tilde) || p.at(K::ConjugatesKw) {
        p.bump();
        owned(p, K::OwnedConjugation, true, "KerML 8.2.4.1.3");
    }
    relationship_parts(p, false);
    body(p, body_context, clause);
    m.complete(p, kind);
}

/// TypeRelationshipPart (KerML 8.2.4.1.1) and, for a feature, FeatureRelationshipPart (8.2.4.3.1).
fn relationship_parts(p: &mut Parser, feature: bool) {
    loop {
        match p.current() {
            K::DisjointKw if p.nth_at(1, K::FromKw) => {
                p.bump();
                p.bump();
                owned_list(p, K::OwnedDisjoining, true, "KerML 8.2.4.1.4");
            }
            K::UnionsKw => {
                p.bump();
                owned_list(p, K::Unioning, true, "KerML 8.2.4.1.5");
            }
            K::IntersectsKw => {
                p.bump();
                owned_list(p, K::Intersecting, true, "KerML 8.2.4.1.5");
            }
            K::DifferencesKw => {
                p.bump();
                owned_list(p, K::Differencing, true, "KerML 8.2.4.1.5");
            }
            // ChainingPart: one OwnedFeatureChaining, or a FeatureChain (8.2.4.3.1).
            K::ChainsKw if feature => {
                p.bump();
                qualified_name_or_chain(p, "KerML 8.2.4.3.1");
            }
            K::InverseKw if feature && p.nth_at(1, K::OfKw) => {
                p.bump();
                p.bump();
                owned(p, K::OwnedFeatureInverting, true, "KerML 8.2.4.3.6");
            }
            K::FeaturedKw if feature && p.nth_at(1, K::ByKw) => {
                p.bump();
                p.bump();
                owned_list(p, K::OwnedTypeFeaturing, false, "KerML 8.2.4.3.7");
            }
            _ => return,
        }
    }
}

/// Whether a FeatureDeclaration (KerML 8.2.4.3.1) can start with `kind`.
pub(super) fn at_feature_declaration(kind: K) -> bool {
    is_name(kind)
        || matches!(
            kind,
            K::AllKw
                | K::Lt
                | K::Colon
                | K::TypedKw
                | K::ColonGt
                | K::SubsetsKw
                | K::ColonColonGt
                | K::ReferencesKw
                | K::FatArrow
                | K::CrossesKw
                | K::ColonGtGt
                | K::RedefinesKw
                | K::LBracket
                | K::OrderedKw
                | K::NonuniqueKw
                | K::Tilde
                | K::ConjugatesKw
        )
}

/// Feature (KerML 8.2.4.3.1):
///
/// ```text
/// ( FeaturePrefix ( 'feature' | PrefixMetadataMember ) FeatureDeclaration?
/// | ( EndFeaturePrefix | BasicFeaturePrefix ) FeatureDeclaration )
/// ValuePart? TypeBody
/// ```
///
/// where FeaturePrefix is `( EndFeaturePrefix OwnedCrossFeatureMember? | BasicFeaturePrefix )
/// PrefixMetadataMember*`. Also the `member` of a TypeFeatureMember (8.2.4.1.6), the `return` of
/// a ReturnFeatureMember (8.2.5.7.1), and the kernel features that follow a FeaturePrefix
/// (8.2.5.5–8.2.5.9).
pub(super) fn feature(p: &mut Parser, m: Marker, context: Context) {
    if p.at(K::MemberKw) {
        if !matches!(context, Context::Type | Context::Function) {
            p.error(
                "syntax::misplaced-prefix",
                "`member` is only allowed in the body of a type",
                Some("KerML 8.2.4.1.6"),
            );
        }
        p.bump();
    } else if p.at(K::ReturnKw) {
        if context != Context::Function {
            p.error(
                "syntax::misplaced-prefix",
                "`return` is only allowed in the body of a function or expression",
                Some("KerML 8.2.5.7.1"),
            );
        }
        p.bump();
    }
    if p.at(K::EndKw) || (p.at(K::ConstKw) && p.nth_at(1, K::EndKw)) {
        // EndFeaturePrefix: `( 'const' )? 'end'`.
        p.eat(K::ConstKw);
        p.bump();
        if !p.at(K::FeatureKw) && !p.at(K::Hash) && !p.at_any(kernel::KERNEL_FEATURES) {
            // Either the FeatureDeclaration of the end feature, or an OwnedCrossFeature
            // (`BasicFeaturePrefix FeatureDeclaration`) if `feature` or `#` follows.
            let cross = p.start();
            let basic = basic_feature_prefix(p);
            feature_declaration(p, true);
            if p.at(K::FeatureKw) || p.at(K::Hash) {
                cross.complete(p, K::CrossFeature);
            } else {
                if basic {
                    p.error(
                        "syntax::expected-token",
                        "expected `feature`: after `end`, a feature prefix belongs to a cross feature",
                        Some("KerML 8.2.4.3.1"),
                    );
                }
                cross.abandon(p);
                value_and_body(p, m);
                return;
            }
        }
    } else {
        basic_feature_prefix(p);
    }
    let metadata = kernel::prefix_metadata_list(p);
    misplaced_prefixes(p, "KerML 8.2.4.3.1");
    if p.at_any(kernel::KERNEL_FEATURES) {
        kernel::kernel_feature(p, m);
        return;
    }
    if p.eat(K::FeatureKw) || metadata {
        if at_feature_declaration(p.current()) {
            feature_declaration(p, false);
        }
    } else {
        feature_declaration(p, true);
    }
    value_and_body(p, m);
}

/// `ValuePart? TypeBody` (KerML 8.2.4.3.1), completing the feature.
fn value_and_body(p: &mut Parser, m: Marker) {
    kernel::feature_value(p);
    body(p, Context::Type, "KerML 8.2.4.3.1");
    m.complete(p, K::Feature);
}

/// BasicFeaturePrefix (KerML 8.2.4.3.1): `FeatureDirection? 'derived'? 'abstract'?
/// ( 'composite' | 'portion' )? ( 'var' | 'const' )?`. Returns whether there was any.
fn basic_feature_prefix(p: &mut Parser) -> bool {
    let mut any = false;
    if p.at(K::InKw) || p.at(K::OutKw) || p.at(K::InoutKw) {
        p.bump();
        any = true;
    }
    for group in [
        &[K::DerivedKw][..],
        &[K::AbstractKw],
        &[K::CompositeKw, K::PortionKw],
        &[K::VarKw, K::ConstKw],
    ] {
        if p.at_any(group) {
            p.bump();
            any = true;
        }
    }
    any
}

/// FeatureDeclaration (KerML 8.2.4.3.1): `'all'? ( FeatureIdentification
/// ( FeatureSpecializationPart | ConjugationPart )? | FeatureSpecializationPart | ConjugationPart )
/// FeatureRelationshipPart*`.
pub(super) fn feature_declaration(p: &mut Parser, required: bool) {
    if !at_feature_declaration(p.current()) {
        if required {
            p.error(
                "syntax::expected-feature-declaration",
                "expected a feature name, typing, subsetting, redefinition or multiplicity",
                Some("KerML 8.2.4.3.1"),
            );
        }
        return;
    }
    p.eat(K::AllKw);
    let named = p.at(K::Lt) || is_name(p.current());
    identification(p);
    let specialized = if p.at(K::Tilde) || p.at(K::ConjugatesKw) {
        p.bump();
        owned(p, K::OwnedConjugation, true, "KerML 8.2.4.1.3");
        true
    } else {
        feature_specialization_part(p)
    };
    if !named && !specialized {
        p.error(
            "syntax::expected-feature-declaration",
            "expected a feature name, typing, subsetting, redefinition or multiplicity",
            Some("KerML 8.2.4.3.1"),
        );
    }
    relationship_parts(p, true);
}

/// FeatureSpecializationPart (KerML 8.2.4.3.1): FeatureSpecializations with at most one
/// MultiplicityPart among them. Returns whether there was any.
pub(super) fn feature_specialization_part(p: &mut Parser) -> bool {
    let mut any = false;
    let mut multiplicity = false;
    loop {
        match p.current() {
            // Typings: `TYPED_BY OwnedFeatureTyping ( ',' OwnedFeatureTyping )*`.
            K::Colon | K::TypedKw => {
                eat_typed_by(p);
                owned_list(p, K::OwnedFeatureTyping, true, "KerML 8.2.4.3.2");
            }
            // Subsettings: `SUBSETS OwnedSubsetting ( ',' OwnedSubsetting )*`.
            K::ColonGt | K::SubsetsKw => {
                p.bump();
                owned_list(p, K::OwnedSubsetting, true, "KerML 8.2.4.3.3");
            }
            // References: `REFERENCES OwnedReferenceSubsetting`.
            K::ColonColonGt | K::ReferencesKw => {
                p.bump();
                owned(p, K::OwnedReferenceSubsetting, true, "KerML 8.2.4.3.3");
            }
            // Crosses: `CROSSES OwnedCrossSubsetting`.
            K::FatArrow | K::CrossesKw => {
                p.bump();
                owned(p, K::OwnedCrossSubsetting, true, "KerML 8.2.4.3.3");
            }
            // Redefinitions: `REDEFINES OwnedRedefinition ( ',' OwnedRedefinition )*`.
            K::ColonGtGt | K::RedefinesKw => {
                p.bump();
                owned_list(p, K::OwnedRedefinition, true, "KerML 8.2.4.3.4");
            }
            K::LBracket | K::OrderedKw | K::NonuniqueKw => {
                if multiplicity {
                    p.error(
                        "syntax::duplicate-multiplicity",
                        "a feature has at most one multiplicity part",
                        Some("KerML 8.2.4.3.1"),
                    );
                }
                multiplicity_part(p);
                multiplicity = true;
            }
            _ => return any,
        }
        any = true;
    }
}

/// MultiplicityPart (KerML 8.2.4.3.1): `OwnedMultiplicity | OwnedMultiplicity?
/// ( 'ordered' 'nonunique'? | 'nonunique' 'ordered'? )`.
fn multiplicity_part(p: &mut Parser) {
    if p.at(K::LBracket) {
        kernel::multiplicity_bounds(p);
    }
    if p.eat(K::OrderedKw) {
        p.eat(K::NonuniqueKw);
    } else if p.eat(K::NonuniqueKw) {
        p.eat(K::OrderedKw);
    }
}

/// Specialization (KerML 8.2.4.1.2), Subclassification (8.2.4.2.2), FeatureTyping (8.2.4.3.2),
/// Subsetting (8.2.4.3.3) and Redefinition (8.2.4.3.4), which share
/// `( 'specialization' Identification )?` and differ in the keyword after it.
pub(super) fn specialization(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.4.1.2");
    if p.eat(K::SpecializationKw) {
        identification(p);
    }
    let kind = match p.current() {
        K::SubtypeKw => {
            p.bump();
            qualified_name_or_chain(p, "KerML 8.2.4.1.2");
            expect_either(p, K::ColonGt, K::SpecializesKw, "KerML 8.2.4.1.2");
            qualified_name_or_chain(p, "KerML 8.2.4.1.2");
            K::Specialization
        }
        K::SubclassifierKw => {
            p.bump();
            qualified_name(p, "KerML 8.2.4.2.2");
            expect_either(p, K::ColonGt, K::SpecializesKw, "KerML 8.2.4.2.2");
            qualified_name(p, "KerML 8.2.4.2.2");
            K::Subclassification
        }
        K::TypingKw => {
            p.bump();
            qualified_name(p, "KerML 8.2.4.3.2");
            if !eat_typed_by(p) {
                expect_either(p, K::Colon, K::TypedKw, "KerML 8.2.4.3.2");
            }
            qualified_name_or_chain(p, "KerML 8.2.4.3.2");
            K::FeatureTyping
        }
        K::SubsetKw => {
            p.bump();
            qualified_name_or_chain(p, "KerML 8.2.4.3.3");
            expect_either(p, K::ColonGt, K::SubsetsKw, "KerML 8.2.4.3.3");
            qualified_name_or_chain(p, "KerML 8.2.4.3.3");
            K::Subsetting
        }
        K::RedefinitionKw => {
            p.bump();
            qualified_name_or_chain(p, "KerML 8.2.4.3.4");
            expect_either(p, K::ColonGtGt, K::RedefinesKw, "KerML 8.2.4.3.4");
            qualified_name_or_chain(p, "KerML 8.2.4.3.4");
            K::Redefinition
        }
        _ => {
            p.error(
                "syntax::expected-token",
                "expected `subtype`, `subclassifier`, `typing`, `subset` or `redefinition`",
                Some("KerML 8.2.4.1.2"),
            );
            K::Specialization
        }
    };
    body(p, Context::Relationship, "KerML 8.2.4.1.2");
    m.complete(p, kind);
}

/// Conjugation (KerML 8.2.4.1.3): `( 'conjugation' Identification )? 'conjugate'
/// ( [QualifiedName] | FeatureChain ) CONJUGATES ( [QualifiedName] | FeatureChain )
/// RelationshipBody`.
pub(super) fn conjugation(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.4.1.3");
    if p.eat(K::ConjugationKw) {
        identification(p);
    }
    p.expect(K::ConjugateKw, "syntax::expected-token", "KerML 8.2.4.1.3");
    qualified_name_or_chain(p, "KerML 8.2.4.1.3");
    expect_either(p, K::Tilde, K::ConjugatesKw, "KerML 8.2.4.1.3");
    qualified_name_or_chain(p, "KerML 8.2.4.1.3");
    body(p, Context::Relationship, "KerML 8.2.4.1.3");
    m.complete(p, K::Conjugation);
}

/// Disjoining (KerML 8.2.4.1.4): `( 'disjoining' Identification )? 'disjoint'
/// ( [QualifiedName] | FeatureChain ) 'from' ( [QualifiedName] | FeatureChain ) RelationshipBody`.
pub(super) fn disjoining(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.4.1.4");
    if p.eat(K::DisjoiningKw) {
        identification(p);
    }
    p.expect(K::DisjointKw, "syntax::expected-token", "KerML 8.2.4.1.4");
    qualified_name_or_chain(p, "KerML 8.2.4.1.4");
    p.expect(K::FromKw, "syntax::expected-token", "KerML 8.2.4.1.4");
    qualified_name_or_chain(p, "KerML 8.2.4.1.4");
    body(p, Context::Relationship, "KerML 8.2.4.1.4");
    m.complete(p, K::Disjoining);
}

/// FeatureInverting (KerML 8.2.4.3.6): `( 'inverting' Identification? )? 'inverse'
/// ( [QualifiedName] | OwnedFeatureChain ) 'of' ( [QualifiedName] | OwnedFeatureChain )
/// RelationshipBody`.
pub(super) fn feature_inverting(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.4.3.6");
    if p.eat(K::InvertingKw) {
        identification(p);
    }
    p.expect(K::InverseKw, "syntax::expected-token", "KerML 8.2.4.3.6");
    qualified_name_or_chain(p, "KerML 8.2.4.3.6");
    p.expect(K::OfKw, "syntax::expected-token", "KerML 8.2.4.3.6");
    qualified_name_or_chain(p, "KerML 8.2.4.3.6");
    body(p, Context::Relationship, "KerML 8.2.4.3.6");
    m.complete(p, K::FeatureInverting);
}

/// TypeFeaturing (KerML 8.2.4.3.7): `'featuring' ( Identification 'of' )? [QualifiedName]
/// 'by' [QualifiedName] RelationshipBody`.
pub(super) fn type_featuring(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.4.3.7");
    p.bump(); // `featuring`
              // `of` decides: `featuring f by T` has no identification, `featuring t of f by T` has.
    if p.at(K::Lt) || p.at(K::OfKw) || (is_name(p.current()) && p.nth_at(1, K::OfKw)) {
        identification(p);
        p.expect(K::OfKw, "syntax::expected-token", "KerML 8.2.4.3.7");
    }
    qualified_name(p, "KerML 8.2.4.3.7");
    p.expect(K::ByKw, "syntax::expected-token", "KerML 8.2.4.3.7");
    qualified_name(p, "KerML 8.2.4.3.7");
    body(p, Context::Relationship, "KerML 8.2.4.3.7");
    m.complete(p, K::TypeFeaturing);
}
