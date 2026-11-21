// SPDX-License-Identifier: Apache-2.0
//! The semantics of KerML and SysML models (PROJECT_PLAN §4.2).
//!
//! Since step 1a.6 it holds the lowering from syntax trees to the element graph ([`lower`]).
//! Name resolution, imports and incremental queries follow in step 1a.7.

mod lower;

pub use lower::{lower, Lowered};
