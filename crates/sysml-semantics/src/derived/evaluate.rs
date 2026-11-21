// SPDX-License-Identifier: Apache-2.0
//! Model-level evaluation (KerML 8.4.4.9.8, 8.3.4.8; ADR 0021): `evaluate`, `modelLevelEvaluable`
//! and what uses them: `checkCondition`, multiplicity bounds, semantic metadata and filters.
//!
//! An evaluation gives a list of [`Item`]s. KerML gives Elements: LiteralExpressions and Features
//! that are not Expressions. A literal that evaluation computes, the `3` of `1 + 2`, is not in the
//! model, so a literal is an item of its value. The reflective metadata of an element (8.4.4.9.8
//! rule 4, 7.4.14) is an item of its own.

use std::collections::HashSet;
use std::sync::Arc;
use sysml_model::{Attr, Metaclass as M, Value};

use super::kernel::{
    argument, chain_target_feature, filter_condition, instantiated_type, lower_bound,
    referenced_element, referent, upper_bound,
};
use super::root::{membership_element, name, owned_element, owning_namespace, qualified_name};
use super::types::{
    all_redefined_features, feature, featuring_type, library, owned_feature, owning_type, r#type,
    specializes,
};
use super::{at, metaclass};
use crate::resolve::{structure, Global, Project};

/// A value of model-level evaluation.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A Feature that is not an Expression, or a MetadataFeature: an element of the model.
    Element(Global),
    /// The reflective metadata of an element: an implicit MetadataFeature typed by the metaclass
    /// of the element in the KerML library model (8.4.4.9.8 rule 4, 7.4.14).
    Reflective(Global),
    /// A Boolean literal.
    Boolean(bool),
    /// An Integer literal.
    Integer(i64),
    /// A Rational (Real) literal.
    Rational(f64),
    /// A String literal.
    String(Arc<str>),
    /// The literal `*`.
    Infinity,
}

/// A natural number or `*`, the value of a multiplicity bound (`UnlimitedNatural`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Natural {
    /// A natural number.
    Value(u64),
    /// `*`.
    Infinite,
}

fn kind_of(db: &dyn salsa::Database, x: Global, m: M) -> bool {
    metaclass(db, x).specializes(m)
}

fn qualified(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Arc<str>> {
    qualified_name(db, project, x).clone()
}

/// The value Expression of a feature's FeatureValue (KerML 8.3.4.10).
fn valuation(db: &dyn salsa::Database, f: Global) -> Option<Global> {
    let s = structure(db, f.file);
    let g = s.graph();
    let r = g
        .all_owned_relationships(f.ix())
        .find(|r| g.metaclass(*r).specializes(M::FeatureValue))?;
    g.owned_related_elements(r).first().map(|e| at(f.file, *e))
}

/// The library functions of Tables 5 and 7 (KerML 8.2.5.8.1, 8.2.5.8.2) that are model-level
/// evaluable, by qualified name.
const EVALUABLE: &[&str] = &[
    "BaseFunctions::'=='",
    "BaseFunctions::'!='",
    "BaseFunctions::'==='",
    "BaseFunctions::'!=='",
    "BaseFunctions::istype",
    "BaseFunctions::hastype",
    "BaseFunctions::'@'",
    "BaseFunctions::'@@'",
    "BaseFunctions::as",
    "BaseFunctions::meta",
    "BaseFunctions::'#'",
    "BaseFunctions::','",
    "DataFunctions::not",
    "DataFunctions::xor",
    "DataFunctions::'|'",
    "DataFunctions::'&'",
    "DataFunctions::'<'",
    "DataFunctions::'>'",
    "DataFunctions::'<='",
    "DataFunctions::'>='",
    "DataFunctions::'+'",
    "DataFunctions::'-'",
    "DataFunctions::'*'",
    "DataFunctions::'/'",
    "DataFunctions::'%'",
    "DataFunctions::'^'",
    "DataFunctions::'..'",
    "ControlFunctions::'??'",
    "ControlFunctions::if",
    "ControlFunctions::or",
    "ControlFunctions::and",
    "ControlFunctions::implies",
    "ControlFunctions::'.'",
    "ControlFunctions::collect",
    "ControlFunctions::select",
];

/// `Function::isModelLevelEvaluable` (KerML 8.3.4.7.4): true for the library functions listed as
/// model-level evaluable in Tables 5 and 7, false for every other function.
pub fn function_is_model_level_evaluable(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> bool {
    qualified(db, project, f).is_some_and(|q| EVALUABLE.contains(&&*q))
}

// ---- operands ----

/// An operand of an OperatorExpression: a value, a type (after a classification operator), or an
/// expression that the function evaluates itself (`and`, `if`, `collect` …).
enum Operand {
    Value(Global),
    Type(Global),
    Body(Global),
}

/// The owned expression an argument refers to, when it is passed unevaluated: an
/// `OwnedExpressionReference` (KerML 8.2.5.8.1), a FeatureReferenceExpression that owns its
/// referent.
fn body_of(db: &dyn salsa::Database, project: Project, value: Global) -> Option<Global> {
    if !kind_of(db, value, M::FeatureReferenceExpression) {
        return None;
    }
    let r = referent(db, project, value).or_else(|| {
        // The referent of an owned body need not be a Feature: it is an Expression.
        let s = structure(db, value.file);
        let g = s.graph();
        g.all_owned_relationships(value.ix())
            .find(|m| g.metaclass(*m) == M::FeatureMembership)
            .and_then(|m| {
                g.owned_related_elements(m)
                    .first()
                    .map(|e| at(value.file, *e))
            })
    })?;
    let owned = structure(db, r.file)
        .graph()
        .owner(r.ix())
        .map(|o| at(r.file, o))
        == Some(value);
    (owned && kind_of(db, r, M::Expression)).then_some(r)
}

fn operands(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Operand> {
    let s = structure(db, x.file);
    let g = s.graph();
    let mut out = Vec::new();
    for r in g.all_owned_relationships(x.ix()) {
        let m = g.metaclass(r);
        if !m.specializes(M::ParameterMembership) || m.specializes(M::ReturnParameterMembership) {
            continue;
        }
        let Some(p) = g.owned_related_elements(r).first().map(|e| at(x.file, *e)) else {
            continue;
        };
        match valuation(db, p) {
            Some(v) => out.push(match body_of(db, project, v) {
                Some(b) => Operand::Body(b),
                None => Operand::Value(v),
            }),
            None => {
                if let Some(t) = r#type(db, project, p).first() {
                    out.push(Operand::Type(*t));
                }
            }
        }
    }
    out
}

/// The type of a cast, written as the result's typing (`as`, `meta`: KerML 8.2.5.8.1,
/// TypeResultMember).
fn cast_type(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    let s = structure(db, x.file);
    let g = s.graph();
    let r = g
        .all_owned_relationships(x.ix())
        .find(|r| g.metaclass(*r).specializes(M::ReturnParameterMembership))?;
    let p = g
        .owned_related_elements(r)
        .first()
        .map(|e| at(x.file, *e))?;
    r#type(db, project, p).first().copied()
}

// ---- evaluation ----

struct Evaluation<'a> {
    db: &'a dyn salsa::Database,
    project: Project,
    /// The values bound to the parameters of the bodies being evaluated (`collect`, `select`).
    bindings: Vec<(Global, Vec<Item>)>,
    /// The expressions being evaluated, on which target: a value that refers to itself has none.
    active: HashSet<(Global, Option<Global>)>,
}

fn target_key(t: &Option<Item>) -> Option<Global> {
    match t {
        Some(Item::Element(g) | Item::Reflective(g)) => Some(*g),
        _ => None,
    }
}

impl Evaluation<'_> {
    fn evaluate(&mut self, x: Global, target: &Option<Item>) -> Vec<Item> {
        let key = (x, target_key(target));
        if !self.active.insert(key) {
            return Vec::new();
        }
        let out = self.evaluate_once(x, target);
        self.active.remove(&key);
        out
    }

    fn evaluate_once(&mut self, x: Global, target: &Option<Item>) -> Vec<Item> {
        let (db, project) = (self.db, self.project);
        let m = metaclass(db, x);
        if m.specializes(M::NullExpression) {
            return Vec::new();
        }
        if m.specializes(M::LiteralExpression) {
            return literal(db, x).into_iter().collect();
        }
        if m.specializes(M::FeatureReferenceExpression) {
            let Some(r) = referent(db, project, x) else {
                return Vec::new();
            };
            return self.feature_value(r, target);
        }
        if m.specializes(M::MetadataAccessExpression) {
            return referenced_element(db, project, x)
                .map(|e| metadata_of(db, project, e))
                .unwrap_or_default();
        }
        if m.specializes(M::InvocationExpression) {
            return self.invoke(x, target);
        }
        // Expression::evaluate: the first result expression's value (KerML 8.3.4.7.3).
        let s = structure(db, x.file);
        let g = s.graph();
        let result = g
            .all_owned_relationships(x.ix())
            .find(|r| g.metaclass(*r).specializes(M::ResultExpressionMembership))
            .and_then(|r| g.owned_related_elements(r).first().map(|e| at(x.file, *e)));
        match result {
            Some(e) => self.evaluate(e, target),
            None => Vec::new(),
        }
    }

    /// A FeatureReferenceExpression's value (KerML 8.4.4.9.8 rule 3): the value of the target's
    /// feature that is or redefines the referent; else, for a referent without featuring types,
    /// the value of its own FeatureValue; else the referent itself, if it is not an Expression.
    fn feature_value(&mut self, r: Global, target: &Option<Item>) -> Vec<Item> {
        let (db, project) = (self.db, self.project);
        if let Some((_, items)) = self.bindings.iter().rev().find(|(p, _)| *p == r) {
            return items.clone();
        }
        // `Anything::self`, explicitly or as the omitted operand: the target (7.4.9.2).
        if is_anything_self(db, project, r) {
            return target.clone().into_iter().collect();
        }
        match target {
            Some(Item::Reflective(e)) => {
                if let Some(n) = name(db, project, r) {
                    let values = reflective_property(db, project, *e, &n);
                    if !values.is_empty() || library_owner(db, project, r) {
                        return values;
                    }
                }
            }
            Some(Item::Element(t)) if kind_of(db, *t, M::Type) => {
                let found = feature(db, project, *t)
                    .into_iter()
                    .find(|f| all_redefined_features(db, project, *f).contains(&r));
                if let Some(f) = found {
                    // That feature's value; without one, the referent (8.4.4.9.8 rule 3).
                    return match valuation(db, f) {
                        Some(v) => self.evaluate(v, target),
                        None if !kind_of(db, r, M::Expression) => vec![Item::Element(r)],
                        None => Vec::new(),
                    };
                }
            }
            _ => {}
        }
        // A referent with featuring types that the target does not give a value has none (the
        // OCL's `else Sequence{}`), so a filter does not hold on metadata without that feature.
        if !featuring_type(db, project, r).is_empty() {
            return Vec::new();
        }
        if let Some(v) = valuation(db, r) {
            return self.evaluate(v, target);
        }
        if kind_of(db, r, M::Expression) {
            Vec::new()
        } else {
            vec![Item::Element(r)]
        }
    }

    fn operand(&mut self, o: &Operand, target: &Option<Item>) -> Vec<Item> {
        match o {
            Operand::Value(v) | Operand::Body(v) => self.evaluate(*v, target),
            Operand::Type(_) => Vec::new(),
        }
    }

    /// Evaluate a body with its first input parameter bound to an item.
    fn apply(&mut self, body: Global, item: Item, target: &Option<Item>) -> Vec<Item> {
        let (db, project) = (self.db, self.project);
        let s = structure(db, body.file);
        let g = s.graph();
        let param = owned_feature(db, project, body).into_iter().find(|f| {
            matches!(
                g.values(f.ix(), Attr::Direction).next(),
                Some(Value::Enumeration("in"))
            )
        });
        if let Some(p) = param {
            self.bindings.push((p, vec![item]));
        }
        let out = self.evaluate(body, target);
        if param.is_some() {
            self.bindings.pop();
        }
        out
    }

    fn invoke(&mut self, x: Global, target: &Option<Item>) -> Vec<Item> {
        let (db, project) = (self.db, self.project);
        let Some(function) = instantiated_type(db, project, x) else {
            return Vec::new();
        };
        let Some(q) = qualified(db, project, function) else {
            return Vec::new();
        };
        let ops: Vec<Operand> = if kind_of(db, x, M::OperatorExpression) {
            operands(db, project, x)
        } else {
            argument(db, project, x)
                .into_iter()
                .map(Operand::Value)
                .collect()
        };
        let values = |this: &mut Self, i: usize| -> Vec<Item> {
            ops.get(i)
                .map(|o| this.operand(o, target))
                .unwrap_or_default()
        };
        let boolean = |v: &[Item]| match v {
            [Item::Boolean(b)] => Some(*b),
            _ => None,
        };
        let q: &str = &q;
        match q {
            "ControlFunctions::'.'" => {
                let source = values(self, 0);
                let Some(f) = chain_target_feature(db, project, x) else {
                    return Vec::new();
                };
                source
                    .into_iter()
                    .flat_map(|item| match item {
                        Item::Element(_) | Item::Reflective(_) => {
                            self.feature_value(f, &Some(item))
                        }
                        _ => Vec::new(),
                    })
                    .collect()
            }
            "ControlFunctions::and" | "ControlFunctions::or" | "ControlFunctions::implies" => {
                let Some(a) = boolean(&values(self, 0)) else {
                    return Vec::new();
                };
                let short = match q {
                    "ControlFunctions::and" => (!a).then_some(false),
                    "ControlFunctions::or" => a.then_some(true),
                    _ => (!a).then_some(true),
                };
                if let Some(r) = short {
                    return vec![Item::Boolean(r)];
                }
                boolean(&values(self, 1))
                    .map(|b| vec![Item::Boolean(b)])
                    .unwrap_or_default()
            }
            "ControlFunctions::if" => match boolean(&values(self, 0)) {
                Some(true) => values(self, 1),
                Some(false) => values(self, 2),
                None => Vec::new(),
            },
            "ControlFunctions::'??'" => {
                let a = values(self, 0);
                if a.is_empty() {
                    values(self, 1)
                } else {
                    a
                }
            }
            "ControlFunctions::collect" | "ControlFunctions::select" => {
                let source = values(self, 0);
                let Some(Operand::Body(body) | Operand::Value(body)) = ops.get(1) else {
                    return Vec::new();
                };
                let body = *body;
                let mut out = Vec::new();
                for item in source {
                    let r = self.apply(body, item.clone(), target);
                    if q == "ControlFunctions::collect" {
                        out.extend(r);
                    } else if boolean(&r) == Some(true) {
                        out.push(item);
                    }
                }
                out
            }
            "BaseFunctions::istype"
            | "BaseFunctions::hastype"
            | "BaseFunctions::'@'"
            | "BaseFunctions::'@@'" => {
                // Without a first operand, it is `Anything::self`, the target (7.4.9.2).
                let (subject, ty) = match ops.as_slice() {
                    [Operand::Type(t)] => (target.clone().into_iter().collect(), *t),
                    [o, Operand::Type(t)] => (self.operand(o, target), *t),
                    _ => return Vec::new(),
                };
                let direct = q == "BaseFunctions::hastype";
                let any = matches!(q, "BaseFunctions::'@'" | "BaseFunctions::'@@'");
                let test = |i: &Item| classified(db, project, i, ty, direct);
                let r = if any {
                    subject.iter().any(test)
                } else {
                    subject.iter().all(test)
                };
                vec![Item::Boolean(r)]
            }
            "BaseFunctions::as" | "BaseFunctions::meta" => {
                let Some(ty) = cast_type(db, project, x) else {
                    return Vec::new();
                };
                let subject = match ops.first() {
                    Some(o) => self.operand(o, target),
                    None => target.clone().into_iter().collect(),
                };
                subject
                    .into_iter()
                    .filter(|i| classified(db, project, i, ty, false))
                    .collect()
            }
            _ => {
                let args: Vec<Vec<Item>> = (0..ops.len()).map(|i| values(self, i)).collect();
                apply_data_function(q, &args)
            }
        }
    }
}

/// A literal's value (KerML 8.3.4.8.9–8.3.4.8.14).
fn literal(db: &dyn salsa::Database, x: Global) -> Option<Item> {
    let s = structure(db, x.file);
    let g = s.graph();
    if g.metaclass(x.ix()).specializes(M::LiteralInfinity) {
        return Some(Item::Infinity);
    }
    Some(match g.values(x.ix(), Attr::Value).next()? {
        Value::Boolean(b) => Item::Boolean(*b),
        Value::Integer(i) => Item::Integer(*i),
        Value::Real(r) => Item::Rational(*r),
        Value::String(s) => Item::String(Arc::from(&**s)),
        Value::Enumeration(_) => return None,
    })
}

/// Whether a feature is `Anything::self` or specializes it (`conformsTo`, 8.3.4.8.5).
fn is_anything_self(db: &dyn salsa::Database, project: Project, r: Global) -> bool {
    library(db, project, "Base::Anything::self")
        .is_some_and(|s| r == s || specializes(db, project, r, s))
}

/// Whether a feature is declared in the KerML library model, which reflects the abstract syntax.
fn library_owner(db: &dyn salsa::Database, project: Project, f: Global) -> bool {
    owning_type(db, project, f)
        .and_then(|t| qualified(db, project, t))
        .is_some_and(|q| q.starts_with("KerML::"))
}

/// The metadata of an element (KerML 8.4.4.9.8 rule 4): the MetadataFeatures among its owned
/// elements that annotate it, then its reflective metadata.
fn metadata_of(db: &dyn salsa::Database, project: Project, e: Global) -> Vec<Item> {
    let mut out: Vec<Item> = owned_element(db, project, e)
        .into_iter()
        .chain(
            super::root::owned_annotation(db, project, e)
                .into_iter()
                .filter_map(|a| super::root::owned_annotating_element(db, project, a)),
        )
        .filter(|m| kind_of(db, *m, M::MetadataFeature))
        .filter(|m| super::root::annotated_element(db, project, *m).contains(&e))
        .map(Item::Element)
        .collect();
    let mut seen = HashSet::new();
    out.retain(|i| seen.insert(target_key(&Some(i.clone()))));
    out.push(Item::Reflective(e));
    out
}

/// The metaclass of an element in the KerML library model (`KerML::Classifier` …, 9.2.17).
fn reflective_metaclass(db: &dyn salsa::Database, project: Project, e: Global) -> Option<Global> {
    library(db, project, &format!("KerML::{}", metaclass(db, e).name()))
}

/// The library type of a literal value (ScalarValues, 9.3.2): Boolean, String, Rational, and for
/// an integer the most specific of Positive, Natural and Integer; `*` is a Positive (ADR 0021).
fn value_type(db: &dyn salsa::Database, project: Project, i: &Item) -> Option<Global> {
    let n = match i {
        Item::Boolean(_) => "ScalarValues::Boolean",
        Item::String(_) => "ScalarValues::String",
        Item::Rational(_) => "ScalarValues::Rational",
        Item::Integer(v) if *v > 0 => "ScalarValues::Positive",
        Item::Integer(0) => "ScalarValues::Natural",
        Item::Integer(_) => "ScalarValues::Integer",
        Item::Infinity => "ScalarValues::Positive",
        _ => return None,
    };
    library(db, project, n)
}

/// Whether a value is classified by a type (`istype`), or directly (`hastype`).
fn classified(
    db: &dyn salsa::Database,
    project: Project,
    i: &Item,
    ty: Global,
    direct: bool,
) -> bool {
    let types: Vec<Global> = match i {
        Item::Element(e) => r#type(db, project, *e),
        Item::Reflective(e) => reflective_metaclass(db, project, *e).into_iter().collect(),
        _ => value_type(db, project, i).into_iter().collect(),
    };
    if direct {
        types.contains(&ty)
    } else {
        types.iter().any(|t| specializes(db, project, *t, ty))
    }
}

/// A MOF property of an element, read reflectively (7.4.14): its stored attributes, and the
/// derived properties `name`, `shortName`, `qualifiedName`, `owner`, `owningNamespace`,
/// `ownedElement`, `ownedMember`, `feature`, `ownedFeature` and `type`. An element-valued
/// property gives the elements' reflective metadata, so that a chain can go on.
pub fn reflective_property(
    db: &dyn salsa::Database,
    project: Project,
    e: Global,
    property: &str,
) -> Vec<Item> {
    use super::root as r;
    let strings = |v: Option<Arc<str>>| v.map(Item::String).into_iter().collect::<Vec<_>>();
    let elements = |v: Vec<Global>| v.into_iter().map(Item::Reflective).collect::<Vec<_>>();
    match property {
        "name" => return strings(r::name(db, project, e)),
        "shortName" => return strings(r::short_name(db, project, e)),
        "qualifiedName" => return strings(r::qualified_name(db, project, e).clone()),
        "owner" => return elements(r::owner(db, project, e).into_iter().collect()),
        "owningNamespace" => {
            return elements(r::owning_namespace(db, project, e).into_iter().collect())
        }
        "ownedElement" => return elements(r::owned_element(db, project, e)),
        "ownedMember" => return elements(r::owned_member(db, project, e)),
        "feature" => return elements(feature(db, project, e)),
        "ownedFeature" => return elements(owned_feature(db, project, e)),
        "type" => return elements(r#type(db, project, e)),
        _ => {}
    }
    let s = structure(db, e.file);
    let g = s.graph();
    let m = g.metaclass(e.ix());
    let Some(a) = m.stored_attributes().iter().find(|a| a.name == property) else {
        return Vec::new();
    };
    match a.slot {
        sysml_model::Slot::Value(attr) => {
            let values: Vec<&Value> = g.values(e.ix(), attr).collect();
            if values.is_empty() {
                // The metamodel's default, for a flag that is not written.
                return match a.default {
                    Some("true") => vec![Item::Boolean(true)],
                    Some("false") => vec![Item::Boolean(false)],
                    _ => Vec::new(),
                };
            }
            // An enumeration value is the literal of the library model's enumeration
            // (`KerML::FeatureDirectionKind::'in'` …, 9.2.17), so it compares with it.
            let enumeration = match a.kind {
                sysml_model::PropertyKind::Enumeration { name } => Some(name),
                _ => None,
            };
            values
                .into_iter()
                .map(|v| match v {
                    Value::Boolean(b) => Item::Boolean(*b),
                    Value::Integer(i) => Item::Integer(*i),
                    Value::Real(x) => Item::Rational(*x),
                    Value::String(s) => Item::String(Arc::from(&**s)),
                    Value::Enumeration(s) => enumeration
                        .and_then(|k| library(db, project, &format!("KerML::{k}::{s}")))
                        .map_or_else(|| Item::String(Arc::from(*s)), Item::Element),
                })
                .collect()
        }
        slot @ (sysml_model::Slot::Source | sysml_model::Slot::Target) => {
            let ends = if slot == sysml_model::Slot::Source {
                super::root::source(db, project, e)
            } else {
                super::root::target(db, project, e)
            };
            elements(ends)
        }
        _ => Vec::new(),
    }
}

/// The longest range `..` gives, a bound on what one evaluation builds (ADR 0021).
pub const MAX_RANGE: i64 = 100_000;

fn number(i: &Item) -> Option<f64> {
    match i {
        Item::Integer(v) => Some(*v as f64),
        Item::Rational(v) => Some(*v),
        _ => None,
    }
}

fn same(a: &Item, b: &Item) -> bool {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// The DataFunctions and the remaining BaseFunctions of Tables 5 and 7, on evaluated arguments.
fn apply_data_function(q: &str, args: &[Vec<Item>]) -> Vec<Item> {
    let one = |i: usize| -> Option<&Item> {
        match args.get(i).map(Vec::as_slice) {
            Some([x]) => Some(x),
            _ => None,
        }
    };
    let b = |i: usize| match one(i) {
        Some(Item::Boolean(v)) => Some(*v),
        _ => None,
    };
    let bool_item = |v: bool| vec![Item::Boolean(v)];
    let equal = || {
        let (x, y) = (
            args.first().cloned().unwrap_or_default(),
            args.get(1).cloned().unwrap_or_default(),
        );
        x.len() == y.len() && x.iter().zip(&y).all(|(a, b)| same(a, b))
    };
    let name = q.rsplit("::").next().unwrap_or_default().trim_matches('\'');
    match (q.split("::").next().unwrap_or_default(), name) {
        ("BaseFunctions", "==" | "===") => bool_item(equal()),
        ("BaseFunctions", "!=" | "!==") => bool_item(!equal()),
        ("BaseFunctions", ",") => args.iter().flatten().cloned().collect(),
        ("BaseFunctions", "#") => match (args.first(), one(1)) {
            (Some(seq), Some(Item::Integer(i))) if *i >= 1 => usize::try_from(*i - 1)
                .ok()
                .and_then(|i| seq.get(i))
                .cloned()
                .into_iter()
                .collect(),
            _ => Vec::new(),
        },
        ("DataFunctions", "not") => b(0).map(|v| bool_item(!v)).unwrap_or_default(),
        ("DataFunctions", "xor" | "|" | "&") => match (b(0), b(1)) {
            (Some(x), Some(y)) => bool_item(match name {
                "xor" => x != y,
                "|" => x || y,
                _ => x && y,
            }),
            _ => Vec::new(),
        },
        ("DataFunctions", "<" | ">" | "<=" | ">=") => {
            let ord = match (one(0), one(1)) {
                (Some(Item::String(x)), Some(Item::String(y))) => x.partial_cmp(y),
                (Some(x), Some(y)) => number(x)
                    .zip(number(y))
                    .and_then(|(x, y)| x.partial_cmp(&y)),
                _ => None,
            };
            ord.map(|o| {
                bool_item(match name {
                    "<" => o.is_lt(),
                    ">" => o.is_gt(),
                    "<=" => o.is_le(),
                    _ => o.is_ge(),
                })
            })
            .unwrap_or_default()
        }
        ("DataFunctions", "+" | "-") if args.len() == 1 => match one(0) {
            Some(Item::Integer(v)) => if name == "-" {
                v.checked_neg()
            } else {
                Some(*v)
            }
            .map(Item::Integer)
            .into_iter()
            .collect(),
            Some(Item::Rational(v)) => vec![Item::Rational(if name == "-" { -v } else { *v })],
            _ => Vec::new(),
        },
        ("DataFunctions", "+")
            if matches!(
                (one(0), one(1)),
                (Some(Item::String(_)), Some(Item::String(_)))
            ) =>
        {
            match (one(0), one(1)) {
                (Some(Item::String(x)), Some(Item::String(y))) => {
                    vec![Item::String(format!("{x}{y}").into())]
                }
                _ => Vec::new(),
            }
        }
        ("DataFunctions", "+" | "-" | "*" | "%" | "^" | "/") => match (one(0), one(1)) {
            // IntegerFunctions; `^` of a negative exponent is RationalFunctions::'^'.
            (Some(Item::Integer(x)), Some(Item::Integer(y)))
                if name != "/" && !(name == "^" && *y < 0) =>
            {
                let (x, y) = (*x, *y);
                let v = match name {
                    "+" => x.checked_add(y),
                    "-" => x.checked_sub(y),
                    "*" => x.checked_mul(y),
                    "%" => x.checked_rem_euclid(y),
                    _ => u32::try_from(y).ok().and_then(|e| x.checked_pow(e)),
                };
                v.map(|v| vec![Item::Integer(v)]).unwrap_or_default()
            }
            (Some(x), Some(y)) => match (number(x), number(y)) {
                (Some(x), Some(y)) => {
                    let v = match name {
                        "+" => x + y,
                        "-" => x - y,
                        "*" => x * y,
                        "/" if y != 0.0 => x / y,
                        "%" if y != 0.0 => x.rem_euclid(y),
                        "^" => x.powf(y),
                        _ => return Vec::new(),
                    };
                    vec![Item::Rational(v)]
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        },
        // A range longer than MAX_RANGE has no value, so that evaluation stays bounded.
        ("DataFunctions", "..") => match (one(0), one(1)) {
            (Some(Item::Integer(x)), Some(Item::Integer(y))) => match y.checked_sub(*x) {
                Some(n) if n < MAX_RANGE => (*x..=*y).map(Item::Integer).collect(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

// ---- the operations ----

/// `evaluate(target)` (KerML 8.3.4.7.3 Expression, 8.3.4.8 for each kind of expression;
/// 8.4.4.9.8): the values of a model-level evaluable expression on a target.
pub fn evaluate(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    target: Option<Item>,
) -> Vec<Item> {
    Evaluation {
        db,
        project,
        bindings: Vec::new(),
        active: HashSet::new(),
    }
    .evaluate(x, &target)
}

/// `checkCondition(target)` (KerML 8.3.4.7.3): whether the expression evaluates to the single
/// value true. The OCL's `result->size()` is read as `results->size()`.
pub fn check_condition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    target: Option<Item>,
) -> bool {
    evaluate(db, project, x, target) == [Item::Boolean(true)]
}

/// `modelLevelEvaluable(visited)` (KerML 8.3.4.7.3 and its redefinitions in 8.3.4.8).
pub fn model_level_evaluable(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    visited: &[Global],
) -> bool {
    let m = metaclass(db, x);
    if m.specializes(M::NullExpression)
        || m.specializes(M::LiteralExpression)
        || m.specializes(M::MetadataAccessExpression)
    {
        return true;
    }
    if m.specializes(M::FeatureReferenceExpression) {
        let Some(r) = referent(db, project, x) else {
            return body_of(db, project, x)
                .is_some_and(|b| model_level_evaluable(db, project, b, visited));
        };
        return feature_reference_evaluable(db, project, r, visited);
    }
    if m.specializes(M::InvocationExpression) {
        let function_ok = instantiated_type(db, project, x)
            .is_some_and(|f| function_is_model_level_evaluable(db, project, f));
        let args_ok = if m.specializes(M::OperatorExpression) {
            operands(db, project, x).iter().all(|o| match o {
                Operand::Value(v) | Operand::Body(v) => {
                    model_level_evaluable(db, project, *v, visited)
                }
                Operand::Type(_) => true,
            })
        } else {
            argument(db, project, x)
                .iter()
                .all(|a| model_level_evaluable(db, project, *a, visited))
        };
        return function_ok && args_ok;
    }
    if m.specializes(M::ConstructorExpression) {
        return super::kernel::constructor_argument(db, project, x)
            .iter()
            .all(|a| model_level_evaluable(db, project, *a, visited));
    }
    // Expression::modelLevelEvaluable: only implied specializations, and owned features that are
    // inputs or the result without features or values, or result expressions that are evaluable.
    let s = structure(db, x.file);
    let g = s.graph();
    let written = super::types::owned_specialization(db, project, x)
        .iter()
        .all(|r| g.flag(r.ix(), Attr::IsImplied));
    let result = super::kernel::result(db, project, x);
    written
        && owned_feature(db, project, x).into_iter().all(|f| {
            let fg = structure(db, f.file);
            let fg = fg.graph();
            let input = matches!(
                fg.values(f.ix(), Attr::Direction).next(),
                Some(Value::Enumeration("in"))
            );
            let plain = (input || Some(f) == result)
                && owned_feature(db, project, f).is_empty()
                && valuation(db, f).is_none();
            let result_expression = fg
                .owning_relationship(f.ix())
                .is_some_and(|r| fg.metaclass(r).specializes(M::ResultExpressionMembership))
                && model_level_evaluable(db, project, f, visited);
            plain || result_expression
        })
}

/// `FeatureReferenceExpression::modelLevelEvaluable(visited)` (KerML 8.3.4.8.5): a reference to
/// `Anything::self`; to an evaluable expression; to a feature of a metaclass or a metadata
/// feature; to a feature without featuring types whose value, if any, is evaluable; and, as a
/// reading, to an input parameter of a body expression, which `collect` and `select` bind (ADR
/// 0021).
fn feature_reference_evaluable(
    db: &dyn salsa::Database,
    project: Project,
    r: Global,
    visited: &[Global],
) -> bool {
    if is_anything_self(db, project, r) {
        return true;
    }
    if visited.contains(&r) {
        return false;
    }
    let mut visited = visited.to_vec();
    visited.push(r);
    if kind_of(db, r, M::Expression) {
        return model_level_evaluable(db, project, r, &visited);
    }
    if let Some(t) = owning_type(db, project, r) {
        if kind_of(db, t, M::Metaclass) || kind_of(db, t, M::MetadataFeature) {
            return true;
        }
        let s = structure(db, r.file);
        let input = matches!(
            s.graph().values(r.ix(), Attr::Direction).next(),
            Some(Value::Enumeration("in"))
        );
        if input && kind_of(db, t, M::Expression) {
            return true;
        }
    }
    featuring_type(db, project, r).is_empty()
        && valuation(db, r).is_none_or(|v| model_level_evaluable(db, project, v, &visited))
}

/// `isModelLevelEvaluable` (deriveExpressionIsModelLevelEvaluable, and
/// deriveLiteralExpressionIsModelLevelEvaluable, which is true):
/// `modelLevelEvaluable(Set(Element){})`.
pub fn is_model_level_evaluable(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    model_level_evaluable(db, project, x, &[])
}

/// `metaclassFeature()` (KerML 8.3.4.8.15, no OCL): the reflective metadata of the referenced
/// element.
pub fn metaclass_feature(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Item> {
    referenced_element(db, project, x).map(Item::Reflective)
}

/// `syntaxElement()` (KerML 8.3.4.12.3, no OCL): the element that reflective metadata
/// represents; none for a written metadata feature (ADR 0021).
pub fn syntax_element(item: &Item) -> Option<Global> {
    match item {
        Item::Reflective(e) => Some(*e),
        _ => None,
    }
}

/// `evaluateFeature(baseFeature)` (KerML 8.3.4.12.3): the value, on the metadata feature, of the
/// first of its features that is or redefines `baseFeature`, from the first FeatureValue along
/// its redefinitions. The OCL's `ownedMember->selectAsKind(FeatureValue)` is read as
/// `ownedMembership` (ADR 0021).
pub fn evaluate_feature(
    db: &dyn salsa::Database,
    project: Project,
    mf: Global,
    base: Global,
) -> Vec<Item> {
    let Some(selected) = feature(db, project, mf)
        .into_iter()
        .find(|f| all_redefined_features(db, project, *f).contains(&base))
    else {
        return Vec::new();
    };
    let Some(value) = all_redefined_features(db, project, selected)
        .into_iter()
        .find_map(|f| valuation(db, f))
    else {
        return Vec::new();
    };
    evaluate(db, project, value, Some(Item::Element(mf)))
}

/// `valueOf(bound)` (KerML 8.3.4.11.2): a natural number or `*` for an evaluable bound that
/// evaluates to one value, a non-negative integer or `*`. The target is the type that owns the
/// multiplicity (ADR 0021).
pub fn value_of(
    db: &dyn salsa::Database,
    project: Project,
    bound: Option<Global>,
) -> Option<Natural> {
    let bound = bound?;
    if !is_model_level_evaluable(db, project, bound) {
        return None;
    }
    let range = structure(db, bound.file)
        .graph()
        .owner(bound.ix())
        .map(|o| at(bound.file, o));
    let target = range
        .and_then(|r| owning_namespace(db, project, r))
        .map(Item::Element);
    match evaluate(db, project, bound, target).as_slice() {
        [Item::Infinity] => Some(Natural::Infinite),
        [Item::Integer(v)] if *v >= 0 => Some(Natural::Value(*v as u64)),
        _ => None,
    }
}

/// `hasBounds(lower, upper)` (KerML 8.3.4.11.2): whether the range's bounds are these.
pub fn has_bounds(
    db: &dyn salsa::Database,
    project: Project,
    range: Global,
    lower: Natural,
    upper: Natural,
) -> bool {
    if value_of(db, project, upper_bound(db, project, range)) != Some(upper) {
        return false;
    }
    match value_of(db, project, lower_bound(db, project, range)) {
        Some(l) => l == lower,
        None => lower == upper || (lower == Natural::Value(0) && upper == Natural::Infinite),
    }
}

/// `includeAsMember(element)` (KerML 8.3.4.13.4): whether every filter condition holds on some
/// metadata of the element. Its metadata includes the reflective one (7.4.14), which the OCL
/// leaves out (ADR 0021).
pub fn include_as_member(
    db: &dyn salsa::Database,
    project: Project,
    package: Global,
    element: Global,
) -> bool {
    let conditions = filter_condition(db, project, package);
    if conditions.is_empty() {
        return true;
    }
    let metadata = metadata_of(db, project, element);
    conditions.into_iter().all(|c| {
        metadata
            .iter()
            .any(|m| check_condition(db, project, c, Some(m.clone())))
    })
}

/// `Package::importedMemberships(excluded)` (KerML 8.3.4.13.4): the imported memberships whose
/// member element the filter conditions include.
pub fn package_imported_memberships(
    db: &dyn salsa::Database,
    project: Project,
    package: Global,
) -> Vec<Global> {
    super::root::imported_membership(db, project, package)
        .into_iter()
        .filter(|m| {
            membership_element(db, project, *m)
                .is_some_and(|e| include_as_member(db, project, package, e))
        })
        .collect()
}
