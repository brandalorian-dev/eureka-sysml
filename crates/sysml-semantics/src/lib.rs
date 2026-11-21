// SPDX-License-Identifier: Apache-2.0
//! The semantics of KerML and SysML models (PROJECT_PLAN §4.2).
//!
//! Since step 1a.6 it holds the lowering from syntax trees to the element graph ([`lower`]).
//! Since step 1a.7 it resolves names (KerML 8.2.3.5) as incremental `salsa` queries (ADR 0019).
//! Since step 1a.8, [`derived`] computes the derived properties and operations of KerML 8.3
//! (ADR 0021).

/// The iterations after which a cycle head keeps its last value (ADR 0020). The queries that
/// iterate to a fixpoint are not all monotone (a lookup takes the first match, and imported
/// memberships that collide are excluded), so a cycle in a user model might not converge;
/// `salsa` panics after 200 iterations. Every cycle head settles well before that.
const SETTLE_AFTER: u32 = 100;

/// A `cycle_fn` (ADR 0020), whose signature `salsa` sets: the value just computed, or, after [`SETTLE_AFTER`] iterations, the
/// last one (`last`) or a given one, which ends the iteration on the next comparison.
macro_rules! settle {
    ($vis:vis $name:ident, $out:ty, ($($arg:ty),*)) => {
        settle!($vis $name, $out, ($($arg),*), |last: &$out| last.clone());
    };
    ($vis:vis $name:ident, $out:ty, ($($arg:ty),*), $settled:expr) => {
        #[allow(clippy::redundant_closure_call, clippy::too_many_arguments)]
        $vis fn $name(
            _db: &dyn salsa::Database,
            cycle: &salsa::Cycle,
            last: &$out,
            value: $out,
            $(_: $arg),*
        ) -> $out {
            if cycle.iteration() >= crate::SETTLE_AFTER {
                ($settled)(last)
            } else {
                value
            }
        }
    };
}

pub mod derived;
mod implied;
mod lower;
mod resolve;

pub use implied::{implied_generals, implied_redefinitions, Implied};
pub use lower::{lower, Lowered};
pub use resolve::{
    effective_names, global_namespace, imported_memberships, inherited_memberships, lowered,
    member_element, memberships, owned_memberships, parsed, resolve_end, resolved_names, structure,
    visible_memberships, Database, Entry, Global, Name, Project, SourceFile, Structure, Visibility,
};
