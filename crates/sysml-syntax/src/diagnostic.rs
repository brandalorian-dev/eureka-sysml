// SPDX-License-Identifier: Apache-2.0
//! The diagnostic type of every phase (ADR 0012).

use rowan::TextRange;

/// How serious a diagnostic is: the four severities of the Language Server Protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    /// A violation of the specification.
    Error,
    /// A finding that is not a violation, for example a deprecated library element.
    Warning,
    /// Information.
    Information,
    /// A hint.
    Hint,
}

/// A further location that explains a diagnostic, for example "first declared here".
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Related {
    /// The byte range in the source text.
    pub range: TextRange,
    /// What this location shows.
    pub message: String,
}

/// A problem found in a model (ADR 0012).
///
/// The range is in bytes, relative to the text that was lexed or parsed. Line and column numbers
/// are computed at the boundary that needs them, for example UTF-16 columns in the language
/// server.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    /// A stable identifier: `syntax::…`, `resolve::…`, or a metamodel constraint name.
    pub code: &'static str,
    /// How serious it is.
    pub severity: Severity,
    /// The byte range in the source text.
    pub range: TextRange,
    /// A human-readable sentence.
    pub message: String,
    /// The specification clause, for example `KerML 8.2.2.2`.
    pub clause: Option<&'static str>,
    /// Further locations that explain the diagnostic.
    pub related: Vec<Related>,
}

impl Diagnostic {
    /// An error.
    pub fn error(code: &'static str, range: TextRange, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Error,
            range,
            message: message.into(),
            clause: None,
            related: Vec::new(),
        }
    }

    /// The same diagnostic, citing a specification clause.
    pub fn with_clause(mut self, clause: &'static str) -> Diagnostic {
        self.clause = Some(clause);
        self
    }
}
