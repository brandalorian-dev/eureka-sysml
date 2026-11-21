# ADR 0014: Readings of the KerML grammar where the spec disagrees with itself

- Status: Accepted
- Date: 2026-11-20
- Context: step 1a.3, the Root and Core grammar

## Context

The KerML textual grammar (KerML 8.2) is normative, and the examples in clause 7 are informative. In two places they disagree. CLAUDE.md asks for an ADR whenever the spec is ambiguous. SysML repeats both productions word for word (SysML 8.2.2), so the readings apply to both languages.

## Decisions

### 1. `standard` is optional in a library package

KerML 8.2.5.13 reads:

```text
LibraryPackage =
       ( isStandard ?= 'standard' ) 'library'
       ( ownedRelationship += PrefixMetadataMember )*
       PackageDeclaration PackageBody
```

Taken literally, every library package would be a standard one. But:
- KerML 7.4.14 shows `library package AddressBooks { … }`, and says that only the libraries of the specification, or other recognized standard libraries, should be marked `standard`.
- `isStandard` defaults to `false` in the abstract syntax (KerML 8.3.4.13.3).
- `?=` assigns `true` only "if the concrete syntax Element is parsed" (8.2.1, Table 3), which presumes it can be absent.

**Reading:** `( isStandard ?= 'standard' )?`. The `?` is missing from the grammar text. `library package P;` parses as a non-standard library package.

### 2. An import needs a visibility

KerML 8.2.3.4.2 reads `Import = visibility = VisibilityIndicator 'import' …`, so the visibility is required. The example in KerML 7.2.5 nevertheless has `import C;`, inside a nested namespace.

Here the grammar is explicit, and no other production, nor the abstract syntax, says otherwise. The example is informative, and every import in the standard libraries has a visibility.

**Reading:** the grammar is right and the example is not. An import without a visibility is reported as `syntax::import-without-visibility` (an error). It is still parsed into an `Import` node, so the rest of the file, and the editor, are unaffected.

## Also confirmed, without an ambiguity

TypeDeclaration (8.2.4.1.1) requires `( SpecializationPart | ConjugationPart )+`, so `type T;` is an error (`syntax::expected-specialization`). Unlike for classifiers and features, nothing implies a default supertype for a plain type, and every clause 7 example of a `type` declares one. This is recorded here because it surprises readers; it is the grammar as written.

## Consequences

- `crates/sysml-syntax` implements both readings, citing this ADR. The tests `library_package_forms` and `an_import_needs_a_visibility` pin them.
- If the OMG corrects either production, the corresponding test changes and this ADR is superseded.
