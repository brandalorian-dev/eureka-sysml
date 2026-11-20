// SPDX-License-Identifier: Apache-2.0
//! The generator is deterministic, hits its size targets, and produces models whose every
//! reference resolves (checked textually here; by the parser from Phase 1b).

use std::collections::{BTreeMap, BTreeSet};
use sysml_synth::{generate, Config, Stats};

fn tiny() -> Config {
    Config {
        seed: 7,
        packages: 2,
        defs_per_package: 3,
        attributes_per_def: 1,
        ports_per_def: 2,
        parts_per_def: 2,
        connections_per_def: 1,
        imports_per_package: 1,
        docs: true,
    }
}

#[test]
fn output_is_deterministic() {
    let a = generate(&tiny());
    let b = generate(&tiny());
    assert_eq!(a.files, b.files);
    let other = generate(&Config { seed: 8, ..tiny() });
    assert_ne!(
        a.files, other.files,
        "the seed changes the references chosen"
    );
    assert_eq!(a.stats, other.stats, "but not the shape");
}

#[test]
fn one_file_per_package() {
    let m = generate(&tiny());
    let names: Vec<&str> = m.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["P0.sysml", "P1.sysml"]);
    assert!(m.files[0].text.starts_with("package P0 {\n"));
    assert!(m.files.iter().all(|f| f.text.ends_with("}\n")));
}

#[test]
fn tiny_model_counts() {
    let s = generate(&tiny()).stats;
    let expected = Stats {
        files: 2,
        packages: 2,
        imports: 2 + 1, // ScalarValues in both, P0 in P1
        docs: 2 + 6,    // each package and each definition
        part_defs: 6,
        port_defs: 2,
        specializations: 4, // every definition but each package's root
        attributes: 6,
        ports: 4, // on the two root definitions
        parts: 8,
        multiplicities: 4, // every second part
        connections: 4,
    };
    assert_eq!(s, expected);
}

#[test]
fn element_estimate_follows_the_counting_rules() {
    // Documented in Stats::elements; checked against the parser in Phase 1b.
    let s = Stats {
        files: 1,
        packages: 1,
        imports: 1,
        docs: 1,
        part_defs: 1,
        port_defs: 1,
        specializations: 1,
        attributes: 1,
        ports: 1,
        parts: 1,
        multiplicities: 1,
        connections: 1,
    };
    assert_eq!(s.elements(), 1 + 2 + 1 + 2 + 2 + 2 + 1 + 3 + 3 + 3 + 4 + 14);
}

#[test]
fn size_targets() {
    for (target, tolerance) in [
        (Config::SMALL, 0.05),
        (Config::WORKING_SET, 0.02),
        (200_000, 0.02),
    ] {
        let s = generate(&Config::with_target_elements(target)).stats;
        let ratio = s.elements() as f64 / target as f64;
        assert!(
            (ratio - 1.0).abs() <= tolerance,
            "target {target}: {} elements",
            s.elements()
        );
    }
}

#[test]
fn every_reference_resolves() {
    check_references(&generate(&tiny()), 2);
    check_references(
        &generate(&Config::with_target_elements(20_000)),
        Config::default().ports_per_def,
    );
}

/// Check, from the text alone, that each name used is declared where it can be seen: in the same
/// package, in an imported package, or in ScalarValues. Relies on the generator writing one
/// declaration per line.
fn check_references(m: &sysml_synth::Model, ports: usize) {
    const SCALARS: [&str; 4] = ["Boolean", "Integer", "Real", "String"];
    // package -> declared definitions
    let mut defs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in &m.files {
        let pkg = f
            .text
            .lines()
            .next()
            .unwrap()
            .trim_start_matches("package ")
            .trim_end_matches(" {")
            .to_owned();
        let declared = f
            .text
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                l.strip_prefix("part def ")
                    .or_else(|| l.strip_prefix("port def "))
            })
            .map(|rest| rest.split([' ', ';', '{']).next().unwrap().to_owned())
            .collect();
        defs.insert(pkg, declared);
    }
    for f in &m.files {
        let pkg = f
            .text
            .lines()
            .next()
            .unwrap()
            .trim_start_matches("package ")
            .trim_end_matches(" {");
        let mut visible: BTreeSet<&str> = defs[pkg].iter().map(String::as_str).collect();
        let mut parts_in_def: BTreeSet<String> = BTreeSet::new();
        let mut depth = 0i32;
        for line in f.text.lines() {
            let l = line.trim();
            depth += l.matches('{').count() as i32 - l.matches('}').count() as i32;
            assert!(depth >= 0, "{}: unbalanced braces", f.name);
            if let Some(imported) = l
                .strip_prefix("private import ")
                .and_then(|r| r.strip_suffix("::*;"))
            {
                if imported != "ScalarValues" {
                    let d = defs
                        .get(imported)
                        .unwrap_or_else(|| panic!("{}: imports unknown {imported}", f.name));
                    visible.extend(d.iter().map(String::as_str));
                }
            } else if l.starts_with("part def ") {
                parts_in_def.clear();
                if let Some((_, sup)) = l.split_once(" :> ") {
                    let sup = sup.trim_end_matches(" {");
                    assert!(
                        visible.contains(sup),
                        "{}: {l}: {sup} is not visible",
                        f.name
                    );
                }
            } else if l.starts_with("port def ") {
                // A declaration, collected above.
            } else if let Some(rest) = l.strip_prefix("attribute ") {
                let ty = rest.split(" : ").nth(1).unwrap().trim_end_matches(';');
                assert!(SCALARS.contains(&ty), "{}: {l}", f.name);
            } else if let Some(rest) = l.strip_prefix("port ") {
                let ty = rest.split(" : ").nth(1).unwrap().trim_end_matches(';');
                assert!(visible.contains(ty), "{}: {l}: {ty} is not visible", f.name);
            } else if let Some(rest) = l.strip_prefix("part ") {
                let (name, ty) = rest.split_once(" : ").unwrap();
                let ty = ty.split(['[', ';']).next().unwrap();
                assert!(visible.contains(ty), "{}: {l}: {ty} is not visible", f.name);
                parts_in_def.insert(name.to_owned());
            } else if let Some(rest) = l.strip_prefix("connect ") {
                let (a, b) = rest.trim_end_matches(';').split_once(" to ").unwrap();
                for end in [a, b] {
                    let (part, port) = end.split_once('.').unwrap();
                    assert!(
                        parts_in_def.contains(part),
                        "{}: {l}: no part {part} in this definition",
                        f.name
                    );
                    let n: usize = port.strip_prefix('p').unwrap().parse().unwrap();
                    assert!(n < ports, "{}: {l}: no inherited port {port}", f.name);
                }
            }
        }
        assert_eq!(depth, 0, "{}: unbalanced braces", f.name);
    }
}
