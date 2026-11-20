// SPDX-License-Identifier: Apache-2.0
//! The grammar. Step 1a.2 has only the root namespace; the KerML productions (KerML 8.2.3–8.2.5)
//! are added in steps 1a.3–1a.4.

use crate::kinds::SyntaxKind;
use crate::parser::Parser;

/// A root namespace: a file (KerML 8.2.3.4.1). Its members are not parsed yet, so any
/// significant token is reported once and kept in an `Error` node.
pub(crate) fn root_namespace(p: &mut Parser) {
    let root = p.start();
    while !p.at_eof() {
        p.err_recover(
            "syntax::expected-member",
            "expected a namespace member",
            Some("KerML 8.2.3.4.1"),
            &[],
        );
    }
    root.complete(p, SyntaxKind::RootNamespace);
}
