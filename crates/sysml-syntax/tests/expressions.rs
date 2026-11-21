// SPDX-License-Identifier: Apache-2.0
//! The KerML expression grammar (KerML 8.2.5.8): operator precedence and associativity from
//! Table 6, checked as s-expressions, so that `a + b * c` is `(+ a (* b c))`.

use sysml_syntax::{parse, Language, SyntaxKind as K, SyntaxNode};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The significant tokens of `node` that are its own (not its children's), joined.
fn own_tokens(node: &SyntaxNode) -> Vec<String> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .map(|t| t.text().to_owned())
        .collect()
}

/// The expression tree as an s-expression: operators first, names and literals as their text.
fn sexp(node: &SyntaxNode) -> String {
    let kids: Vec<SyntaxNode> = node.children().collect();
    let text = || {
        node.text()
            .to_string()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    match node.kind() {
        K::QualifiedName | K::FeatureChain | K::FeatureReferenceExpression => text(),
        K::LiteralBoolean
        | K::LiteralString
        | K::LiteralInteger
        | K::LiteralReal
        | K::LiteralInfinity
        | K::NullExpression => text(),
        K::SequenceExpression if kids.len() == 1 => sexp(&kids[0]),
        _ => {
            let ops: Vec<String> = own_tokens(node)
                .into_iter()
                .filter(|t| !matches!(t.as_str(), "(" | ")" | "," | "?" | "else"))
                .collect();
            let args: Vec<String> = kids.iter().map(sexp).collect();
            if ops.is_empty() {
                format!("({})", args.join(" "))
            } else {
                format!("({} {})", ops.join(""), args.join(" "))
            }
        }
    }
}

/// The s-expression of the value of `feature f = <expr>;`, with no diagnostics.
fn tree(expr: &str) -> String {
    let text = format!("feature f = {expr};");
    let p = parse(Language::KerML, &text);
    assert!(p.diagnostics().is_empty(), "{text}: {:?}", p.diagnostics());
    let value = p
        .syntax()
        .descendants()
        .find(|n| n.kind() == K::FeatureValue)
        .expect("a feature value");
    sexp(&value.children().next().expect("an expression"))
}

#[test]
fn table_6_precedence_and_associativity() {
    // KerML 8.2.5.8.1, Table 6 (highest first): unary `all`, `+ - ~ not`; binary `^ **`,
    // `* / %`, `+ -`, `..`, `< > <= >=`, the classification operators, `== != === !==`,
    // `& and`, `xor`, `| or`, `implies`, `??`; the ternary `if`. Binary operators are left-
    // associative except the exponentiation operators, which are right-associative (Note 2).
    let cases = [
        ("a ^ b ^ c", "(^ a (^ b c))"),
        ("a ** b ^ c", "(** a (^ b c))"),
        ("a * b / c % d", "(% (/ (* a b) c) d)"),
        ("a + b * c", "(+ a (* b c))"),
        ("a * b + c", "(+ (* a b) c)"),
        ("a - b - c", "(- (- a b) c)"),
        ("a ^ b * c", "(* (^ a b) c)"),
        ("a .. b + c", "(.. a (+ b c))"),
        ("a < b .. c", "(< a (.. b c))"),
        ("a <= b >= c", "(>= (<= a b) c)"),
        ("a < b istype T", "(istype (< a b) T)"),
        ("a istype T == b", "(== (istype a T) b)"),
        ("a hastype T != b", "(!= (hastype a T) b)"),
        ("a == b === c !== d", "(!== (=== (== a b) c) d)"),
        ("a == b & c", "(& (== a b) c)"),
        ("a & b and c", "(and (& a b) c)"),
        ("a and b xor c", "(xor (and a b) c)"),
        ("a xor b | c", "(| (xor a b) c)"),
        ("a | b or c", "(or (| a b) c)"),
        ("a or b implies c", "(implies (or a b) c)"),
        ("a implies b ?? c", "(?? (implies a b) c)"),
        ("a ?? b ?? c", "(?? (?? a b) c)"),
        ("-a ^ b", "(^ (- a) b)"),
        ("- - a", "(- (- a))"),
        ("not a and b", "(and (not a) b)"),
        ("~a + +b", "(+ (~ a) (+ b))"),
        ("all T == x", "(== (all T) x)"),
        ("if a ? b else c", "(if a b c)"),
        ("if a ? b else c ?? d", "(if a b (?? c d))"),
        ("a ?? if b ? c else d", "(?? a (if b c d))"),
        ("if a ? if b ? c else d else e", "(if a (if b c d) e)"),
        ("a as T", "(as a T)"),
        ("a @ T", "(@ a T)"),
        ("a @@ M", "(@@ a M)"),
        ("a meta M", "(meta a M)"),
        ("istype T", "(istype T)"),
        ("(a + b) * c", "(* (+ a b) c)"),
        ("a.b.c + d", "(+ (. a b.c) d)"),
        ("x.y->sum() * 2", "(* (-> (. x y) sum ()) 2)"),
    ];
    for (expr, expected) in cases {
        assert_eq!(tree(expr), expected, "{expr}");
    }
}
