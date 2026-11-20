// SPDX-License-Identifier: Apache-2.0
//! The lexer (KerML 8.2.2; SysML 8.2.2.1.2), hand-written per ADR 0011.
//!
//! Every byte of the input belongs to exactly one token, so the token lengths add up to the text.
//! Malformed input never fails: it becomes [`SyntaxKind::ErrorToken`]s, or well-formed tokens with a
//! diagnostic, for example for an invalid escape sequence.

use crate::kinds::{Language, SyntaxKind, SYMBOLS};
use crate::Diagnostic;
use rowan::{TextRange, TextSize};

/// A token: its kind and its length in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Token {
    /// The kind.
    pub kind: SyntaxKind,
    /// The length in bytes, always at least 1.
    pub len: u32,
}

/// The tokens of a text, and the lexical diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lexed {
    /// The tokens, in order, covering the whole text.
    pub tokens: Vec<Token>,
    /// Diagnostics for malformed tokens.
    pub diagnostics: Vec<Diagnostic>,
}

/// Split `text` into tokens, with the reserved words of `language`.
pub fn lex(language: Language, text: &str) -> Lexed {
    let mut l = Lexer {
        text,
        bytes: text.as_bytes(),
        pos: 0,
        language,
        diagnostics: Vec::new(),
    };
    let mut tokens = Vec::with_capacity(text.len() / 4);
    while l.pos < text.len() {
        let start = l.pos;
        let kind = l.token();
        debug_assert!(l.pos > start, "every token consumes input");
        tokens.push(Token {
            kind,
            len: (l.pos - start) as u32,
        });
    }
    Lexed {
        tokens,
        diagnostics: l.diagnostics,
    }
}

struct Lexer<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    language: Language,
    diagnostics: Vec<Diagnostic>,
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
}

impl Lexer<'_> {
    fn at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn error(
        &mut self,
        code: &'static str,
        start: usize,
        end: usize,
        message: String,
        clause: &'static str,
    ) {
        self.diagnostics
            .push(Diagnostic::error(code, range(start, end), message).with_clause(clause));
    }

    fn token(&mut self) -> SyntaxKind {
        let start = self.pos;
        // ADR 0013: a byte-order mark at the very start is trivia, kept so that the tree is lossless.
        if start == 0 && self.text.starts_with('\u{feff}') {
            self.pos = '\u{feff}'.len_utf8();
            return SyntaxKind::ByteOrderMark;
        }
        match self.bytes[self.pos] {
            // KerML 8.2.2.1: space, tab, form feed and line terminators (CR, LF, CRLF).
            b' ' | b'\t' | 0x0c | b'\n' | b'\r' => {
                while matches!(self.at(0), Some(b' ' | b'\t' | 0x0c | b'\n' | b'\r')) {
                    self.pos += 1;
                }
                SyntaxKind::Whitespace
            }
            // KerML 8.2.2.2: `//*` starts a multiline note, `//` a single-line note, `/*` a comment.
            b'/' if self.at(1) == Some(b'/') && self.at(2) == Some(b'*') => self.multiline_note(),
            b'/' if self.at(1) == Some(b'/') => {
                while !matches!(self.at(0), None | Some(b'\n' | b'\r')) {
                    self.pos += 1;
                }
                SyntaxKind::SingleLineNote
            }
            b'/' if self.at(1) == Some(b'*') => self.block(
                2,
                SyntaxKind::RegularComment,
                "syntax::unterminated-comment",
                "comment",
            ),
            // KerML 8.2.2.3 BASIC_NAME, or a reserved word (KerML 8.2.2.6, SysML 8.2.2.1.2).
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while matches!(
                    self.at(0),
                    Some(b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9')
                ) {
                    self.pos += 1;
                }
                self.language
                    .keyword(&self.text[start..self.pos])
                    .unwrap_or(SyntaxKind::BasicName)
            }
            // KerML 8.2.2.4: DECIMAL_VALUE, or EXPONENTIAL_VALUE = DECIMAL_VALUE ('e'|'E') ('+'|'-')? DECIMAL_VALUE.
            b'0'..=b'9' => {
                while self.at(0).is_some_and(|b| b.is_ascii_digit()) {
                    self.pos += 1;
                }
                if matches!(self.at(0), Some(b'e' | b'E')) {
                    let sign = usize::from(matches!(self.at(1), Some(b'+' | b'-')));
                    if self.at(1 + sign).is_some_and(|b| b.is_ascii_digit()) {
                        self.pos += 1 + sign;
                        while self.at(0).is_some_and(|b| b.is_ascii_digit()) {
                            self.pos += 1;
                        }
                        return SyntaxKind::ExponentialValue;
                    }
                }
                SyntaxKind::DecimalValue
            }
            // KerML 8.2.2.3 UNRESTRICTED_NAME and 8.2.2.5 STRING_VALUE.
            b'\'' => self.quoted(
                '\'',
                SyntaxKind::UnrestrictedName,
                "syntax::unterminated-name",
                "name",
                "KerML 8.2.2.3",
            ),
            b'"' => self.quoted(
                '"',
                SyntaxKind::StringValue,
                "syntax::unterminated-string",
                "string",
                "KerML 8.2.2.5",
            ),
            _ => {
                // KerML 8.2.2.7: the longest symbol that matches.
                let rest = &self.bytes[self.pos..];
                for len in (1..=3).rev() {
                    if let Some(&(_, kind)) = rest
                        .get(..len)
                        .and_then(|s| SYMBOLS.iter().find(|(text, _)| text.as_bytes() == s))
                    {
                        self.pos += len;
                        return kind;
                    }
                }
                let c = self.text[self.pos..].chars().next().unwrap_or('\u{fffd}');
                self.pos += c.len_utf8();
                self.error(
                    "syntax::unexpected-character",
                    start,
                    self.pos,
                    format!("unexpected character {c:?}"),
                    "KerML 8.2.2",
                );
                SyntaxKind::ErrorToken
            }
        }
    }

    /// `//* … */` (KerML 8.2.2.2). With no `*/` ahead, MULTILINE_NOTE cannot match, so the text is
    /// a SINGLE_LINE_NOTE (`//` LINE_TEXT) instead, up to the line terminator.
    fn multiline_note(&mut self) -> SyntaxKind {
        match self.text[self.pos + 3..].find("*/") {
            Some(i) => {
                self.pos += 3 + i + 2;
                SyntaxKind::MultilineNote
            }
            None => {
                while !matches!(self.at(0), None | Some(b'\n' | b'\r')) {
                    self.pos += 1;
                }
                SyntaxKind::SingleLineNote
            }
        }
    }

    /// `/* … */` (KerML 8.2.2.2). Unterminated, it runs to the end of the input.
    fn block(
        &mut self,
        open: usize,
        kind: SyntaxKind,
        code: &'static str,
        what: &str,
    ) -> SyntaxKind {
        let start = self.pos;
        match self.text[start + open..].find("*/") {
            Some(i) => {
                self.pos = start + open + i + 2;
                kind
            }
            None => {
                self.pos = self.text.len();
                self.error(
                    code,
                    start,
                    self.pos,
                    format!("unterminated {what}: expected `*/`"),
                    "KerML 8.2.2.2",
                );
                SyntaxKind::ErrorToken
            }
        }
    }

    /// `'…'` or `"…"`. Escape sequences are KerML 8.2.2.3 Table 4; other characters must be
    /// printable. A line terminator ends an unterminated name or string.
    fn quoted(
        &mut self,
        quote: char,
        kind: SyntaxKind,
        code: &'static str,
        what: &str,
        clause: &'static str,
    ) -> SyntaxKind {
        let start = self.pos;
        self.pos += 1;
        while let Some(c) = self.text[self.pos..].chars().next() {
            match c {
                c if c == quote => {
                    self.pos += 1;
                    return kind;
                }
                '\n' | '\r' => break,
                '\\' => {
                    let escape_start = self.pos;
                    self.pos += 1;
                    match self.text[self.pos..].chars().next() {
                        None | Some('\n' | '\r') => break,
                        Some(e) => {
                            self.pos += e.len_utf8();
                            if !matches!(e, '\'' | '"' | 'b' | 'f' | 't' | 'n' | '\\') {
                                self.error(
                                    "syntax::invalid-escape",
                                    escape_start,
                                    self.pos,
                                    format!("invalid escape sequence `\\{e}`"),
                                    "KerML 8.2.2.3",
                                );
                            }
                        }
                    }
                }
                c => {
                    if c.is_control() {
                        self.error(
                            "syntax::invalid-character",
                            self.pos,
                            self.pos + c.len_utf8(),
                            format!("{c:?} is not a printable character; write it as an escape sequence"),
                            clause,
                        );
                    }
                    self.pos += c.len_utf8();
                }
            }
        }
        self.error(
            code,
            start,
            self.pos,
            format!("unterminated {what}: expected `{quote}`"),
            clause,
        );
        SyntaxKind::ErrorToken
    }
}
