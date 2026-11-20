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
//! - [`Id`], [`Identified`], [`ConstraintValue`], [`DateTime`] and [`Iri`].
//!
//! Field presence follows `required` in the schema. Request bodies are checked strictly; response
//! records accept the omissions listed in ADR 0005. A property that is optional and nullable in a
//! request is an `Option<Option<T>>`, so that "absent" (leave unchanged) and `null` (clear) stay
//! distinct. `minItems` is enforced on deserialization.

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::fmt;
use std::str::FromStr;
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

/// A record or element identifier (Schema.json `"format": "uuid"`).
///
/// Only the RFC 9562 §4 string form is accepted: 8-4-4-4-12 hexadecimal digits with hyphens. The
/// URN, braced and unhyphenated forms are rejected. Hex digits are case-insensitive on input and
/// always written in lower case, as RFC 9562 recommends (ADR 0005).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub Uuid);

/// The error for a string that is not a hyphenated UUID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidId(pub String);

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid id {:?}: expected a hyphenated UUID (8-4-4-4-12 hex digits)",
            self.0
        )
    }
}

impl std::error::Error for InvalidId {}

impl FromStr for Id {
    type Err = InvalidId;

    fn from_str(s: &str) -> Result<Self, InvalidId> {
        let b = s.as_bytes();
        let hyphenated = b.len() == 36 && [8, 13, 18, 23].iter().all(|&i| b[i] == b'-');
        if !hyphenated {
            return Err(InvalidId(s.to_owned()));
        }
        Uuid::try_parse(s)
            .map(Id)
            .map_err(|_| InvalidId(s.to_owned()))
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), f)
    }
}

impl From<Uuid> for Id {
    fn from(u: Uuid) -> Self {
        Id(u)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(D::Error::custom)
    }
}

/// A reference to another record or element by its identifier: `{"@id": "…"}` (KerML 10.4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identified {
    /// The identifier of the referenced record or element.
    #[serde(rename = "@id")]
    pub id: Id,
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
    pub id: Id,
    /// The metaclass named by `@type`.
    #[serde(rename = "@type", with = "metaclass_name")]
    pub metaclass: Metaclass,
    /// Every other serialized property. Must not contain `@id` or `@type`; serialization fails if
    /// it does.
    #[serde(flatten, serialize_with = "element_properties")]
    pub properties: Map<String, Value>,
}

/// An element in a commit request (`ElementRequest` in `Schema.json`). `@id` is optional.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementRequest {
    /// The element identifier (`@id`), if the client supplies one.
    #[serde(rename = "@id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// The metaclass named by `@type`.
    #[serde(rename = "@type", with = "metaclass_name")]
    pub metaclass: Metaclass,
    /// Every other serialized property. Must not contain `@id` or `@type`; serialization fails if
    /// it does.
    #[serde(flatten, serialize_with = "element_properties")]
    pub properties: Map<String, Value>,
}

fn element_properties<S: Serializer>(m: &Map<String, Value>, s: S) -> Result<S::Ok, S::Error> {
    without_reserved(m, &["@id", "@type"], s)
}

/// Serializes a flattened property map, failing if it repeats a named property of its record.
fn without_reserved<S: Serializer>(
    m: &Map<String, Value>,
    reserved: &[&str],
    s: S,
) -> Result<S::Ok, S::Error> {
    if let Some(k) = reserved.iter().find(|k| m.contains_key(**k)) {
        return Err(serde::ser::Error::custom(format!(
            "property map repeats the named property `{k}`"
        )));
    }
    m.serialize(s)
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

// ---- deserialization helpers used by the generated code ----

/// A required, nullable request property: must be present; `null` reads as `None`. Using
/// `deserialize_with` without `default` makes serde report a missing property as an error.
fn present<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
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

fn check_len<E: serde::de::Error>(len: usize, min: usize) -> Result<(), E> {
    if len < min {
        Err(E::custom(format!(
            "expected at least {min} items, found {len}"
        )))
    } else {
        Ok(())
    }
}

/// A list with `minItems: N`.
fn min_items<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v = Vec::<T>::deserialize(d)?;
    check_len(v.len(), N)?;
    Ok(v)
}

/// A nullable list with `minItems: N`. Without `default`, the property must also be present.
fn min_items_opt<'de, D, T, const N: usize>(d: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v = Option::<Vec<T>>::deserialize(d)?;
    if let Some(items) = &v {
        check_len(items.len(), N)?;
    }
    Ok(v)
}

/// The `@type` of a union member, or an error naming the union.
fn type_of<E: serde::de::Error>(v: &Value, union: &str) -> Result<String, E> {
    match v.get("@type") {
        Some(Value::String(t)) => Ok(t.clone()),
        Some(other) => Err(E::custom(format!(
            "{union}: `@type` must be a string, found {other}"
        ))),
        None => Err(E::custom(format!("{union}: missing `@type`"))),
    }
}

/// Deserializes one union member, prefixing errors with the member's name.
fn member<T: DeserializeOwned, E: serde::de::Error>(v: Value, name: &str) -> Result<T, E> {
    serde_json::from_value(v).map_err(|e| E::custom(format!("{name}: {e}")))
}
