// SPDX-License-Identifier: Apache-2.0
//! The lowering of each production (KerML 8.2.3–8.2.5) into elements and relationships, against
//! snapshots (`UPDATE_EXPECT=1 cargo test -p sysml-semantics` rewrites them, for review), and the
//! properties of the element graph it builds: walkable by ownership and by relationship, with a
//! source span on every element (step 1a.6).

use expect_test::{expect, Expect};
use sysml_model::{Attr, ElementIx, Endpoint, FileId, Graph, Metaclass, Slot, Value};
use sysml_semantics::{lower, Lowered};
use sysml_syntax::{parse, Language};
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn lowered(text: &str) -> Lowered {
    let p = parse(Language::KerML, text);
    assert!(p.diagnostics().is_empty(), "{text}: {:?}", p.diagnostics());
    let mut n = 0u128;
    lower(FileId(0), &p.syntax(), &mut || {
        n += 1;
        Uuid::from_u128(n)
    })
}

/// An end as text: a name as written, `owner` for the element that owns the relationship, or the
/// metaclass of an element the relationship owns (a feature chain).
fn end(g: &Graph, rel: ElementIx, e: &Endpoint) -> String {
    match e {
        Endpoint::Name(n) => {
            let name = g.name(*n);
            let segments: Vec<&str> = name.segments.iter().map(|s| &**s).collect();
            format!(
                "{}{}",
                if name.global { "$::" } else { "" },
                segments.join("::")
            )
        }
        Endpoint::Element(x) if g.owning_related_element(rel) == Some(*x) => "owner".to_owned(),
        Endpoint::Element(x) => format!("<{}>", g.metaclass(*x).name()),
    }
}

fn dump(g: &Graph, ix: ElementIx, depth: usize, role: &str, out: &mut String) {
    let m = g.metaclass(ix);
    let mut line = format!("{}{role}{}", "  ".repeat(depth), m.name());
    for attr in Attr::ALL {
        for v in g.values(ix, *attr) {
            let v = match v {
                Value::Boolean(b) => b.to_string(),
                Value::String(s) => format!("{s:?}"),
                Value::Integer(i) => i.to_string(),
                Value::Real(r) => format!("{r:?}"),
                Value::Enumeration(e) => (*e).to_owned(),
            };
            line.push_str(&format!(" {}={v}", attr.name()));
        }
    }
    for (slot, ends) in [(Slot::Source, g.source(ix)), (Slot::Target, g.target(ix))] {
        if ends.is_empty() {
            continue;
        }
        let name = m
            .stored_attributes()
            .iter()
            .find(|a| a.slot == slot)
            .map_or("?", |a| a.name);
        let ends: Vec<String> = ends.iter().map(|e| end(g, ix, e)).collect();
        line.push_str(&format!(" {name}->{}", ends.join(",")));
    }
    out.push_str(&line);
    out.push('\n');
    for r in g.owned_relationships(ix) {
        dump(g, *r, depth + 1, "", out);
    }
    for e in g.owned_related_elements(ix) {
        dump(g, *e, depth + 1, "· ", out);
    }
}

/// Lower `text` and compare the graph, below the root namespace, with the snapshot.
fn check(text: &str, expect: Expect) {
    let l = lowered(text);
    assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
    let mut out = String::new();
    for r in l.graph.owned_relationships(l.root) {
        dump(&l.graph, *r, 0, "", &mut out);
    }
    expect.assert_eq(&out);
}

// KerML 8.2.3: Root.

#[test]
fn namespace_members_and_visibility() {
    check(
        "namespace <n1> N { private classifier C; protected feature f; alias A for C; }",
        expect![[r#"
            OwningMembership
              · Namespace declaredName="N" declaredShortName="n1"
                OwningMembership visibility=private
                  · Classifier declaredName="C"
                OwningMembership visibility=protected
                  · Feature declaredName="f"
                Membership memberName="A" memberElement->C
        "#]],
    );
}

#[test]
fn dependency_with_clients_suppliers_and_metadata() {
    check(
        "#Trace dependency d from A, B to C;",
        expect![[r#"
            OwningMembership
              · Dependency declaredName="d" client->A,B supplier->C
                Annotation
                  · MetadataFeature
                    FeatureTyping typedFeature->owner type->Trace
        "#]],
    );
}

#[test]
fn comments_documentation_and_textual_representation() {
    check(
        r#"comment c about A, B locale "en" /* c */ doc /* d */ rep language "ocl" /* x > 0 */"#,
        expect![[r#"
            OwningMembership
              · Comment body="c " declaredName="c" locale="en"
                Annotation annotatedElement->A
                Annotation annotatedElement->B
            OwningMembership
              · Documentation body="d "
            OwningMembership
              · TextualRepresentation body="x > 0 " language="ocl"
        "#]],
    );
}

#[test]
fn imports() {
    check(
        "public import A::B; private import all A::*::**; private import F::*[x];",
        expect![[r#"
            MembershipImport visibility=public importedMembership->A::B
            NamespaceImport isImportAll=true isRecursive=true visibility=private importedNamespace->A
            NamespaceImport visibility=private importedNamespace-><Package>
              · Package
                NamespaceImport importedNamespace->F
                ElementFilterMembership
                  · FeatureReferenceExpression
                    Membership memberElement->x
                    ReturnParameterMembership
                      · Feature direction=out
        "#]],
    );
}

#[test]
fn a_relationship_body_owns_elements_and_annotations() {
    check(
        "alias A for B { doc /* d */ classifier C; }",
        expect![[r#"
        Membership memberName="A" memberElement->B
          Annotation
            · Documentation body="d "
          · Classifier declaredName="C"
    "#]],
    );
}

// KerML 8.2.4: Core.

#[test]
fn type_with_every_relationship_part() {
    check(
        "abstract type all T[1] :> A ~ B disjoint from C unions D intersects E differences F;",
        expect![[r#"
            OwningMembership
              · Type declaredName="T" isAbstract=true isSufficient=true
                OwningMembership
                  · MultiplicityRange
                    OwningMembership
                      · LiteralInteger value=1
                Specialization specific->owner general->A
                Conjugation conjugatedType->owner originalType->B
                Disjoining typeDisjoined->owner disjoiningType->C
                Unioning unioningType->D
                Intersecting intersectingType->E
                Differencing differencingType->F
        "#]],
    );
}

#[test]
fn classifier_and_feature_declarations() {
    check(
        "classifier K :> A { in derived abstract composite var feature x : T[0..*] ordered nonunique :> y :>> z ::> r => c; const end e : T; }",
        expect![[r#"
            OwningMembership
              · Classifier declaredName="K"
                Subclassification subclassifier->owner superclassifier->A
                FeatureMembership
                  · Feature declaredName="x" direction=in isAbstract=true isComposite=true isDerived=true isOrdered=true isUnique=false isVariable=true
                    FeatureTyping typedFeature->owner type->T
                    OwningMembership
                      · MultiplicityRange
                        OwningMembership
                          · LiteralInteger value=0
                        OwningMembership
                          · LiteralInfinity
                    Subsetting subsettingFeature->owner subsettedFeature->y
                    Redefinition redefiningFeature->owner redefinedFeature->z
                    ReferenceSubsetting referencedFeature->r
                    CrossSubsetting crossedFeature->c
                FeatureMembership
                  · Feature declaredName="e" isConstant=true isEnd=true isVariable=true
                    FeatureTyping typedFeature->owner type->T
        "#]],
    );
}

#[test]
fn feature_chains_inverting_and_featuring() {
    check(
        "feature f : a.b chains c.d inverse of g featured by H;",
        expect![[r#"
            OwningMembership
              · Feature declaredName="f"
                FeatureTyping typedFeature->owner type-><Feature>
                  · Feature
                    FeatureChaining chainingFeature->a
                    FeatureChaining chainingFeature->b
                FeatureChaining chainingFeature->c
                FeatureChaining chainingFeature->d
                FeatureInverting featureInverted->owner invertingFeature->g
                TypeFeaturing featureOfType->owner featuringType->H
        "#]],
    );
}

#[test]
fn standalone_relationships() {
    check(
        "specialization s subtype A :> B; subclassifier C :> D; typing t : T; subset a.b :> c; \
         redefinition x :>> y; conjugate P ~ Q; disjoint R from S; inverse u of v; featuring w by W;",
        expect![[r#"
            OwningMembership
              · Specialization declaredName="s" specific->A general->B
            OwningMembership
              · Subclassification subclassifier->C superclassifier->D
            OwningMembership
              · FeatureTyping typedFeature->t type->T
            OwningMembership
              · Subsetting subsettingFeature-><Feature> subsettedFeature->c
                · Feature
                  FeatureChaining chainingFeature->a
                  FeatureChaining chainingFeature->b
            OwningMembership
              · Redefinition redefiningFeature->x redefinedFeature->y
            OwningMembership
              · Conjugation conjugatedType->P originalType->Q
            OwningMembership
              · Disjoining typeDisjoined->R disjoiningType->S
            OwningMembership
              · FeatureInverting featureInverted->u invertingFeature->v
            OwningMembership
              · TypeFeaturing featureOfType->w featuringType->W
        "#]],
    );
}

#[test]
fn an_end_feature_with_a_cross_feature() {
    check(
        "assoc A { end x [1] feature y : T; }",
        expect![[r#"
        OwningMembership
          · Association declaredName="A"
            FeatureMembership
              · Feature declaredName="y" isEnd=true
                OwningMembership
                  · Feature declaredName="x"
                    OwningMembership
                      · MultiplicityRange
                        OwningMembership
                          · LiteralInteger value=1
                FeatureTyping typedFeature->owner type->T
    "#]],
    );
}

// KerML 8.2.5: Kernel.

#[test]
fn function_with_parameters_return_and_result() {
    check(
        "function f { in x : Real; return : Real; x * 2 }",
        expect![[r#"
        OwningMembership
          · Function declaredName="f"
            FeatureMembership
              · Feature declaredName="x" direction=in
                FeatureTyping typedFeature->owner type->Real
            ReturnParameterMembership
              · Feature direction=out
                FeatureTyping typedFeature->owner type->Real
            ResultExpressionMembership
              · OperatorExpression operator="*"
                ParameterMembership
                  · Feature direction=in
                    FeatureValue
                      · FeatureReferenceExpression
                        Membership memberElement->x
                        ReturnParameterMembership
                          · Feature direction=out
                ParameterMembership
                  · Feature direction=in
                    FeatureValue
                      · LiteralInteger value=2
                ReturnParameterMembership
                  · Feature direction=out
    "#]],
    );
}

#[test]
fn feature_values() {
    check(
        "feature a = 1; feature b := 2.5; feature c default \"s\"; feature d default = true;",
        expect![[r#"
            OwningMembership
              · Feature declaredName="a"
                FeatureValue
                  · LiteralInteger value=1
            OwningMembership
              · Feature declaredName="b"
                FeatureValue isInitial=true
                  · LiteralRational value=2.5
            OwningMembership
              · Feature declaredName="c"
                FeatureValue isDefault=true
                  · LiteralString value="s"
            OwningMembership
              · Feature declaredName="d"
                FeatureValue isDefault=true
                  · LiteralBoolean value=true
        "#]],
    );
}

#[test]
fn operator_expressions() {
    check(
        "feature a = if p ? q else r; feature b = x ?? y; feature c = -z; feature d = w istype T; \
         feature e = v as U; feature f = all V;",
        expect![[r#"
            OwningMembership
              · Feature declaredName="a"
                FeatureValue
                  · OperatorExpression operator="if"
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->p
                            ReturnParameterMembership
                              · Feature direction=out
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            FeatureMembership
                              · FeatureReferenceExpression
                                Membership memberElement->q
                                ReturnParameterMembership
                                  · Feature direction=out
                            ReturnParameterMembership
                              · Feature direction=out
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            FeatureMembership
                              · FeatureReferenceExpression
                                Membership memberElement->r
                                ReturnParameterMembership
                                  · Feature direction=out
                            ReturnParameterMembership
                              · Feature direction=out
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="b"
                FeatureValue
                  · OperatorExpression operator="??"
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->x
                            ReturnParameterMembership
                              · Feature direction=out
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            FeatureMembership
                              · FeatureReferenceExpression
                                Membership memberElement->y
                                ReturnParameterMembership
                                  · Feature direction=out
                            ReturnParameterMembership
                              · Feature direction=out
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="c"
                FeatureValue
                  · OperatorExpression operator="-"
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->z
                            ReturnParameterMembership
                              · Feature direction=out
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="d"
                FeatureValue
                  · OperatorExpression operator="istype"
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->w
                            ReturnParameterMembership
                              · Feature direction=out
                    ParameterMembership
                      · Feature direction=in
                        FeatureTyping typedFeature->owner type->T
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="e"
                FeatureValue
                  · OperatorExpression operator="as"
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->v
                            ReturnParameterMembership
                              · Feature direction=out
                    ReturnParameterMembership
                      · Feature direction=out
                        FeatureTyping typedFeature->owner type->U
            OwningMembership
              · Feature declaredName="f"
                FeatureValue
                  · OperatorExpression operator="all"
                    ParameterMembership
                      · Feature direction=in
                        FeatureTyping typedFeature->owner type->V
        "#]],
    );
}

#[test]
fn primary_and_base_expressions() {
    check(
        "feature a = x.y; feature b = xs->f(1, 2); feature c = g(p = 1); feature d = new N(); \
         feature e = (1, 2, 3); feature f = v#(1); feature g = null; feature h = m.metadata;",
        expect![[r##"
            OwningMembership
              · Feature declaredName="a"
                FeatureValue
                  · FeatureChainExpression operator="."
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->x
                            ReturnParameterMembership
                              · Feature direction=out
                        FeatureMembership
                          · Feature
                    Membership memberElement->y
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="b"
                FeatureValue
                  · InvocationExpression
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->xs
                            ReturnParameterMembership
                              · Feature direction=out
                    Membership memberElement->f
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · LiteralInteger value=1
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · LiteralInteger value=2
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="c"
                FeatureValue
                  · InvocationExpression
                    Membership memberElement->g
                    FeatureMembership
                      · Feature
                        Redefinition redefiningFeature->owner redefinedFeature->p
                        FeatureValue
                          · LiteralInteger value=1
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="d"
                FeatureValue
                  · ConstructorExpression
                    Membership memberElement->N
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="e"
                FeatureValue
                  · OperatorExpression operator=","
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · LiteralInteger value=1
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · OperatorExpression operator=","
                            ParameterMembership
                              · Feature direction=in
                                FeatureValue
                                  · LiteralInteger value=2
                            ParameterMembership
                              · Feature direction=in
                                FeatureValue
                                  · LiteralInteger value=3
                            ReturnParameterMembership
                              · Feature direction=out
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="f"
                FeatureValue
                  · IndexExpression operator="#"
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->v
                            ReturnParameterMembership
                              · Feature direction=out
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · LiteralInteger value=1
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="g"
                FeatureValue
                  · NullExpression
            OwningMembership
              · Feature declaredName="h"
                FeatureValue
                  · MetadataAccessExpression
                    Membership memberElement->m
        "##]],
    );
}

#[test]
fn body_expressions_and_function_references() {
    check(
        "feature a = xs->select {in x; x > 0}; feature b = xs->reduce '+';",
        expect![[r#"
            OwningMembership
              · Feature declaredName="a"
                FeatureValue
                  · InvocationExpression
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->xs
                            ReturnParameterMembership
                              · Feature direction=out
                    Membership memberElement->select
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            FeatureMembership
                              · Expression
                                FeatureMembership
                                  · Feature declaredName="x" direction=in
                                ResultExpressionMembership
                                  · OperatorExpression operator=">"
                                    ParameterMembership
                                      · Feature direction=in
                                        FeatureValue
                                          · FeatureReferenceExpression
                                            Membership memberElement->x
                                            ReturnParameterMembership
                                              · Feature direction=out
                                    ParameterMembership
                                      · Feature direction=in
                                        FeatureValue
                                          · LiteralInteger value=0
                                    ReturnParameterMembership
                                      · Feature direction=out
                            ReturnParameterMembership
                              · Feature direction=out
                    ReturnParameterMembership
                      · Feature direction=out
            OwningMembership
              · Feature declaredName="b"
                FeatureValue
                  · InvocationExpression
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            Membership memberElement->xs
                            ReturnParameterMembership
                              · Feature direction=out
                    Membership memberElement->reduce
                    ParameterMembership
                      · Feature direction=in
                        FeatureValue
                          · FeatureReferenceExpression
                            FeatureMembership
                              · Expression
                                FeatureTyping typedFeature->owner type->+
                            ReturnParameterMembership
                              · Feature direction=out
                    ReturnParameterMembership
                      · Feature direction=out
        "#]],
    );
}

#[test]
fn connectors_bindings_and_successions() {
    check(
        "feature f { connector c : C from [1] a references x to b; binding p = q; succession first s then t; }",
        expect![[r#"
            OwningMembership
              · Feature declaredName="f"
                FeatureMembership
                  · Connector declaredName="c"
                    FeatureTyping typedFeature->owner type->C
                    EndFeatureMembership
                      · Feature declaredName="a" isEnd=true
                        OwningMembership
                          · Feature
                            OwningMembership
                              · MultiplicityRange
                                OwningMembership
                                  · LiteralInteger value=1
                        ReferenceSubsetting referencedFeature->x
                    EndFeatureMembership
                      · Feature isEnd=true
                        ReferenceSubsetting referencedFeature->b
                FeatureMembership
                  · BindingConnector
                    EndFeatureMembership
                      · Feature isEnd=true
                        ReferenceSubsetting referencedFeature->p
                    EndFeatureMembership
                      · Feature isEnd=true
                        ReferenceSubsetting referencedFeature->q
                FeatureMembership
                  · Succession
                    EndFeatureMembership
                      · Feature isEnd=true
                        ReferenceSubsetting referencedFeature->s
                    EndFeatureMembership
                      · Feature isEnd=true
                        ReferenceSubsetting referencedFeature->t
        "#]],
    );
}

#[test]
fn flows_with_a_payload() {
    check(
        "feature f { flow of p : P from a.x to b.y; }",
        expect![[r#"
        OwningMembership
          · Feature declaredName="f"
            FeatureMembership
              · Flow
                FeatureMembership
                  · PayloadFeature declaredName="p"
                    FeatureTyping typedFeature->owner type->P
                EndFeatureMembership
                  · FlowEnd isEnd=true
                    ReferenceSubsetting referencedFeature->a
                    FeatureMembership
                      · Feature
                        Redefinition redefiningFeature->owner redefinedFeature->x
                EndFeatureMembership
                  · FlowEnd isEnd=true
                    ReferenceSubsetting referencedFeature->b
                    FeatureMembership
                      · Feature
                        Redefinition redefiningFeature->owner redefinedFeature->y
    "#]],
    );
}

#[test]
fn metadata_features() {
    check(
        "metaclass M; #M classifier C; @M about C { x = 1; }",
        expect![[r#"
            OwningMembership
              · Metaclass declaredName="M"
            OwningMembership
              · Classifier declaredName="C"
                OwningMembership
                  · MetadataFeature
                    FeatureTyping typedFeature->owner type->M
            OwningMembership
              · MetadataFeature
                FeatureTyping typedFeature->owner type->M
                Annotation annotatedElement->C
                FeatureMembership
                  · Feature
                    Redefinition redefiningFeature->owner redefinedFeature->x
                    FeatureValue
                      · LiteralInteger value=1
        "#]],
    );
}

#[test]
fn multiplicities_and_packages() {
    check(
        "standard library package L { multiplicity m [0..*]; package P { filter @M; } }",
        expect![[r#"
            OwningMembership
              · LibraryPackage declaredName="L" isStandard=true
                OwningMembership
                  · MultiplicityRange declaredName="m"
                    OwningMembership
                      · LiteralInteger value=0
                    OwningMembership
                      · LiteralInfinity
                OwningMembership
                  · Package declaredName="P"
                    ElementFilterMembership
                      · OperatorExpression operator="@"
                        ParameterMembership
                          · Feature direction=in
                            FeatureTyping typedFeature->owner type->M
                        ReturnParameterMembership
                          · Feature direction=out
        "#]],
    );
}

// The graph (step 1a.6, done when): walkable by ownership and by relationship, with spans.

#[test]
fn the_kerml_library_is_walkable_by_ownership_and_relationship() {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    for s in corpus.sources() {
        if s.language != sysml_corpus::Language::KerML {
            continue;
        }
        let p = parse(Language::KerML, &s.text);
        let mut n = 0u128;
        let l = lower(FileId(0), &p.syntax(), &mut || {
            n += 1;
            Uuid::from_u128(n)
        });
        let g = &l.graph;
        // By ownership: every element is reached from the root, once, and its owners agree.
        let mut seen = vec![false; g.len()];
        let mut stack = vec![l.root];
        while let Some(x) = stack.pop() {
            assert!(!seen[x.index()], "{}: reached twice", s.id);
            seen[x.index()] = true;
            for r in g.owned_relationships(x) {
                assert_eq!(g.owning_related_element(*r), Some(x));
                stack.push(*r);
            }
            for e in g.owned_related_elements(x) {
                assert_eq!(g.owning_relationship(*e), Some(x));
                stack.push(*e);
            }
        }
        assert!(seen.iter().all(|s| *s), "{}: unreachable elements", s.id);
        // By relationship: every end is an element of the graph or a name to resolve, and every
        // element has a span inside the file.
        for x in g.elements() {
            for e in g.source(x).iter().chain(g.target(x)) {
                if let Endpoint::Element(y) = e {
                    assert!(y.index() < g.len());
                }
            }
            let span = g.span(x).expect("a span");
            assert!(span.start <= span.end && span.end as usize <= s.text.len());
        }
    }
}

#[test]
fn every_element_has_the_span_of_its_text() {
    let text = "package P { classifier C; feature f : C; }";
    let l = lowered(text);
    let g = &l.graph;
    let named = |name: &str| {
        g.elements()
            .find(|x| g.string(*x, Attr::DeclaredName) == Some(name))
            .unwrap()
    };
    let span = |x| {
        let s = g.span(x).unwrap();
        &text[s.start as usize..s.end as usize]
    };
    assert_eq!(span(named("C")), "classifier C;");
    assert_eq!(span(named("f")), "feature f : C;");
    let typing = g.owned_relationships(named("f"))[0];
    assert_eq!(g.metaclass(typing), Metaclass::FeatureTyping);
    assert_eq!(span(typing), "C");
    assert_eq!(span(l.root), text);
}

#[test]
fn the_kerml_library_element_counts() {
    // The elements of the 36 KerML library files by metaclass, against the snapshot. The oracle
    // test checks the same files element by element against the pilot implementation.
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for s in corpus.sources() {
        if s.language != sysml_corpus::Language::KerML {
            continue;
        }
        let p = parse(Language::KerML, &s.text);
        let mut n = 0u128;
        let l = lower(FileId(0), &p.syntax(), &mut || {
            n += 1;
            Uuid::from_u128(n)
        });
        for x in l.graph.elements() {
            *counts.entry(l.graph.metaclass(x).name()).or_default() += 1;
        }
    }
    let total: usize = counts.values().sum();
    let mut out = format!("total {total}\n");
    for (m, n) in counts {
        out.push_str(&format!("{m} {n}\n"));
    }
    expect![[r#"
        total 24931
        Association 26
        AssociationStructure 3
        Behavior 29
        BindingConnector 24
        BooleanExpression 11
        Class 2
        Classifier 1
        Comment 27
        Connector 41
        ConstructorExpression 2
        CrossSubsetting 29
        DataType 31
        Disjoining 6
        Documentation 334
        EndFeatureMembership 180
        Expression 65
        Feature 4323
        FeatureChainExpression 104
        FeatureChaining 258
        FeatureInverting 16
        FeatureMembership 1595
        FeatureReferenceExpression 657
        FeatureTyping 1502
        FeatureValue 1185
        Flow 5
        Function 305
        IndexExpression 23
        Interaction 5
        Intersecting 43
        Invariant 68
        InvocationExpression 243
        LibraryPackage 36
        LiteralBoolean 13
        LiteralInfinity 365
        LiteralInteger 1770
        LiteralRational 26
        Membership 902
        MembershipImport 167
        Metaclass 84
        MultiplicityRange 1498
        Namespace 36
        NamespaceImport 37
        NullExpression 11
        OperatorExpression 233
        OwningMembership 4717
        Package 3
        ParameterMembership 980
        Predicate 3
        Redefinition 293
        ReferenceSubsetting 184
        ResultExpressionMembership 103
        ReturnParameterMembership 1585
        Step 32
        Structure 16
        Subclassification 370
        Subsetting 292
        Succession 27
        Unioning 5
    "#]]
    .assert_eq(&out);
}

#[test]
fn a_literal_too_large_is_reported() {
    let l = lowered("feature f [99999999999999999999];");
    assert_eq!(l.diagnostics.len(), 1);
    assert_eq!(l.diagnostics[0].code, "lower::integer-too-large");
}

#[test]
fn the_valid_kerml_snippets_lower_without_diagnostics() {
    for s in sysml_corpus::snippets().unwrap() {
        if s.source.language != sysml_corpus::Language::KerML
            || s.source.expect != sysml_corpus::Expectation::Valid
        {
            continue;
        }
        let p = parse(Language::KerML, &s.source.text);
        let mut n = 0u128;
        let l = lower(FileId(0), &p.syntax(), &mut || {
            n += 1;
            Uuid::from_u128(n)
        });
        assert!(
            l.diagnostics.is_empty(),
            "{}: {:?}",
            s.source.id,
            l.diagnostics
        );
    }
}
