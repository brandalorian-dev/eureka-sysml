// SPDX-License-Identifier: Apache-2.0
//! Name resolution (KerML 8.2.3.5) on the three KerML libraries, loaded as one circular unit, and
//! on the clause examples (step 1a.7).

use expect_test::expect;
use salsa::Setter;
use sysml_model::{FileId, Slot};
use sysml_semantics::{
    effective_names, implied_generals, lowered, resolved_names, structure, Database, Global,
    Project, SourceFile,
};
use sysml_syntax::Language;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The KerML library sources, as files of one project.
fn kerml_library(db: &Database) -> (Project, Vec<(SourceFile, String, String)>) {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let mut files = Vec::new();
    for s in corpus.sources() {
        if s.language != sysml_corpus::Language::KerML {
            continue;
        }
        let f = SourceFile::new(
            db,
            FileId(files.len() as u32),
            s.id.clone(),
            Language::KerML,
            s.text.clone(),
        );
        files.push((f, s.id.clone(), s.text.clone()));
    }
    let project = Project::new(db, files.iter().map(|f| f.0).collect());
    (project, files)
}

#[test]
fn the_kerml_libraries_resolve_with_no_unresolved_names() {
    let db = Database::default();
    let (project, files) = kerml_library(&db);
    let mut unresolved = Vec::new();
    let mut total = 0;
    for (f, id, text) in &files {
        let l = lowered(&db, *f);
        for n in resolved_names(&db, project, *f) {
            total += 1;
            if n.target.is_none() {
                let rel = sysml_model::ElementIx::from_index(n.relationship as usize);
                let ends = match n.slot {
                    Slot::Source => l.graph.source(rel),
                    _ => l.graph.target(rel),
                };
                let name = match ends[n.position as usize] {
                    sysml_model::Endpoint::Name(x) => l.graph.name(x).clone(),
                    _ => unreachable!(),
                };
                let span = name.span.unwrap();
                let line = text[..span.start as usize].matches('\n').count() + 1;
                unresolved.push(format!(
                    "{id}:{line} {} `{}`",
                    l.graph.metaclass(rel).name(),
                    &text[span.start as usize..span.end as usize]
                ));
            }
        }
    }
    // Every name of the libraries, counted against the pilot's (tests/resolve_oracle.rs).
    assert_eq!(total, 4003, "the number of names");
    assert!(
        unresolved.is_empty(),
        "{} of {total} names unresolved:\n{}",
        unresolved.len(),
        unresolved
            .iter()
            .take(60)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ---- clause examples ----

/// A project of the given KerML sources, after the KerML libraries if `library`.
fn project(db: &Database, sources: &[&str], library: bool) -> (Project, Vec<SourceFile>) {
    let mut files: Vec<SourceFile> = Vec::new();
    if library {
        files.extend(kerml_library(db).1.into_iter().map(|f| f.0));
    }
    let mine: Vec<SourceFile> = sources
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let n = (files.len() + i) as u32;
            SourceFile::new(
                db,
                FileId(n),
                format!("test{i}.kerml"),
                Language::KerML,
                text.to_string(),
            )
        })
        .collect();
    files.extend(mine.iter().copied());
    (Project::new(db, files), mine)
}

/// The qualified effective name of an element: the names of it and its owners.
fn path(db: &Database, project: Project, g: Global) -> String {
    let l = lowered(db, g.file);
    let start = sysml_model::ElementIx::from_index(g.index as usize);
    // A membership (the target of a MembershipImport): its namespace and its member name.
    if l.graph
        .metaclass(start)
        .specializes(sysml_model::Metaclass::Membership)
    {
        let owner = l.graph.owning_related_element(start).unwrap();
        let name = l
            .graph
            .string(start, sysml_model::Attr::MemberName)
            .map(str::to_string)
            .or_else(|| {
                let e = *l.graph.owned_related_elements(start).first()?;
                effective_names(db, project, g.file, e.index() as u32)
                    .0
                    .as_deref()
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let ns = path(
            db,
            project,
            Global {
                file: g.file,
                index: owner.index() as u32,
            },
        );
        if l.graph.owner(owner).is_none() {
            return format!("membership {name}");
        }
        return format!("membership {ns}::{name}");
    }
    let mut x = Some(start);
    let mut names = Vec::new();
    while let Some(e) = x {
        let (name, short) = effective_names(db, project, g.file, e.index() as u32).clone();
        if let Some(n) = name.or(short) {
            names.push(n.to_string());
        } else if e == start {
            names.push(format!("({})", l.graph.metaclass(e).name()));
        }
        x = l.graph.owner(e);
    }
    names.reverse();
    names.join("::")
}

/// Each name of a file, with what it resolves to.
fn resolutions(db: &Database, project: Project, file: SourceFile) -> String {
    let l = lowered(db, file);
    let text = file.text(db);
    let mut out = String::new();
    for n in resolved_names(db, project, file) {
        let rel = sysml_model::ElementIx::from_index(n.relationship as usize);
        let ends = match n.slot {
            Slot::Source => l.graph.source(rel),
            _ => l.graph.target(rel),
        };
        let sysml_model::Endpoint::Name(x) = ends[n.position as usize] else {
            continue;
        };
        let span = l.graph.name(x).span.unwrap();
        let target = n
            .target
            .map_or("unresolved".to_string(), |t| path(db, project, t));
        out.push_str(&format!(
            "{} {} -> {}\n",
            l.graph.metaclass(rel).name(),
            &text[span.start as usize..span.end as usize],
            target
        ));
    }
    out
}

fn snippet(id: &str) -> String {
    sysml_corpus::snippets()
        .unwrap()
        .into_iter()
        .find(|s| s.source.id == id)
        .unwrap_or_else(|| panic!("no snippet {id}"))
        .source
        .text
}

#[test]
fn private_members_are_not_visible_outside_their_namespace() {
    // KerML 7.2.5.2, 8.2.3.5.3: an import brings in the visible (public) memberships only.
    let db = Database::default();
    let user = "namespace Use { private import N3::*; feature x : C; feature y : D; }";
    let (project, files) = project(&db, &[&snippet("kerml/7.2.5.2/2"), user], false);
    expect![[r#"
        NamespaceImport N3 -> N3
        FeatureTyping C -> N3::C
        FeatureTyping D -> unresolved
    "#]]
    .assert_eq(&resolutions(&db, project, files[1]));
}

#[test]
fn aliases_resolve_by_name_and_short_name_and_keep_their_visibility() {
    // KerML 7.2.5.2: an alias membership names its member element; a private alias is not
    // visible outside.
    let db = Database::default();
    let user = "namespace Use { feature a : N4::CCC; feature b : N4::C; feature c : N4::D; }";
    let (project, files) = project(&db, &[&snippet("kerml/7.2.5.2/3"), user], false);
    expect![[r#"
        Membership B -> N4::B
        Membership B -> N4::B
        FeatureTyping N4::CCC -> N4::B
        FeatureTyping N4::C -> N4::B
        FeatureTyping N4::D -> unresolved
    "#]]
    .assert_eq(&format!(
        "{}{}",
        resolutions(&db, project, files[0]),
        resolutions(&db, project, files[1])
    ));
}

#[test]
fn imported_memberships_are_reimported_and_recursive_imports_reach_nested_members() {
    // KerML 7.2.5.4: a membership import keeps the member's name; `::**` also imports the
    // members of nested namespaces, and `N8::*::**` does not import N8 itself.
    let db = Database::default();
    let n6 = snippet("kerml/7.2.5.4/1").replace(
        "import C; // \"C\" is re-imported from N4 into M.",
        "import C; feature x : C; feature y : A;",
    );
    let recursive = "namespace N9 { private import N8::**; feature a : A; feature c : C; }
        namespace N10 { private import N8::*::**; alias m for M; feature c : C; }";
    let (project, files) = project(
        &db,
        &[
            &snippet("kerml/7.2.5.2/3"),
            &n6,
            &snippet("kerml/7.2.5.4/3"),
            recursive,
        ],
        false,
    );
    expect![[r#"
        MembershipImport N4::A -> membership N4::A
        MembershipImport N4::C -> membership N4::CCC
        MembershipImport C -> membership N4::CCC
        FeatureTyping C -> N4::B
        FeatureTyping A -> N4::A
    "#]]
    .assert_eq(&resolutions(&db, project, files[1]));
    expect![[r#"
        MembershipImport N8 -> membership N8
        FeatureTyping A -> N8::A
        FeatureTyping C -> N8::M::C
        NamespaceImport N8 -> N8
        Membership M -> N8::M
        FeatureTyping C -> N8::M::C
    "#]]
    .assert_eq(&resolutions(&db, project, files[3]));
}

#[test]
fn a_type_inherits_its_generals_public_and_protected_members_only() {
    // KerML 7.3.2.3, 8.3.3.1.10: B inherits f and g from A, but not the private h.
    let db = Database::default();
    let user =
        "type C specializes B { feature x subsets f; feature y subsets g; feature z subsets h; }";
    let source = snippet("kerml/7.3.2.3/4").replace("specializes Base::Anything ", "");
    let (project, files) = project(&db, &[&source, user], false);
    expect![[r#"
        Specialization B -> B
        Subsetting f -> A::f
        Subsetting g -> A::g
        Subsetting h -> unresolved
    "#]]
    .assert_eq(&resolutions(&db, project, files[1]));
}

#[test]
fn redefinitions_resolve_in_the_generals_and_name_the_redefining_feature() {
    // KerML 7.3.4.5 and 8.2.3.5.1: `redefines identifier` resolves in the general types of the
    // owning type (the snippet's `: RegisteredAsset` is `:>`, `snippets.json`); a feature with no declared name takes its effective name from the feature it
    // redefines (8.3.3.3.4), and hides it from inheritance (8.3.3.1.10).
    let db = Database::default();
    let user = "classifier Use specializes Automobile { feature x subsets wheels; }";
    let (project, files) = project(
        &db,
        &[
            &snippet("kerml/7.3.4.5/4").replace(": RegisteredAsset", ":> RegisteredAsset"),
            &snippet("kerml/7.3.4.5/5"),
            user,
        ],
        false,
    );
    expect![[r#"
        FeatureTyping Identifier -> unresolved
        Subclassification RegisteredAsset -> RegisteredAsset
        Redefinition identifier -> RegisteredAsset::identifier
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
    expect![[r#"
        FeatureTyping Wheel -> unresolved
        Subclassification WheeledVehicle -> WheeledVehicle
        Redefinition wheels -> WheeledVehicle::wheels
        Subclassification MotorizedVehicle -> MotorizedVehicle
        Redefinition wheels -> MotorizedVehicle::wheels
        FeatureTyping AutomobileWheel -> unresolved
    "#]]
    .assert_eq(&resolutions(&db, project, files[1]));
    expect![[r#"
        Subclassification Automobile -> Automobile
        Subsetting wheels -> Automobile::wheels
    "#]]
    .assert_eq(&resolutions(&db, project, files[2]));
}

#[test]
fn implied_specializations_and_redefinitions_take_part_in_resolution() {
    // KerML 8.4.2: a behavior implicitly specializes Performances::Performance
    // (checkBehaviorSpecialization), so its steps see enclosedPerformances; a parameter
    // implicitly redefines the parameter at its position (checkFeatureParameterRedefinition),
    // which hides it, so `x` resolves outside, and names an unnamed parameter (8.3.3.3.4). A
    // feature implicitly subsets Base::things (checkFeatureSpecialization), so `that` is visible
    // in it.
    let db = Database::default();
    let user = "package P {
        feature x;
        behavior B { in x; in y; }
        behavior C specializes B {
            in a; in b;
            step s subsets enclosedPerformances;
            feature u subsets x;
            feature v subsets y; // hidden by `b`, and found only by the last resort (ADR 0020)
        }
        behavior D { in p; }
        behavior E specializes D { in; feature q subsets p; feature w { feature t subsets that; } }
    }";
    let (project, files) = project(&db, &[user], true);
    expect![[r#"
        Subclassification B -> P::B
        Subsetting enclosedPerformances -> Performances::Performance::enclosedPerformances
        Subsetting x -> P::x
        Subsetting y -> P::B::y
        Subclassification D -> P::D
        Subsetting p -> P::E::p
        Subsetting that -> Base::things::that
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn an_edit_reruns_only_the_queries_that_depend_on_it() {
    // ADR 0019: an edit of layout reruns the edited file's parse and lowering, and stops at its
    // span-free structure. An edit of a file's names reruns that file's resolution, and no other
    // file's, because the memberships the others read backdate.
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = log.clone();
    let mut db = Database::with_log(move |e| sink.lock().unwrap().push(e));
    let (project, files) = project(
        &db,
        &[
            "package A { class X; }",
            "package B { private import A::*; feature f : X; }",
            "package C { class Y; feature g : Y; }",
        ],
        false,
    );
    let resolve_all = |db: &Database| -> Vec<String> {
        files.iter().map(|f| resolutions(db, project, *f)).collect()
    };
    let before = resolve_all(&db);
    let edit = |db: &mut Database, text: &str| -> Vec<String> {
        log.lock().unwrap().clear();
        files[2].set_text(db).to(text.to_string());
        for f in &files {
            resolved_names(db, project, *f);
        }
        log.lock().unwrap().clone()
    };

    let reran = edit(
        &mut db,
        "package C {\n    class Y;\n    feature g : Y;\n}\n",
    );
    let kinds: Vec<&str> = reran.iter().map(|e| e.split('(').next().unwrap()).collect();
    assert_eq!(kinds, ["parsed", "lowered", "structure"], "{reran:#?}");
    assert_eq!(resolve_all(&db), before);

    let reran = edit(&mut db, "package C { class Z; feature g : Z; }");
    let count = |q: &str| reran.iter().filter(|e| e.starts_with(q)).count();
    assert_eq!(count("resolved_names("), 1, "{reran:#?}");
    let after = resolve_all(&db);
    assert_eq!(after[..2], before[..2]);
    assert_eq!(after[2], "FeatureTyping Z -> C::Z\n");
}

#[test]
fn a_recursive_import_descends_only_into_owned_namespaces() {
    // KerML 8.3.2.4.5 visibleMemberships: recursion follows `ownedMember`, so C, which A only
    // imports, is not entered, and X is not imported into U.
    let db = Database::default();
    let user = "package B { package C { class X; } }
        package A { public import B::*; }
        package U { private import A::**; alias c for C; feature g : X; }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        NamespaceImport B -> B
        MembershipImport A -> membership A
        Membership C -> B::C
        FeatureTyping X -> unresolved
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn import_all_applies_at_every_level_of_a_recursive_import() {
    // KerML 8.3.2.4.4, 8.3.2.4.5: `all` is passed down as includeAll, so private members of
    // nested namespaces, and private nested namespaces, are imported too.
    let db = Database::default();
    let user = "package N { package M { private class P; } private package Q { class R; } }
        package U { private import all N::**; feature f : P; feature g : R; }
        package V { private import N::**; feature f : P; feature g : R; }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        MembershipImport N -> membership N
        FeatureTyping P -> N::M::P
        FeatureTyping R -> N::Q::R
        MembershipImport N -> membership N
        FeatureTyping P -> unresolved
        FeatureTyping R -> unresolved
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn import_all_with_a_filter_does_not_reach_private_members() {
    // KerML 8.2.3.4.2: `all` is on the import of the FilterPackage; the FilterPackage's own
    // import, which cannot be `all`, imports the visible memberships of A (ADR 0020).
    let db = Database::default();
    let user = "package A { private class S; public class T; }
        package U { private import all A::*[true]; feature f : S; feature g : T; }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        NamespaceImport A -> A
        FeatureTyping S -> unresolved
        FeatureTyping T -> A::T
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn a_parameter_redefines_by_position_even_when_it_subsets_that_parameter() {
    // KerML 8.3.3.3.4 checkFeatureParameterRedefinition: `a` must redefine B::x, which it only
    // subsets; that hides B::x in C, so `x` resolves outside (decision 1 of ADR 0020).
    let db = Database::default();
    let user = "package P {
        feature x;
        behavior B { in x; }
        behavior C specializes B { in a subsets B::x; feature u subsets x; }
    }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        Subclassification B -> P::B
        Subsetting B::x -> P::B::x
        Subsetting x -> P::x
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

/// The implied relationships of every element of a metaclass in a file.
fn implied_of(
    db: &Database,
    project: Project,
    file: SourceFile,
    m: sysml_model::Metaclass,
) -> String {
    let s = structure(db, file);
    let mut out = String::new();
    for e in s.graph().elements() {
        if s.graph().metaclass(e) != m {
            continue;
        }
        let at = Global {
            file,
            index: e.index() as u32,
        };
        out.push_str(&format!("{}:", path(db, project, at)));
        for i in implied_generals(db, project, file, at.index) {
            out.push_str(&format!(
                " {} {};",
                i.kind.name(),
                path(db, project, i.general)
            ));
        }
        out.push('\n');
    }
    out
}

#[test]
fn flow_features_redefine_by_the_position_of_their_flow_end_after_a_payload() {
    // KerML 8.3.3.3.4 checkFeatureFlowFeatureRedefinition: the first and second FlowEnds, not
    // the first and second owned features, which start with the payload (ADR 0020).
    let db = Database::default();
    let user = "package P {
        struct S { feature out; feature in; }
        feature a : S; feature b : S;
        flow of Anything from a.out to b.in;
    }";
    let (project, files) = project(&db, &[user], true);
    let s = structure(&db, files[0]);
    let g = s.graph();
    let mut out = String::new();
    for end in g
        .elements()
        .filter(|e| g.metaclass(*e) == sysml_model::Metaclass::FlowEnd)
    {
        let feature = g
            .owned_relationships(end)
            .iter()
            .filter(|r| {
                g.metaclass(**r)
                    .specializes(sysml_model::Metaclass::FeatureMembership)
            })
            .find_map(|r| g.owned_related_elements(*r).first().copied())
            .unwrap();
        for i in implied_generals(&db, project, files[0], feature.index() as u32) {
            if i.kind == sysml_model::Metaclass::Redefinition {
                out.push_str(&format!("{}\n", path(&db, project, i.general)));
            }
        }
    }
    expect![[r#"
        Transfers::Transfer::source::sourceOutput
        Transfers::Transfer::target::targetInput
    "#]]
    .assert_eq(&out);
}

#[test]
fn a_connector_without_owned_ends_is_binary_by_its_inherited_ends() {
    // KerML 8.3.4.5.3 checkConnectorBinarySpecialization: `connectorEnd` includes the ends a
    // connector inherits from its association.
    let db = Database::default();
    let user = "package P { assoc A { end feature s; end feature t; } connector c : A; }";
    let (project, files) = project(&db, &[user], true);
    expect![[r#"
        P::c: Subsetting Links::binaryLinks;
    "#]]
    .assert_eq(&implied_of(
        &db,
        project,
        files[0],
        sysml_model::Metaclass::Connector,
    ));
}

#[test]
fn a_redefinition_looks_in_every_general_before_their_namespaces() {
    // KerML 8.2.3.5.1: B declares `x`, so the redefinition does not reach `P::x` through the
    // namespace of A, the first general (ADR 0020).
    let db = Database::default();
    let user = "package P {
        feature x;
        classifier A;
        classifier B { feature x; }
        classifier T specializes A, B { feature redefines x; }
    }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        Subclassification A -> P::A
        Subclassification B -> P::B
        Redefinition x -> P::B::x
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn a_user_package_named_like_a_library_package_does_not_hide_it() {
    // KerML 8.4.2: `Links::BinaryLink` resolves globally; a user's `package Links`, listed before
    // the libraries, has no `BinaryLink` and does not stop the lookup (ADR 0020).
    let db = Database::default();
    let user = SourceFile::new(
        &db,
        FileId(0),
        "user.kerml".into(),
        Language::KerML,
        "package Links { assoc A { end feature s; end feature t; } }".into(),
    );
    let mut files = vec![user];
    files.extend(kerml_library(&db).1.into_iter().map(|f| f.0));
    let project = Project::new(&db, files);
    expect![[r#"
        Links::A: Subclassification Links::BinaryLink;
    "#]]
    .assert_eq(&implied_of(
        &db,
        project,
        user,
        sysml_model::Metaclass::Association,
    ));
}

#[test]
fn a_chained_name_resolves_only_among_the_features_of_the_argument() {
    // KerML 8.2.3.5.2: in `a.foo`, `foo` is a feature of the result of `a`; C has none.
    let db = Database::default();
    let user = "package P { feature foo; classifier C; feature a : C; feature b = a.foo; }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        FeatureTyping C -> P::C
        Membership a -> P::a
        Membership foo -> unresolved
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn imported_memberships_that_collide_are_excluded() {
    // KerML 8.3.2.4.5 importedMemberships, 8.3.2.4.3 isDistinguishableFrom: two classes named X
    // collide; a class and a feature named Y do not, so the first import's is found.
    let db = Database::default();
    let user = "package A { public class X; public class Y; }
        package B { public class X; public feature Y; }
        package U { public import A::*; public import B::*; feature f : X; feature g : Y; }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        NamespaceImport A -> A
        NamespaceImport B -> B
        FeatureTyping X -> unresolved
        FeatureTyping Y -> A::Y
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn circular_aliases_and_imports_settle_without_a_panic() {
    // KerML 8.2.3.5.1 allows circular imports; aliases of each other resolve to nothing, and
    // every fixpoint query settles (ADR 0020).
    let db = Database::default();
    let user = "package A { public import B::*; public class X; alias a for b; alias b for a; }
        package B { public import A::*; public class X; feature f : a; feature g : X; }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        NamespaceImport B -> B
        Membership b -> unresolved
        Membership a -> unresolved
        NamespaceImport A -> A
        FeatureTyping a -> unresolved
        FeatureTyping X -> B::X
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn an_implied_redefinition_makes_the_implied_subsetting_of_things_redundant() {
    // KerML 8.4.2 rule 1: B::x already subsets Base::things, and `a` redefines it by implication.
    let db = Database::default();
    let user = "package P { behavior B { in x; } behavior C specializes B { in a; } }";
    let (project, files) = project(&db, &[user], true);
    expect![[r#"
        P::B::x: Subsetting Base::things;
        P::C::a: Redefinition P::B::x;
    "#]]
    .assert_eq(&implied_of(
        &db,
        project,
        files[0],
        sysml_model::Metaclass::Feature,
    ));
}

#[test]
fn an_inherited_feature_is_hidden_only_where_every_path_or_an_inherited_feature_redefines_it() {
    // KerML 8.3.3.1.10 removeRedefinedFeatures. In D, B's `g` (inherited) redefines A::f, so A::f
    // is removed and `f` resolves outside. In E, B2's `g` is private and not inherited; A::f
    // comes unhidden through C, so `f` is A::f.
    let db = Database::default();
    let user = "package P {
        feature f;
        class A { feature f; }
        class B specializes A { feature g redefines f; }
        class B2 specializes A { private feature g redefines f; }
        class C specializes A;
        class D specializes B, C { feature h subsets f; }
        class E specializes B2, C { feature h subsets f; }
    }";
    let (project, files) = project(&db, &[user], false);
    let all = resolutions(&db, project, files[0]);
    let subsets: Vec<&str> = all
        .lines()
        .filter(|l| l.starts_with("Subsetting"))
        .collect();
    assert_eq!(
        subsets,
        ["Subsetting f -> P::f", "Subsetting f -> P::A::f"],
        "{all}"
    );
}

#[test]
fn a_qualified_redefined_feature_is_tried_with_each_general() {
    // KerML 8.2.3.5.1: the whole name is resolved with each general in turn; G1's `A` has no `b`,
    // so G2's resolution, which reaches P::A, is used.
    let db = Database::default();
    let user = "package P {
        package A { feature b; }
        classifier G1 { package A; }
        classifier G2;
        classifier T specializes G1, G2 { feature redefines A::b; }
    }";
    let (project, files) = project(&db, &[user], false);
    let all = resolutions(&db, project, files[0]);
    assert!(all.contains("Redefinition A::b -> P::A::b"), "{all}");
}

#[test]
fn a_name_of_an_element_of_the_wrong_metaclass_has_no_resolution() {
    // KerML 8.2.3.5.1: a FeatureTyping needs a Type, a Subclassification a Classifier and a
    // Subsetting a Feature; a feature, which is a Type, may type another.
    let db = Database::default();
    let user = "package P {
        package Q; classifier K; feature x;
        feature y : Q; classifier L specializes Q; feature z subsets K; feature w : x;
    }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        FeatureTyping Q -> unresolved
        Subclassification Q -> unresolved
        Subsetting K -> unresolved
        FeatureTyping x -> P::x
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn a_redefinition_in_a_type_that_specializes_itself_does_not_find_itself() {
    // KerML 8.2.3.5.1: the owning type is not one of its own generals here (ADR 0020 decision 2).
    let db = Database::default();
    let user = "package P { feature x; classifier T specializes T { feature redefines x; } }";
    let (project, files) = project(&db, &[user], false);
    expect![[r#"
        Subclassification T -> P::T
        Redefinition x -> P::x
    "#]]
    .assert_eq(&resolutions(&db, project, files[0]));
}

#[test]
fn subobjects_are_implied_by_a_structure_owner_not_by_a_classifier_specializing_one() {
    // KerML 8.3.3.3.4 checkFeatureSubobjectSpecialization: `owningType.oclIsKindOf(Structure)`,
    // which K, a classifier specializing S, is not.
    let db = Database::default();
    let user = "package P {
        struct S;
        classifier K specializes S { composite feature p : S; }
        struct T { composite feature q : S; }
    }";
    let (project, files) = project(&db, &[user], true);
    expect![[r#"
        P::K::p: Subsetting Objects::objects;
        P::T::q: Subsetting Objects::Object::subobjects;
    "#]]
    .assert_eq(&implied_of(
        &db,
        project,
        files[0],
        sysml_model::Metaclass::Feature,
    ));
}

#[test]
fn private_imports_in_a_ring_do_not_make_one_cycle_of_the_ring() {
    // KerML 8.3.2.4.5 visibleMemberships: membershipsOfVisibility(public) evaluates the public
    // imports only, so what a private import brings is no part of what a package shows. Thirty
    // packages, each privately importing the next, resolve their names; an edit of one reruns the
    // queries of its neighbours, not those of the whole ring.
    let n = 30;
    let texts: Vec<String> = (0..n)
        .map(|i| {
            format!(
                "package P{i} {{ private import P{}::*; classifier C{i}; feature f : C{}; }}",
                (i + 1) % n,
                (i + 1) % n
            )
        })
        .collect();
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = log.clone();
    let mut db = Database::with_log(move |e| sink.lock().unwrap().push(e));
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let (project, files) = project(&db, &refs, false);
    for (i, f) in files.iter().enumerate() {
        let r = resolutions(&db, project, *f);
        assert!(
            r.contains(&format!(
                "FeatureTyping C{} -> P{}::C{}",
                (i + 1) % n,
                (i + 1) % n,
                (i + 1) % n
            )),
            "{r}"
        );
    }
    log.lock().unwrap().clear();
    files[15].set_text(&mut db).to(
        "package P15 { private import P16::*; classifier C15; classifier D15; feature f : C16; }"
            .to_string(),
    );
    for f in &files {
        resolved_names(&db, project, *f);
    }
    let reran = log.lock().unwrap().clone();
    let outside = reran
        .iter()
        .filter(|e| e.starts_with("visible_memberships(") || e.starts_with("outside_memberships("))
        .count();
    assert!(
        outside < 10,
        "{outside} of the queries of what packages show reran:\n{reran:#?}"
    );
}
