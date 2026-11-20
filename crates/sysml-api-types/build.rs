// SPDX-License-Identifier: Apache-2.0
//! Generates the API record types from the normative `Schema.json` of the Systems Modeling API
//! and Services 1.0 (the JSON models of the REST/HTTP PSM, API 8.1.2).
//!
//! The schema is not stored in git. It is fetched into `docs/specs/` by
//! `python scripts/fetch_specs.py fetch`. Set `EUREKA_SPECS_DIR` to use another location.
//!
//! How the schema is laid out (checked by `scripts/check_schema_assumptions.py`):
//! - 30 API definitions, recognized by `SystemsModelingAPI/` in their `$id`: 26 record objects and
//!   4 unions (`oneOf` of `$ref`s: Data, DataRequest, Constraint, ConstraintRequest).
//! - 182 `<Metaclass>Request` element schemas plus `Identified`. These are not generated here:
//!   element payloads use the hand-written `Element` and `ElementRequest` (see ADR 0005).
//! - Record properties: `@id` (uuid), `@type` (`const`), `Identified` references (with the target
//!   record in `$comment`), `oneOf [X, null]`, arrays, inline string enums, strings (plain,
//!   `uuid`, `date-time`, `uri`), booleans, and the PrimitiveConstraint value list.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=EUREKA_SPECS_DIR");
    let specs_dir = std::env::var_os("EUREKA_SPECS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../docs/specs")
        });
    let schema_path = specs_dir.join("api/omg/normative/Schema.json");
    println!("cargo:rerun-if-changed={}", schema_path.display());

    let text = std::fs::read_to_string(&schema_path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nThe spec files are not in git. Run `python scripts/fetch_specs.py fetch` first.",
            schema_path.display()
        )
    });
    let root: Value = serde_json::from_str(&text).expect("Schema.json is not valid JSON");
    let defs = root["$defs"].as_object().expect("Schema.json has no $defs");

    // Schema order is kept: it groups each record with its request type.
    let api: Vec<&str> = defs
        .iter()
        .filter(|(_, d)| {
            d["$id"]
                .as_str()
                .is_some_and(|id| id.contains("SystemsModelingAPI/"))
        })
        .map(|(n, _)| n.as_str())
        .collect();

    let mut gen = Gen::default();
    for name in &api {
        let def = defs[*name].as_object().unwrap();
        if def.contains_key("oneOf") {
            gen.union(name, def);
        } else {
            gen.record(name, def, &api);
        }
    }
    gen.finish(&api);

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("generated.rs");
    std::fs::write(&dest, gen.out).expect("cannot write generated code");
}

#[derive(Default)]
struct Gen {
    out: String,
    /// Every `@type` constant, for the `type_tag` module.
    tags: BTreeSet<String>,
    /// Inline enumerations: Rust name -> literals.
    enums: BTreeMap<&'static str, Vec<String>>,
    /// (record, property) -> referenced record, from `$comment` (after ADR 0005 corrections).
    references: BTreeMap<(String, String), String>,
}

/// A record name without its `Request` suffix: the PIM concept it carries.
fn base(name: &str) -> &str {
    name.strip_suffix("Request").unwrap_or(name)
}

/// The clause that defines each PIM concept, cited in the generated documentation.
fn clause(base: &str) -> &'static str {
    match base {
        "Project" | "Branch" | "Tag" | "Commit" | "Data" | "DataIdentity" | "DataVersion"
        | "ProjectUsage" => "API 7.1.2",
        "ExternalData" | "ExternalRelationship" => "API 7.1.3",
        "Query" | "Constraint" | "CompositeConstraint" | "PrimitiveConstraint" => "API 7.1.4",
        "DataDifference" => "API 7.2.3",
        "Error" => "API 8.1, REST/HTTP PSM only",
        other => {
            panic!("no clause recorded for API definition {other}; add it to clause() in build.rs")
        }
    }
}

/// The record a reference points to. `Schema.json` gives it in `$comment`; corrections are ADR 0005.
fn reference_target(record: &str, prop: &str, comment: Option<&str>) -> Option<String> {
    match (base(record), prop) {
        // API 7.1.2: "owningProject is the Project that owns the Commit". Schema.json says Branch.
        ("Commit", "owningProject") => Some("Project".to_owned()),
        _ => comment.map(|c| last_segment(c).to_owned()),
    }
}

/// Names for the inline string enumerations, taken from the PIM (API 7.1.4, Figure "Query").
fn inline_enum(record: &str, prop: &str) -> &'static str {
    match (base(record), prop) {
        ("PrimitiveConstraint", "operator") => "Operator",
        ("CompositeConstraint", "operator") => "JoinOperator",
        _ => panic!("unexpected inline enum {record}.{prop}; name it in inline_enum() in build.rs"),
    }
}

fn variant(literal: &str) -> String {
    match literal {
        "<" => "Lt".into(),
        "<=" => "Le".into(),
        "=" => "Eq".into(),
        ">" => "Gt".into(),
        ">=" => "Ge".into(),
        other => pascal(other),
    }
}

/// How one property is represented in Rust.
struct Ty {
    rust: String,
    nullable: bool,
    many: bool,
}

impl Gen {
    fn ty(&mut self, record: &str, prop: &str, s: &Value) -> Ty {
        let plain = |rust: &str| Ty {
            rust: rust.to_owned(),
            nullable: false,
            many: false,
        };
        if let Some(alts) = s.get("oneOf").and_then(Value::as_array) {
            let is_null = |a: &&Value| a.get("type").and_then(Value::as_str) == Some("null");
            let non_null: Vec<&Value> = alts.iter().filter(|a| !is_null(a)).collect();
            if non_null.len() == 1 && alts.len() == 2 {
                let inner = self.ty(record, prop, non_null[0]);
                assert!(!inner.nullable, "{record}.{prop}: nested nullable");
                return Ty {
                    nullable: true,
                    ..inner
                };
            }
            // API 7.1.4: PrimitiveConstraint.value items.
            let kinds: BTreeSet<String> = non_null
                .iter()
                .map(|a| match a.get("$ref").and_then(Value::as_str) {
                    Some(r) => last_segment(r).to_owned(),
                    None => a["type"].as_str().unwrap_or("?").to_owned(),
                })
                .collect();
            let expected: BTreeSet<String> = ["Identified", "boolean", "number", "string"]
                .map(String::from)
                .into();
            assert!(
                kinds == expected && alts.len() == 5,
                "{record}.{prop}: unrecognized oneOf {s}"
            );
            return plain("ConstraintValue");
        }
        if s.get("type").and_then(Value::as_str) == Some("array") {
            let item = self.ty(record, prop, &s["items"]);
            assert!(
                !item.nullable && !item.many,
                "{record}.{prop}: unsupported array item"
            );
            return Ty {
                rust: format!("Vec<{}>", item.rust),
                nullable: false,
                many: true,
            };
        }
        if let Some(r) = s.get("$ref").and_then(Value::as_str) {
            let target = last_segment(r);
            if target == "Identified" {
                let comment = s.get("$comment").and_then(Value::as_str);
                if let Some(to) = reference_target(record, prop, comment) {
                    self.references
                        .insert((record.to_owned(), prop.to_owned()), to);
                }
            }
            return plain(target);
        }
        if let Some(lits) = s.get("enum").and_then(Value::as_array) {
            let name = inline_enum(record, prop);
            let lits: Vec<String> = lits
                .iter()
                .map(|l| l.as_str().unwrap().to_owned())
                .collect();
            let prev = self.enums.entry(name).or_insert_with(|| lits.clone());
            assert_eq!(
                *prev, lits,
                "{record}.{prop}: literals differ from earlier {name}"
            );
            return plain(name);
        }
        let kind = match (s["type"].as_str(), s.get("format").and_then(Value::as_str)) {
            (Some("string"), Some("uuid")) => "Uuid",
            (Some("string"), Some("date-time")) => "DateTime",
            (Some("string"), Some("uri")) => "Iri",
            (Some("string"), None) => "String",
            (Some("boolean"), None) => "bool",
            _ => panic!("{record}.{prop}: unrecognized property schema {s}"),
        };
        plain(kind)
    }

    fn record(&mut self, name: &str, def: &Map<String, Value>, api: &[&str]) {
        assert_eq!(def["type"], "object", "{name}: not an object schema");
        let props = def["properties"].as_object().unwrap();
        let required: Vec<&str> = def
            .get("required")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let open = def.get("additionalProperties") == Some(&Value::Bool(true));
        let b = base(name);
        let doc = if name.ends_with("Request") {
            format!("Request body for a `{b}` ({}).", clause(b))
        } else {
            format!("The `{b}` record ({}).", clause(b))
        };

        let mut fields = String::new();
        for (pname, ps) in props {
            let is_required = required.contains(&pname.as_str());
            if pname == "@type" {
                let tag = ps["const"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{name}: @type has no const"));
                assert_eq!(tag, b, "{name}: @type const differs from the record name");
                self.tags.insert(tag.to_owned());
                fields
                    .push_str("    /// The `@type` discriminator.\n    #[serde(rename = \"@type\"");
                if !is_required {
                    fields.push_str(", default");
                }
                writeln!(fields, ")]\n    pub type_tag: type_tag::{tag},").unwrap();
                continue;
            }
            let ty = self.ty(name, pname, ps);
            for t in ty
                .rust
                .trim_start_matches("Vec<")
                .trim_end_matches('>')
                .split(['<', '>'])
            {
                let known = [
                    "Uuid",
                    "DateTime",
                    "Iri",
                    "String",
                    "bool",
                    "Identified",
                    "Element",
                    "ElementRequest",
                    "ConstraintValue",
                    "Operator",
                    "JoinOperator",
                ];
                assert!(
                    known.contains(&t) || api.contains(&t),
                    "{name}.{pname}: unknown type {t}"
                );
            }
            let field = if pname == "@id" {
                "id".to_owned()
            } else {
                snake(pname)
            };
            let mut doc = format!("`{pname}`");
            if let Some(to) = self.references.get(&(name.to_owned(), pname.clone())) {
                write!(doc, ": reference to a `{to}`").unwrap();
            }
            let (rust, attrs, note) = match (is_required, ty.nullable) {
                (true, false) if ty.many => (ty.rust, ", default", "Required; a missing list reads as empty (ADR 0005)."),
                (true, false) => (ty.rust, "", "Required."),
                (true, true) => (format!("Option<{}>", ty.rust), ", default", "Required, nullable: `None` is written as `null`."),
                (false, false) => (
                    format!("Option<{}>", ty.rust),
                    ", default, skip_serializing_if = \"Option::is_none\"",
                    "Optional: `None` is omitted.",
                ),
                (false, true) => (
                    format!("Option<Option<{}>>", ty.rust),
                    ", default, skip_serializing_if = \"Option::is_none\", deserialize_with = \"crate::double_option\"",
                    "Optional, nullable: `None` is absent, `Some(None)` is `null`.",
                ),
            };
            writeln!(fields, "    /// {doc}. {note}\n    #[serde(rename = \"{pname}\"{attrs})]\n    pub {}: {rust},", raw(&field)).unwrap();
        }
        if open {
            fields.push_str(
                "    /// Properties not named by the schema (`additionalProperties: true`).\n",
            );
            fields.push_str("    #[serde(flatten)]\n    pub additional: Map<String, Value>,\n");
        }

        writeln!(self.out, "/// {doc}").unwrap();
        self.out
            .push_str("#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n");
        if !open {
            self.out.push_str("#[serde(deny_unknown_fields)]\n");
        }
        writeln!(self.out, "pub struct {name} {{\n{fields}}}\n").unwrap();
    }

    fn union(&mut self, name: &str, def: &Map<String, Value>) {
        let mut variants: Vec<(String, String)> = def["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                let target = last_segment(a["$ref"].as_str().expect("union member is not a $ref"))
                    .to_owned();
                (base(&target).to_owned(), target)
            })
            .collect();
        // Elements are matched last: they accept any metaclass and open-ended properties.
        variants.sort_by_key(|(v, _)| v == "Element");
        let b = base(name);
        writeln!(
            self.out,
            "/// {} ({}). Members are told apart by `@type`.",
            if name.ends_with("Request") {
                format!("Request form of `{b}`")
            } else {
                format!("`{b}`")
            },
            clause(b)
        )
        .unwrap();
        writeln!(self.out, "#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[serde(untagged)]\npub enum {name} {{").unwrap();
        for (v, target) in &variants {
            writeln!(self.out, "    /// `{target}`\n    {v}({target}),").unwrap();
        }
        self.out.push_str("}\n\n");
    }

    fn finish(&mut self, api: &[&str]) {
        let mut out = String::from(
            "// @generated by crates/sysml-api-types/build.rs from Schema.json. Do not edit.\n\n",
        );

        out.push_str(
            "/// Marker types for the `@type` discriminator of each record.\npub mod type_tag {\n",
        );
        for t in &self.tags {
            writeln!(out, "    type_tag!({t});").unwrap();
        }
        out.push_str("}\n\n");

        for (name, lits) in &self.enums {
            let all = lits
                .iter()
                .map(|l| format!("{name}::{}", variant(l)))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(out, "/// `{name}` (API 7.1.4).").unwrap();
            out.push_str(
                "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]\n",
            );
            writeln!(out, "pub enum {name} {{").unwrap();
            for l in lits {
                writeln!(
                    out,
                    "    /// `{l}`\n    #[serde(rename = \"{l}\")]\n    {},",
                    variant(l)
                )
                .unwrap();
            }
            writeln!(out, "}}\n\nimpl {name} {{\n    /// All literals, in specification order.\n    pub const ALL: &'static [{name}] = &[{all}];\n").unwrap();
            out.push_str("    /// The literal as written in JSON.\n    pub const fn as_str(self) -> &'static str {\n        match self {\n");
            for l in lits {
                writeln!(out, "            {name}::{} => \"{l}\",", variant(l)).unwrap();
            }
            out.push_str("        }\n    }\n}\n\n");
        }

        writeln!(
            out,
            "/// Every API definition in `Schema.json`, in schema order.\npub const API_DEFINITIONS: &[&str] = &[{}];\n",
            api.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")
        )
        .unwrap();

        out.push_str("/// The record that a reference property points to, for example `(\"Branch\", \"head\")` -> `Commit`.\n");
        out.push_str("/// Corrections to `Schema.json` are recorded in ADR 0005.\n");
        out.push_str("pub fn reference_target(record: &str, property: &str) -> Option<&'static str> {\n    match (record, property) {\n");
        for ((r, p), to) in &self.references {
            writeln!(out, "        (\"{r}\", \"{p}\") => Some(\"{to}\"),").unwrap();
        }
        out.push_str("        _ => None,\n    }\n}\n\n");

        out.push_str(&self.out);
        self.out = out;
    }
}

fn last_segment(iri: &str) -> &str {
    iri.rsplit('/').next().unwrap_or(iri)
}

/// `owningProject` -> `owning_project`.
fn snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Raw identifier for Rust keywords (`where` -> `r#where`).
fn raw(ident: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod",
        "move", "mut", "pub", "ref", "return", "static", "struct", "trait", "true", "try", "type",
        "unsafe", "use", "where", "while", "yield",
    ];
    if KEYWORDS.contains(&ident) {
        format!("r#{ident}")
    } else {
        ident.to_owned()
    }
}

/// `instanceOf` -> `InstanceOf`.
fn pascal(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
