// SPDX-License-Identifier: Apache-2.0
//! The Root layer (KerML 8.2.3), and the namespace members, which every body shares.

use super::names::{body, identification, qualified_name};
use super::{at_qualified_name, core, is_name, kernel, Context, PREFIX, VISIBILITY};
use crate::kinds::SyntaxKind as K;
use crate::parser::{Marker, Parser};

/// RootNamespace (KerML 8.2.3.4.1): `NamespaceBodyElement*`.
pub(crate) fn root_namespace(p: &mut Parser) {
    let root = p.start();
    while !p.at_eof() {
        if p.at(K::RBrace) {
            // A `}` with no `{`: report it, and go on with the next member.
            p.error(
                "syntax::unexpected-token",
                "unexpected `}`",
                Some("KerML 8.2.3.4.1"),
            );
            let m = p.start();
            p.bump();
            m.complete(p, K::Error);
        } else {
            member(p, Context::Namespace);
        }
    }
    root.complete(p, K::RootNamespace);
}

/// The members of a body, up to its `}`.
pub(super) fn members(p: &mut Parser, context: Context) {
    while !p.at_eof() && !p.at(K::RBrace) {
        member(p, context);
    }
}

/// The keyword (or other token) that decides what the member at the current token is, after its
/// visibility, its prefix keywords and its prefix metadata (`#` and a qualified name or chain).
fn deciding_token(p: &Parser) -> (K, bool) {
    let mut n = usize::from(p.at_any(VISIBILITY));
    let mut prefixed = false;
    loop {
        let k = p.nth(n);
        if PREFIX.contains(&k) {
            n += 1;
        } else if k == K::Hash {
            n += 1;
            if p.nth_at(n, K::Dollar) {
                n += 2;
            }
            if is_name(p.nth(n)) {
                n += 1;
            }
            while (p.nth_at(n, K::ColonColon) || p.nth_at(n, K::Dot)) && is_name(p.nth(n + 1)) {
                n += 2;
            }
        } else {
            return (k, prefixed);
        }
        prefixed = true;
    }
}

/// The production a member is, from the token that decides it ([`deciding_token`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Production {
    Namespace,
    Package,
    Dependency,
    Comment,
    Documentation,
    TextualRepresentation,
    Alias,
    Import,
    Type,
    Classifier,
    Specialization,
    Conjugation,
    Disjoining,
    FeatureInverting,
    TypeFeaturing,
    Multiplicity,
    /// A feature that starts with `feature` or a prefix.
    Feature,
    /// A feature that starts with its declaration: a name, `:`, `[`, `ordered` …
    FeatureDeclaration,
    /// A Kernel construct that step 1a.3 does not parse yet.
    NotYet,
}

/// The production that `deciding` starts, if any. Both [`member`] and [`at_member_start`] use it,
/// so that recovery stops at every member the parser can parse.
fn production(deciding: K, prefixed: bool) -> Option<Production> {
    use Production as P;
    Some(match deciding {
        K::NamespaceKw => P::Namespace,
        K::PackageKw | K::StandardKw | K::LibraryKw => P::Package,
        K::DependencyKw => P::Dependency,
        K::CommentKw | K::LocaleKw | K::RegularComment => P::Comment,
        K::DocKw => P::Documentation,
        K::RepKw | K::LanguageKw => P::TextualRepresentation,
        K::AliasKw => P::Alias,
        K::ImportKw => P::Import,
        K::TypeKw => P::Type,
        K::ClassifierKw => P::Classifier,
        K::SpecializationKw
        | K::SubtypeKw
        | K::SubclassifierKw
        | K::TypingKw
        | K::SubsetKw
        | K::RedefinitionKw => P::Specialization,
        K::ConjugationKw | K::ConjugateKw => P::Conjugation,
        K::DisjoiningKw | K::DisjointKw => P::Disjoining,
        K::InvertingKw | K::InverseKw => P::FeatureInverting,
        K::FeaturingKw => P::TypeFeaturing,
        K::MultiplicityKw => P::Multiplicity,
        K::FeatureKw => P::Feature,
        k if core::at_feature_declaration(k) => P::FeatureDeclaration,
        // `#M;` or `in;`: prefixes with no keyword are a feature (8.2.4.3.1).
        K::Semicolon | K::LBrace | K::Eq | K::ColonEq | K::DefaultKw if prefixed => P::Feature,
        k if kernel::is_not_yet(k) => P::NotYet,
        _ => return None,
    })
}

/// A member of a body: NamespaceBodyElement (KerML 8.2.3.4.1), TypeBodyElement (8.2.4.1.1),
/// a PackageBody element (8.2.5.13) or RelationshipOwnedElement (8.2.3.1). Consumes a token
/// unless it is at the `}` that closes the body.
fn member(p: &mut Parser, context: Context) {
    use Production as P;
    let m = p.start();
    // MemberPrefix (8.2.3.4.1).
    let visible = p.at_any(VISIBILITY);
    if visible {
        if context == Context::Relationship {
            p.error(
                "syntax::misplaced-visibility",
                "an element in a relationship body has no visibility",
                Some("KerML 8.2.3.1"),
            );
        }
        p.bump();
    }
    let (deciding, prefixed) = deciding_token(p);
    match production(deciding, prefixed) {
        Some(P::Namespace) => namespace(p, m),
        Some(P::Package) => kernel::package(p, m),
        Some(P::Dependency) => dependency(p, m),
        Some(P::Comment) => comment(p, m),
        Some(P::Documentation) => documentation(p, m),
        Some(P::TextualRepresentation) => textual_representation(p, m),
        Some(P::Alias | P::Import) if context == Context::Relationship => {
            p.error(
                "syntax::misplaced-member",
                format!(
                    "a relationship body cannot have {}",
                    if deciding == K::AliasKw {
                        "an alias"
                    } else {
                        "an import"
                    }
                ),
                Some("KerML 8.2.3.1"),
            );
            recover(p, m);
        }
        Some(P::Alias) => alias_member(p, m),
        Some(P::Import) => import(p, m, visible),
        Some(P::Type) => core::type_(p, m),
        Some(P::Classifier) => core::classifier(p, m),
        Some(P::Specialization) => core::specialization(p, m),
        Some(P::Conjugation) => core::conjugation(p, m),
        Some(P::Disjoining) => core::disjoining(p, m),
        Some(P::FeatureInverting) => core::feature_inverting(p, m),
        Some(P::TypeFeaturing) => core::type_featuring(p, m),
        Some(P::Multiplicity) => kernel::multiplicity(p, m),
        Some(P::Feature | P::FeatureDeclaration) => core::feature(p, m, context),
        Some(P::NotYet) => kernel::not_yet(p, m, deciding),
        None => {
            p.error(
                "syntax::expected-member",
                "expected a namespace member",
                Some("KerML 8.2.3.4.1"),
            );
            recover(p, m);
        }
    }
}

/// Whether a member can start at the current token: where recovery stops. A member that starts
/// with its feature declaration (a name, `:`, `[` …) is not a stop, since those tokens are as
/// likely to be the rest of a broken member.
pub(super) fn at_member_start(p: &Parser) -> bool {
    let k = p.current();
    VISIBILITY.contains(&k)
        || PREFIX.contains(&k)
        || k == K::Hash
        || !matches!(
            production(k, false),
            None | Some(Production::FeatureDeclaration)
        )
}

/// After an error at a member: put the member's tokens in an `Error` node, up to the start of the
/// next member, a `;` (included), or the `}` that closes the body (never included, also when it
/// is the first token: the body owns it). Braces are skipped in pairs.
pub(super) fn recover(p: &mut Parser, m: Marker) {
    let mut depth = 0u32;
    let mut first = true;
    while !p.at_eof() {
        if depth == 0 && (p.at(K::RBrace) || (!first && at_member_start(p))) {
            break;
        }
        first = false;
        match p.current() {
            K::LBrace => depth += 1,
            K::RBrace => depth = depth.saturating_sub(1),
            K::Semicolon if depth == 0 => {
                p.bump();
                break;
            }
            _ => {}
        }
        p.bump();
    }
    m.complete(p, K::Error);
}

/// Prefix keywords or prefix metadata that the element at hand does not allow, or not in this
/// order: one error, and the tokens in an `Error` node.
pub(super) fn misplaced_prefixes(p: &mut Parser, clause: &'static str) {
    if !(p.at_any(PREFIX) || p.at(K::Hash) || p.at_any(VISIBILITY)) {
        return;
    }
    let text = p.current().text().unwrap_or("");
    p.error(
        "syntax::misplaced-prefix",
        format!("`{text}` is not allowed here"),
        Some(clause),
    );
    let m = p.start();
    while p.at_any(PREFIX) || p.at(K::Hash) || p.at_any(VISIBILITY) {
        if p.at(K::Hash) {
            kernel::prefix_metadata(p);
        } else {
            p.bump();
        }
    }
    m.complete(p, K::Error);
}

/// Namespace (KerML 8.2.3.4.1):
/// `PrefixMetadataMember* 'namespace' Identification NamespaceBody`.
fn namespace(p: &mut Parser, m: Marker) {
    kernel::prefix_metadata_list(p);
    misplaced_prefixes(p, "KerML 8.2.3.4.1");
    p.bump(); // `namespace`
    identification(p);
    body(p, Context::Namespace, "KerML 8.2.3.4.1");
    m.complete(p, K::Namespace);
}

/// Dependency (KerML 8.2.3.2): `PrefixMetadataAnnotation* 'dependency' ( Identification? 'from' )?
/// client ( ',' client )* 'to' supplier ( ',' supplier )* RelationshipBody`.
fn dependency(p: &mut Parser, m: Marker) {
    kernel::prefix_metadata_list(p);
    misplaced_prefixes(p, "KerML 8.2.3.2");
    p.bump(); // `dependency`
              // `from` decides: `dependency A to B` has no identification, `dependency d from A to B` has.
    if p.at(K::Lt) || p.at(K::FromKw) || (is_name(p.current()) && p.nth_at(1, K::FromKw)) {
        identification(p);
        p.expect(K::FromKw, "syntax::expected-token", "KerML 8.2.3.2");
    }
    qualified_names(p, "KerML 8.2.3.2");
    p.expect(K::ToKw, "syntax::expected-token", "KerML 8.2.3.2");
    qualified_names(p, "KerML 8.2.3.2");
    body(p, Context::Relationship, "KerML 8.2.3.2");
    m.complete(p, K::Dependency);
}

/// `[QualifiedName] ( ',' [QualifiedName] )*`.
fn qualified_names(p: &mut Parser, clause: &'static str) {
    qualified_name(p, clause);
    while p.eat(K::Comma) {
        qualified_name(p, clause);
    }
}

/// `( 'locale' locale = STRING_VALUE )? body = REGULAR_COMMENT`, the end of a comment,
/// documentation or textual representation (KerML 8.2.3.3.2, 8.2.3.3.3).
fn comment_body(p: &mut Parser, locale: bool, clause: &'static str) {
    if locale && p.eat(K::LocaleKw) {
        p.expect(K::StringValue, "syntax::expected-token", clause);
    }
    p.expect(K::RegularComment, "syntax::expected-comment", clause);
}

/// Comment (KerML 8.2.3.3.2): `( 'comment' Identification ( 'about' Annotation
/// ( ',' Annotation )* )? )? ( 'locale' STRING_VALUE )? REGULAR_COMMENT`.
fn comment(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.3.3.2");
    if p.eat(K::CommentKw) {
        identification(p);
        if p.eat(K::AboutKw) {
            annotation(p);
            while p.eat(K::Comma) {
                annotation(p);
            }
        }
    }
    comment_body(p, true, "KerML 8.2.3.3.2");
    m.complete(p, K::Comment);
}

/// Annotation (KerML 8.2.3.3.1): `annotatedElement = [QualifiedName]`.
pub(super) fn annotation(p: &mut Parser) {
    if !at_qualified_name(p, 0) {
        qualified_name(p, "KerML 8.2.3.3.1"); // reports the error
        return;
    }
    let m = p.start();
    qualified_name(p, "KerML 8.2.3.3.1");
    m.complete(p, K::Annotation);
}

/// Documentation (KerML 8.2.3.3.2):
/// `'doc' Identification ( 'locale' STRING_VALUE )? REGULAR_COMMENT`.
fn documentation(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.3.3.2");
    p.bump(); // `doc`
    identification(p);
    comment_body(p, true, "KerML 8.2.3.3.2");
    m.complete(p, K::Documentation);
}

/// TextualRepresentation (KerML 8.2.3.3.3):
/// `( 'rep' Identification )? 'language' STRING_VALUE REGULAR_COMMENT`.
fn textual_representation(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.3.3.3");
    if p.eat(K::RepKw) {
        identification(p);
    }
    p.expect(K::LanguageKw, "syntax::expected-token", "KerML 8.2.3.3.3");
    p.expect(K::StringValue, "syntax::expected-token", "KerML 8.2.3.3.3");
    comment_body(p, false, "KerML 8.2.3.3.3");
    m.complete(p, K::TextualRepresentation);
}

/// AliasMember (KerML 8.2.3.4.1): `MemberPrefix 'alias' ( '<' NAME '>' )? NAME?
/// 'for' [QualifiedName] RelationshipBody`.
fn alias_member(p: &mut Parser, m: Marker) {
    misplaced_prefixes(p, "KerML 8.2.3.4.1");
    p.bump(); // `alias`
    identification(p);
    p.expect(K::ForKw, "syntax::expected-token", "KerML 8.2.3.4.1");
    qualified_name(p, "KerML 8.2.3.4.1");
    body(p, Context::Relationship, "KerML 8.2.3.4.1");
    m.complete(p, K::AliasMember);
}

/// Import (KerML 8.2.3.4.2): `VisibilityIndicator 'import' 'all'? ImportDeclaration
/// RelationshipBody`, where ImportDeclaration is a MembershipImport `[QualifiedName] ( '::' '**' )?`
/// or a NamespaceImport `[QualifiedName] '::' '*' ( '::' '**' )?`, either optionally followed by
/// filter conditions (FilterPackage, `[ … ]`).
///
/// The visibility is required by the grammar, although an example in KerML 7.2.5 omits it
/// (ADR 0014): without one the import is reported, and still parsed.
fn import(p: &mut Parser, m: Marker, visible: bool) {
    misplaced_prefixes(p, "KerML 8.2.3.4.2");
    if !visible {
        p.error(
            "syntax::import-without-visibility",
            "an import needs a visibility: `public`, `private` or `protected`",
            Some("KerML 8.2.3.4.2"),
        );
    }
    p.bump(); // `import`
    p.eat(K::AllKw);
    qualified_name(p, "KerML 8.2.3.4.2");
    if p.at(K::ColonColon) && p.nth_at(1, K::Star) {
        p.bump();
        p.bump();
    }
    if p.at(K::ColonColon) && p.nth_at(1, K::StarStar) {
        p.bump();
        p.bump();
    }
    // FilterPackageMember (8.2.3.4.2): `'[' OwnedExpression ']'`, an expression (step 1a.4).
    while p.at(K::LBracket) {
        kernel::not_yet_bracketed(p, "filter conditions are", "KerML 8.2.3.4.2");
    }
    body(p, Context::Relationship, "KerML 8.2.3.4.2");
    m.complete(p, K::Import);
}
