// SPDX-License-Identifier: Apache-2.0
//! SysML v2 / KerML metaclass definitions.
//!
//! [`Metaclass`], its property descriptors and the metamodel enumerations are generated at
//! build time from the normative `SysML.json` (see `build.rs`). This crate will grow the
//! arena-based element graph in Phase 1a (see `docs/proj-mgmt/PROJECT_PLAN.md` §4.2).

#[allow(clippy::all, missing_docs)] // lints are not useful on generated code
mod generated {
    use super::{PropertyDef, PropertyKind};
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
pub use generated::*;

/// The value type of a serialized metaclass property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyKind {
    /// A reference to another element (`{"@id": …}`). `target` is the metaclass name from the schema.
    Reference {
        target: &'static str,
    },
    /// A value of one of the metamodel enumerations, such as `VisibilityKind`.
    Enumeration {
        name: &'static str,
    },
    Boolean,
    String,
    Integer,
    Real,
    Uuid,
}

/// One serialized property of a metaclass (KerML 10.4.5: owned and inherited properties).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyDef {
    pub name: &'static str,
    pub kind: PropertyKind,
    /// The multiplicity upper bound is greater than 1, so the property is serialized as a JSON array.
    pub many: bool,
    /// `null` is allowed (multiplicity lower bound 0, single-valued).
    pub nullable: bool,
    /// The property is listed in the schema's `required` array.
    pub required: bool,
}

/// The metamodel IRI these definitions were generated from.
pub const METAMODEL_IRI: &str = "https://www.omg.org/spec/SysML/20250201";

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn counts_match_schema() {
        // SysML.json (20250201): 175 metaclasses and 7 enumerations, plus `Identified`.
        assert_eq!(Metaclass::ALL.len(), 175);
        assert_eq!(VisibilityKind::ALL.len(), 3);
        assert_eq!(FeatureDirectionKind::ALL.len(), 3);
    }

    #[test]
    fn names_round_trip() {
        for &m in Metaclass::ALL {
            assert_eq!(Metaclass::parse(m.name()), Some(m));
        }
        assert_eq!(Metaclass::parse("NotAMetaclass"), None);
        assert_eq!(
            VisibilityKind::parse("public"),
            Some(VisibilityKind::Public)
        );
        assert_eq!(FeatureDirectionKind::Inout.as_str(), "inout");
    }

    #[test]
    fn element_hierarchy_roots() {
        assert_eq!(
            Metaclass::Element.direct_subtypes(),
            &[
                Metaclass::Namespace,
                Metaclass::AnnotatingElement,
                Metaclass::Relationship
            ]
        );
    }

    #[test]
    fn part_usage_properties() {
        let props = Metaclass::PartUsage.properties();
        let owning = props.iter().find(|p| p.name == "owningMembership").unwrap();
        assert_eq!(
            owning.kind,
            PropertyKind::Reference {
                target: "OwningMembership"
            }
        );
        assert!(owning.nullable && !owning.many);
        let composite = props.iter().find(|p| p.name == "isComposite").unwrap();
        assert_eq!(composite.kind, PropertyKind::Boolean);
        assert!(props.iter().all(|p| p.name != "@id" && p.name != "@type"));
    }
}
