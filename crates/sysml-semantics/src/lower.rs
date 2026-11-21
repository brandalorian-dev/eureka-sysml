// SPDX-License-Identifier: Apache-2.0
//! The lowering from a syntax tree to model elements (step 1a.6): the mapping each production of
//! KerML 8.2.3–8.2.5 states in its abstract syntax synthesis notation (KerML 8.2.1, Table 3),
//! into a per-file fragment of the element graph (ADR 0017). Names are not resolved here: every
//! reference is an [`Endpoint::Name`] until step 1a.7. ADR 0018 records the readings.
//!
//! The tree has a node per element and per owned relationship (ADR 0011), in source order, and
//! `ownedRelationship` is ordered by source position, so the lowering walks each node's children
//! in order.

use sysml_model::{
    Attr, ElementIx, Endpoint, FileId, Graph, Metaclass as M, QualifiedName, Slot, Span, Value,
};
use sysml_syntax::{Diagnostic, SyntaxElement, SyntaxKind as K, SyntaxNode, SyntaxToken};
use uuid::Uuid;

/// The elements lowered from one file.
pub struct Lowered {
    /// The fragment: the root namespace and everything it owns.
    pub graph: Graph,
    /// The root namespace (KerML 8.2.3.4.1).
    pub root: ElementIx,
    /// Problems the syntax does not show, such as a literal too large for its type.
    pub diagnostics: Vec<Diagnostic>,
}

/// Lower the tree of one file. `ids` gives the `DataIdentity` id of each new element, in creation
/// order (ADR 0017): random v4 ids for user models, v5 ids for the libraries (step 1a.11).
pub fn lower(file: FileId, root: &SyntaxNode, ids: &mut dyn FnMut() -> Uuid) -> Lowered {
    let mut l = Lowerer {
        g: Graph::new(),
        file,
        ids,
        diagnostics: Vec::new(),
    };
    // RootNamespace : Namespace = NamespaceBodyElement* (KerML 8.2.3.4.1)
    let ns = l.element(M::Namespace, root);
    l.members(ns, root, Context::Namespace);
    Lowered {
        graph: l.g,
        root: ns,
        diagnostics: l.diagnostics,
    }
}

/// Which members a body holds, which decides their memberships.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Context {
    /// A namespace or package body: every member is an OwningMembership (KerML 8.2.3.4.1).
    Namespace,
    /// A type body: features are FeatureMemberships (KerML 8.2.4.1.6).
    Type,
    /// A function body: a type body with `return` features and a result expression (8.2.5.7.1).
    Function,
    /// A metadata body: its features are MetadataBodyFeatures (KerML 8.2.5.12).
    Metadata,
    /// A relationship body: owned related elements and annotations (KerML 8.2.3.1).
    Relationship,
}

struct Lowerer<'a> {
    g: Graph,
    file: FileId,
    ids: &'a mut dyn FnMut() -> Uuid,
    diagnostics: Vec<Diagnostic>,
}

// ---- the syntax tree ----

fn nodes(n: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    n.children()
}

fn tokens(n: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    n.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|t| !t.kind().is_trivia())
}

fn has(n: &SyntaxNode, kind: K) -> bool {
    tokens(n).any(|t| t.kind() == kind)
}

fn child(n: &SyntaxNode, kind: K) -> Option<SyntaxNode> {
    nodes(n).find(|c| c.kind() == kind)
}

/// Whether a node kind is an expression (KerML 8.2.5.8).
fn is_expression(kind: K) -> bool {
    matches!(
        kind,
        K::ConditionalExpression
            | K::BinaryExpression
            | K::UnaryExpression
            | K::ClassificationExpression
            | K::ExtentExpression
            | K::FeatureChainExpression
            | K::CollectExpression
            | K::SelectExpression
            | K::FunctionOperationExpression
            | K::BracketExpression
            | K::IndexExpression
            | K::SequenceExpression
            | K::NullExpression
            | K::MetadataAccessExpression
            | K::InvocationExpression
            | K::ConstructorExpression
            | K::BodyExpression
            | K::FeatureReferenceExpression
            | K::LiteralBoolean
            | K::LiteralString
            | K::LiteralInteger
            | K::LiteralReal
            | K::LiteralInfinity
    )
}

fn expressions(n: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    nodes(n).filter(|c| is_expression(c.kind()))
}

/// The metaclass of a classifier-like definition or other element node.
fn element_metaclass(kind: K) -> Option<M> {
    Some(match kind {
        K::Namespace => M::Namespace,
        K::Package => M::Package,
        K::LibraryPackage => M::LibraryPackage,
        K::Type => M::Type,
        K::Classifier => M::Classifier,
        K::DataType => M::DataType,
        K::Class => M::Class,
        K::Structure => M::Structure,
        K::Association => M::Association,
        K::AssociationStructure => M::AssociationStructure,
        K::Behavior => M::Behavior,
        K::Function => M::Function,
        K::Predicate => M::Predicate,
        K::Interaction => M::Interaction,
        K::Metaclass => M::Metaclass,
        K::Feature => M::Feature,
        K::Step => M::Step,
        K::Expression => M::Expression,
        K::BooleanExpression => M::BooleanExpression,
        K::Invariant => M::Invariant,
        K::Connector => M::Connector,
        K::BindingConnector => M::BindingConnector,
        K::Succession => M::Succession,
        K::Flow => M::Flow,
        K::SuccessionFlow => M::SuccessionFlow,
        K::Comment => M::Comment,
        K::Documentation => M::Documentation,
        K::TextualRepresentation => M::TextualRepresentation,
        K::MetadataFeature => M::MetadataFeature,
        K::Dependency => M::Dependency,
        K::Specialization => M::Specialization,
        K::Conjugation => M::Conjugation,
        K::Disjoining => M::Disjoining,
        K::Subclassification => M::Subclassification,
        K::FeatureTyping => M::FeatureTyping,
        K::Subsetting => M::Subsetting,
        K::Redefinition => M::Redefinition,
        K::FeatureInverting => M::FeatureInverting,
        K::TypeFeaturing => M::TypeFeaturing,
        K::MultiplicitySubset => M::Multiplicity,
        K::MultiplicityRange => M::MultiplicityRange,
        _ => return None,
    })
}

/// The features of KerML: the elements a type body owns through a FeatureMembership.
fn is_feature(kind: K) -> bool {
    matches!(
        kind,
        K::Feature
            | K::Step
            | K::Expression
            | K::BooleanExpression
            | K::Invariant
            | K::Connector
            | K::BindingConnector
            | K::Succession
            | K::Flow
            | K::SuccessionFlow
    )
}

/// The annotating elements (KerML 8.2.3.3.1).
fn is_annotating(kind: K) -> bool {
    matches!(
        kind,
        K::Comment | K::Documentation | K::TextualRepresentation | K::MetadataFeature
    )
}

/// The owned relationships of a declaration that are a relationship with its target (KerML 8.2.4).
fn owned_relationship(kind: K) -> Option<M> {
    Some(match kind {
        K::OwnedSpecialization => M::Specialization,
        K::OwnedSubclassification => M::Subclassification,
        K::OwnedConjugation => M::Conjugation,
        K::OwnedDisjoining => M::Disjoining,
        K::Unioning => M::Unioning,
        K::Intersecting => M::Intersecting,
        K::Differencing => M::Differencing,
        K::OwnedFeatureTyping => M::FeatureTyping,
        K::OwnedSubsetting => M::Subsetting,
        K::OwnedReferenceSubsetting => M::ReferenceSubsetting,
        K::OwnedCrossSubsetting => M::CrossSubsetting,
        K::OwnedRedefinition => M::Redefinition,
        K::OwnedFeatureInverting => M::FeatureInverting,
        K::OwnedTypeFeaturing => M::TypeFeaturing,
        _ => return None,
    })
}

/// Whether a metaclass stores its `source`: then the source of an owned relationship of that
/// metaclass is its owner (`specific`, `typedFeature` …); otherwise the source is derived
/// (KerML 8.3, ADR 0017).
fn stores_source(m: M) -> bool {
    m.stored_attributes().iter().any(|a| a.slot == Slot::Source)
}

// ---- lexical values (KerML 8.2.2) ----

/// The value of a NAME (KerML 8.2.2.3): an unrestricted name without its quotes and escapes.
fn name_value(t: &SyntaxToken) -> String {
    let text = t.text();
    if t.kind() == K::UnrestrictedName && text.len() >= 2 {
        unescape(&text[1..text.len() - 1])
    } else {
        text.to_owned()
    }
}

/// The value of a STRING_VALUE (KerML 8.2.2.5).
fn string_value(t: &SyntaxToken) -> String {
    let text = t.text();
    unescape(&text[1..text.len().saturating_sub(1).max(1)])
}

/// The escape sequences of KerML 8.2.2.5, Table 4.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('b') => out.push('\u{8}'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('f') => out.push('\u{c}'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// The body of a comment (KerML 8.2.3.3.2, Note 1): without `/*` and `*/`, without the white
/// space after `/*` up to and including the first line terminator, and on each line without its
/// leading white space, then a `*`, then one space.
fn comment_body(text: &str) -> String {
    let inner = text
        .strip_prefix("/*")
        .unwrap_or(text)
        .strip_suffix("*/")
        .unwrap_or(text);
    let mut rest = inner;
    let leading = rest.len() - rest.trim_start_matches([' ', '\t', '\u{c}']).len();
    rest = &rest[leading..];
    if let Some(r) = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .or_else(|| rest.strip_prefix('\r'))
    {
        rest = r;
    }
    let mut out = String::new();
    for line in rest.split_inclusive('\n') {
        let mut l = line.trim_start_matches([' ', '\t', '\u{c}']);
        if let Some(r) = l.strip_prefix('*') {
            l = r;
        }
        if let Some(r) = l.strip_prefix(' ') {
            l = r;
        }
        out.push_str(l);
    }
    out
}

impl Lowerer<'_> {
    fn span(&self, n: &SyntaxNode) -> Span {
        let r = n.text_range();
        Span {
            file: self.file,
            start: r.start().into(),
            end: r.end().into(),
        }
    }

    /// A new element for a node.
    fn element(&mut self, m: M, n: &SyntaxNode) -> ElementIx {
        let id = (self.ids)();
        let ix = self
            .g
            .add(m, id)
            .expect("the id source gave the same id twice");
        let span = self.span(n);
        self.g.set_span(ix, span);
        ix
    }

    /// A new relationship owned by `owner`.
    fn rel(&mut self, owner: ElementIx, m: M, n: &SyntaxNode) -> ElementIx {
        let r = self.element(m, n);
        self.g.add_owned_relationship(owner, r);
        r
    }

    /// A new element owned by the relationship `rel`.
    fn related(&mut self, rel: ElementIx, m: M, n: &SyntaxNode) -> ElementIx {
        let e = self.element(m, n);
        self.g.add_owned_related_element(rel, e);
        e
    }

    fn set_bool(&mut self, ix: ElementIx, attr: Attr, b: bool) {
        self.g.set(ix, attr, Value::Boolean(b));
    }

    fn set_string(&mut self, ix: ElementIx, attr: Attr, s: String) {
        self.g.set(ix, attr, Value::String(s.into()));
    }

    /// A qualified name (KerML 8.2.3.4.1) as an unresolved end.
    fn name(&mut self, qn: &SyntaxNode) -> Endpoint {
        let span = self.span(qn);
        let name = QualifiedName {
            global: has(qn, K::Dollar),
            segments: tokens(qn)
                .filter(|t| matches!(t.kind(), K::BasicName | K::UnrestrictedName))
                .map(|t| name_value(&t).into())
                .collect(),
            span: Some(span),
        };
        self.g.add_name(name)
    }

    /// `[QualifiedName] | OwnedFeatureChain` (KerML 8.2.4.1.2 GeneralType and its kin): a name,
    /// or a feature chain that `rel` owns as a related element, which the end then is.
    fn reference(&mut self, rel: ElementIx, n: &SyntaxNode) -> Option<Endpoint> {
        match n.kind() {
            K::QualifiedName => Some(self.name(n)),
            K::FeatureChain => {
                let chain = self.related(rel, M::Feature, n);
                self.chainings(chain, n);
                Some(Endpoint::Element(chain))
            }
            _ => None,
        }
    }

    /// The first name or chain among a node's children.
    fn reference_in(&mut self, rel: ElementIx, n: &SyntaxNode) -> Option<Endpoint> {
        let target = nodes(n).find(|c| matches!(c.kind(), K::QualifiedName | K::FeatureChain))?;
        self.reference(rel, &target)
    }

    /// FeatureChain (KerML 8.2.4.3.5): an OwnedFeatureChaining per qualified name.
    fn chainings(&mut self, feature: ElementIx, chain: &SyntaxNode) {
        for qn in nodes(chain).filter(|c| c.kind() == K::QualifiedName) {
            let c = self.rel(feature, M::FeatureChaining, &qn);
            let end = self.name(&qn);
            self.g.add_target(c, end);
        }
    }

    // ---- members (KerML 8.2.3.4, 8.2.4.1.6, 8.2.5.7.1) ----

    /// The members of a body node (or of the root), into `owner`.
    fn members(&mut self, owner: ElementIx, body: &SyntaxNode, ctx: Context) {
        for n in nodes(body) {
            self.member(owner, &n, ctx);
        }
    }

    fn member(&mut self, owner: ElementIx, n: &SyntaxNode, ctx: Context) {
        let kind = n.kind();
        match kind {
            K::AliasMember => self.alias(owner, n),
            K::Import => self.import(owner, n),
            K::ElementFilterMember => {
                // ElementFilterMember : ElementFilterMembership (KerML 8.2.5.13)
                let m = self.rel(owner, M::ElementFilterMembership, n);
                self.visibility(m, n);
                if let Some(e) = expressions(n).next() {
                    self.expression(m, &e);
                }
            }
            K::ResultExpressionMember => {
                // ResultExpressionMember : ResultExpressionMembership (KerML 8.2.5.7.1)
                let m = self.rel(owner, M::ResultExpressionMembership, n);
                self.visibility(m, n);
                if let Some(e) = expressions(n).next() {
                    self.expression(m, &e);
                }
            }
            _ if element_metaclass(kind).is_some() => {
                if ctx == Context::Relationship {
                    // RelationshipOwnedElement (KerML 8.2.3.1): an owned related element, or an
                    // annotating element through an OwnedAnnotation.
                    if is_annotating(kind) {
                        let a = self.rel(owner, M::Annotation, n);
                        self.lower_element(a, n, ctx);
                    } else {
                        self.lower_element(owner, n, ctx);
                    }
                    return;
                }
                let membership = if is_feature(kind) && ctx != Context::Namespace {
                    if has(n, K::MemberKw) {
                        M::OwningMembership // TypeFeatureMember (KerML 8.2.4.1.6)
                    } else if has(n, K::ReturnKw) {
                        M::ReturnParameterMembership // ReturnFeatureMember (KerML 8.2.5.7.1)
                    } else {
                        M::FeatureMembership // OwnedFeatureMember (KerML 8.2.4.1.6)
                    }
                } else if kind == K::Feature && ctx == Context::Metadata {
                    M::FeatureMembership // MetadataBodyFeatureMember (KerML 8.2.5.12)
                } else {
                    M::OwningMembership // NonFeatureMember, NamespaceFeatureMember (8.2.3.4.1)
                };
                let m = self.rel(owner, membership, n);
                self.visibility(m, n);
                let e = self.lower_element(m, n, ctx);
                if membership == M::ReturnParameterMembership
                    && self.g.values(e, Attr::Direction).next().is_none()
                {
                    // The parameter of a ReturnParameterMembership has direction `out`
                    // (KerML 8.3.4.7.4, validateParameterMembershipParameterDirection).
                    self.g.set(e, Attr::Direction, Value::Enumeration("out"));
                }
            }
            _ => {} // an Error node: the parser reported it
        }
    }

    /// MemberPrefix (KerML 8.2.3.4.1): `visibility = VisibilityIndicator`.
    fn visibility(&mut self, membership: ElementIx, n: &SyntaxNode) {
        let v = tokens(n).find_map(|t| match t.kind() {
            K::PublicKw => Some("public"),
            K::PrivateKw => Some("private"),
            K::ProtectedKw => Some("protected"),
            _ => None,
        });
        if let Some(v) = v {
            self.g
                .set(membership, Attr::Visibility, Value::Enumeration(v));
        }
    }

    /// AliasMember : Membership (KerML 8.2.3.4.1).
    fn alias(&mut self, owner: ElementIx, n: &SyntaxNode) {
        let m = self.rel(owner, M::Membership, n);
        self.visibility(m, n);
        if let Some(id) = child(n, K::Identification) {
            if let Some(short) = child(&id, K::ShortName) {
                if let Some(t) =
                    tokens(&short).find(|t| matches!(t.kind(), K::BasicName | K::UnrestrictedName))
                {
                    self.set_string(m, Attr::MemberShortName, name_value(&t));
                }
            }
            if let Some(t) =
                tokens(&id).find(|t| matches!(t.kind(), K::BasicName | K::UnrestrictedName))
            {
                self.set_string(m, Attr::MemberName, name_value(&t));
            }
        }
        if let Some(end) = self.reference_in(m, n) {
            self.g.add_target(m, end);
        }
        self.relationship_body(m, n);
    }

    /// Import (KerML 8.2.3.4.2): a MembershipImport, a NamespaceImport, or a NamespaceImport of a
    /// FilterPackage when filter conditions follow.
    fn import(&mut self, owner: ElementIx, n: &SyntaxNode) {
        let namespace = tokens(n).any(|t| t.kind() == K::Star);
        let declaration = if namespace {
            M::NamespaceImport
        } else {
            M::MembershipImport
        };
        let filters: Vec<SyntaxNode> = nodes(n)
            .filter(|c| c.kind() == K::FilterPackageMember)
            .collect();
        let outer = if filters.is_empty() {
            declaration
        } else {
            M::NamespaceImport
        };
        let i = self.rel(owner, outer, n);
        self.visibility(i, n);
        if has(n, K::AllKw) {
            self.set_bool(i, Attr::IsImportAll, true);
        }
        let inner = if filters.is_empty() {
            i
        } else {
            // FilterPackage : Package = ImportDeclaration FilterPackageMember+ (KerML 8.2.3.4.2)
            let package = self.related(i, M::Package, n);
            self.g.add_target(i, Endpoint::Element(package));
            let d = self.rel(package, declaration, n);
            for f in &filters {
                let m = self.rel(package, M::ElementFilterMembership, f);
                if let Some(e) = expressions(f).next() {
                    self.expression(m, &e);
                }
            }
            d
        };
        if has(n, K::StarStar) {
            self.set_bool(inner, Attr::IsRecursive, true);
        }
        if let Some(qn) = child(n, K::QualifiedName) {
            let end = self.name(&qn);
            self.g.add_target(inner, end);
        }
        self.relationship_body(i, n);
    }

    fn relationship_body(&mut self, rel: ElementIx, n: &SyntaxNode) {
        if let Some(body) = child(n, K::Body) {
            self.members(rel, &body, Context::Relationship);
        }
    }

    // ---- elements ----

    /// Lower an element node as an owned related element of `rel`.
    fn lower_element(&mut self, rel: ElementIx, n: &SyntaxNode, ctx: Context) -> ElementIx {
        let kind = n.kind();
        let m = element_metaclass(kind).expect("an element node");
        let e = self.related(rel, m, n);
        match kind {
            K::Comment | K::Documentation | K::TextualRepresentation => self.annotating(e, n),
            K::Dependency
            | K::Specialization
            | K::Conjugation
            | K::Disjoining
            | K::Subclassification
            | K::FeatureTyping
            | K::Subsetting
            | K::Redefinition
            | K::FeatureInverting
            | K::TypeFeaturing => self.standalone_relationship(e, n),
            _ if kind == K::Feature && ctx == Context::Metadata => {
                self.declaration(e, n, Context::Metadata)
            }
            _ => {
                let body = match kind {
                    K::Namespace | K::Package | K::LibraryPackage => Context::Namespace,
                    K::Function
                    | K::Predicate
                    | K::Expression
                    | K::BooleanExpression
                    | K::Invariant => Context::Function,
                    K::MetadataFeature => Context::Metadata,
                    _ => Context::Type,
                };
                self.declaration(e, n, body);
            }
        }
        e
    }

    /// The names, flags and owned relationships of a declaration, and its body, in source order:
    /// namespaces and packages (KerML 8.2.3.4, 8.2.5.13), types and classifiers (8.2.4.1–8.2.4.2,
    /// 8.2.5), features (8.2.4.3, 8.2.5), multiplicities (8.2.5.11) and metadata (8.2.5.12).
    fn declaration(&mut self, e: ElementIx, n: &SyntaxNode, body: Context) {
        let m = self.g.metaclass(e);
        let mut chains = false;
        for c in n.children_with_tokens() {
            match c {
                SyntaxElement::Token(t) => match t.kind() {
                    K::AbstractKw => self.set_bool(e, Attr::IsAbstract, true),
                    K::AllKw => self.set_bool(e, Attr::IsSufficient, true),
                    K::StandardKw => self.set_bool(e, Attr::IsStandard, true),
                    K::InKw => self.g.set(e, Attr::Direction, Value::Enumeration("in")),
                    K::OutKw => self.g.set(e, Attr::Direction, Value::Enumeration("out")),
                    K::InoutKw => self.g.set(e, Attr::Direction, Value::Enumeration("inout")),
                    K::DerivedKw => self.set_bool(e, Attr::IsDerived, true),
                    K::CompositeKw => self.set_bool(e, Attr::IsComposite, true),
                    K::PortionKw => self.set_bool(e, Attr::IsPortion, true),
                    K::VarKw => self.set_bool(e, Attr::IsVariable, true),
                    // `isConstant ?= 'const' { isVariable = true }` (KerML 8.2.4.3.1)
                    K::ConstKw => {
                        self.set_bool(e, Attr::IsConstant, true);
                        self.set_bool(e, Attr::IsVariable, true);
                    }
                    K::EndKw => self.set_bool(e, Attr::IsEnd, true),
                    K::OrderedKw => self.set_bool(e, Attr::IsOrdered, true),
                    K::NonuniqueKw => self.set_bool(e, Attr::IsUnique, false),
                    // Invariant: `( 'true' | isNegated ?= 'false' )` (KerML 8.2.5.7.4)
                    K::FalseKw if m == M::Invariant => self.set_bool(e, Attr::IsNegated, true),
                    K::ChainsKw => chains = true,
                    _ => {}
                },
                SyntaxElement::Node(c) => match c.kind() {
                    K::Identification => self.identification(e, &c),
                    K::PrefixMetadata => {
                        // PrefixMetadataMember : OwningMembership (KerML 8.2.5.12)
                        let r = self.rel(e, M::OwningMembership, &c);
                        self.prefix_metadata(r, &c);
                    }
                    K::MultiplicityBounds => {
                        // OwnedMultiplicity : OwningMembership (KerML 8.2.5.11)
                        if m == M::MultiplicityRange {
                            self.bounds(e, &c);
                        } else {
                            let r = self.rel(e, M::OwningMembership, &c);
                            let range = self.related(r, M::MultiplicityRange, &c);
                            self.bounds(range, &c);
                        }
                    }
                    K::CrossFeature => {
                        // OwnedCrossFeatureMember : OwningMembership (KerML 8.2.4.3.1)
                        let r = self.rel(e, M::OwningMembership, &c);
                        let cross = self.related(r, M::Feature, &c);
                        self.declaration(cross, &c, Context::Type);
                    }
                    K::QualifiedName | K::FeatureChain if chains => {
                        // ChainingPart (KerML 8.2.4.3.1): the feature's own chainings.
                        if c.kind() == K::FeatureChain {
                            self.chainings(e, &c);
                        } else {
                            let r = self.rel(e, M::FeatureChaining, &c);
                            let end = self.name(&c);
                            self.g.add_target(r, end);
                        }
                        chains = false;
                    }
                    K::FeatureValue => self.feature_value(e, &c),
                    K::Annotation => {
                        // `about` Annotations of a MetadataFeature (KerML 8.2.5.12).
                        let r = self.rel(e, M::Annotation, &c);
                        if let Some(end) = self.reference_in(r, &c) {
                            self.g.add_target(r, end);
                        }
                    }
                    K::ConnectorEnd => {
                        // ConnectorEndMember : EndFeatureMembership (KerML 8.2.5.5.1)
                        let r = self.rel(e, M::EndFeatureMembership, &c);
                        self.connector_end(r, &c);
                    }
                    K::FlowEnd => {
                        // FlowEndMember : EndFeatureMembership (KerML 8.2.5.9.2)
                        let r = self.rel(e, M::EndFeatureMembership, &c);
                        self.flow_end(r, &c);
                    }
                    K::PayloadFeature => {
                        // PayloadFeatureMember : FeatureMembership (KerML 8.2.5.9.2)
                        let r = self.rel(e, M::FeatureMembership, &c);
                        let p = self.related(r, M::PayloadFeature, &c);
                        self.declaration(p, &c, Context::Type);
                    }
                    K::Body => self.members(e, &c, body),
                    kind => {
                        if let Some(rm) = owned_relationship(kind) {
                            let r = self.rel(e, rm, &c);
                            if stores_source(rm) {
                                self.g.add_source(r, Endpoint::Element(e));
                            }
                            if let Some(end) = self.reference_in(r, &c) {
                                self.g.add_target(r, end);
                            }
                        }
                    }
                },
            }
        }
    }

    /// Identification (KerML 8.2.3.1): `declaredShortName` and `declaredName`.
    fn identification(&mut self, e: ElementIx, n: &SyntaxNode) {
        if let Some(short) = child(n, K::ShortName) {
            if let Some(t) =
                tokens(&short).find(|t| matches!(t.kind(), K::BasicName | K::UnrestrictedName))
            {
                self.set_string(e, Attr::DeclaredShortName, name_value(&t));
            }
        }
        if let Some(t) = tokens(n).find(|t| matches!(t.kind(), K::BasicName | K::UnrestrictedName))
        {
            self.set_string(e, Attr::DeclaredName, name_value(&t));
        }
    }

    /// PrefixMetadataFeature : MetadataFeature = OwnedFeatureTyping (KerML 8.2.5.12).
    fn prefix_metadata(&mut self, rel: ElementIx, n: &SyntaxNode) {
        let mf = self.related(rel, M::MetadataFeature, n);
        let t = self.rel(mf, M::FeatureTyping, n);
        self.g.add_source(t, Endpoint::Element(mf));
        if let Some(end) = self.reference_in(t, n) {
            self.g.add_target(t, end);
        }
    }

    /// MultiplicityBounds (KerML 8.2.5.11): each bound in an OwningMembership.
    fn bounds(&mut self, range: ElementIx, n: &SyntaxNode) {
        for b in expressions(n) {
            let r = self.rel(range, M::OwningMembership, &b);
            self.expression(r, &b);
        }
    }

    /// FeatureValue (KerML 8.2.5.10).
    fn feature_value(&mut self, e: ElementIx, n: &SyntaxNode) {
        let r = self.rel(e, M::FeatureValue, n);
        if has(n, K::DefaultKw) {
            self.set_bool(r, Attr::IsDefault, true);
        }
        if has(n, K::ColonEq) {
            self.set_bool(r, Attr::IsInitial, true);
        }
        if let Some(x) = expressions(n).next() {
            self.expression(r, &x);
        }
    }

    /// ConnectorEnd : Feature (KerML 8.2.5.5.1): its cross multiplicity, its name before
    /// `references`, and its OwnedReferenceSubsetting. EndFeatureMembership requires `isEnd`
    /// (KerML 8.3.3.3.3, validateEndFeatureMembershipIsEnd).
    fn connector_end(&mut self, rel: ElementIx, n: &SyntaxNode) {
        let end = self.related(rel, M::Feature, n);
        self.set_bool(end, Attr::IsEnd, true);
        for c in n.children_with_tokens() {
            match c {
                SyntaxElement::Token(t)
                    if matches!(t.kind(), K::BasicName | K::UnrestrictedName) =>
                {
                    self.set_string(end, Attr::DeclaredName, name_value(&t));
                }
                SyntaxElement::Node(c) if c.kind() == K::MultiplicityBounds => {
                    // OwnedCrossMultiplicityMember : OwningMembership, OwnedCrossMultiplicity :
                    // Feature = OwnedMultiplicity (KerML 8.2.5.5.1)
                    let r = self.rel(end, M::OwningMembership, &c);
                    let cross = self.related(r, M::Feature, &c);
                    let r2 = self.rel(cross, M::OwningMembership, &c);
                    let range = self.related(r2, M::MultiplicityRange, &c);
                    self.bounds(range, &c);
                }
                SyntaxElement::Node(c) if c.kind() == K::OwnedReferenceSubsetting => {
                    let r = self.rel(end, M::ReferenceSubsetting, &c);
                    if let Some(t) = self.reference_in(r, &c) {
                        self.g.add_target(r, t);
                    }
                }
                _ => {}
            }
        }
    }

    /// FlowEnd (KerML 8.2.5.9.2): `( OwnedReferenceSubsetting '.' )? FlowFeatureMember`. The last
    /// segment of the written name is the flow feature, which redefines it; what comes before it
    /// is the reference subsetting.
    fn flow_end(&mut self, rel: ElementIx, n: &SyntaxNode) {
        let end = self.related(rel, M::FlowEnd, n);
        self.set_bool(end, Attr::IsEnd, true);
        let Some(written) =
            nodes(n).find(|c| matches!(c.kind(), K::QualifiedName | K::FeatureChain))
        else {
            return;
        };
        let names: Vec<SyntaxNode> = if written.kind() == K::FeatureChain {
            nodes(&written)
                .filter(|c| c.kind() == K::QualifiedName)
                .collect()
        } else {
            vec![written.clone()]
        };
        if names.len() > 1 {
            let r = self.rel(end, M::ReferenceSubsetting, &written);
            let target = if names.len() == 2 {
                self.name(&names[0])
            } else {
                let chain = self.related(r, M::Feature, &written);
                for qn in &names[..names.len() - 1] {
                    let c = self.rel(chain, M::FeatureChaining, qn);
                    let t = self.name(qn);
                    self.g.add_target(c, t);
                }
                Endpoint::Element(chain)
            };
            self.g.add_target(r, target);
        }
        // FlowFeatureMember : FeatureMembership, FlowFeature : Feature = FlowFeatureRedefinition
        let last = names.last().unwrap();
        let r = self.rel(end, M::FeatureMembership, last);
        let flow_feature = self.related(r, M::Feature, last);
        let redef = self.rel(flow_feature, M::Redefinition, last);
        self.g.add_source(redef, Endpoint::Element(flow_feature));
        let t = self.name(last);
        self.g.add_target(redef, t);
    }

    /// Comment, Documentation and TextualRepresentation (KerML 8.2.3.3).
    fn annotating(&mut self, e: ElementIx, n: &SyntaxNode) {
        for c in n.children_with_tokens() {
            match c {
                SyntaxElement::Token(t) => match t.kind() {
                    K::RegularComment => self.set_string(e, Attr::Body, comment_body(t.text())),
                    K::StringValue => {
                        let attr = if self.g.metaclass(e) == M::TextualRepresentation {
                            Attr::Language
                        } else {
                            Attr::Locale
                        };
                        self.set_string(e, attr, string_value(&t));
                    }
                    _ => {}
                },
                SyntaxElement::Node(c) => match c.kind() {
                    K::Identification => self.identification(e, &c),
                    K::Annotation => {
                        // `about` Annotations (KerML 8.2.3.3.2)
                        let r = self.rel(e, M::Annotation, &c);
                        if let Some(end) = self.reference_in(r, &c) {
                            self.g.add_target(r, end);
                        }
                    }
                    _ => {}
                },
            }
        }
    }

    /// A relationship written as a member (KerML 8.2.3.2, 8.2.4.1.2–8.2.4.1.4, 8.2.4.2.2,
    /// 8.2.4.3.2–8.2.4.3.7): its identification, then its ends in order, the first the source and
    /// the second the target (a Dependency: its clients, then after `to` its suppliers).
    fn standalone_relationship(&mut self, e: ElementIx, n: &SyntaxNode) {
        let mut target = false;
        let mut ends = 0;
        let dependency = self.g.metaclass(e) == M::Dependency;
        for c in n.children_with_tokens() {
            match c {
                SyntaxElement::Token(t) if dependency && t.kind() == K::ToKw => target = true,
                SyntaxElement::Token(_) => {}
                SyntaxElement::Node(c) => match c.kind() {
                    K::Identification => self.identification(e, &c),
                    K::PrefixMetadata => {
                        // PrefixMetadataAnnotation : Annotation (KerML 8.2.5.12)
                        let r = self.rel(e, M::Annotation, &c);
                        self.prefix_metadata(r, &c);
                    }
                    K::QualifiedName | K::FeatureChain => {
                        if let Some(end) = self.reference(e, &c) {
                            if target || (!dependency && ends > 0) {
                                self.g.add_target(e, end);
                            } else {
                                self.g.add_source(e, end);
                            }
                            ends += 1;
                        }
                    }
                    K::Body => self.members(e, &c, Context::Relationship),
                    _ => {}
                },
            }
        }
    }

    // ---- expressions (KerML 8.2.5.8) ----

    /// EmptyResultMember : ReturnParameterMembership (KerML 8.2.5.8.1): an empty Feature, whose
    /// direction a ReturnParameterMembership requires to be `out` (KerML 8.3.4.7.4).
    fn empty_result(&mut self, e: ElementIx, n: &SyntaxNode) {
        let r = self.rel(e, M::ReturnParameterMembership, n);
        let f = self.related(r, M::Feature, n);
        self.g.set(f, Attr::Direction, Value::Enumeration("out"));
    }

    /// A ParameterMembership owning a parameter Feature, whose direction a ParameterMembership
    /// requires to be `in` (KerML 8.3.4.6.4, validateParameterMembershipParameterDirection).
    fn parameter(&mut self, e: ElementIx, n: &SyntaxNode) -> ElementIx {
        let r = self.rel(e, M::ParameterMembership, n);
        let f = self.related(r, M::Feature, n);
        self.g.set(f, Attr::Direction, Value::Enumeration("in"));
        f
    }

    /// ArgumentMember (KerML 8.2.5.8.1): a parameter whose FeatureValue is the expression.
    fn argument(&mut self, e: ElementIx, x: &SyntaxNode) -> ElementIx {
        let f = self.parameter(e, x);
        let v = self.rel(f, M::FeatureValue, x);
        self.expression(v, x);
        f
    }

    /// ArgumentExpressionMember (KerML 8.2.5.8.1): a parameter whose value is an
    /// OwnedExpressionReference to the expression. The grammar writes it `: FeatureMembership`, but
    /// its feature is an input parameter of the expression: KerML 8.4.4.9.6 parses the operand as a
    /// body expression passed as an argument (`ControlFunctions::'if' (e1, { e2 }, { e3 })`). So
    /// it is a ParameterMembership, direction `in` (8.3.4.6.4), as the pilot builds it (ADR 0018).
    fn argument_expression(&mut self, e: ElementIx, x: &SyntaxNode) {
        let f = self.parameter(e, x);
        let v = self.rel(f, M::FeatureValue, x);
        let reference = self.related(v, M::FeatureReferenceExpression, x);
        let m = self.rel(reference, M::FeatureMembership, x);
        self.expression(m, x);
        // A FeatureReferenceExpression has its EmptyResultMember (ADR 0018).
        self.empty_result(reference, x);
    }

    /// TypeReferenceMember (KerML 8.2.5.8.1): a parameter typed by the name (ReferenceTyping); or,
    /// for the cast, TypeResultMember, its result.
    fn type_reference(&mut self, e: ElementIx, qn: &SyntaxNode, result: bool) {
        let f = if result {
            let r = self.rel(e, M::ReturnParameterMembership, qn);
            let f = self.related(r, M::Feature, qn);
            self.g.set(f, Attr::Direction, Value::Enumeration("out"));
            f
        } else {
            self.parameter(e, qn)
        };
        let t = self.rel(f, M::FeatureTyping, qn);
        self.g.add_source(t, Endpoint::Element(f));
        let end = self.name(qn);
        self.g.add_target(t, end);
    }

    /// Lower an expression node as an owned related element of `rel`.
    fn expression(&mut self, rel: ElementIx, x: &SyntaxNode) -> Option<ElementIx> {
        let operator = |x: &SyntaxNode| -> String {
            tokens(x)
                .filter(|t| {
                    !matches!(
                        t.kind(),
                        K::LParen | K::RParen | K::Comma | K::Question | K::ElseKw
                    )
                })
                .map(|t| t.text().to_owned())
                .collect()
        };
        Some(match x.kind() {
            // LiteralExpression (KerML 8.2.5.8.4)
            K::LiteralBoolean => {
                let e = self.related(rel, M::LiteralBoolean, x);
                self.set_bool(e, Attr::Value, has(x, K::TrueKw));
                e
            }
            K::LiteralString => {
                let e = self.related(rel, M::LiteralString, x);
                if let Some(t) = tokens(x).next() {
                    self.set_string(e, Attr::Value, string_value(&t));
                }
                e
            }
            K::LiteralInteger => {
                let e = self.related(rel, M::LiteralInteger, x);
                let text: String = tokens(x).map(|t| t.text().to_owned()).collect();
                match text.parse::<i64>() {
                    Ok(v) => self.g.set(e, Attr::Value, Value::Integer(v)),
                    Err(_) => self.diagnostics.push(Diagnostic::error(
                        "lower::integer-too-large",
                        x.text_range(),
                        format!("the integer {text} is too large"),
                    )),
                }
                e
            }
            K::LiteralReal => {
                // LiteralReal: `value = RealValue`, a LiteralRational (KerML 8.3.4.8.13)
                let e = self.related(rel, M::LiteralRational, x);
                let text: String = tokens(x).map(|t| t.text().to_owned()).collect();
                if let Ok(v) = text.parse::<f64>() {
                    self.g.set(e, Attr::Value, Value::Real(v));
                }
                e
            }
            K::LiteralInfinity => self.related(rel, M::LiteralInfinity, x),
            // NullExpression (KerML 8.2.5.8.3)
            K::NullExpression => self.related(rel, M::NullExpression, x),
            // FeatureReferenceExpression: FeatureReferenceMember, EmptyResultMember (8.2.5.8.3)
            K::FeatureReferenceExpression => {
                let e = self.related(rel, M::FeatureReferenceExpression, x);
                if let Some(qn) = child(x, K::QualifiedName) {
                    let m = self.rel(e, M::Membership, &qn);
                    let end = self.name(&qn);
                    self.g.add_target(m, end);
                }
                self.empty_result(e, x);
                e
            }
            // SequenceExpression (KerML 8.2.5.8.2): one expression in parentheses is itself; a
            // list is a SequenceOperatorExpression `,` of the first and the rest.
            K::SequenceExpression => {
                let items: Vec<SyntaxNode> = expressions(x).collect();
                return self.sequence(rel, x, &items);
            }
            K::ConditionalExpression => {
                let e = self.related(rel, M::OperatorExpression, x);
                self.g.set(e, Attr::Operator, Value::String("if".into()));
                let operands: Vec<SyntaxNode> = expressions(x).collect();
                if let Some(c) = operands.first() {
                    self.argument(e, c);
                }
                for o in operands.iter().skip(1) {
                    self.argument_expression(e, o);
                }
                self.empty_result(e, x);
                e
            }
            K::BinaryExpression => {
                let e = self.related(rel, M::OperatorExpression, x);
                let op = operator(x);
                // ConditionalBinaryOperatorExpression: the second operand is an
                // ArgumentExpressionMember (KerML 8.2.5.8.1).
                let conditional = matches!(op.as_str(), "??" | "or" | "and" | "implies");
                self.g.set(e, Attr::Operator, Value::String(op.into()));
                let operands: Vec<SyntaxNode> = expressions(x).collect();
                for (i, o) in operands.iter().enumerate() {
                    if conditional && i == 1 {
                        self.argument_expression(e, o);
                    } else {
                        self.argument(e, o);
                    }
                }
                self.empty_result(e, x);
                e
            }
            K::UnaryExpression => {
                let e = self.related(rel, M::OperatorExpression, x);
                self.g
                    .set(e, Attr::Operator, Value::String(operator(x).into()));
                if let Some(o) = expressions(x).next() {
                    self.argument(e, &o);
                }
                self.empty_result(e, x);
                e
            }
            K::ClassificationExpression => {
                let op = operator(x);
                let e = self.related(rel, M::OperatorExpression, x);
                self.g
                    .set(e, Attr::Operator, Value::String(op.clone().into()));
                if let Some(o) = expressions(x).next() {
                    if matches!(op.as_str(), "@@" | "meta") {
                        // MetadataArgumentMember (KerML 8.2.5.8.1): a MetadataAccessExpression of
                        // the element named.
                        let f = self.parameter(e, &o);
                        let v = self.rel(f, M::FeatureValue, &o);
                        let access = self.related(v, M::MetadataAccessExpression, &o);
                        if let Some(qn) = child(&o, K::QualifiedName) {
                            let m = self.rel(access, M::Membership, &qn);
                            let end = self.name(&qn);
                            self.g.add_target(m, end);
                        }
                    } else {
                        self.argument(e, &o);
                    }
                }
                let cast = matches!(op.as_str(), "as" | "meta");
                if let Some(qn) = child(x, K::QualifiedName) {
                    self.type_reference(e, &qn, cast);
                }
                // A cast's TypeResultMember is its result, so it has no EmptyResultMember: an
                // expression has one result parameter (ADR 0018).
                if !cast {
                    self.empty_result(e, x);
                }
                e
            }
            K::ExtentExpression => {
                let e = self.related(rel, M::OperatorExpression, x);
                self.g.set(e, Attr::Operator, Value::String("all".into()));
                if let Some(qn) = child(x, K::QualifiedName) {
                    self.type_reference(e, &qn, false);
                }
                e
            }
            K::FeatureChainExpression => {
                let e = self.related(rel, M::FeatureChainExpression, x);
                self.g.set(e, Attr::Operator, Value::String(".".into()));
                if let Some(o) = expressions(x).next() {
                    let source = self.argument(e, &o);
                    // sourceTargetFeature (KerML 8.3.4.8.4): the first owned feature of the first
                    // input parameter, which the check constraints make redefine the target
                    // feature (the redefinitions are implied, step 1a.9; ADR 0018).
                    let m = self.rel(source, M::FeatureMembership, &o);
                    self.related(m, M::Feature, &o);
                }
                // FeatureChainMember: a FeatureReferenceMember, or an OwnedFeatureChainMember.
                if let Some(target) =
                    nodes(x).find(|c| matches!(c.kind(), K::QualifiedName | K::FeatureChain))
                {
                    if target.kind() == K::QualifiedName {
                        let m = self.rel(e, M::Membership, &target);
                        let end = self.name(&target);
                        self.g.add_target(m, end);
                    } else {
                        let m = self.rel(e, M::OwningMembership, &target);
                        let chain = self.related(m, M::Feature, &target);
                        self.chainings(chain, &target);
                    }
                }
                self.empty_result(e, x);
                e
            }
            K::CollectExpression | K::SelectExpression => {
                let (m, op) = if x.kind() == K::CollectExpression {
                    (M::CollectExpression, "collect")
                } else {
                    (M::SelectExpression, "select")
                };
                let e = self.related(rel, m, x);
                self.g.set(e, Attr::Operator, Value::String(op.into()));
                for o in expressions(x) {
                    self.argument(e, &o);
                }
                self.empty_result(e, x);
                e
            }
            K::FunctionOperationExpression => {
                // FunctionOperationExpression : InvocationExpression (KerML 8.2.5.8.2)
                let e = self.related(rel, M::InvocationExpression, x);
                if let Some(o) = expressions(x).find(|c| c.kind() != K::BodyExpression) {
                    self.argument(e, &o);
                }
                let mut refs =
                    nodes(x).filter(|c| matches!(c.kind(), K::QualifiedName | K::FeatureChain));
                if let Some(function) = refs.next() {
                    self.instantiated_type(e, &function);
                }
                if let Some(function) = refs.next() {
                    // FunctionReferenceArgumentMember (KerML 8.2.5.8.2)
                    let f = self.parameter(e, &function);
                    let v = self.rel(f, M::FeatureValue, &function);
                    let reference = self.related(v, M::FeatureReferenceExpression, &function);
                    let m = self.rel(reference, M::FeatureMembership, &function);
                    let fr = self.related(m, M::Expression, &function);
                    let t = self.rel(fr, M::FeatureTyping, &function);
                    self.g.add_source(t, Endpoint::Element(fr));
                    let end = self.name(&function);
                    self.g.add_target(t, end);
                    // A FeatureReferenceExpression has its EmptyResultMember (ADR 0018).
                    self.empty_result(reference, &function);
                }
                if let Some(body) = child(x, K::BodyExpression) {
                    self.argument(e, &body);
                }
                if let Some(args) = child(x, K::ArgumentList) {
                    self.arguments(e, &args);
                }
                self.empty_result(e, x);
                e
            }
            K::IndexExpression | K::BracketExpression => {
                let (m, op) = if x.kind() == K::IndexExpression {
                    (M::IndexExpression, "#")
                } else {
                    (M::OperatorExpression, "[")
                };
                let e = self.related(rel, m, x);
                self.g.set(e, Attr::Operator, Value::String(op.into()));
                let items: Vec<SyntaxNode> = expressions(x).collect();
                if let Some(first) = items.first() {
                    self.argument(e, first);
                }
                // SequenceExpressionListMember: the rest, as one argument (ADR 0018).
                if items.len() > 1 {
                    self.sequence_argument(e, x, &items[1..]);
                }
                self.empty_result(e, x);
                e
            }
            K::MetadataAccessExpression => {
                let e = self.related(rel, M::MetadataAccessExpression, x);
                if let Some(qn) = expressions(x)
                    .next()
                    .and_then(|o| child(&o, K::QualifiedName))
                {
                    let m = self.rel(e, M::Membership, &qn);
                    let end = self.name(&qn);
                    self.g.add_target(m, end);
                }
                e
            }
            K::InvocationExpression => {
                let e = self.related(rel, M::InvocationExpression, x);
                if let Some(t) =
                    nodes(x).find(|c| matches!(c.kind(), K::QualifiedName | K::FeatureChain))
                {
                    self.instantiated_type(e, &t);
                }
                if let Some(args) = child(x, K::ArgumentList) {
                    self.arguments(e, &args);
                }
                self.empty_result(e, x);
                e
            }
            K::ConstructorExpression => {
                // ConstructorExpression: InstantiatedTypeMember, ConstructorResultMember
                // (KerML 8.2.5.8.3), whose result Feature owns the arguments.
                let e = self.related(rel, M::ConstructorExpression, x);
                if let Some(t) =
                    nodes(x).find(|c| matches!(c.kind(), K::QualifiedName | K::FeatureChain))
                {
                    self.instantiated_type(e, &t);
                }
                let r = self.rel(e, M::ReturnParameterMembership, x);
                let result = self.related(r, M::Feature, x);
                self.g
                    .set(result, Attr::Direction, Value::Enumeration("out"));
                if let Some(args) = child(x, K::ArgumentList) {
                    self.arguments(result, &args);
                }
                e
            }
            K::BodyExpression => {
                // BodyExpression : FeatureReferenceExpression = ExpressionBodyMember
                // (KerML 8.2.5.8.3)
                let e = self.related(rel, M::FeatureReferenceExpression, x);
                let m = self.rel(e, M::FeatureMembership, x);
                let body = self.related(m, M::Expression, x);
                if let Some(b) = child(x, K::Body) {
                    self.members(body, &b, Context::Function);
                }
                // A FeatureReferenceExpression has its EmptyResultMember (ADR 0018).
                self.empty_result(e, x);
                e
            }
            _ => return None,
        })
    }

    /// `( a, b, c )` (KerML 8.2.5.8.2): one item is itself, more are SequenceOperatorExpressions
    /// `,` of the first and the rest. The grammar owns both through FeatureMemberships
    /// (OwnedExpressionMember, SequenceExpressionListMember), but they are the two input
    /// parameters of the operator, which KerML 8.4.4.9.5 binds by FeatureValues to the arguments:
    /// arguments, with a result, as the pilot builds them (ADR 0018).
    fn sequence(
        &mut self,
        rel: ElementIx,
        x: &SyntaxNode,
        items: &[SyntaxNode],
    ) -> Option<ElementIx> {
        match items {
            [] => None,
            [one] => self.expression(rel, one),
            [first, rest @ ..] => {
                let e = self.related(rel, M::OperatorExpression, x);
                self.g.set(e, Attr::Operator, Value::String(",".into()));
                self.argument(e, first);
                self.sequence_argument(e, x, rest);
                self.empty_result(e, x);
                Some(e)
            }
        }
    }

    /// An argument whose value is the sequence of `items` (one item: that expression).
    fn sequence_argument(&mut self, e: ElementIx, x: &SyntaxNode, items: &[SyntaxNode]) {
        if let [one] = items {
            self.argument(e, one);
            return;
        }
        let f = self.parameter(e, x);
        let v = self.rel(f, M::FeatureValue, x);
        self.sequence(v, x, items);
    }

    /// InstantiatedTypeMember (KerML 8.2.5.8.3): a Membership of the type named, or an
    /// OwnedFeatureChainMember.
    fn instantiated_type(&mut self, e: ElementIx, t: &SyntaxNode) {
        if t.kind() == K::QualifiedName {
            let m = self.rel(e, M::Membership, t);
            let end = self.name(t);
            self.g.add_target(m, end);
        } else {
            let m = self.rel(e, M::OwningMembership, t);
            let chain = self.related(m, M::Feature, t);
            self.chainings(chain, t);
        }
    }

    /// ArgumentList (KerML 8.2.5.8.3): positional ArgumentMembers, or NamedArgumentMembers, each a
    /// FeatureMembership of a Feature with a ParameterRedefinition and an ArgumentValue.
    fn arguments(&mut self, e: ElementIx, args: &SyntaxNode) {
        for a in nodes(args) {
            if a.kind() == K::NamedArgument {
                let r = self.rel(e, M::FeatureMembership, &a);
                let f = self.related(r, M::Feature, &a);
                if let Some(qn) = child(&a, K::QualifiedName) {
                    let redef = self.rel(f, M::Redefinition, &qn);
                    self.g.add_source(redef, Endpoint::Element(f));
                    let end = self.name(&qn);
                    self.g.add_target(redef, end);
                }
                if let Some(x) = expressions(&a).next() {
                    let v = self.rel(f, M::FeatureValue, &x);
                    self.expression(v, &x);
                }
            } else if is_expression(a.kind()) {
                self.argument(e, &a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn comment_bodies_follow_the_note_of_8_2_3_3_2() {
        assert_eq!(comment_body("/* text */"), "text ");
        assert_eq!(
            comment_body("/*\n * first\n * second\n */"),
            "first\nsecond\n"
        );
        assert_eq!(comment_body("/*   \n   *no space\n*/"), "no space\n");
    }

    #[test]
    fn names_and_strings_are_unescaped() {
        assert_eq!(unescape(r"a\'b\nc\\"), "a'b\nc\\");
    }
}
