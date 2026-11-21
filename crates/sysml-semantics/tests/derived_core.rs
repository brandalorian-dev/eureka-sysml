// SPDX-License-Identifier: Apache-2.0
//! Derived properties and operations of the Core layer (KerML 8.3.3; step 1a.8, ADR 0021): one
//! test per `derive…` rule and operation, from its OCL, named after it
//! (`tests/derived_inventory.rs` checks).

mod common;

use common::{model, model_with_library, Model};
use sysml_semantics::derived::{self as d, Direction};
use sysml_semantics::Global;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn names<T: std::borrow::Borrow<Global>>(
    m: &Model,
    xs: impl IntoIterator<Item = T>,
) -> Vec<String> {
    m.labels(xs)
}

// ---- Type (KerML 8.3.3.1.10) ----

#[test]
fn derive_type_owned_specialization() {
    // ownedSpecialization: the owned Specializations whose special is the type
    let m = model(
        "classifier A; classifier B specializes A; specialization S subtype B specializes A;",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::owned_specialization(db, p, m.get("B"))),
        ["Subclassification"]
    );
    assert!(
        d::owned_specialization(db, p, m.root()).is_empty(),
        "S's special is B"
    );
}

#[test]
fn derive_type_owned_conjugator() {
    // ownedConjugator: the first owned Conjugation
    let m = model("classifier A; classifier C conjugates A; classifier D;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::owned_conjugator(db, p, m.get("C"))),
        Some("Conjugation".into())
    );
    assert_eq!(d::owned_conjugator(db, p, m.get("D")), None);
}

const SETS: &str = "classifier A; classifier B;
    classifier U unions A, B; classifier I intersects A, B; classifier D differences A, B;
    classifier J disjoint from A;";

#[test]
fn derive_type_owned_unioning() {
    let m = model(SETS);
    assert_eq!(
        names(&m, d::owned_unioning(&m.db, m.project, m.get("U"))),
        ["Unioning", "Unioning"]
    );
}

#[test]
fn derive_type_owned_intersecting() {
    let m = model(SETS);
    let i = d::owned_intersecting(&m.db, m.project, m.get("I"));
    assert_eq!(names(&m, i), ["Intersecting", "Intersecting"]);
}

#[test]
fn derive_type_owned_differencing() {
    let m = model(SETS);
    let x = d::owned_differencing(&m.db, m.project, m.get("D"));
    assert_eq!(names(&m, x), ["Differencing", "Differencing"]);
}

#[test]
fn derive_type_owned_disjoining() {
    let m = model(SETS);
    assert_eq!(
        names(&m, d::owned_disjoining(&m.db, m.project, m.get("J"))),
        ["Disjoining"]
    );
    assert!(d::owned_disjoining(&m.db, m.project, m.get("U")).is_empty());
}

#[test]
fn derive_type_unioning_type() {
    // unioningType = ownedUnioning.unioningType
    let m = model(SETS);
    assert_eq!(
        names(&m, d::unioning_type(&m.db, m.project, m.get("U"))),
        ["A", "B"]
    );
}

#[test]
fn derive_type_intersecting_type() {
    let m = model(SETS);
    assert_eq!(
        names(&m, d::intersecting_type(&m.db, m.project, m.get("I"))),
        ["A", "B"]
    );
}

#[test]
fn derive_type_differencing_type() {
    let m = model(SETS);
    assert_eq!(
        names(&m, d::differencing_type(&m.db, m.project, m.get("D"))),
        ["A", "B"]
    );
}

const INHERIT: &str = "classifier A { feature x; private feature p; protected feature q; }
    classifier B specializes A { feature b; member feature n; }";

#[test]
fn derive_type_owned_feature_membership() {
    // ownedFeatureMembership: the owned FeatureMemberships, not the other OwningMemberships
    let m = model(INHERIT);
    let ms = d::owned_feature_membership(&m.db, m.project, m.get("B"));
    assert_eq!(names(&m, ms), ["FeatureMembership(B::b)"]);
}

#[test]
fn derive_type_owned_feature() {
    // ownedFeature = ownedFeatureMembership.ownedMemberFeature
    let m = model(INHERIT);
    assert_eq!(
        names(&m, d::owned_feature(&m.db, m.project, m.get("B"))),
        ["B::b"]
    );
}

#[test]
fn derive_type_inherited_membership() {
    // inheritedMembership = inheritedMemberships(Set{}, Set{}, false): not the private ones
    let m = model(INHERIT);
    assert_eq!(
        names(&m, d::inherited_membership(&m.db, m.project, m.get("B"))),
        ["FeatureMembership(A::x)", "FeatureMembership(A::q)"]
    );
}

#[test]
fn derive_type_inherited_feature() {
    // inheritedFeature: the member features of the inherited FeatureMemberships
    let m = model(INHERIT);
    assert_eq!(
        names(&m, d::inherited_feature(&m.db, m.project, m.get("B"))),
        ["A::x", "A::q"]
    );
}

#[test]
fn derive_type_feature_membership() {
    // featureMembership = ownedFeatureMembership->union(inherited FeatureMemberships)
    let m = model(INHERIT);
    assert_eq!(
        names(&m, d::feature_membership(&m.db, m.project, m.get("B"))),
        [
            "FeatureMembership(B::b)",
            "FeatureMembership(A::x)",
            "FeatureMembership(A::q)"
        ]
    );
}

#[test]
fn derive_type_feature() {
    // feature = featureMembership.ownedMemberFeature
    let m = model(INHERIT);
    assert_eq!(
        names(&m, d::feature(&m.db, m.project, m.get("B"))),
        ["B::b", "A::x", "A::q"]
    );
}

const ENDS: &str =
    "classifier A { end feature e1; feature f; } classifier B specializes A { end feature e2; }";

#[test]
fn derive_type_end_feature() {
    // endFeature = feature->select(isEnd); B's e2 redefines A's e1 by position
    // (checkFeatureEndRedefinition), which hides it
    let m = model(ENDS);
    assert_eq!(
        names(&m, d::end_feature(&m.db, m.project, m.get("A"))),
        ["A::e1"]
    );
    assert_eq!(
        names(&m, d::end_feature(&m.db, m.project, m.get("B"))),
        ["B::e2"]
    );
}

#[test]
fn derive_type_owned_end_feature() {
    // ownedEndFeature = ownedFeature->select(isEnd)
    let m = model(ENDS);
    assert_eq!(
        names(&m, d::owned_end_feature(&m.db, m.project, m.get("A"))),
        ["A::e1"]
    );
}

#[test]
fn derive_type_multiplicity() {
    // multiplicity: the first owned member that is a Multiplicity
    let m = model("feature f[3]; feature g;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::multiplicity(db, p, m.get("f"))),
        Some("f::(MultiplicityRange)".into())
    );
    assert_eq!(d::multiplicity(db, p, m.get("g")), None);
}

const DIRECTIONS: &str =
    "classifier C { in feature i; out feature o; inout feature io; feature n; }
    classifier D conjugates C;";

#[test]
fn derive_type_input() {
    // input: the features whose direction relative to the type is in or inout; a conjugated
    // type reverses in and out
    let m = model(DIRECTIONS);
    assert_eq!(
        names(&m, d::input(&m.db, m.project, m.get("C"))),
        ["C::i", "C::io"]
    );
    assert_eq!(
        names(&m, d::input(&m.db, m.project, m.get("D"))),
        ["C::o", "C::io"]
    );
}

#[test]
fn derive_type_output() {
    let m = model(DIRECTIONS);
    assert_eq!(
        names(&m, d::output(&m.db, m.project, m.get("C"))),
        ["C::o", "C::io"]
    );
    assert_eq!(
        names(&m, d::output(&m.db, m.project, m.get("D"))),
        ["C::i", "C::io"]
    );
}

#[test]
fn derive_type_directed_feature() {
    let m = model(DIRECTIONS);
    let x = d::directed_feature(&m.db, m.project, m.get("C"));
    assert_eq!(names(&m, x), ["C::i", "C::o", "C::io"]);
}

#[test]
fn type_direction_of() {
    // directionOf(feature) = directionOfExcluding(feature, Set{})
    let m = model(DIRECTIONS);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::direction_of(db, p, m.get("C"), m.get("C::i")),
        Some(Direction::In)
    );
    assert_eq!(
        d::direction_of(db, p, m.get("D"), m.get("C::i")),
        Some(Direction::Out)
    );
    assert_eq!(d::direction_of(db, p, m.get("C"), m.get("C::n")), None);
}

#[test]
fn type_direction_of_excluding() {
    // directionOfExcluding(feature, excluded): the excluded types are not visited
    let m = model(DIRECTIONS);
    let (db, p) = (&m.db, m.project);
    let (c, i) = (m.get("C"), m.get("C::i"));
    assert_eq!(d::direction_of_excluding(db, p, m.get("D"), i, &[c]), None);
    assert_eq!(
        d::direction_of_excluding(db, p, m.get("D"), i, &[]),
        Some(Direction::Out)
    );
}

#[test]
fn feature_direction_for() {
    // directionFor(type) = type.directionOf(self)
    let m = model(DIRECTIONS);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::direction_for(db, p, m.get("C::o"), m.get("D")),
        Some(Direction::In)
    );
}

#[test]
fn type_supertypes() {
    // supertypes(excludeImplied): the original type of a conjugated type, else the generals,
    // with the implied ones unless excluded
    let m = model_with_library(
        "classifier A; classifier B specializes A; classifier C conjugates A; classifier X;",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::supertypes(db, p, m.get("B"), false)), ["A"]);
    assert_eq!(names(&m, d::supertypes(db, p, m.get("C"), false)), ["A"]);
    assert_eq!(
        names(&m, d::supertypes(db, p, m.get("X"), false)),
        ["Base::Anything"]
    );
    assert!(d::supertypes(db, p, m.get("X"), true).is_empty());
}

#[test]
fn feature_supertypes() {
    // Feature::supertypes(excludeImplied): also the featureTarget of a chained feature
    let m = model("feature a { feature b; } feature s; feature c subsets s chains a.b;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::supertypes(db, p, m.get("c"), true)),
        ["s", "a::b"]
    );
}

#[test]
fn type_all_supertypes() {
    // allSupertypes() = OrderedSet{self}->closure(supertypes(false)), circular ones once
    let m = model(
        "classifier A; classifier B specializes A; classifier C specializes B;
        classifier P specializes Q; classifier Q specializes P;",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::all_supertypes(db, p, m.get("C")).clone()),
        ["C", "B", "A"]
    );
    assert_eq!(
        names(&m, d::all_supertypes(db, p, m.get("P")).clone()),
        ["P", "Q"]
    );
}

#[test]
fn type_specializes() {
    // specializes(supertype): in allSupertypes(), or for a conjugated type, its original's
    let m = model("classifier A; classifier B specializes A; classifier C specializes B; classifier D conjugates C;");
    let (db, p) = (&m.db, m.project);
    assert!(d::specializes(db, p, m.get("C"), m.get("A")));
    assert!(!d::specializes(db, p, m.get("A"), m.get("C")));
    assert!(d::specializes(db, p, m.get("D"), m.get("A")));
}

#[test]
fn type_specializes_from_library() {
    // specializesFromLibrary(libraryTypeName)
    let m = model_with_library("classifier X; datatype V;");
    let (db, p) = (&m.db, m.project);
    assert!(d::specializes_from_library(
        db,
        p,
        m.get("X"),
        "Base::Anything"
    ));
    assert!(!d::specializes_from_library(
        db,
        p,
        m.get("X"),
        "Base::DataValue"
    ));
    assert!(d::specializes_from_library(
        db,
        p,
        m.get("V"),
        "Base::DataValue"
    ));
    assert!(!d::specializes_from_library(
        db,
        p,
        m.get("X"),
        "Base::Nothing"
    ));
}

#[test]
fn type_is_compatible_with() {
    // Type::isCompatibleWith(otherType) = specializes(otherType)
    let m = model("classifier A; classifier B specializes A;");
    let (db, p) = (&m.db, m.project);
    assert!(d::is_compatible_with(db, p, m.get("B"), m.get("A")));
    assert!(!d::is_compatible_with(db, p, m.get("A"), m.get("B")));
}

#[test]
fn feature_is_compatible_with() {
    // Feature::isCompatibleWith(otherType): also two features without owned features that
    // redefine a common feature, one accessing the other
    let m = model(
        "classifier A { feature x; feature y redefines A::x; feature z redefines A::x; feature w; }",
    );
    let (db, p) = (&m.db, m.project);
    assert!(d::is_compatible_with(db, p, m.get("A::y"), m.get("A::z")));
    assert!(!d::is_compatible_with(db, p, m.get("A::y"), m.get("A::w")));
    assert!(
        d::is_compatible_with(db, p, m.get("A::y"), m.get("A::x")),
        "y specializes x"
    );
}

#[test]
fn type_multiplicities() {
    // multiplicities(): the multiplicity, else those of the nearest generals with one
    let m = model("feature f[2]; feature g subsets f; feature h subsets g;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::multiplicities(db, p, m.get("h"))),
        ["f::(MultiplicityRange)"]
    );
    assert_eq!(
        names(&m, d::multiplicities(db, p, m.get("f"))),
        ["f::(MultiplicityRange)"]
    );
}

const LIBRARY_INHERIT: &str = "classifier A { feature x; } classifier B specializes A;";

#[test]
fn type_inherited_memberships() {
    // inheritedMemberships(…, excludeImplied): through the written specializations only, when
    // implied ones are excluded
    let m = model_with_library(LIBRARY_INHERIT);
    let (db, p) = (&m.db, m.project);
    let b = m.get("B");
    assert_eq!(
        names(&m, d::type_inherited_memberships(db, p, b, true)),
        ["FeatureMembership(A::x)"]
    );
    let all = names(&m, d::type_inherited_memberships(db, p, b, false));
    assert!(
        all.contains(&"FeatureMembership(Base::Anything::self)".to_owned()),
        "{all:?}"
    );
}

#[test]
fn type_visible_memberships() {
    // Type::visibleMemberships(excluded, isRecursive, includeAll): its inherited memberships are
    // `inheritedMemberships(…, isRecursive)`, through written specializations only when recursive
    let m = model_with_library(LIBRARY_INHERIT);
    let (db, p) = (&m.db, m.project);
    let b = m.get("B");
    assert_eq!(
        names(&m, d::visible_memberships(db, p, b, true, false)),
        ["FeatureMembership(A::x)"]
    );
    let flat = names(&m, d::visible_memberships(db, p, b, false, false));
    assert!(
        flat.contains(&"FeatureMembership(Base::Anything::self)".to_owned()),
        "{flat:?}"
    );
}

#[test]
fn type_inheritable_memberships() {
    // inheritableMemberships: before removeRedefinedFeatures
    let m =
        model("classifier A { feature x; } classifier B specializes A { feature redefines x; }");
    let (db, p) = (&m.db, m.project);
    let b = m.get("B");
    assert_eq!(
        names(&m, d::inheritable_memberships(db, p, b, true)),
        ["FeatureMembership(A::x)"]
    );
    assert!(d::type_inherited_memberships(db, p, b, true).is_empty());
}

#[test]
fn type_non_private_memberships() {
    // nonPrivateMemberships: the public memberships, the protected ones, and the inherited ones
    let m = model(
        "classifier A { protected feature q; feature x; private feature p; }
        classifier B specializes A { feature b; }",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::non_private_memberships(db, p, m.get("B"), true)),
        [
            "FeatureMembership(B::b)",
            "FeatureMembership(A::x)",
            "FeatureMembership(A::q)"
        ]
    );
}

const REDEFINE: &str = "classifier A { feature x; feature y; }
    classifier B specializes A { feature x1 redefines x; }
    classifier C specializes A { feature x2 redefines x; }";

#[test]
fn type_remove_redefined_features() {
    // removeRedefinedFeatures(memberships): without what another of them redefines, and without
    // what an owned feature redefines
    let m = model(REDEFINE);
    let (db, p) = (&m.db, m.project);
    let (x, y, x2) = (
        m.membership("A::x"),
        m.membership("A::y"),
        m.membership("C::x2"),
    );
    let b = m.get("B");
    assert_eq!(
        names(&m, d::remove_redefined_features(db, p, b, &[x, y])),
        ["FeatureMembership(A::y)"]
    );
    let a = m.get("A");
    assert_eq!(
        names(&m, d::remove_redefined_features(db, p, a, &[x, x2])),
        ["FeatureMembership(C::x2)"]
    );
}

#[test]
fn type_all_redefined_features_of() {
    // allRedefinedFeaturesOf(membership): the member feature's allRedefinedFeatures()
    let m = model(REDEFINE);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(
            &m,
            d::all_redefined_features_of(db, p, m.membership("B::x1"))
        ),
        ["B::x1", "A::x"]
    );
    assert!(d::all_redefined_features_of(db, p, m.membership("A")).is_empty());
}

#[test]
fn feature_all_redefined_features() {
    // allRedefinedFeatures(): the feature, then what it redefines, directly or indirectly
    let m = model(
        "classifier A { feature x; } classifier B specializes A { feature x1 redefines x; }
        classifier C specializes B { feature x2 redefines x1; }",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::all_redefined_features(db, p, m.get("C::x2"))),
        ["C::x2", "B::x1", "A::x"]
    );
}

#[test]
fn feature_redefines() {
    // redefines(redefinedFeature): ownedRedefinition.redefinedFeature->includes(…)
    let m = model(REDEFINE);
    let (db, p) = (&m.db, m.project);
    assert!(d::redefines(db, p, m.get("B::x1"), m.get("A::x")));
    assert!(!d::redefines(db, p, m.get("B::x1"), m.get("A::y")));
}

#[test]
fn feature_redefines_from_library() {
    // redefinesFromLibrary(libraryFeatureName)
    let m = model_with_library(
        "classifier K specializes Base::Anything { feature redefines self; } feature f;",
    );
    let (db, p) = (&m.db, m.project);
    let k = m.get("K::Feature#1");
    assert!(d::redefines_from_library(db, p, k, "Base::Anything::self"));
    assert!(!d::redefines_from_library(
        db,
        p,
        m.get("f"),
        "Base::Anything::self"
    ));
}

#[test]
fn feature_naming_feature() {
    // namingFeature(): the redefined feature of the first owned redefinition, written or implied
    let m = model(
        "classifier A { feature x; end feature e; }
        classifier B specializes A { feature redefines x; end feature; feature n; }",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::naming_feature(db, p, m.get("B::Feature#1"))),
        Some("A::x".into())
    );
    assert_eq!(
        m.label_of(d::naming_feature(db, p, m.get("B::Feature#2"))),
        Some("A::e".into())
    );
    assert_eq!(d::naming_feature(db, p, m.get("B::n")), None);
}

#[test]
fn feature_effective_name() {
    // Feature::effectiveName(): the declared name, else the naming feature's effective name
    let m =
        model("classifier A { feature x; } classifier B specializes A { feature redefines x; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::effective_name(db, p, m.get("B::Feature#1")).as_deref(),
        Some("x")
    );
}

#[test]
fn feature_effective_short_name() {
    // Feature::effectiveShortName(): likewise, the short name
    let m = model(
        "classifier A { feature <s> x; } classifier B specializes A { feature redefines x; }",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::effective_short_name(db, p, m.get("B::Feature#1")).as_deref(),
        Some("s")
    );
}

// ---- Classifier (KerML 8.3.3.2.2) ----

#[test]
fn derive_classifier_owned_subclassification() {
    // ownedSubclassification = ownedSpecialization->selectByKind(Subclassification)
    let m = model("classifier A; classifier B specializes A; classifier C;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::owned_subclassification(db, p, m.get("B"))),
        ["Subclassification"]
    );
    assert!(d::owned_subclassification(db, p, m.get("C")).is_empty());
}

// ---- Feature (KerML 8.3.3.3.4) ----

const SUBSETS: &str =
    "classifier T; feature f; feature h; feature r; feature g : T subsets f redefines h ::> r;";

#[test]
fn derive_feature_owned_subsetting() {
    // ownedSubsetting = ownedSpecialization->selectByKind(Subsetting)
    let m = model(SUBSETS);
    assert_eq!(
        names(&m, d::owned_subsetting(&m.db, m.project, m.get("g"))),
        ["Subsetting", "Redefinition", "ReferenceSubsetting"]
    );
}

#[test]
fn derive_feature_owned_redefinition() {
    // ownedRedefinition = ownedSubsetting->selectByKind(Redefinition)
    let m = model(SUBSETS);
    assert_eq!(
        names(&m, d::owned_redefinition(&m.db, m.project, m.get("g"))),
        ["Redefinition"]
    );
}

#[test]
fn derive_feature_owned_typing() {
    // ownedTyping = ownedGeneralization->selectByKind(FeatureTyping)
    let m = model(SUBSETS);
    assert_eq!(
        names(&m, d::owned_typing(&m.db, m.project, m.get("g"))),
        ["FeatureTyping"]
    );
}

#[test]
fn derive_feature_owned_reference_subsetting() {
    // ownedReferenceSubsetting: the first owned ReferenceSubsetting
    let m = model(SUBSETS);
    let (db, p) = (&m.db, m.project);
    let r = d::owned_reference_subsetting(db, p, m.get("g"));
    assert_eq!(m.label_of(r), Some("ReferenceSubsetting".into()));
    assert_eq!(d::owned_reference_subsetting(db, p, m.get("f")), None);
}

const CHAINS: &str = "feature a { feature b; } feature c chains a.b; feature f;";

#[test]
fn derive_feature_owned_feature_chaining() {
    let m = model(CHAINS);
    let x = d::owned_feature_chaining(&m.db, m.project, m.get("c"));
    assert_eq!(names(&m, x), ["FeatureChaining", "FeatureChaining"]);
}

#[test]
fn derive_feature_chaining_feature() {
    // chainingFeature = ownedFeatureChaining.chainingFeature
    let m = model(CHAINS);
    assert_eq!(
        names(&m, d::chaining_feature(&m.db, m.project, m.get("c"))),
        ["a", "a::b"]
    );
}

#[test]
fn derive_feature_feature_target() {
    // featureTarget: the last chaining feature, else the feature itself
    let m = model(CHAINS);
    let (db, p) = (&m.db, m.project);
    assert_eq!(m.label(d::feature_target(db, p, m.get("c"))), "a::b");
    assert_eq!(m.label(d::feature_target(db, p, m.get("f"))), "f");
}

const CROSS: &str = "classifier T; feature f { feature g; }
    assoc A { end feature e crosses f.g; end [1] feature e2 : T; end feature e3 : T; }";

#[test]
fn derive_feature_owned_cross_subsetting() {
    // ownedCrossSubsetting: the first owned CrossSubsetting
    let m = model(CROSS);
    let (db, p) = (&m.db, m.project);
    let c = d::owned_cross_subsetting(db, p, m.get("A::e"));
    assert_eq!(
        m.label_of(c),
        Some("CrossSubsetting(A::e::(Feature))".into())
    );
    assert_eq!(d::owned_cross_subsetting(db, p, m.get("A::e3")), None);
}

#[test]
fn derive_feature_cross_feature() {
    // crossFeature: the second chaining feature of the crossed feature
    let m = model(CROSS);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::cross_feature(db, p, m.get("A::e"))),
        Some("f::g".into())
    );
    assert_eq!(d::cross_feature(db, p, m.get("A::e3")), None);
}

#[test]
fn feature_owned_cross_feature() {
    // ownedCrossFeature(): an end feature's first owned member feature not owned through a
    // FeatureMembership (the feature of `end [1] feature`)
    let m = model(CROSS);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::owned_cross_feature(db, p, m.get("A::e2"))),
        Some("A::e2::(Feature)".into())
    );
    assert_eq!(d::owned_cross_feature(db, p, m.get("A::e3")), None);
    assert_eq!(
        d::owned_cross_feature(db, p, m.get("f")),
        None,
        "not an end"
    );
}

#[test]
fn feature_is_owned_cross_feature() {
    // isOwnedCrossFeature(): whether the feature is its owning namespace's ownedCrossFeature()
    let m = model(CROSS);
    let (db, p) = (&m.db, m.project);
    assert!(d::is_owned_cross_feature(db, p, m.get("A::e2::Feature#1")));
    assert!(!d::is_owned_cross_feature(db, p, m.get("A::e3")));
}

#[test]
fn derive_feature_owned_type_featuring() {
    // ownedTypeFeaturing: the owned TypeFeaturings whose featureOfType is the feature
    let m = model("classifier B; feature g featured by B; feature h;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::owned_type_featuring(db, p, m.get("g"))),
        ["TypeFeaturing"]
    );
    assert!(d::owned_type_featuring(db, p, m.get("h")).is_empty());
}

#[test]
fn derive_feature_owned_feature_inverting() {
    // ownedFeatureInverting: the owned FeatureInvertings whose featureInverted is the feature
    let m = model("feature f; feature h inverse of f;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::owned_feature_inverting(db, p, m.get("h"))),
        ["FeatureInverting"]
    );
    assert!(d::owned_feature_inverting(db, p, m.get("f")).is_empty());
}

#[test]
fn derive_feature_featuring_type() {
    // featuringType: the types of the featurings (the owned TypeFeaturings, and the one a
    // FeatureMembership implies), and those of the first chaining feature
    let m = model(
        "classifier A { feature x { feature y; } } classifier B; feature g featured by B;
        feature c chains A::x.y; feature t;",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::featuring_type(db, p, m.get("g"))), ["B"]);
    assert_eq!(names(&m, d::featuring_type(db, p, m.get("A::x"))), ["A"]);
    assert_eq!(names(&m, d::featuring_type(db, p, m.get("c"))), ["A"]);
    assert!(d::featuring_type(db, p, m.get("t")).is_empty());
}

#[test]
fn feature_typing_features() {
    // typingFeatures(): the features subsetted (not crossed), and the last chaining feature; for
    // a conjugated feature, its original type if that is a feature
    let m = model(
        "feature a { feature b; } feature f; feature g subsets f; feature c chains a.b;
        feature k ~ f;",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::typing_features(db, p, m.get("g"))), ["f"]);
    assert_eq!(names(&m, d::typing_features(db, p, m.get("c"))), ["a::b"]);
    assert_eq!(names(&m, d::typing_features(db, p, m.get("k"))), ["f"]);
}

#[test]
fn derive_feature_type() {
    // type: the types of the typings of the feature and its typing features, without those
    // another specializes
    let m = model(
        "classifier T; classifier U specializes T; feature f : T; feature g : U subsets f;
        feature h subsets f;",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::r#type(db, p, m.get("g"))), ["U"]);
    assert_eq!(names(&m, d::r#type(db, p, m.get("h"))), ["T"]);
    // With the libraries, an untyped feature is a Base::things, of type Base::Anything.
    let l = model_with_library("feature u;");
    assert_eq!(
        names(&l, d::r#type(&l.db, l.project, l.get("u"))),
        ["Base::Anything"]
    );
}

#[test]
fn feature_subsets_chain() {
    // subsetsChain(first, second): a supertype is a chain ending in first, second
    let m = model("feature f { feature g; } feature c chains f.g; feature s subsets c;");
    let (db, p) = (&m.db, m.project);
    let (f, g) = (m.get("f"), m.get("f::g"));
    assert!(d::subsets_chain(db, p, m.get("s"), f, g));
    assert!(!d::subsets_chain(db, p, m.get("s"), g, f));
}

#[test]
fn feature_is_cartesian_product() {
    // isCartesianProduct(): one type, one featuring type, not owned by the feature unless it is
    // itself a cartesian product
    let m = model("classifier T; classifier A { feature x : T; feature y; }");
    let (db, p) = (&m.db, m.project);
    assert!(d::is_cartesian_product(db, p, m.get("A::x")));
    assert!(!d::is_cartesian_product(db, p, m.get("A::y")), "no type");
}

#[test]
fn feature_as_cartesian_product() {
    // asCartesianProduct(): the featuring types not owned by the feature, then its types
    let m = model("classifier T; classifier A { feature x : T; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        names(&m, d::as_cartesian_product(db, p, m.get("A::x"))),
        ["A", "T"]
    );
}

const FEATURING: &str = "classifier A { feature x; feature y; } classifier C specializes A;
    classifier B { feature z; var feature v; } feature t;";

#[test]
fn feature_is_featured_within() {
    // isFeaturedWithin(type): without a type, whether every featuring type is Base::Anything;
    // else whether the type is compatible with each
    let m = model(FEATURING);
    let (db, p) = (&m.db, m.project);
    let x = m.get("A::x");
    assert!(
        d::is_featured_within(db, p, m.get("t"), None),
        "no featuring types"
    );
    assert!(!d::is_featured_within(db, p, x, None));
    assert!(d::is_featured_within(db, p, x, Some(m.get("A"))));
    assert!(
        d::is_featured_within(db, p, x, Some(m.get("C"))),
        "C specializes A"
    );
    assert!(!d::is_featured_within(db, p, x, Some(m.get("B"))));
}

#[test]
fn feature_can_access() {
    // canAccess(feature): the other feature is featured within this one's featuring types
    let m = model(FEATURING);
    let (db, p) = (&m.db, m.project);
    assert!(d::can_access(db, p, m.get("A::x"), m.get("A::y")));
    assert!(!d::can_access(db, p, m.get("A::x"), m.get("B::z")));
}

#[test]
fn feature_is_featuring_type() {
    // isFeaturingType(type): the owning type for a feature that is not variable; for a variable
    // one outside Occurrence, a feature featured by the owning type that redefines snapshots
    let m = model(FEATURING);
    let (db, p) = (&m.db, m.project);
    assert!(d::is_featuring_type(db, p, m.get("A::x"), m.get("A")));
    assert!(!d::is_featuring_type(db, p, m.get("A::x"), m.get("B")));
    assert!(
        !d::is_featuring_type(db, p, m.get("B::v"), m.get("B")),
        "variable"
    );
    assert!(
        !d::is_featuring_type(db, p, m.get("t"), m.get("A")),
        "no owning type"
    );
}

// ---- regressions from review ----

#[test]
fn circular_conjugations_and_featurings_settle_without_a_stack_overflow() {
    // Conjugations, chains and featurings may be circular in a model being edited.
    let m = model(
        "type A ~ B; type B ~ A; type X;
        feature a chains b.x; feature b chains a.y;
        classifier T { var feature f featured by g; var feature g featured by f; }",
    );
    let (db, p) = (&m.db, m.project);
    assert!(!d::specializes(db, p, m.get("A"), m.get("X")));
    let _ = d::featuring_type(db, p, m.get("a"));
    let (f, g) = (m.get("T::f"), m.get("T::g"));
    let _ = d::featuring_type(db, p, f);
    let _ = d::is_featuring_type(db, p, f, g);
    let _ = d::is_compatible_with(db, p, f, g);
}

#[test]
fn a_type_does_not_inherit_its_own_memberships_around_a_cycle() {
    // KerML 8.3.3.1.10 inheritableMemberships: `excludedTypes->including(self)`
    let m = model("classifier A specializes B { feature x; } classifier B specializes A;");
    let (db, p) = (&m.db, m.project);
    assert!(d::inherited_membership(db, p, m.get("A")).is_empty());
    assert_eq!(
        names(&m, d::inherited_membership(db, p, m.get("B"))),
        ["FeatureMembership(A::x)"]
    );
    assert_eq!(names(&m, d::feature(db, p, m.get("A"))), ["A::x"]);
}

#[test]
fn a_conjugated_type_inherits_from_its_original_type_alone() {
    // KerML 8.3.3.1.10 supertypes: `if isConjugated then Sequence{conjugator.originalType}`, in
    // resolution as in the derived properties. A type, unlike a classifier, may be written with
    // both (KerML 8.2.4.1.1).
    let m = model(
        "type D { feature d; } type E { feature e; }
        type C specializes D conjugates E { feature z subsets d; feature w subsets e; }",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::supertypes(db, p, m.get("C"), true)), ["E"]);
    let typings = names(&m, d::inherited_feature(db, p, m.get("C")));
    assert_eq!(typings, ["E::e"]);
    let z = m.relationship("C::z", sysml_model::Metaclass::Subsetting, 1);
    let w = m.relationship("C::w", sysml_model::Metaclass::Subsetting, 1);
    assert_eq!(
        names(&m, d::target(db, p, z)),
        Vec::<String>::new(),
        "d is not inherited"
    );
    assert_eq!(names(&m, d::target(db, p, w)), ["E::e"]);
}

#[test]
fn a_specialization_written_in_a_type_does_not_make_the_type_specialize() {
    // `specialization X subtype B specializes A` in T's body: its special is B, not T
    let m = model(
        "classifier A { feature a; } classifier B;
        classifier T { specialization X subtype B specializes A; feature t subsets a; }",
    );
    let (db, p) = (&m.db, m.project);
    assert!(d::supertypes(db, p, m.get("T"), true).is_empty());
    let t = m.relationship("T::t", sysml_model::Metaclass::Subsetting, 1);
    assert!(d::target(db, p, t).is_empty(), "a is not inherited by T");
}
