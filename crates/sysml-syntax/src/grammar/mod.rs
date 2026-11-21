// SPDX-License-Identifier: Apache-2.0
//! The grammar: one function per production of KerML 8.2.3–8.2.5, named after it.
//!
//! - [`names`]: identifications, qualified names, feature chains and bodies
//! - [`root`]: the Root layer (KerML 8.2.3) and the namespace members
//! - [`core`]: the Core layer (KerML 8.2.4)
//! - [`kernel`]: the Kernel layer (KerML 8.2.5) apart from expressions
//! - [`expressions`]: the expressions (KerML 8.2.5.8)
//!
//! The tree has a node for each element and each owned relationship of the abstract syntax; a
//! membership's visibility and an element's prefix keywords are tokens of the element's node.

mod core;
mod expressions;
mod kernel;
mod names;
mod root;

pub(crate) use root::root_namespace;

use crate::kinds::SyntaxKind;
use crate::parser::Parser;

/// BASIC_NAME or UNRESTRICTED_NAME: NAME (KerML 8.2.2.3).
fn is_name(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::BasicName | SyntaxKind::UnrestrictedName)
}

/// Whether a qualified name can start at the `n`-th token.
fn at_qualified_name(p: &Parser, n: usize) -> bool {
    is_name(p.peek(n)) || p.peek(n) == SyntaxKind::Dollar
}

/// The visibility indicators (KerML 8.2.3.4.1).
const VISIBILITY: &[SyntaxKind] = &[
    SyntaxKind::PublicKw,
    SyntaxKind::PrivateKw,
    SyntaxKind::ProtectedKw,
];

/// The keywords that can come before the keyword that decides what a member is: the
/// `member` of a type feature member (KerML 8.2.4.1.6), the `return` of a return feature member
/// (8.2.5.7.1), the type prefix (8.2.4.1.1) and the feature prefixes (8.2.4.3.1). Prefix metadata (`#`) can also come before it.
const PREFIX: &[SyntaxKind] = &[
    SyntaxKind::MemberKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::AbstractKw,
    SyntaxKind::InKw,
    SyntaxKind::OutKw,
    SyntaxKind::InoutKw,
    SyntaxKind::DerivedKw,
    SyntaxKind::CompositeKw,
    SyntaxKind::PortionKw,
    SyntaxKind::VarKw,
    SyntaxKind::ConstKw,
    SyntaxKind::EndKw,
];

/// Where a member is: what its body allows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Context {
    /// A root namespace or a namespace body (KerML 8.2.3.4.1).
    Namespace,
    /// A package body, which also allows element filters (KerML 8.2.5.13).
    Package,
    /// A type body, which also allows `member` (KerML 8.2.4.1.1, 8.2.4.1.6).
    Type,
    /// A relationship body: owned related elements and annotations, without visibility, aliases
    /// or imports (KerML 8.2.3.1).
    Relationship,
    /// A function body (KerML 8.2.5.7.1) or a body expression (8.2.5.8.3): a type body that also
    /// allows `return` features and ends with an optional result expression.
    Function,
    /// A metadata body (KerML 8.2.5.12), whose features redefine.
    Metadata,
}
