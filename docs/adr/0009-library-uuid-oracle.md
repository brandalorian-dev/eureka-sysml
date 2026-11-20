# ADR 0009: Library UUID oracle (decision D7)

- Status: Accepted
- Date: 2026-11-20
- Decision: D7 of the Phase 1 execution plan, evaluated in step 1a.0 under its §2.5 methodology

## Context

KerML 9.1 and SysML 9.1 make the `elementId` of every standard library element a version 5 UUID:
- The top-level library package's UUID is `uuid5(NAMESPACE_URL, prefix + package name)`, with the prefix `https://www.omg.org/spec/KerML/` (KerML libraries) or `https://www.omg.org/spec/SysML/` (SysML libraries).
- Every element inside the package gets `uuid5(package UUID, path())`, with `path()` defined in KerML 8.3.2.1.2, 8.3.2.1.3 and 8.3.2.4.8.

Phase 1 is test-first (CLAUDE.md), so the UUIDs our engine computes need an oracle that is not our own code. The normative `.kpar` archives hold only the libraries' text, with no element ids.

## Criteria

- The oracle's library text must be identical to the normative `.kpar` files pinned in `specs.lock.json`. Otherwise the paths, and so the UUIDs, refer to different models.
- Its ids must be shown, independently, to follow KerML 9.1 wherever that can be checked without the engine we are about to build.
- It must be pinnable, with a fixed URL and checksum, like every other spec file (ADR 0004), and its license must allow local use.
- Using it must not reuse pilot code (CLAUDE.md). Using its output data as a behavioral reference is allowed.

## Options

1. **The XMI library serializations (`sysml.library.xmi/`) of the SysML-v2-Release repository, at the release whose library text matches the normative files.** This was the recommended default.
2. The same, at a later release.
3. No external oracle: test only what the clause text gives directly (the top-level package UUIDs and the `path()` rules), and report full UUID conformance as unverified.

## Evidence

**Matching text.** The git blob SHA-1 of each of the 94 normative library files (`.kerml` and `.sysml` from the 10 `.kpar` archives) was compared with the files in `sysml.library/` at every SysML-v2-Release release from 2024-11 to 2026-09.1:

| Releases | Identical library files |
|---|---|
| 2025-02, 2025-04 | **94 of 94** |
| 2025-06 | 91 |
| 2025-07 to 2026-04 | 71 |
| 2026-05 to 2026-09.1 | 67 |
| 2024-12, 2024-11 | 56, 45 |

The `sysml.library.xmi/` files of 2025-02 and 2025-04 are also identical to each other (94 of 94).

**Ids follow KerML 9.1.** `scripts/check_uuid_oracle.py` recomputes the expected UUIDs without pilot code, from the clause text:
- effective names (KerML 8.3.3): the declared name when there is a declared name or short name, otherwise the name of the feature redefined by the first owned redefinition
- escaped names: the name, else the short name, written as an unrestricted name where needed (8.3.2.1, 8.2.2.3)
- qualified names, null for any member after the first with the same name in its namespace (8.3.2.1)
- the owning-membership kinds from the `SysML.json` subtype closure, which includes `FeatureValue`
- positional paths, and the `/owningMembership` form

Unit tests in `scripts/tests/` cover each of these rules on small models. Over the 109,199 elements of the 2025-02 libraries:

| Elements | Match KerML 9.1 | Not recomputable by the script |
|---|---|---|
| Top-level library packages | **94 of 94** | 0 |
| Named elements | **9,662 of 9,662** | 0 |
| Unnamed elements | 33,048 | 2,518 |
| Relationships | 60,637 | 3,240 |
| **All** | **103,441 (94.73%)** | 5,758 |

The script cannot recompute the remaining ids, because their paths depend on effective names that come from implied redefinitions (KerML 8.4), which this XMI does not contain. A sample confirms the explanation. In `SampledFunctions::Domain`, the unnamed `ReferenceUsage` of a `ReturnParameterMembership` has the id `uuid5(package, "SampledFunctions::Domain::result")`, and its membership has the id for `SampledFunctions::Domain::result/owningMembership`: the effective name `result` comes from the implied redefinition of the library's `result` parameter. Our engine computes implied relationships in step 1a.9, after which the full set can be checked.

The 94 root namespaces have ids that follow another rule. KerML 9.1 covers only the top-level library package and the elements it contains, so root namespace ids are not normative, and our tests do not compare them.

A corrupted id (a named element's id replaced) makes the script fail and name the element, so the check is not vacuous.

**Pinning and license.** The repository is EPL-2.0 (reported by GitHub). The release tag 2025-02 is commit `b17ab0ed8c85e3713c8c27ab89f995d764f8974c`. Each of the 94 files was checked against that commit's git tree, then pinned in `specs.lock.json` by a commit-based raw URL and SHA-256 (25.8 MB in total). Like everything under `docs/specs/`, the files are fetched locally and never committed.

## Decision

Adopt option 1, at release **2025-02**. The files are pinned under `docs/specs/SysML-v2-Release/2025-02/sysml.library.xmi/`.
- Option 2 is rejected: later releases' library text has drifted from the normative files.
- Release 2025-04 is equivalent to 2025-02. 2025-02 is preferred as the release that matches metamodel 20250201.
- Option 3 is rejected as much weaker: it could not test any id below the top-level package.

`scripts/check_uuid_oracle.py` runs in CI after the specs are fetched. It fails unless every top-level package and named element matches KerML 9.1.

## Consequences

- Step 1a.11 tests every library element id our engine computes against this oracle. Root namespaces are skipped as non-normative.
- After step 1a.9 (implied relationships), the ids the script cannot recompute become checkable as well, through our engine.
- The oracle stays tied to the normative library version. If the pinned `.kpar` files change, this evaluation is repeated: find the matching release, re-pin, and rerun the script.
- The pilot's output is used only as data. None of its code is used.
