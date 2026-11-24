// SPDX-License-Identifier: Apache-2.0
//! The identity of standard library elements (KerML 9.1, SysML 9.1; step 1a.11, ADR 0009).
//!
//! The `elementId` of every element of a standard library model is a name-based UUID (version 5,
//! SHA-1). A top-level standard library package's is the UUID of its URL, in the `NameSpace_URL`
//! name space; that of any element inside it is the UUID of its `path()`, in the package's. The
//! implied elements of the overlay are elements of the library model too, and get theirs the same
//! way: their `path()` counts their position after the owned relationships (ADR 0022).
//!
//! Elements of other models get their ids as API DataIdentity ids, outside the queries
//! (ADR 0019), so they have none here.

use sysml_model::{Attr, Metaclass as M};
use sysml_syntax::Language;
use uuid::Uuid;

use crate::derived as d;
use crate::resolve::{Global, Project};

/// The URL prefix of the KerML standard libraries (KerML 9.1).
pub const KERML: &str = "https://www.omg.org/spec/KerML/";

/// The URL prefix of the SysML standard libraries (SysML 9.1).
pub const SYSML: &str = "https://www.omg.org/spec/SysML/";

/// The UUID of a top-level standard library package (KerML 9.1): version 5, in the
/// `NameSpace_URL` name space (6ba7b811-9dad-11d1-80b4-00c04fd430c8), of the URL `prefix` + name,
/// as UTF-8.
pub fn library_package_id(prefix: &str, name: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("{prefix}{name}").as_bytes())
}

/// The `elementId` that KerML 9.1 gives an element of a top-level standard library package, the
/// package included; none for any other element. The URL prefix is that of the language of the
/// file, KerML or SysML.
pub fn element_id(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Uuid> {
    let package = d::library_namespace(db, project, x)?;
    let v = crate::view(db, project, package.file);
    // A standard library package (KerML 8.3.4.13.3 isStandard), owned by its root namespace.
    let top_level = d::owning_namespace(db, project, package)
        .is_some_and(|ns| d::owning_namespace(db, project, ns).is_none());
    if !v.metaclass(package.ix()).specializes(M::LibraryPackage)
        || !v.flag(package.ix(), Attr::IsStandard)
        || !top_level
    {
        return None;
    }
    let name = d::name(db, project, package)?;
    let prefix = match package.file.language(db) {
        Language::KerML => KERML,
        Language::SysML => SYSML,
    };
    let package_id = library_package_id(prefix, &name);
    if x == package {
        return Some(package_id);
    }
    Some(Uuid::new_v5(
        &package_id,
        d::path(db, project, x).as_bytes(),
    ))
}
