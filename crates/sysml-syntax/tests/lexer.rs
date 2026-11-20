// SPDX-License-Identifier: Apache-2.0
//! The lexer, tested from KerML 8.2.2 (and SysML 8.2.2.1.2) and on the whole corpus.

use sysml_syntax::{lex, Language, SyntaxKind as K};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The kinds and texts of the tokens of `text`.
fn tokens(lang: Language, text: &str) -> Vec<(K, &str)> {
    let lexed = lex(lang, text);
    let mut out = Vec::new();
    let mut offset = 0;
    for t in &lexed.tokens {
        let len = t.len as usize;
        out.push((t.kind, &text[offset..offset + len]));
        offset += len;
    }
    out
}

/// The tokens of `text` without trivia (white space and notes).
fn significant(lang: Language, text: &str) -> Vec<(K, &str)> {
    tokens(lang, text)
        .into_iter()
        .filter(|(k, _)| !k.is_trivia())
        .collect()
}

fn codes(lang: Language, text: &str) -> Vec<&'static str> {
    lex(lang, text).diagnostics.iter().map(|d| d.code).collect()
}

// ---- the corpus ----

#[test]
fn the_corpus_lexes_without_errors_and_losslessly() {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let mut sources = 0;
    for s in corpus.sources() {
        let lang = match s.language {
            sysml_corpus::Language::KerML => Language::KerML,
            sysml_corpus::Language::SysML => Language::SysML,
        };
        let lexed = lex(lang, &s.text);
        assert!(
            lexed.diagnostics.is_empty(),
            "{}: {:?}",
            s.id,
            lexed.diagnostics.first()
        );
        assert!(
            lexed.tokens.iter().all(|t| t.kind != K::ErrorToken),
            "{}",
            s.id
        );
        let rebuilt: String = tokens(lang, &s.text).iter().map(|(_, t)| *t).collect();
        assert_eq!(rebuilt, s.text, "{} is not reproduced byte for byte", s.id);
        sources += 1;
    }
    assert_eq!(sources, 95);
}

// ---- KerML 8.2.2.1 line terminators and white space ----

#[test]
fn white_space_is_one_trivia_token() {
    for ws in [" ", "\t", "\x0c", "\n", "\r", "\r\n", " \t\r\n\x0c "] {
        assert_eq!(
            tokens(Language::KerML, ws),
            vec![(K::Whitespace, ws)],
            "{ws:?}"
        );
    }
    assert!(K::Whitespace.is_trivia());
}

// ---- KerML 8.2.2.2 notes and comments ----

#[test]
fn single_line_notes_end_at_a_line_terminator() {
    for (text, note) in [
        ("// a note\nx", "// a note"),
        ("// cr\rx", "// cr"),
        ("//\r\nx", "//"),
        ("// at the end", "// at the end"),
    ] {
        assert_eq!(
            tokens(Language::KerML, text)[0],
            (K::SingleLineNote, note),
            "{text:?}"
        );
    }
    assert!(K::SingleLineNote.is_trivia());
}

#[test]
fn multiline_notes_and_regular_comments() {
    // `//*` starts a multiline note (trivia); `/*` starts a regular comment (a token).
    assert_eq!(
        tokens(Language::KerML, "//* a\n note */"),
        vec![(K::MultilineNote, "//* a\n note */")]
    );
    assert_eq!(
        tokens(Language::KerML, "/* a\n comment */"),
        vec![(K::RegularComment, "/* a\n comment */")]
    );
    assert_eq!(
        tokens(Language::KerML, "/**/"),
        vec![(K::RegularComment, "/**/")]
    );
    assert_eq!(
        tokens(Language::KerML, "/* a */ b */")[0],
        (K::RegularComment, "/* a */")
    );
    assert!(K::MultilineNote.is_trivia());
    assert!(
        !K::RegularComment.is_trivia(),
        "a regular comment becomes a Comment element"
    );
}

#[test]
fn unterminated_comments_are_errors() {
    assert_eq!(
        tokens(Language::KerML, "/* open"),
        vec![(K::ErrorToken, "/* open")]
    );
    assert_eq!(
        codes(Language::KerML, "/* open"),
        ["syntax::unterminated-comment"]
    );
}

#[test]
fn a_note_opener_without_a_terminator_is_a_single_line_note() {
    // KerML 8.2.2.2: with no `*/` ahead, MULTILINE_NOTE cannot match, and `//` LINE_TEXT does.
    let text = "//******** Section ********\npackage P;\n";
    assert_eq!(
        tokens(Language::KerML, text)[0],
        (K::SingleLineNote, "//******** Section ********")
    );
    assert!(codes(Language::KerML, text).is_empty());
    assert_eq!(
        significant(Language::KerML, text).first(),
        Some(&(K::PackageKw, "package"))
    );
    assert_eq!(
        tokens(Language::KerML, "//* open"),
        vec![(K::SingleLineNote, "//* open")]
    );
}

#[test]
fn a_note_opener_with_a_later_terminator_is_a_multiline_note() {
    // KerML 8.2.2.7: the longest token wins, so a later `*/` ends a multiline note even past code.
    let text = "//** banner\npackage P;\n/* doc */";
    assert_eq!(
        tokens(Language::KerML, text)[0],
        (K::MultilineNote, "//** banner\npackage P;\n/* doc */")
    );
}

// ---- KerML 8.2.2.3 names ----

#[test]
fn basic_names() {
    for name in ["a", "_", "A1", "_x_2", "partDef", "Package"] {
        assert_eq!(
            tokens(Language::KerML, name),
            vec![(K::BasicName, name)],
            "{name}"
        );
    }
    // Keywords are whole words and case-sensitive.
    assert_eq!(
        tokens(Language::KerML, "packages"),
        vec![(K::BasicName, "packages")]
    );
}

#[test]
fn unrestricted_names_with_every_escape() {
    // Table 4: \' \" \b \f \t \n \\
    let name = r#"'it\'s \"a\" \b\f\t\n\\ é €'"#;
    assert_eq!(
        tokens(Language::KerML, name),
        vec![(K::UnrestrictedName, name)]
    );
    assert!(codes(Language::KerML, name).is_empty());
}

#[test]
fn invalid_escapes_and_characters_in_names_are_diagnosed() {
    assert_eq!(
        tokens(Language::KerML, r"'a\x'"),
        vec![(K::UnrestrictedName, r"'a\x'")]
    );
    assert_eq!(codes(Language::KerML, r"'a\x'"), ["syntax::invalid-escape"]);
    assert_eq!(
        codes(Language::KerML, "'a\u{7}b'"),
        ["syntax::invalid-character"]
    );
}

#[test]
fn unterminated_names_end_at_the_line() {
    assert_eq!(
        tokens(Language::KerML, "'open\nx")[0],
        (K::ErrorToken, "'open")
    );
    assert_eq!(
        codes(Language::KerML, "'open\nx"),
        ["syntax::unterminated-name"]
    );
}

// ---- KerML 8.2.2.4 numeric values ----

#[test]
fn decimal_and_exponential_values() {
    assert_eq!(tokens(Language::KerML, "0"), vec![(K::DecimalValue, "0")]);
    assert_eq!(
        tokens(Language::KerML, "123"),
        vec![(K::DecimalValue, "123")]
    );
    for e in ["1e5", "1E5", "2e+10", "3E-2"] {
        assert_eq!(
            tokens(Language::KerML, e),
            vec![(K::ExponentialValue, e)],
            "{e}"
        );
    }
    // No sign (negation is an operator) and no decimal point (that is the real-literal syntax).
    assert_eq!(
        significant(Language::KerML, "-1"),
        vec![(K::Minus, "-"), (K::DecimalValue, "1")]
    );
    assert_eq!(
        significant(Language::KerML, "1.5"),
        vec![
            (K::DecimalValue, "1"),
            (K::Dot, "."),
            (K::DecimalValue, "5")
        ]
    );
    assert_eq!(
        significant(Language::KerML, "1..5"),
        vec![
            (K::DecimalValue, "1"),
            (K::DotDot, ".."),
            (K::DecimalValue, "5")
        ]
    );
    // An exponent needs digits; otherwise the letters are a name.
    assert_eq!(
        significant(Language::KerML, "1e"),
        vec![(K::DecimalValue, "1"), (K::BasicName, "e")]
    );
    assert_eq!(
        significant(Language::KerML, "1e+"),
        vec![(K::DecimalValue, "1"), (K::BasicName, "e"), (K::Plus, "+")]
    );
}

// ---- KerML 8.2.2.5 string values ----

#[test]
fn string_values() {
    let s = r#""say \"hi\" \n é""#;
    assert_eq!(tokens(Language::KerML, s), vec![(K::StringValue, s)]);
    assert!(codes(Language::KerML, s).is_empty());
    assert_eq!(
        tokens(Language::KerML, "\"open\nx")[0],
        (K::ErrorToken, "\"open")
    );
    assert_eq!(
        codes(Language::KerML, "\"open\nx"),
        ["syntax::unterminated-string"]
    );
    assert_eq!(
        codes(Language::KerML, r#""a\qb""#),
        ["syntax::invalid-escape"]
    );
}

// ---- KerML 8.2.2.6 reserved words; SysML 8.2.2.1.2 ----

/// KerML 8.2.2.6, as listed in the specification.
const KERML_RESERVED: &[&str] = &[
    "about",
    "abstract",
    "alias",
    "all",
    "and",
    "as",
    "assoc",
    "behavior",
    "binding",
    "bool",
    "by",
    "chains",
    "class",
    "classifier",
    "comment",
    "composite",
    "conjugate",
    "conjugates",
    "conjugation",
    "connector",
    "const",
    "crosses",
    "datatype",
    "default",
    "dependency",
    "derived",
    "differences",
    "disjoining",
    "disjoint",
    "doc",
    "else",
    "end",
    "expr",
    "false",
    "feature",
    "featured",
    "featuring",
    "filter",
    "first",
    "flow",
    "for",
    "from",
    "function",
    "hastype",
    "if",
    "implies",
    "import",
    "in",
    "inout",
    "interaction",
    "intersects",
    "inv",
    "inverse",
    "inverting",
    "istype",
    "language",
    "library",
    "locale",
    "member",
    "meta",
    "metaclass",
    "metadata",
    "multiplicity",
    "namespace",
    "nonunique",
    "not",
    "null",
    "of",
    "or",
    "ordered",
    "out",
    "package",
    "portion",
    "predicate",
    "private",
    "protected",
    "public",
    "redefines",
    "redefinition",
    "references",
    "rep",
    "return",
    "specialization",
    "specializes",
    "standard",
    "step",
    "struct",
    "subclassifier",
    "subset",
    "subsets",
    "subtype",
    "succession",
    "then",
    "to",
    "true",
    "type",
    "typed",
    "typing",
    "unions",
    "var",
    "xor",
];

/// SysML 8.2.2.1.2, as listed in the specification.
const SYSML_RESERVED: &[&str] = &[
    "about",
    "abstract",
    "accept",
    "action",
    "actor",
    "after",
    "alias",
    "all",
    "allocate",
    "allocation",
    "analysis",
    "and",
    "as",
    "assert",
    "assign",
    "assume",
    "at",
    "attribute",
    "bind",
    "binding",
    "by",
    "calc",
    "case",
    "comment",
    "concern",
    "connect",
    "connection",
    "constant",
    "constraint",
    "crosses",
    "decide",
    "def",
    "default",
    "defined",
    "dependency",
    "derived",
    "do",
    "doc",
    "else",
    "end",
    "entry",
    "enum",
    "event",
    "exhibit",
    "exit",
    "expose",
    "false",
    "filter",
    "first",
    "flow",
    "for",
    "fork",
    "frame",
    "from",
    "hastype",
    "if",
    "implies",
    "import",
    "in",
    "include",
    "individual",
    "inout",
    "interface",
    "istype",
    "item",
    "join",
    "language",
    "library",
    "locale",
    "loop",
    "merge",
    "message",
    "meta",
    "metadata",
    "nonunique",
    "not",
    "null",
    "objective",
    "occurrence",
    "of",
    "or",
    "ordered",
    "out",
    "package",
    "parallel",
    "part",
    "perform",
    "port",
    "private",
    "protected",
    "public",
    "redefines",
    "ref",
    "references",
    "render",
    "rendering",
    "rep",
    "require",
    "requirement",
    "return",
    "satisfy",
    "send",
    "snapshot",
    "specializes",
    "stakeholder",
    "standard",
    "state",
    "subject",
    "subsets",
    "succession",
    "terminate",
    "then",
    "timeslice",
    "to",
    "transition",
    "true",
    "until",
    "use",
    "variant",
    "variation",
    "verification",
    "verify",
    "via",
    "view",
    "viewpoint",
    "when",
    "while",
    "xor",
];

#[test]
fn every_kerml_reserved_word_is_a_keyword_in_kerml() {
    assert_eq!(KERML_RESERVED.len(), 101);
    for w in KERML_RESERVED {
        let t = tokens(Language::KerML, w);
        assert_eq!(t.len(), 1, "{w}");
        assert!(
            t[0].0.is_keyword(),
            "{w} is not a keyword in KerML: {:?}",
            t[0].0
        );
        assert_eq!(t[0].0.text(), Some(*w));
    }
}

#[test]
fn every_sysml_reserved_word_is_a_keyword_in_sysml() {
    assert_eq!(SYSML_RESERVED.len(), 128);
    for w in SYSML_RESERVED {
        let t = tokens(Language::SysML, w);
        assert_eq!(t.len(), 1, "{w}");
        assert!(
            t[0].0.is_keyword(),
            "{w} is not a keyword in SysML: {:?}",
            t[0].0
        );
        assert_eq!(t[0].0.text(), Some(*w));
    }
}

#[test]
fn reserved_words_of_the_other_language_are_names() {
    let mut checked = 0;
    for w in KERML_RESERVED
        .iter()
        .filter(|w| !SYSML_RESERVED.contains(w))
    {
        assert_eq!(
            tokens(Language::SysML, w),
            vec![(K::BasicName, *w)],
            "{w} in SysML"
        );
        checked += 1;
    }
    for w in SYSML_RESERVED
        .iter()
        .filter(|w| !KERML_RESERVED.contains(w))
    {
        assert_eq!(
            tokens(Language::KerML, w),
            vec![(K::BasicName, *w)],
            "{w} in KerML"
        );
        checked += 1;
    }
    assert_eq!(checked, (101 - 57) + (128 - 57));
}

#[test]
fn no_other_word_is_a_keyword() {
    for kind in K::ALL.iter().filter(|k| k.is_keyword()) {
        let text = kind.text().unwrap();
        assert!(
            KERML_RESERVED.contains(&text) || SYSML_RESERVED.contains(&text),
            "{text} is not reserved in either language"
        );
    }
}

// ---- KerML 8.2.2.7 symbols ----

/// KerML 8.2.2.7, as listed in the specification.
const SYMBOLS: &[&str] = &[
    "(", ")", "{", "}", "[", "]", ";", ",", "~", "@", "#", "%", "&", "^", "|", "*", "**", "+", "-",
    "/", "->", "$", ".", "..", ":", "::", ":>", ":>>", "::>", "=>", "<", "<=", "=", ":=", "==",
    "===", "!=", "!==", ">", ">=", "?", "??", ".?",
];

#[test]
fn every_symbol_is_one_token() {
    assert_eq!(SYMBOLS.len(), 43);
    for s in SYMBOLS {
        let t = tokens(Language::KerML, s);
        assert_eq!(t.len(), 1, "{s}: {t:?}");
        assert!(t[0].0.is_symbol(), "{s}");
        assert_eq!(t[0].0.text(), Some(*s));
    }
    assert_eq!(K::ALL.iter().filter(|k| k.is_symbol()).count(), 43);
}

#[test]
fn symbols_are_matched_longest_first() {
    // The specification's own example: "a:::b" is "a", "::", ":" and "b".
    assert_eq!(
        significant(Language::KerML, "a:::b"),
        vec![
            (K::BasicName, "a"),
            (K::ColonColon, "::"),
            (K::Colon, ":"),
            (K::BasicName, "b")
        ]
    );
    assert_eq!(
        significant(Language::KerML, "::>"),
        vec![(K::ColonColonGt, "::>")]
    );
    assert_eq!(
        significant(Language::KerML, ":>>="),
        vec![(K::ColonGtGt, ":>>"), (K::Eq, "=")]
    );
    assert_eq!(
        significant(Language::KerML, "!==="),
        vec![(K::NotEqEq, "!=="), (K::Eq, "=")]
    );
}

#[test]
fn characters_outside_the_lexical_structure_are_errors() {
    for (text, bad) in [
        ("a é b", "é"),
        ("x ! y", "!"),
        ("`", "`"),
        ("a\u{feff}b", "\u{feff}"),
    ] {
        let t = tokens(Language::KerML, text);
        assert!(t.contains(&(K::ErrorToken, bad)), "{text:?}: {t:?}");
        assert_eq!(
            codes(Language::KerML, text),
            ["syntax::unexpected-character"],
            "{text:?}"
        );
    }
}

// ---- a leading byte-order mark (ADR 0013) ----

#[test]
fn a_leading_byte_order_mark_is_trivia() {
    let text = "\u{feff}package P;";
    assert_eq!(
        tokens(Language::KerML, text)[0],
        (K::ByteOrderMark, "\u{feff}")
    );
    assert!(K::ByteOrderMark.is_trivia());
    assert!(codes(Language::KerML, text).is_empty());
    // Anywhere else it is an unexpected character (characters_outside_the_lexical_structure_are_errors).
}

// ---- robustness ----

/// SplitMix64, so that the property tests run the same everywhere, wasm32 included.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[test]
fn any_input_lexes_losslessly_without_panicking() {
    // Pieces chosen to hit every rule and its edges: comment and quote openers, escapes, line
    // terminators, digits and exponents, multi-character symbols, non-ASCII and control characters.
    const PIECES: &[&str] = &[
        "/", "*", "/*", "*/", "//", "//*", "'", "\"", "\\", "\n", "\r", " ", "\t", "a", "Z", "_",
        "0", "9", "e", "E", "+", "-", ".", ":", ">", "=", "!", "?", "é", "€", "\u{0}", "\u{7}",
        "package", "part", "feature",
    ];
    let mut rng = Rng(42);
    for case in 0..20_000 {
        let len = (rng.next() % 40) as usize;
        let text: String = (0..len)
            .map(|_| PIECES[(rng.next() % PIECES.len() as u64) as usize])
            .collect();
        let lang = if case % 2 == 0 {
            Language::KerML
        } else {
            Language::SysML
        };
        let lexed = lex(lang, &text);
        let total: usize = lexed.tokens.iter().map(|t| t.len as usize).sum();
        assert_eq!(total, text.len(), "{text:?}");
        assert!(lexed.tokens.iter().all(|t| t.len > 0), "{text:?}");
        // Every error token is explained by a diagnostic at its position.
        let mut offset = 0u32;
        for t in &lexed.tokens {
            if t.kind == K::ErrorToken {
                assert!(
                    lexed
                        .diagnostics
                        .iter()
                        .any(|d| u32::from(d.range.start()) == offset),
                    "{text:?}"
                );
            }
            offset += t.len;
        }
    }
}
