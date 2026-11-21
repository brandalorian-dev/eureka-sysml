// SPDX-License-Identifier: Apache-2.0
//! The semantics of KerML and SysML models (PROJECT_PLAN §4.2).
//!
//! Since step 1a.6 it holds the lowering from syntax trees to the element graph ([`lower`]).
//! Since step 1a.7 it resolves names (KerML 8.2.3.5) as incremental `salsa` queries (ADR 0019).

mod implied;
mod lower;
mod resolve;

pub use lower::{lower, Lowered};
pub use resolve::{
    effective_names, global_namespace, imported_memberships, inherited_memberships, lowered,
    member_element, memberships, owned_memberships, parsed, resolve_end, resolved_names, structure,
    visible_memberships, Database, Entry, Global, Name, Project, SourceFile, Structure, Visibility,
};
