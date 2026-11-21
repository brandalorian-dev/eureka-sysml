// SPDX-License-Identifier: Apache-2.0
//! Identifications, qualified names, feature chains and bodies.

use super::{at_qualified_name, is_name, root, Context};
use crate::kinds::SyntaxKind as K;
use crate::parser::{CompletedMarker, Parser};

/// Identification (KerML 8.2.3.1): `( '<' declaredShortName = NAME '>' )? ( declaredName = NAME )?`.
/// Both parts are optional, so an empty identification makes no node. The same shape is the
/// `FeatureIdentification` of a feature (8.2.4.3.1) and the names of an alias (8.2.3.4.1).
pub(super) fn identification(p: &mut Parser) {
    if !p.at(K::Lt) && !is_name(p.current()) {
        return;
    }
    let m = p.start();
    if p.at(K::Lt) {
        let s = p.start();
        p.bump();
        name(p, "KerML 8.2.3.1");
        p.expect(K::Gt, "syntax::expected-token", "KerML 8.2.3.1");
        s.complete(p, K::ShortName);
    }
    if is_name(p.current()) {
        p.bump();
    }
    m.complete(p, K::Identification);
}

/// NAME (KerML 8.2.2.3), or an error.
pub(super) fn name(p: &mut Parser, clause: &'static str) -> bool {
    if is_name(p.current()) {
        p.bump();
        return true;
    }
    p.error("syntax::expected-name", "expected a name", Some(clause));
    false
}

/// QualifiedName (KerML 8.2.3.4.1): `( '$' '::' )? ( NAME '::' )* NAME`. A `::` followed by `*` or
/// `**` is left to the import (8.2.3.4.2).
pub(super) fn qualified_name(p: &mut Parser, clause: &'static str) -> Option<CompletedMarker> {
    if !at_qualified_name(p, 0) {
        p.error(
            "syntax::expected-qualified-name",
            "expected a qualified name",
            Some(clause),
        );
        return None;
    }
    let m = p.start();
    if p.eat(K::Dollar) {
        p.expect(K::ColonColon, "syntax::expected-token", "KerML 8.2.3.4.1");
    }
    name(p, "KerML 8.2.3.4.1");
    while p.at(K::ColonColon) && is_name(p.nth(1)) {
        p.bump();
        p.bump();
    }
    Some(m.complete(p, K::QualifiedName))
}

/// `[QualifiedName] | FeatureChain`, the two forms of a general type and of most relationship
/// ends (KerML 8.2.4.1.2–8.2.4.1.5, 8.2.4.3). FeatureChain (8.2.4.3.5) is
/// `OwnedFeatureChaining ( '.' OwnedFeatureChaining )+`, each chaining a qualified name.
pub(super) fn qualified_name_or_chain(p: &mut Parser, clause: &'static str) {
    let Some(first) = qualified_name(p, clause) else {
        return;
    };
    if !(p.at(K::Dot) && at_qualified_name(p, 1)) {
        return;
    }
    let chain = first.precede(p);
    while p.at(K::Dot) && at_qualified_name(p, 1) {
        p.bump();
        qualified_name(p, "KerML 8.2.4.3.5");
    }
    chain.complete(p, K::FeatureChain);
}

/// An owned relationship whose only content is its target, `[QualifiedName] | FeatureChain`,
/// for example OwnedSpecialization (KerML 8.2.4.1.2). Reports an error, and makes no node, if
/// there is no target.
pub(super) fn owned(p: &mut Parser, kind: K, chain: bool, clause: &'static str) {
    if !at_qualified_name(p, 0) {
        p.error(
            "syntax::expected-qualified-name",
            "expected a qualified name",
            Some(clause),
        );
        return;
    }
    let m = p.start();
    if chain {
        qualified_name_or_chain(p, clause);
    } else {
        qualified_name(p, clause);
    }
    m.complete(p, kind);
}

/// `owned ( ',' owned )*`: the list after a keyword such as `specializes` (KerML 8.2.4).
pub(super) fn owned_list(p: &mut Parser, kind: K, chain: bool, clause: &'static str) {
    owned(p, kind, chain, clause);
    while p.eat(K::Comma) {
        owned(p, kind, chain, clause);
    }
}

/// `';' | '{' Element* '}'`: RelationshipBody (KerML 8.2.3.1), NamespaceBody (8.2.3.4.1), TypeBody
/// (8.2.4.1.1) and PackageBody (8.2.5.13), which differ in the members they allow (`context`).
pub(super) fn body(p: &mut Parser, context: Context, clause: &'static str) {
    if p.at(K::Semicolon) {
        let m = p.start();
        p.bump();
        m.complete(p, K::Body);
        return;
    }
    if !p.at(K::LBrace) {
        p.error("syntax::expected-body", "expected `;` or `{`", Some(clause));
        // Unexpected tokens up to the next member are part of this error, not another one.
        if !p.at_eof() && !p.at(K::RBrace) && !root::at_member_start(p) {
            let m = p.start();
            root::recover(p, m);
        }
        return;
    }
    let m = p.start();
    p.bump();
    root::members(p, context);
    p.expect(K::RBrace, "syntax::expected-token", clause);
    m.complete(p, K::Body);
}
