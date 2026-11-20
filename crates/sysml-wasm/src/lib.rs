// SPDX-License-Identifier: Apache-2.0
//! The `wasm-bindgen` interface that the browser UI calls (PROJECT_PLAN §4.2, §4.7).
//!
//! The UI talks to the Rust core in a web worker through a few coarse calls ("tree page",
//! "diagram view model", "validate scope") rather than element by element. This crate is the
//! skeleton of that interface (Phase 1a step 1a.1): it proves the build, the exported bindings and
//! the WebAssembly test run. The real calls arrive with the engine.

use wasm_bindgen::prelude::*;

/// The version of the Rust core.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// The metamodel IRI the core implements, for example `https://www.omg.org/spec/SysML/20250201`.
#[wasm_bindgen(js_name = metamodelIri)]
pub fn metamodel_iri() -> String {
    sysml_model::METAMODEL_IRI.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn reports_the_core_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn reports_the_metamodel() {
        assert_eq!(metamodel_iri(), "https://www.omg.org/spec/SysML/20250201");
    }
}
