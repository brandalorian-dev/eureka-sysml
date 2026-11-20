// SPDX-License-Identifier: Apache-2.0
//! Deterministic generator of synthetic SysML models, for the PROJECT_PLAN §7 benchmarks and
//! scale tests (from 1k elements up to the 1M-element branch).
//!
//! A model has one file per package. Package `P{k}` imports `ScalarValues` and up to
//! [`Config::imports_per_package`] earlier packages, and declares a port definition `Port{k}` and
//! part definitions `D{k}_{i}`:
//!
//! ```text
//! package P1 {
//!     private import ScalarValues::*;
//!     private import P0::*;
//!     doc /* Synthetic package P1. */
//!     port def Port1;
//!     part def D1_0 {                        // the package's root definition
//!         doc /* Synthetic part definition D1_0. */
//!         attribute x1_0_0 : Real;
//!         port p0 : Port1;
//!     }
//!     part def D1_1 :> D0_2 {                // specializes an earlier definition
//!         doc /* Synthetic part definition D1_1. */
//!         attribute x1_1_0 : Integer;
//!         part q1_1_0 : D1_0;
//!         part q1_1_1 : D0_1[2];
//!         connect q1_1_0.p0 to q1_1_1.p0;
//!     }
//! }
//! ```
//!
//! The constructs and their syntax follow SysML Part 1 and the SimpleVehicleModel example: 7.4.2
//! (`doc` comments), 7.5.3 (imports), 7.6.2 and 7.6.3 (definitions, usages and `:>`
//! specialization), 7.6.6 (the feature chains at connection ends), 7.7.2 (attributes), 7.11.2
//! (parts), 7.12.2 (ports) and 7.13.2 (connections). The model is valid by construction:
//! - Every name is declared before use, in the same package or in an imported one.
//! - Feature names are unique to their definition (`x{k}_{i}_{n}`, `q{k}_{i}_{n}`), so no
//!   definition redeclares a name it inherits.
//! - Only root definitions declare ports `p{n}`. Every definition inherits them through its
//!   specialization chain, so each part has the ports that connections use.
//! - Specializations and part types point only at earlier definitions, so neither forms a cycle.
//!
//! The same [`Config`] always produces the same text. The seed chooses which earlier definitions
//! are referenced and which parts and ports each connection joins; it does not change the shape
//! or the [`Stats`].

use std::fmt::Write as _;

/// The shape and size of a synthetic model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Seed for the choice of referenced definitions.
    pub seed: u64,
    /// Number of packages, one file each.
    pub packages: usize,
    /// Part definitions per package, including the root definition `D{k}_0`.
    pub defs_per_package: usize,
    /// Attribute usages per part definition.
    pub attributes_per_def: usize,
    /// Port usages on each root definition, inherited by all other definitions.
    pub ports_per_def: usize,
    /// Part usages per non-root definition.
    pub parts_per_def: usize,
    /// Connections per non-root definition. Needs at least 2 parts and 1 port.
    pub connections_per_def: usize,
    /// Earlier packages each package imports (fewer for the first packages).
    pub imports_per_package: usize,
    /// Whether packages and definitions get a `doc` comment.
    pub docs: bool,
    /// Part definitions in the last package, if it differs from `defs_per_package`. Lets
    /// [`Config::with_target_elements`] size a model to within one definition.
    pub last_package_defs: Option<usize>,
}

impl Default for Config {
    /// One package of the default shape: 1,072 elements.
    fn default() -> Self {
        Config {
            seed: 1,
            packages: 1,
            defs_per_package: 20,
            attributes_per_def: 3,
            ports_per_def: 2,
            parts_per_def: 3,
            connections_per_def: 2,
            imports_per_package: 3,
            docs: true,
            last_package_defs: None,
        }
    }
}

impl Config {
    /// A small model, for quick runs.
    pub const SMALL: usize = 1_000;
    /// The browser working set of PROJECT_PLAN §7 ("working set ≤ 50k elements").
    pub const WORKING_SET: usize = 50_000;
    /// The large branch of PROJECT_PLAN §7 ("1M-element branch").
    pub const LARGE: usize = 1_000_000;

    /// The default shape, scaled to about `elements` elements (see [`Stats::elements`]): full
    /// packages, with a last package of 1 to `defs_per_package` definitions. A non-root
    /// definition is 55 elements, so the result is within 28 elements of the target: within 2%
    /// from 1,400 elements up, and within 3% at 1,000.
    pub fn with_target_elements(elements: usize) -> Config {
        let base = Config::default();
        let full_defs = base.defs_per_package;
        // Elements of a package without imports, by number of definitions. Package k also has
        // min(k, imports_per_package) imports of earlier packages, one element each.
        let package: Vec<usize> = (0..=full_defs)
            .map(|defs| match defs {
                0 => 0,
                _ => generate(&Config {
                    packages: 1,
                    defs_per_package: defs,
                    ..base
                })
                .stats
                .elements(),
            })
            .collect();
        let total = |packages: usize, last: usize| {
            let imports: usize = (0..packages).map(|k| k.min(base.imports_per_package)).sum();
            (packages - 1) * package[full_defs] + package[last] + imports
        };
        // Most packages import imports_per_package earlier ones, so count those in the estimate.
        let around = elements
            .div_ceil(package[full_defs] + base.imports_per_package)
            .max(1);
        let (_, packages, last) = (around.saturating_sub(1).max(1)..=around + 1)
            .flat_map(|packages| (1..=full_defs).map(move |last| (packages, last)))
            .map(|(packages, last)| (total(packages, last).abs_diff(elements), packages, last))
            .min()
            .unwrap();
        Config {
            packages,
            last_package_defs: (last != full_defs).then_some(last),
            ..base
        }
    }
}

/// One generated file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// The file name, `P{k}.sysml`.
    pub name: String,
    /// The SysML text.
    pub text: String,
}

/// A generated model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// One file per package, in package order.
    pub files: Vec<File>,
    /// Counts of what was generated.
    pub stats: Stats,
}

impl Model {
    /// The total size of the text, in bytes.
    pub fn bytes(&self) -> usize {
        self.files.iter().map(|f| f.text.len()).sum()
    }
}

/// Counts of the constructs in a generated model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Files (one per package).
    pub files: usize,
    /// Packages.
    pub packages: usize,
    /// `private import …::*;` declarations.
    pub imports: usize,
    /// `doc` comments.
    pub docs: usize,
    /// Part definitions.
    pub part_defs: usize,
    /// Port definitions.
    pub port_defs: usize,
    /// `:>` specializations.
    pub specializations: usize,
    /// Typed attribute usages.
    pub attributes: usize,
    /// Typed port usages.
    pub ports: usize,
    /// Typed part usages.
    pub parts: usize,
    /// Multiplicities (`[2]`) on part usages.
    pub multiplicities: usize,
    /// `connect a.p to b.q;` connections.
    pub connections: usize,
}

impl Stats {
    /// An estimate of the number of model elements, counting the explicit abstract syntax
    /// (relationships included, implied relationships excluded):
    ///
    /// | Construct | Elements |
    /// |---|---|
    /// | file | 1: its root namespace (KerML 10.2) |
    /// | package, definition, `doc` | 2: the element and its owning membership |
    /// | import | 1: the namespace import |
    /// | specialization | 1: the subclassification |
    /// | typed usage | 3: the usage, its feature membership and its feature typing |
    /// | multiplicity `[n]` | 4: the multiplicity range and the literal, each with an owning membership |
    /// | connection | 14: the connection usage and its membership, and per end: the end feature membership, the end feature, its reference subsetting, the chained feature and its two feature chainings |
    ///
    /// The rules are checked against the parser's element counts in Phase 1b.
    pub fn elements(&self) -> usize {
        self.files
            + 2 * (self.packages + self.part_defs + self.port_defs + self.docs)
            + self.imports
            + self.specializations
            + 3 * (self.attributes + self.ports + self.parts)
            + 4 * self.multiplicities
            + 14 * self.connections
    }
}

/// SplitMix64: a small, fast, deterministic generator (no dependency needed).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const SCALARS: [&str; 4] = ["Real", "Integer", "Boolean", "String"];

/// Generate a model.
pub fn generate(config: &Config) -> Model {
    let c = config;
    let mut rng = Rng(c.seed);
    let mut stats = Stats::default();
    let mut files = Vec::with_capacity(c.packages);
    let connect = c.connections_per_def > 0 && c.parts_per_def >= 2 && c.ports_per_def > 0;

    for k in 0..c.packages {
        let mut t = String::new();
        stats.files += 1;
        stats.packages += 1;
        writeln!(t, "package P{k} {{").unwrap();
        writeln!(t, "    private import ScalarValues::*;").unwrap();
        stats.imports += 1;
        let imported: Vec<usize> = (k.saturating_sub(c.imports_per_package)..k).rev().collect();
        for &j in &imported {
            writeln!(t, "    private import P{j}::*;").unwrap();
            stats.imports += 1;
        }
        if c.docs {
            writeln!(t, "    doc /* Synthetic package P{k}. */").unwrap();
            stats.docs += 1;
        }
        writeln!(t, "    port def Port{k};").unwrap();
        stats.port_defs += 1;

        // Imported packages come earlier, so they always have `defs_per_package` definitions.
        let defs_here = match c.last_package_defs {
            Some(last) if k + 1 == c.packages => last,
            _ => c.defs_per_package,
        };
        for i in 0..defs_here {
            // Definitions visible here and declared earlier: in this package, or in an import.
            let mut earlier = || {
                let choices = i + imported.len() * c.defs_per_package;
                let n = rng.below(choices);
                if n < i {
                    format!("D{k}_{n}")
                } else {
                    let n = n - i;
                    format!(
                        "D{}_{}",
                        imported[n / c.defs_per_package],
                        n % c.defs_per_package
                    )
                }
            };
            stats.part_defs += 1;
            if i == 0 {
                writeln!(t, "    part def D{k}_0 {{").unwrap();
            } else {
                writeln!(t, "    part def D{k}_{i} :> {} {{", earlier()).unwrap();
                stats.specializations += 1;
            }
            if c.docs {
                writeln!(t, "        doc /* Synthetic part definition D{k}_{i}. */").unwrap();
                stats.docs += 1;
            }
            for n in 0..c.attributes_per_def {
                let ty = SCALARS[(i + n) % SCALARS.len()];
                writeln!(t, "        attribute x{k}_{i}_{n} : {ty};").unwrap();
                stats.attributes += 1;
            }
            if i == 0 {
                for n in 0..c.ports_per_def {
                    writeln!(t, "        port p{n} : Port{k};").unwrap();
                    stats.ports += 1;
                }
            } else {
                for n in 0..c.parts_per_def {
                    let ty = earlier();
                    if n % 2 == 1 {
                        writeln!(t, "        part q{k}_{i}_{n} : {ty}[2];").unwrap();
                        stats.multiplicities += 1;
                    } else {
                        writeln!(t, "        part q{k}_{i}_{n} : {ty};").unwrap();
                    }
                    stats.parts += 1;
                }
                if connect {
                    for _ in 0..c.connections_per_def {
                        let a = rng.below(c.parts_per_def);
                        let b = (a + 1 + rng.below(c.parts_per_def - 1)) % c.parts_per_def;
                        let (pa, pb) = (rng.below(c.ports_per_def), rng.below(c.ports_per_def));
                        writeln!(
                            t,
                            "        connect q{k}_{i}_{a}.p{pa} to q{k}_{i}_{b}.p{pb};"
                        )
                        .unwrap();
                        stats.connections += 1;
                    }
                }
            }
            writeln!(t, "    }}").unwrap();
        }
        writeln!(t, "}}").unwrap();
        files.push(File {
            name: format!("P{k}.sysml"),
            text: t,
        });
    }
    Model { files, stats }
}
