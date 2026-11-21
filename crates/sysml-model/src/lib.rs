// SPDX-License-Identifier: Apache-2.0
//! SysML v2 / KerML metaclass definitions.
//!
//! [`Metaclass`], its property descriptors and the metamodel enumerations are generated at
//! build time from the normative `SysML.json` (see `build.rs`), and the stored attributes of the
//! KerML metaclasses from `KerML.xmi`. [`Graph`] is the arena-based element graph (ADR 0017).

#[allow(clippy::all, missing_docs)] // lints are not useful on generated code
mod generated {
    use super::{PropertyDef, PropertyKind, Slot, StoredAttribute};
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
pub use generated::*;

mod graph;
pub use graph::{
    Element, ElementIx, Endpoint, FileId, Graph, GraphError, NameIx, Property, QualifiedName, Span,
    Value,
};

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

/// The slot of the stored core of an element that a stored attribute is a view of (ADR 0017).
///
/// Every stored reference attribute of KerML redefines one of the six reference attributes of
/// `Element` and `Relationship` (KerML 8.3.2), directly or through others: `FeatureTyping::type`,
/// for example, redefines `Specialization::general`, which redefines `Relationship::target`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// `Element::owningRelationship`
    OwningRelationship,
    /// `Element::ownedRelationship`
    OwnedRelationship,
    /// `Relationship::owningRelatedElement`
    OwningRelatedElement,
    /// `Relationship::ownedRelatedElement`
    OwnedRelatedElement,
    /// `Relationship::source`
    Source,
    /// `Relationship::target`
    Target,
    /// An attribute that is not a reference: a name, a flag, an enumeration value or a literal.
    Value(Attr),
}

/// A stored attribute of a KerML metaclass (KerML 8.3), generated from `KerML.xmi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredAttribute {
    /// The name, for example `type`.
    pub name: &'static str,
    /// The slot it is a view of, for example [`Slot::Target`].
    pub slot: Slot,
    /// The value type.
    pub kind: PropertyKind,
    /// Whether it can have several values.
    pub many: bool,
    /// The default value in the metamodel, as text (`true`, `public` …), if it has one.
    pub default: Option<&'static str>,
}

impl Metaclass {
    /// Whether this metaclass is `general` or one of its direct or indirect subtypes.
    pub fn specializes(self, general: Metaclass) -> bool {
        self == general
            || general
                .direct_subtypes()
                .iter()
                .any(|s| self.specializes(*s))
    }
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

    fn stored(m: Metaclass, name: &str) -> StoredAttribute {
        *m.stored_attributes()
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("{m:?}::{name}"))
    }

    #[test]
    fn the_82_kerml_metaclasses_have_stored_attributes() {
        // KerML 8.3: 82 metaclasses (ADR 0017, from KerML.xmi).
        let kerml: Vec<_> = Metaclass::ALL.iter().filter(|m| m.is_kerml()).collect();
        assert_eq!(kerml.len(), 82);
        assert!(kerml.iter().all(|m| !m.stored_attributes().is_empty()));
        assert!(
            Metaclass::PartUsage.stored_attributes().is_empty(),
            "SysML in Phase 1b"
        );
        assert_eq!(Attr::ALL.len(), 31);
    }

    #[test]
    fn relationship_ends_are_views_of_source_and_target() {
        // KerML 8.3.3.3.4: `type` redefines `general` (8.3.3.1.8), which redefines `target`.
        let ty = stored(Metaclass::FeatureTyping, "type");
        assert_eq!(ty.slot, Slot::Target);
        assert!(!ty.many, "a typing has one type");
        assert_eq!(
            stored(Metaclass::FeatureTyping, "typedFeature").slot,
            Slot::Source
        );
        assert_eq!(
            stored(Metaclass::Redefinition, "redefinedFeature").slot,
            Slot::Target
        );
        assert_eq!(
            stored(Metaclass::Membership, "memberElement").slot,
            Slot::Target
        );
        let client = stored(Metaclass::Dependency, "client");
        assert_eq!((client.slot, client.many), (Slot::Source, true));
        let target = stored(Metaclass::Relationship, "target");
        assert_eq!((target.slot, target.many), (Slot::Target, true));
        let owned = stored(Metaclass::Element, "ownedRelationship");
        assert_eq!((owned.slot, owned.many), (Slot::OwnedRelationship, true));
    }

    #[test]
    fn a_derived_redefinition_hides_the_ends_it_redefines() {
        // KerML 8.3.3.3.9: ReferenceSubsetting::referencingFeature is derived and redefines
        // subsettingFeature, so a ReferenceSubsetting stores no source; likewise
        // Unioning::typeUnioned (8.3.3.1.10) and FeatureChaining::featureChained (8.3.3.3.5).
        let has = |m: Metaclass, slot: Slot| m.stored_attributes().iter().any(|a| a.slot == slot);
        for m in [
            Metaclass::ReferenceSubsetting,
            Metaclass::CrossSubsetting,
            Metaclass::Unioning,
            Metaclass::FeatureChaining,
            Metaclass::Membership,
            Metaclass::FeatureValue,
        ] {
            assert!(!has(m, Slot::Source), "{m:?} stores no source");
        }
        assert!(
            !has(Metaclass::FeatureValue, Slot::Target),
            "FeatureValue::value is derived"
        );
        for m in [
            Metaclass::Subsetting,
            Metaclass::Redefinition,
            Metaclass::FeatureTyping,
            Metaclass::Specialization,
            Metaclass::Dependency,
        ] {
            assert!(has(m, Slot::Source), "{m:?} stores its source");
        }
        assert_eq!(
            stored(Metaclass::ReferenceSubsetting, "referencedFeature").slot,
            Slot::Target
        );
        assert!(Metaclass::ReferenceSubsetting
            .stored_attributes()
            .iter()
            .all(|a| a.name != "subsettingFeature" && a.name != "source"));
    }

    #[test]
    fn scalar_attributes_and_their_defaults() {
        let unique = stored(Metaclass::Feature, "isUnique");
        assert_eq!(unique.slot, Slot::Value(Attr::IsUnique));
        assert_eq!(unique.default, Some("true"));
        assert_eq!(
            stored(Metaclass::Feature, "isOrdered").default,
            Some("false")
        );
        assert_eq!(
            stored(Metaclass::Membership, "visibility").default,
            Some("public")
        );
        assert_eq!(stored(Metaclass::Feature, "direction").default, None);
        assert_eq!(
            stored(Metaclass::LiteralInteger, "value").kind,
            PropertyKind::Integer
        );
        assert_eq!(
            stored(Metaclass::LiteralString, "value").kind,
            PropertyKind::String
        );
        assert!(stored(Metaclass::Element, "aliasIds").many);
        // An attribute redefined under the same name is listed once, as the redefinition.
        let operators = Metaclass::FeatureChainExpression
            .stored_attributes()
            .iter()
            .filter(|a| a.name == "operator")
            .count();
        assert_eq!(operators, 1);
    }

    #[test]
    fn specialization_of_metaclasses() {
        assert!(Metaclass::Function.specializes(Metaclass::Behavior));
        assert!(Metaclass::Invariant.specializes(Metaclass::Step));
        assert!(
            Metaclass::Flow.specializes(Metaclass::Step),
            "KerML 8.3.4.9.2"
        );
        assert!(Metaclass::Feature.specializes(Metaclass::Element));
        assert!(!Metaclass::Class.specializes(Metaclass::Behavior));
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
