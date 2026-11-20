// SPDX-License-Identifier: Apache-2.0
//! Record types of the Systems Modeling API and Services 1.0.
//!
//! The records ([`Project`], [`Commit`], [`DataVersion`], [`Query`], …), their request forms and
//! the [`Operator`]/[`JoinOperator`] enumerations are generated at build time from the normative
//! REST/HTTP PSM schema `Schema.json` (see `build.rs`). The JSON mapping follows API 8.1.2.
//!
//! Hand-written here:
//! - [`Element`] and [`ElementRequest`]: element payloads, kept as open property maps keyed by
//!   their metaclass. Typed element access belongs to `sysml-model` (ADR 0005).
//! - [`Identified`], [`ConstraintValue`], [`DateTime`] and [`Iri`].
//!
//! Field presence follows `required` in the schema. A property that is optional and nullable in a
//! request is an `Option<Option<T>>`, so that "absent" (leave unchanged) and `null` (clear) stay
//! distinct.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use sysml_model::Metaclass;

pub use uuid::Uuid;

/// Defines a marker type for one `@type` value. It serializes as that string and fails to
/// deserialize from any other value, so that records cannot be confused with each other.
macro_rules! type_tag {
    ($name:ident) => {
        #[doc = concat!("The `@type` value `\"", stringify!($name), "\"`.")]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name;

        impl $name {
            /// The discriminator value.
            pub const VALUE: &'static str = stringify!($name);
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(Self::VALUE)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                if s == Self::VALUE {
                    Ok($name)
                } else {
                    Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(&s),
                        &Self::VALUE,
                    ))
                }
            }
        }
    };
}

#[allow(clippy::all, missing_docs)] // lints are not useful on generated code
mod generated {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
pub use generated::*;

/// A reference to another record or element by its identifier: `{"@id": "…"}` (KerML 10.4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identified {
    /// The identifier of the referenced record or element.
    #[serde(rename = "@id")]
    pub id: Uuid,
}

/// A timestamp in ISO 8601 format (API 7.1.2, `ISO8601DateTime`), kept as written.
/// Parsing and ordering are left to `sysml-versioning`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DateTime(pub String);

/// An IRI (API 7.1.1 `resourceIdentifier`, API 7.1.3 `ExternalData.resourceIdentifier`), kept as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Iri(pub String);

/// One entry of `PrimitiveConstraint.value` (API 7.1.4): a boolean, a number, a string, an element
/// reference or null.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConstraintValue {
    /// A boolean value.
    Boolean(bool),
    /// A number, kept exactly as written (integer or decimal).
    Number(serde_json::Number),
    /// A string value.
    String(String),
    /// A reference to an element.
    Reference(Identified),
    /// `null`.
    Null,
}

/// An element as API `Data` (API 7.1.2), in the KerML 10.4.5 JSON serialization.
///
/// `@type` must name a SysML metaclass. All other properties are kept as JSON values; validating
/// them against the metaclass is left to the semantic layer (ADR 0005).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    /// The element identifier (`@id`).
    #[serde(rename = "@id")]
    pub id: Uuid,
    /// The metaclass named by `@type`.
    #[serde(rename = "@type", with = "metaclass_name")]
    pub metaclass: Metaclass,
    /// Every other serialized property.
    #[serde(flatten)]
    pub properties: Map<String, Value>,
}

/// An element in a commit request (`ElementRequest` in `Schema.json`). `@id` is optional.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementRequest {
    /// The element identifier (`@id`), if the client supplies one.
    #[serde(rename = "@id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    /// The metaclass named by `@type`.
    #[serde(rename = "@type", with = "metaclass_name")]
    pub metaclass: Metaclass,
    /// Every other serialized property.
    #[serde(flatten)]
    pub properties: Map<String, Value>,
}

/// Serializes a [`Metaclass`] as its `@type` name.
mod metaclass_name {
    use super::*;

    pub fn serialize<S: serde::Serializer>(m: &Metaclass, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(m.name())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Metaclass, D::Error> {
        let name = String::deserialize(d)?;
        Metaclass::parse(&name).ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&name),
                &"a SysML metaclass name",
            )
        })
    }
}

/// Deserializes a present property (including `null`) as `Some(…)`. Used together with
/// `#[serde(default)]`, which yields `None` when the property is absent.
fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}
