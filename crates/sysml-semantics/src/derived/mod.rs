// SPDX-License-Identifier: Apache-2.0
//! Derived properties and operations of the KerML metaclasses (KerML 8.3; step 1a.8, ADR 0021),
//! computed on read from the queries of name resolution (ADR 0019, ADR 0020).
//!
//! Each function implements one `derive…` rule or one operation and cites it. An element is a
//! [`Global`]; a function takes the project, because most properties depend on resolution. The
//! inventory of every rule and operation, with its status, is `crates/sysml-semantics/derived.json`,
//! kept equal to `KerML.xmi` by `tests/derived_inventory.rs`.
//!
//! The functions are grouped by layer: Root (KerML 8.3.2), Core (8.3.3: types, classifiers and
//! features) and Kernel (8.3.4).

mod evaluate;
mod kernel;
mod root;
mod types;

pub use evaluate::*;
pub use kernel::*;
pub use root::*;
pub use types::*;

use sysml_model::{ElementIx, Endpoint, Metaclass as M, Slot};

use crate::materialize::{view, ViewEnd};
use crate::resolve::{member_element, resolve_end, Global, Project};

pub(crate) fn at(file: crate::SourceFile, e: ElementIx) -> Global {
    Global {
        file,
        index: e.index() as u32,
    }
}

pub(crate) fn metaclass(db: &dyn salsa::Database, project: Project, x: Global) -> M {
    view(db, project, x.file).metaclass(x.ix())
}

/// The element an end of a relationship identifies: the element written there, or the member
/// element of the membership its name resolves to (KerML 8.2.3.5).
pub(crate) fn end(
    db: &dyn salsa::Database,
    project: Project,
    rel: Global,
    slot: Slot,
    i: usize,
) -> Option<Global> {
    let s = view(db, project, rel.file);
    let ends = s.ends(rel.ix(), slot);
    // An end whose stored attribute is typed Membership, as `MembershipImport::importedMembership`
    // (KerML 8.3.2.4.4), is the membership itself (8.2.3.4.2, Note 1), not its member element.
    let membership = s
        .metaclass(rel.ix())
        .stored_attributes()
        .iter()
        .find(|a| a.slot == slot)
        .is_some_and(|a| match a.kind {
            sysml_model::PropertyKind::Reference { target } => M::ALL
                .iter()
                .find(|m| m.name() == target)
                .is_some_and(|m| m.specializes(M::Membership)),
            _ => false,
        });
    match ends.get(i)? {
        // An end of an implied relationship (ADR 0022).
        ViewEnd::Implied(g) => Some(*g),
        ViewEnd::Written(Endpoint::Element(e)) => Some(at(rel.file, *e)),
        ViewEnd::Written(Endpoint::Name(_)) => {
            let entry = resolve_end(db, project, rel, slot, i as u32).clone()?;
            if membership {
                Some(entry.membership)
            } else {
                member_element(db, project, &entry)
            }
        }
    }
}

pub(crate) fn ends(
    db: &dyn salsa::Database,
    project: Project,
    rel: Global,
    slot: Slot,
) -> Vec<Global> {
    let n = view(db, project, rel.file).ends(rel.ix(), slot).len();
    (0..n)
        .filter_map(|i| end(db, project, rel, slot, i))
        .collect()
}

/// The elements in order, each once.
pub(crate) fn unique(xs: impl IntoIterator<Item = Global>) -> Vec<Global> {
    let mut seen = std::collections::HashSet::new();
    xs.into_iter().filter(|x| seen.insert(*x)).collect()
}

/// The owned relationships of an element that are of a metaclass, in order.
pub(crate) fn owned_relationships_of(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    kind: M,
) -> Vec<Global> {
    let g = view(db, project, x.file);
    g.all_owned_relationships(x.ix())
        .filter(|r| g.metaclass(*r).specializes(kind))
        .map(|r| at(x.file, r))
        .collect()
}
