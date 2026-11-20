# Eureka SysML — Project Plan

Status: Draft v0.7 · 2026-11-20 · Project name: **Eureka SysML** (`eureka-sysml`)

Change history:
- v0.1: initial plan
- v0.2: KerML 1.0 specification and libraries
- v0.3: multi-user web architecture with a Rust/WASM core, a TypeScript UI and PostgreSQL
- v0.4: aligned with the Systems Modeling API & Services (OpenAPI and pilot implementation)
- v0.5: aligned with the formal OMG Systems Modeling API & Services v1.0 (PIM, PSMs, versioning rules, conformance suite)
- v0.6: owner decisions applied: a solo developer with Claude Code, free and open-source software, all diagram views and document generation in 1.0, ITAR/FedRAMP-ready self-hosting, staged releases
- v0.7: review after the Phase 0 work (PRs #1–#5). Facts corrected against the normative files, Phase 0 status recorded, its remaining items moved to the phases that need them, and the lessons on spec-artifact defects and code review added to the risks.

## 1. Goal

Build a web application that lets **multiple concurrent users** create, edit, view, validate and exchange **very large** SysML v2 models in both textual and graphical notation. The baseline is OMG SysML v2.0 (formal/2026-03-02) and KerML 1.0 (formal/2026-03-01), together with their machine-readable artifacts in `docs/specs/`. Cameo Systems Modeler (CSM), with Teamwork Cloud, is the reference for which features practitioners expect.

**Confirmed parameters** (owner decisions, 2026-11-19):

| Topic | Decision |
|---|---|
| Model size | 100k–1M elements, not counting the standard libraries |
| Concurrency | 10–50 concurrent users per model. Merge by default, with optional locks per project. |
| Deployment | Single organization, **self-hosted**. **ITAR and FedRAMP environments are possible**, so it must be deployable air-gapped and on-premises (§4.8). |
| Access control | Per project in 1.0 |
| Desktop/offline edition | Post-1.0 |
| Team | **One developer working with Claude Code** (§12) |
| Licensing | **Free and open-source software (FOSS)**, hosted at https://github.com/brandalorian-dev/eureka-sysml (§13) |
| Users | A mix of experienced SysML/Cameo engineers and newcomers to MBSE |
| SysML v1 / Cameo migration | Not required |
| Editing | Text and diagrams, both first-class |
| Diagrams | **All standard views** in 1.0 |
| Document generation | **Required in 1.0** |
| Pilot code reuse | None. The OMG pilots are behavioral references only. |

## 2. What the sources tell us

### 2.1 KerML: the foundation (`docs/specs/KerML/`)

- **Three syntactic layers.** SysML builds on all three. Our model core uses the same layering.
  - **Root:** elements, relationships, dependencies, annotations, namespaces, imports.
  - **Core:** types, classifiers, features, specialization, subsetting, redefinition, conjugation. Semantics are defined mathematically.
  - **Kernel:** data types, classes, structures, associations, connectors, behaviors, functions, expressions, interactions, feature values, multiplicities, metadata, packages.
- **One combined metamodel.** `KerML.json` defines 82 metaclasses and 2 enumerations. All 82 also appear in `SysML.json`, which adds 93 SysML metaclasses for 175 in total, plus 7 enumerations. (With the `Identified` reference type, the two schemas have 85 and 183 definitions.) Generating code from `SysML.json` therefore covers both languages; `sysml-model` does this (§4.2).
- **Semantics come from a library written in KerML itself.** Kernel constructs get their meaning by specializing elements of the Kernel Semantic Library. For example, a `DataType` must specialize `Base::DataValue`. The three KerML libraries are textual `.kerml` models (36 files):
  - Semantic Library: 16 files
  - Data Type Library: 3 files
  - Function Library: 17 files

  The three libraries declare **circular usages**, so they must be loaded and resolved together as one unit. The SysML Systems Library then depends on all three.
- **We need a `.kerml` parser as well as a `.sysml` parser.** The libraries are written in KerML notation, so the tool needs both grammars: KerML 8.2.2–8.2.5 and SysML 8.2.2. They share most of their productions, including the full expression language.
- **Implied relationships (KerML 8.4.2).** Parsed or imported models usually don't satisfy the semantic constraints until the tool adds implied relationships. There are four kinds:
  - specializations to library types
  - redefinitions
  - type-featurings
  - binding connectors

  The tables in 8.4.3 and 8.4.4 list every implied relationship. Two rules prevent redundant specializations. This is the main semantic engine of the tool.
- **Interchange (KerML Clause 10):**
  - Each file holds one root namespace, as text (`.kerml` / `.sysml`), JSON or XMI.
  - JSON (10.4) serializes each element as `@id` and `@type` plus all owned and inherited properties. References are written as `{"@id": …}`.
  - A `.kpar` archive is a ZIP holding the model files plus `.project.json` and `.meta.json`. Their schemas are in `KerML-Model-Interchange.json`. Project usages carry version constraints; in the standard libraries these are all bare versions such as `1.0.0`.
  - **The OMG library archives do not follow 10.3 exactly.** Each wraps its project in one folder, two index entries name files that do not exist, two usages name a library IRI that does not exist, and one archive contains macOS metadata. The test corpus corrects these with an errata table that fails when OMG fixes them (ADR 0006).
- **Element identity.** Library elements have **deterministic v5 UUIDs**, derived from the library package URL and each element's `path()`.
- **Execution.** KerML Annex A (informative) describes a procedure for incrementally instantiating structures and behaviors. It is the basis for later simulation work.

### 2.2 SysML v2 essentials that shape the design

- **The model is the abstract syntax.** The textual and graphical notations are both views of it. The textual notation is complete: every model can be written as `.sysml` text. In this architecture, text is something the user edits, not the stored form (§4.3).
- **Definition/usage pattern.** Every construct comes in a `def` (definition) and a usage form. Usages can be nested, specialized and redefined in context.
- **SysML adds its own implicit specializations** to the Systems Library, for example every `part` specializes `Parts::parts`.
- **Diagrams are views.** A diagram is a `view` usage that exposes model elements and filters them. The `StandardViewDefinitions` library defines the standard views:
  - General View (gv)
  - Interconnection View (iv)
  - Action Flow View (afv)
  - State Transition View (stv)
  - Sequence View (sv)
  - Geometry View (gev)
  - Grid View (grv)
  - Browser View (bv)

  The spec does not define how diagram layout is exchanged, so we have to design that ourselves.
- **Variability is built in.** `variation` / `variant` can be used on any definition or usage.
- **Conformance levels:**
  - **Mandatory:** Abstract Syntax Conformance and Model Interchange Conformance (`.kpar` archives with `.sysml` files).
  - **Optional:** Textual, Graphical, Semantic and Domain Library conformance.
- **Part 2: v1 → v2 transformation** (`SysMLv1Tov2.xmi`). This is the route for migrating existing Cameo/SysML v1 models.

### 2.3 Systems Modeling API & Services v1.0 (`docs/specs/api/omg/`)

The source is the formal OMG specification (formal/2026-03-04) and its normative files in `omg/normative/`:
- `OpenAPI.json` (REST PSM) and `Schema.json`
- `Systems-Modeling-API.xmi` (PIM)
- the OSLC vocabularies and shapes for KerML and SysML (`*-vocab.ttl`, `*-shapes.ttl`)

The machine-readable files take precedence over the document. The pilot implementation copies in `docs/specs/api/` are informative. Its OpenAPI has the same 35 endpoints as the normative one.

**Architecture.** A **platform-independent model (PIM)** defines six services. Two **platform-specific models (PSMs)** bind it: REST/HTTP (OpenAPI 3.1) and OSLC 3.0 (OpenAPI 2.0 plus RDF). The PIM says nothing about provider architecture, so our Rust server and PostgreSQL design is fine.

| PIM service | Operations | REST binding |
|---|---|---|
| ProjectService | get/create/update/delete project | `/projects` |
| ElementNavigationService | getElements, getElementById, getRootElements, getRelationshipsByRelatedElement (in/out/both) | `/projects/{p}/commits/{c}/elements`, `/roots`, `/relationships` |
| ProjectDataVersioningService | commits (create, head, by ID, changes filtered by `CREATED`/`UPDATED`/`DELETED`), branches (create, delete, default), tags, **diffCommits**, **mergeIntoBranch** | `/commits`, `/branches`, `/tags`, `/diff`, `/branches/{b}/merge` |
| QueryService | get/create/update/delete query, executeQuery, executeQueryById | `/queries`, `/query-results` |
| ExternalRelationshipService | get (all, by element, by ID), create, delete | **no dedicated endpoints.** Reads are queries on `@type = ExternalRelationship`. Writes are commits. |
| ProjectUsageService | getProjectUsages, create, delete | **no dedicated endpoints.** Reads are queries on `@type = ProjectUsage`. Writes are commits with a ProjectUsage payload. |

**Conformance options:**
- **PSM level:** REST, OSLC or both.
- **Per service:** full conformance, or *Read* conformance (only the query operations).
- **Derived properties:**
  - **Passthrough:** store whatever derived values the client sends.
  - **Full:** the provider always returns correctly computed, up-to-date derived properties and can query on them. This also covers elements affected by commits that never touched them.
- API Annex A has a **conformance test suite** of about 37 test cases, written in OCL:
  - 4 for ProjectService
  - 5 for ElementNavigationService
  - 15 for ProjectDataVersioningService
  - 2 for QueryService
  - 3 for ExternalRelationshipService
  - 1 for ProjectUsageService
  - 7 cross-cutting (missing or nonexistent inputs)

**Versioning semantics (PIM 7.1.2). These are normative and become database and engine rules:**
- **DataIdentity** is permanent. A **DataVersion** wraps the payload at a commit (null means not present). DataVersion IDs are random UUIDs.
- **Create:** the client either leaves the identity empty (the server assigns one) or supplies a **new** UUID, which the server accepts as is. **Update:** a payload with the existing identity. **Delete:** a null payload. Delete is only valid if the identity exists in a parent commit.
- **Deletion cascades.** Deleting an element must also delete its **incoming relationships** and remove references to it from other data.
- **`versionedData`** is the full contents at a commit. It is defined by an exact algorithm (given in OCL): the commit's non-delete changes, plus everything from the parent commits that wasn't changed, with duplicates removed.
- **A merge commit must resolve every conflict among its parents.** Any identity whose payload differs between parents must appear in the merge commit's `change`.
- `mergeIntoBranch(base, commitsToMerge, resolution[], description)` returns either a merge commit or a list of conflicting DataIdentities (409 in REST). The client resolves conflicts by sending resolution data and retrying.
- **Mutability:**
  - Commits are **immutable and never deleted** (only by administrator repair).
  - Branches are mutable and can be deleted. A project always has one default branch.
  - Tags are immutable and can be deleted.
- **Timestamps** are ISO 8601 with an explicit UTC offset (UTC recommended). They must be **strictly increasing** along every commit's parent chain, with fine enough resolution for the commit rate.
- **Data** is one of Element, ExternalData (an IRI), ExternalRelationship (element ↔ external data, with an optional `specification`/`language` mapping) or ProjectUsage (`usedProject` plus `usedCommit`).
- **The Project is the permission target.** Access control is left to the provider.

**Queries.** A `select` list, a `where` tree (primitive constraints `=`, `<`, `<=`, `>`, `>=`, `in`, `instanceOf` with `inverse`; composite constraints `and`/`or`), and in the PIM also `scope` and `orderBy`. **The normative REST `Query` schema leaves out `scope` and `orderBy`**, so we offer them as extensions. Queries run at a given commit, or at the default branch's head if none is given.

**REST conventions.** Cursor paging with `page[size]`, `page[after]` and `page[before]`. The cursors are opaque, and next/prev links go in the RFC 8288 `Link` header. The provider sets the default page size. Errors are `{"@type":"Error","description":…}`.

**OSLC PSM.** It maps PIM concepts to OSLC resources and uses RDF vocabularies and shapes for KerML and SysML. It matters for integration with OSLC-based lifecycle tools such as requirements, ALM and PLM systems. It is post-MVP for us.

**Still not covered by the spec**, so these are our extensions: authentication and authorization mechanics, locks, drafts, live notifications and presence, layouts, comments.

**Version skew:** API records are `20230201`, the metamodel `20250201` and KerML interchange `20240201`. `Schema.json` mixes them: the API records are `SystemsModelingAPI/20230201`, `Identified` is defined as `SysML/20250201` but referenced as `SysML/20230201`, and `Data` refers to a `SysML/20230201/Element` that `Schema.json` does not define.

**Defects in the normative API files** (handled in `sysml-api-types`, ADR 0005): `Schema.json` annotates `Commit.owningProject` as a Branch where the PIM says Project, and gives `PrimitiveConstraintRequest` the `$id` of `PrimitiveConstraint`. In `OpenAPI.json`, the `MergeCommit` example omits required properties and the `Commits` example links to the wrong place.

### 2.4 Reference product: Cameo Systems Modeler and Teamwork Cloud

| CSM / TWC capability | Our equivalent | Priority |
|---|---|---|
| Teamwork Cloud server: shared repository, commits, branches, element locking, users and permissions | Rust model server + PostgreSQL: commits and branches, package locks, role-based permissions, presence | **Essential (MVP)** |
| Containment tree, specification window | Browser View tree plus a specification panel | Essential |
| Diagram editor (BDD, IBD, Activity, State Machine, Sequence, Parametric, Requirement, Use Case, Package) | v2 standard views: GV, IV, then AFV, STV, SV | Essential / Post-MVP |
| Tables and matrices | Grid View, computed on the server across the whole model | Essential |
| Model validation | KerML and SysML well-formedness constraints: incremental in the browser, complete on the server at commit | Essential |
| Requirements and traceability, ReqIF | Requirement / satisfy / verify / derive, matrices, ReqIF later | Essential / Later |
| Report generation | Document export driven by views | Later |
| Simulation | Function Library evaluation, then execution based on KerML Annex A | Later |
| Macros and API | SysML v2 API & Services (REST), scripting | API is MVP, scripting Later |

## 3. Technology choice

The options were compared in detail during planning:

| Option | Main advantage | Main problem |
|---|---|---|
| TypeScript only (Langium) | Fastest path to a working editor | Too slow and memory-limited for models of 1M elements |
| Java with Xtext/EMF | The OMG reference implementation can be reused | The UI is either a dated desktop app or a split stack |
| Kotlin, C#/.NET, Python, C++/Qt | — | Weaker on language tooling, the web route or development speed |
| **Rust core with a TypeScript UI** | **Native speed and memory control on the server, plus the same engine in the browser through WASM** | Slower to develop, a smaller hiring pool, and more of the language tooling must be hand-built |

**Decision: a Rust core compiled two ways, with a TypeScript web UI.**
- Very large models and many concurrent users mean the authoritative model must live on a server. Rust is strongest there: multithreaded, with predictable memory and no garbage-collection pauses.
- The same crates compiled to WASM give every browser instant parsing, completion and validation of the part of the model the user is editing.
- One engine means the browser and the server always give identical validation results.

## 4. Architecture

### 4.1 System overview

```
 Browser (per user)                         Server
┌──────────────────────────────┐     ┌──────────────────────────────────────┐
│ TypeScript UI                │     │ Rust model service (native, axum)    │
│  Monaco · diagrams · grids   │     │  • shared, pre-resolved libraries    │
│            │                 │     │    (loaded once for every user)      │
│ Rust core → WASM (worker)    │◄───►│  • resolved model per active branch  │
│  • parse/validate the        │ WS/ │  • incremental re-resolve on commit  │
│    working set               │HTTP │  • whole-model queries: references,  │
│  • completion, local errors  │     │    impact, matrices, validation      │
│  • read-only index of the    │     │  • SysML v2 API & Services endpoints │
│    rest of the model         │     │  • authentication, permissions,      │
└──────────────────────────────┘     │    locks, presence                   │
                                     └──────────────┬───────────────────────┘
                                                    │
                                     ┌──────────────▼───────────────────────┐
                                     │ PostgreSQL                           │
                                     │  projects · branches · commits       │
                                     │  element versions (JSONB per         │
                                     │  SysML.json) · relationships         │
                                     │  users · permissions · locks         │
                                     │  comments · diagram layouts          │
                                     └──────────────────────────────────────┘
```

### 4.2 Rust core (shared crates)

| Crate | Responsibility | Targets |
|---|---|---|
| `sysml-syntax` | Lexer and parsers for KerML and SysML. Produces a lossless syntax tree (the `rowan` approach from rust-analyzer), so comments and formatting survive edits. Error-tolerant. | native + wasm |
| `sysml-model` | Metaclass types and enums generated from `SysML.json`. An arena-based element graph keyed by ID, so cyclic references work without fighting Rust's ownership rules. Hand-written derived properties and operations. | native + wasm |
| `sysml-semantics` | Name resolution, imports, inheritance and redefinition, implied relationships with the redundancy rules, and validation constraints. Built on **incremental queries** (`salsa`), so an edit only recomputes what depends on it. | native + wasm |
| `sysml-libraries` | Loads and resolves the 10 standard libraries once, computes the v5 UUIDs, and writes a **pre-resolved binary snapshot** shipped with both the server and the client | native + wasm |
| `sysml-interchange` | `.kpar`, JSON (KerML 10.4), text serializer/formatter, and XMI import | native + wasm |
| `sysml-views` | View and filter evaluation, and producing diagram, tree and grid view models for the UI | native + wasm |
| `sysml-store` | PostgreSQL persistence: commits, element versions, snapshots, locks | native |
| `sysml-server` | HTTP and WebSocket API (axum). It provides the standard SysML v2 API & Services endpoints (§4.4), our own extensions under `/x/…`, authentication and permissions, sessions and presence | native |
| `sysml-api-types` | API record types (Project, Branch, Tag, Commit, DataIdentity, DataVersion, ProjectUsage, External*, Query/Constraint, DataDifference, Error) generated from the **normative** `docs/specs/api/omg/normative/Schema.json`, plus a typed TypeScript client generated from `OpenAPI.json` (Phase 3) | native + wasm, TS |
| `sysml-versioning` | The PIM 7.1.2 rules as pure, storage-independent logic: computing `versionedData`, checking create/update/delete validity, delete cascades, three-way diff, merge conflict detection and resolution, monotonic timestamps. Fully covered by property-based tests. | native + wasm |
| `sysml-wasm` | The `wasm-bindgen` interface the UI calls, with small, coarse-grained calls | wasm |

**Test infrastructure** (`publish = false`, never shipped, and not in the SBOM):

| Crate | Responsibility |
|---|---|
| `sysml-corpus` | The conformance corpus (§8): the 10 standard libraries from their `.kpar` archives and the example models, checked against `specs.lock.json` and the KerML 10.3 archive rules (ADR 0006) |
| `sysml-synth` | Deterministic synthetic SysML models of any size, valid by construction, for benchmarks and scale tests (ADR 0007) |
| `sysml-bench` | The §7 budgets as data, kept equal to this plan by a test, and the `divan` benchmarks (ADR 0007) |

**Status at the end of Phase 0.** Built: `sysml-model` (the metaclass and enumeration types and property descriptors; the arena element graph comes in Phase 1a), `sysml-api-types` (the Rust records; the TypeScript client comes in Phase 3), and the three test-infrastructure crates. The other crates start in the phases listed in §6.

### 4.3 Editing model: element storage with text as a view

- **The source of truth is element versions in PostgreSQL**, organized into commits on branches. This replaces the v0.1/v0.2 "text files are the source of truth" decision.
- **Text projection.** A user opens a package or element as SysML text. The text is produced by the serializer, with formatting and comments preserved as stored trivia. On edit:
  1. The browser's WASM core re-parses the text.
  2. It diffs the result against the previous version of that scope.
  3. It sends the server element-level deltas (create, update, delete, move).
- **Diagram and form edits** produce the same kind of deltas directly.
- **The API's versioning model is our storage model.** An element's `elementId` is its API **DataIdentity** `@id`. Each change produces a **DataVersion**. The browser's deltas are sent as API `CommitRequest.change[]`, with a `null` payload meaning delete. The UI and external API clients therefore use one mechanism for writes.
- **Client-assigned IDs.** The browser creates new elements with its own v4 UUID, which the API allows as long as the ID is new. The ID is therefore known immediately for diagrams and references, before the commit happens.
- **Delete cascades are applied on the server.** Deleting an element also deletes its incoming relationships and removes references to it, as required by PIM 7.2.3. The editor shows what will be removed before the user confirms.
- **Derived properties.** We target **Derived Property Full Conformance**, because the semantic engine already computes derived values. API responses include up-to-date derived properties, and queries can filter on them. Derived values sent by clients are ignored and recomputed.
- **Libraries and shared models are ProjectUsages.** The standard libraries are loaded as read-only projects, and user projects reference them (and each other) at a pinned commit. The pre-resolved library snapshot is keyed by that project and commit.
- **Identity.**
  - User elements get a random v4 UUID, assigned once when the element is created and kept across renames and moves.
  - Library elements keep the normative v5 UUIDs.
  - When text is re-parsed, elements are matched to existing IDs by structural position and name. Rename and move refactorings carry IDs explicitly.
- **Drafts.** Each user's uncommitted changes are kept on the server in a private draft on their branch, so work survives a page reload. Commit publishes the draft as one change set.

### 4.4 Concurrency and collaboration

**API surface.** The SysML v2 API endpoints (35 operations on 23 paths) are implemented as defined in the normative `OpenAPI.json`; the pilot's `openapi-sans-schemas.json` is informative only. Our extensions follow the pilot's `/x/` convention, so standard clients never see them.

| Area | Standard API (implemented as defined) | Our extensions (`/x/…`) |
|---|---|---|
| Lifecycle | projects, branches, tags, commits, changes | drafts (`/x/drafts`), commit validation report |
| Model access | elements and roots at a commit, relationships, `excludeUsed`, cursor paging | working-set bundles (binary, for the WASM client), name/type index, view models (tree page, diagram, grid) |
| Versioning | diff (`DataDifference`, filtered by `CREATED`/`UPDATED`/`DELETED`), merge (409 with the conflicting DataIdentities, retried with `resolution` data) | conflict detail (base, ours and theirs payloads), plus a merge preview with semantic validation |
| Query | `Query` / `Constraint` / `query-results` | `scope` and `orderBy` (in the PIM, missing from the REST schema), full-text and qualified-name search, impact and traceability queries |
| External relationships, project usages | Through commits and `@type` queries, as the REST PSM specifies | convenience endpoints |
| Lookup by qualified name | `/x/named/…` (pilot-compatible) | — |
| Collaboration | — | locks, presence, WebSocket event stream, comments, layouts |
| Security | — (not in the API) | OIDC authentication, role-based permissions applied to **every** endpoint, standard ones included |

| Mechanism | Scope | MVP? |
|---|---|---|
| **Commits and branches** (the SysML v2 API model) | All changes are change sets on a branch. Merges follow `mergeIntoBranch`. A conflict is any identity whose payloads differ between the parents. The resulting merge commit must list every conflicting identity in its `change`. On top of the spec, we accept a merge only if the merged model also passes semantic validation. Otherwise it returns 409 with details of the conflict or validation failure. | Yes |
| **Package/element locks** (optional, configured per project) | Prevent conflicting edits on shared branches. Locks expire and administrators can override them. | Yes |
| **Live updates and presence** | A WebSocket pushes commits and presence ("who is editing what") to clients on the same branch. The browser's working set updates incrementally. | Yes |
| **Conflict resolution UI** | Side-by-side element diff and choice of version | Yes (basic) |
| **Real-time co-editing** (CRDT) | Only within a single diagram or package editor | Post-MVP, optional |

### 4.5 Working set and scale

- The **browser** holds:
  - the standard libraries, from the pre-resolved snapshot
  - the user's open packages plus their direct dependencies, up to about 50k elements
  - a compact **name and type index** of the rest of the model, for completion and navigation

  Everything else is fetched on demand. WASM memory is capped at 4 GB, so the browser never loads the whole model.
- The **server** keeps a resolved, incrementally maintained model for each active branch head. Older commits are materialized on demand from snapshots plus the changes since.
- **Whole-model operations run on the server:** find references, impact analysis, traceability matrices, full validation, export.
- **Horizontal scaling:** each project is pinned to one server instance (sticky routing), and projects are spread across instances. PostgreSQL is the only shared state.

### 4.6 PostgreSQL schema outline

The table names follow the API's records. **Unlike the pilot**, which maps each of the 183 metaclasses to its own JPA entity and table, element payloads are stored as **JSONB** in the KerML 10.4 / `SysML.json` format. This keeps the schema stable when the metamodel changes and makes the API responses close to pass-through. The fields that need to be queried are extracted into indexed columns.

| Table | Purpose / key columns |
|---|---|
| `projects` | id, name, description, alias[], created, default_branch_id, settings (lock policy, metamodel IRI) |
| `branches` | id, project_id, name, head_commit_id, referenced_commit_id, created, deleted, protected |
| `tags` | id, project_id, name, tagged_commit_id, created, deleted |
| `commits` | id, project_id, previous_commit_ids[], author_id, name, description, created |
| `data_identities` | id (uuid = elementId), project_id, kind (Element / ExternalData / ExternalRelationship / ProjectUsage) |
| `data_versions` | id, identity_id, commit_id, metaclass, owner_id, qualified_name, `payload JSONB` (null = deleted) |
| `branch_head_index` | branch_id, identity_id → data_version_id (the current state of each branch, for fast loading) |
| `snapshots` | commit_id, full version index plus compressed binary model image. The pilot keeps `CommitDataVersionIndex` for **every** commit, which is too large at 1M elements. We keep it for branch heads, tags and periodic checkpoints, and rebuild other commits from the nearest snapshot plus the changes since. |
| `project_usages` | commit_id, using_project_id, used_project_id, used_commit_id (libraries and shared models) |
| `relationships` | commit_id, relationship_id, kind, source_id, target_id (derived, for the `relationships?direction=` endpoint, traceability and impact queries) |
| `queries` | id, project_id, name, select[], where (JSONB constraint tree) |
| `drafts` | user_id, branch_id, pending deltas (JSONB) |
| `locks` | project_id, element_id (package or element scope), holder_id, acquired_at, expires_at |
| `diagram_layouts` | view_element_id, branch_id, layout JSONB (node positions, sizes, edge routes) |
| `users`, `roles`, `project_members` | identity (from SSO), role-based access per project, and per-package permissions later |
| `comments` | element_id, thread, author, body, resolved |
| `audit_log` | who did what and when |

Rules enforced in the database (from PIM 7.1.2):
- `commits` is append-only: no UPDATE or DELETE grants for the application role, and repair is an administrator procedure.
- `tags.tagged_commit_id` is immutable. Tags and branches use soft delete (`deleted` timestamp).
- Exactly one default branch per project.
- `commits.created` is a `timestamptz` with microsecond precision, stored in UTC, and a trigger enforces that it is later than every parent's.
- At most one DataVersion per identity per commit.

Main indexes: `data_versions(identity_id, commit_id)`, `qualified_name` (trigram, for search and the `/x/named` endpoints), `metaclass` (for `instanceOf` queries), `relationships(source_id)` and `relationships(target_id)`, and GIN on `payload` for selected paths (for `Query` primitive constraints).

### 4.7 Web client (TypeScript)

- A web shell whose UI framework is still open (§10, decision 27; needed by Phase 3). Monaco for text projections. Diagrams with Sprotty or React Flow plus ELK.js, chosen in Phase 5 (decision 28). A virtualized data grid for tables and matrices.
- The WASM core runs in a **web worker**. The UI calls coarse operations ("tree page", "diagram view model", "validate scope") rather than moving model data across element by element.
- Language features (completion, hover, go-to-definition, rename, diagnostics) call the worker directly in the browser.
- **The same crates also ship as a standalone language server and CLI** (`eureka-lsp`, `eureka` for parse, validate, format and convert). This costs little, comes early (Release 0.1), and gives the open-source project users before the web application exists.
- **Two interaction levels for mixed experience.** A *guided* mode with palettes, wizards, form-first editing and inline explanations of SysML v2 concepts, and an *expert* mode that is text-first, keyboard-driven and dense. Both run on the same model and the same deltas.

### 4.8 Deployment, security and compliance

Deployment targets regulated, self-hosted environments first, including ITAR-controlled networks and systems seeking FedRAMP/NIST 800-53 authorization:
- **Self-contained and air-gap capable.**
  - One `eureka-server` binary or container that serves the API, the WebSocket and **all web assets, including the `.wasm` bundle**. No CDN and no runtime calls to the internet.
  - No telemetry by default.
  - An offline install bundle: container images, a Helm chart, Docker Compose, and checksums.
- **PostgreSQL 16+**, self-hosted or managed in the customer's authorized environment.
- **Cryptography:** TLS everywhere. Use `rustls` with a **FIPS 140-3–validated provider** (`aws-lc-rs` in FIPS mode) as a build option. Encryption at rest is delegated to PostgreSQL and storage.
- **Identity:** OIDC/SSO. The provider is still to be decided, and PIV/CAC should be supported through the customer's identity provider. Role-based access per project (viewer / editor / admin). Session timeouts and account lockout are configurable.
- **Auditability:** an append-only audit log of every read of sensitive data and every change, which can be exported to a SIEM (syslog/JSON). Together with immutable commits, this gives a complete change history.
- **Supply chain:**
  - an SBOM (CycloneDX) for every release
  - signed releases and images (Sigstore/cosign)
  - `cargo audit` / `cargo deny` / `pnpm audit` as CI gates
  - reproducible builds where feasible

  **Status:** CI checks licenses, advisories and sources with `cargo deny` on every push, and builds with the declared minimum Rust version. It generates a CycloneDX 1.6 SBOM per shipped crate and target (server and WASM), checks it against `cargo metadata` and `Cargo.lock`, and keeps it as a build artifact. The SBOMs are byte-reproducible across machines and checkout paths (ADR 0008). Still to come: signing (Phase 10), an npm SBOM and `pnpm audit` (Phase 3), and the bundled standard libraries in the release SBOM.
- **Hardening guide:** a NIST 800-53 control mapping, written as documentation for the operator's authorization package.

  FedRAMP authorizes a *service*, not the software. As an open-source product, we provide controls and evidence, and the deploying organization holds the authorization.
- **ITAR:** the software itself holds no controlled technical data. Customer models stay inside the customer's deployment. Nothing leaves the environment, including error reports.
- Optional later: a desktop/offline edition with the same core plus SQLite, synchronizing with the server through the SysML v2 API.

### 4.9 Document generation

- **Documents are driven by SysML v2 views.** The structure of a document is a `view` hierarchy with viewpoints, `expose` and filters, so the document definition is itself part of the model and is versioned with it. Templates add the layout around it.
- **Rust `sysml-docgen` crate** with these outputs:
  - **PDF** through Typst, which is written in Rust and can be embedded, so it works air-gapped
  - **DOCX** for contractual deliverables
  - **HTML** and **Markdown**
- Content: diagrams (SVG/PNG from the view renderer), tables and matrices (Grid View), requirement specifications, element documentation and metadata, and traceability appendices.
- Generated from any commit or tag, so baselined documents can be reproduced exactly. Document metadata records the commit ID.
- Ships with templates for common deliverables: a system specification, interface control document, requirements specification and verification cross-reference matrix. Organizations can add their own templates.

## 5. Scope

**Release 1.0: a multi-user web application**
- Projects, branches, tags, commits, drafts, merging, locks, presence, history and diff
- OIDC authentication, role-based access, audit log, and deployment ready for regulated environments
- Text projection editing with fast local validation and completion, in guided and expert modes
- **All standard views:**
  - Browser
  - General
  - Interconnection
  - Action Flow
  - State Transition
  - Sequence
  - Grid (tables and matrices)
  - Geometry (basic 2D/3D rendering of spatial items and shapes)
- Requirements, traceability and allocation, with server-side matrices
- **Document generation** (PDF, DOCX, HTML, Markdown) from views and templates
- All 10 standard libraries
- `.kpar` and JSON import/export
- SysML v2 API & Services: REST PSM conformance for all six services, plus Derived Property Full Conformance
- The standalone language server and CLI

**After 1.0** (priority order still to be decided):
- Analysis and simulation (still to be decided): Function Library evaluation, Quantities & Units, constraints, trade studies, variant configuration, then execution
- Tool integrations (still to be decided): OSLC PSM, ReqIF and specific connectors
- Scripting API, desktop/offline edition, real-time co-editing inside a diagram, per-package access control

**Out of scope:** SysML v1/Cameo import (not required), authoring pure KerML user models, UML, multi-tenant SaaS.

## 6. Phased plan and releases

**Estimating basis.** One developer and Claude Code. Claude Code writes most of the code. The developer owns the architecture, specification interpretation, review, test design and integration. Estimates are calendar weeks of full-time work. **They are highly uncertain and will be recalibrated after Phase 1a** using measured throughput. The total scope has grown since v0.5, because all views and document generation are now in 1.0.

**Phase 0 data point.** Phase 0 was estimated at 2–3 weeks. Its delivered part took two days (2026-11-19 to 2026-11-20), with the items listed under Phase 0 moved to later phases. Foundations are mostly tooling, which Claude Code produces quickly, so this says little about the language core. The recalibration after Phase 1a stands.

The work is delivered as **staged open-source releases**, so each one is useful on its own. This builds a user and contributor base and reduces risk on a long solo project.

| Release | Contents | Cumulative (calendar) |
|---|---|---|
| **0.1 "Language"** | Phases 0, 1a and 1b: KerML/SysML parser, semantics and validation, standard libraries, **CLI and language server** (VS Code extension), `.kpar`/JSON conversion | ~5–7 months |
| **0.2 "Repository"** | Phase 2: a SysML v2 API & Services server conformant with API Annex A (except merge), on PostgreSQL. Usable headless by other tools. | ~7–9.5 months |
| **0.3 "Editor"** | Phases 3 and 4: web text editor, browser tree and specification panel, collaboration and merge, full API conformance | ~10.5–14 months |
| **0.5 "Diagrams"** | Phases 5 and 6: General and Interconnection Views, requirements and Grid View | ~14.5–19.5 months |
| **0.8 "Behavior"** | Phases 7 and 8: Action Flow, State Transition, Sequence and Geometry Views | ~17.5–24 months |
| **1.0** | Phases 9 and 10: document generation, interchange finalized, security hardening, deployment packaging, documentation | **~20–28 months** |

### Phase 0 — Foundations (estimated 2–3 wks) — done 2026-11-20
Delivered in PRs #1–#5 (ADRs 0004–0008):
- GitHub repository `brandalorian-dev/eureka-sysml`: license, `CLAUDE.md` (§12), contribution guide, code of conduct, issue and PR templates
- A Cargo workspace (`crates/*`), with the toolchain pinned in `rust-toolchain.toml` and a declared minimum Rust version (1.89)
- GitHub Actions CI covering:
  - Rust tests, clippy (warnings are errors) and fmt
  - the `wasm32-unknown-unknown` build of every crate
  - tests with exactly the declared minimum Rust version
  - `cargo deny` (licenses, advisories, crate sources)
  - running every benchmark once
  - SBOM generation: CycloneDX, per shipped crate and target, checked and reproducible (ADR 0008)
  - repository hygiene: nothing under `docs/specs/` is tracked
- Code generators: `SysML.json` → Rust metaclass types (`sysml-model`), and `Schema.json` → Rust API records (`sysml-api-types`, ADR 0005). `scripts/check_schema_assumptions.py` stops CI with a readable message if a schema revision changes the layout the generators rely on.
- **Spec material stays local.** `docs/specs/` is excluded from git entirely (§13). `scripts/fetch_specs.py` downloads every spec file the build and tests need, from pinned upstream URLs with SHA-256 checksums, into `docs/specs/`. CI and contributors run it. The manifest (`specs.lock.json`) lives at the repository root.
- **Spec text extraction:** the spec PDFs are split into per-clause Markdown under `docs/specs/text/` (local only), so Claude Code can cite and load exact clauses
- Test corpus (`sysml-corpus`, ADR 0006): the 10 standard libraries (94 `.kerml`/`.sysml` files) and SimpleVehicleModel, the example model of SysML Annex A: 95 sources, each checked against its SHA-256 in `specs.lock.json`
- The synthetic large-model generator (`sysml-synth`) and the benchmark harness (`sysml-bench`), ADR 0007

**Moved to the phases that need them** (v0.7, 2026-11-20):

| Item | Moved to | Why |
|---|---|---|
| The `wasm-pack` build, and running the test suite (the corpus included) on WASM | Phase 1a | It needs the `sysml-wasm` crate, and Phase 1a's exit already requires the WASM build to pass the same tests. CI already builds every crate for wasm32. |
| Spec text snippets in the corpus | Phase 1a | The extracted text does not reliably separate code from prose. This needs a font-aware extraction, or the parser to tell them apart. |
| PostgreSQL integration tests (service container) | Phase 2 | They need `sysml-store`. |
| The pnpm workspace (`apps/web`, `packages/*`), TypeScript types from `SysML.json`, the TypeScript API client from `OpenAPI.json`, Vitest and Playwright in CI, `pnpm audit`, an npm SBOM | Phase 3 | They depend on the UI framework (decision 27, needed by Phase 3), and before then there is nothing to test. |

**Exit:** met. CI is green, the Rust types are generated, the corpus and the benchmark harness run, and the spec clauses can be looked up. No §7 budget can be measured yet: each needs a component from a later phase.

### Phase 1a — KerML core in Rust (10–14 wks), the critical path
- Error-tolerant KerML parser (8.2.2–8.2.5) producing a lossless syntax tree, mapped to the arena-based model
- Namespaces, imports, aliases, visibility, specialization, subsetting, redefinition, conjugation, feature chains, multiplicities
- Implied relationships (8.4.3/8.4.4) with the redundancy rules. Validation constraints. Incremental recomputation with salsa.
- Load the circular KerML libraries, compute v5 UUIDs, and produce the pre-resolved snapshot
- The `sysml-wasm` crate and the `wasm-pack` build. Run the test suite, the corpus included, on WASM; the corpus harness reads from disk, so WASM tests get the sources passed in or embedded (moved from Phase 0, ADR 0006).
- Spec text snippets added to the corpus (moved from Phase 0)
- The first §7 benchmark, `library-load`, with a timing regression gate on a dedicated runner or with instruction counts (ADR 0007)

**Exit:** the KerML libraries resolve with zero errors, the UUIDs follow the scheme, the WASM build passes the same tests, and the estimates are recalibrated.

### Phase 1b — SysML language, CLI and language server (8–10 wks)
- SysML grammar, abstract syntax mapping, implicit specializations, validation. Systems Library and the six domain libraries.
- Serializer/formatter that preserves comments and formatting, with a text → model → text round-trip
- `.kpar` and JSON (KerML 10.4) import/export
- `eureka` CLI (validate, format, convert, query) and `eureka-lsp`, plus a VS Code extension
- Parse and resolve the synthetic models, and confirm or correct the element-count rules of `sysml-synth` against the parser (ADR 0007). Benchmarks for the `re-resolve`, `language-features` and `find-references` budgets.

**Exit:** all libraries, SimpleVehicleModel (SysML Annex A) and the spec snippets resolve with zero errors, the round-trip is stable, and the `re-resolve` and `find-references` budgets (§7) are met on the 1M-element synthetic model. **Release 0.1.**

### Phase 2 — Server and persistence (8–10 wks)
- PostgreSQL schema with the database rules from PIM 7.1.2, the `sysml-versioning` crate, snapshots
- REST endpoints for the six PIM services (except merge), with cursor paging, `/meta/datatypes` and `/x/named`
- Derived properties computed on read. Standard libraries as read-only ProjectUsages.
- OIDC authentication, role-based access, audit log. A single self-contained server binary.
- PostgreSQL integration tests in CI with a service container (moved from Phase 0). The `branch-cold-load` benchmark.

**Exit:** the 1M-element import/commit/reload/query test passes, the API Annex A suite passes except merge, the OpenAPI contract tests pass, and the Annex B cookbook runs. **Release 0.2.**

### Phase 3 — Web client: text editing and browsing (8–10 wks)
- Web shell (UI framework still to be decided), WASM worker, working set and name index
- Text projection editor with ID matching, deltas and drafts on the server. Language features.
- Browser View tree and specification panel. Guided and expert modes.
- The pnpm workspace, TypeScript types from `SysML.json`, the TypeScript API client from `OpenAPI.json`, Vitest and Playwright in CI, `pnpm audit` and an npm SBOM (moved from Phase 0). The `keystroke-diagnostics` and `browser-memory` benchmarks.

**Exit:** SimpleVehicleModel can be built entirely in the browser, and per-keystroke feedback stays within budget on a 1M-element model.

### Phase 4 — Collaboration (6–8 wks)
- WebSocket live updates and presence, incremental working-set updates
- `mergeIntoBranch` per PIM 7.2.3 plus semantic validation, and the conflict UI
- Locks, history, commit diff
- The `commit-delivery` benchmark

**Exit:** the 20-editor simulation loses no data, and the **full API Annex A suite** passes. **Release 0.3.**

### Phase 5 — Graphical framework, General View and Interconnection View (10–14 wks)
- Rendering engine (diagram library still to be decided): nodes, textual and graphical compartments, edges, boundary ports, nested views
- General View and Interconnection View, with editing on the canvas. Every edit produces deltas.
- Views as model elements. Layouts in the database. ELK auto-layout. SVG/PNG export. Live updates.

**Exit:** the SysML Annex A structure figures render and edit to match the spec, and two users see each other's changes.

### Phase 6 — Requirements, traceability, Grid View (5–7 wks)
- Requirements, satisfy, verify, derivation, allocation
- Grid View tables and matrices computed on the server, with inline editing
- Coverage and impact queries

**Exit:** the SysML Annex A requirements and verification examples work end to end, and a matrix over 1M elements renders within budget. **Release 0.5.**

### Phase 7 — Behavior views (10–14 wks)
- **Action Flow View:** actions, parameters, successions, control nodes, flows, swimlanes, send/accept, if/loop, triggers
- **State Transition View:** states, transitions with trigger/guard/effect, entry/do/exit, parallel regions
- **Sequence View:** lifelines, messages, events, occurrences

**Exit:** the SysML Annex A action and state examples render and edit to match the spec.

### Phase 8 — Geometry View (3–4 wks)
- Rendering of Geometry Domain Library spatial items and shapes (2D, with basic 3D through WebGL) and coordinate frames

**Exit:** the geometry library examples render. **Release 0.8.**

### Phase 9 — Document generation (5–7 wks)
- The `sysml-docgen` crate (§4.9): view-driven structure, templates, and PDF (Typst), DOCX, HTML and Markdown output
- Standard deliverable templates. Generation from a tag or commit, with the commit ID embedded.

**Exit:** a system specification and a verification cross-reference matrix are generated from SimpleVehicleModel in all four formats.

### Phase 10 — Hardening and 1.0 release (4–6 wks)
- Security hardening: the FIPS build option, the NIST 800-53 control mapping, the air-gapped install bundle, SBOM and signed releases, penetration-test fixes
- Final performance pass, user and administrator documentation, tutorials for newcomers to SysML v2

**Exit:** an air-gapped install from the offline bundle passes the full test suite. **Release 1.0.**

## 7. Performance budgets (measured by the benchmark harness, ADR 0007)

`sysml-bench` holds this table as data (`BUDGETS`), with the phase from which each budget can be measured, and a test keeps the two identical: change them together. Each budget is measured once its component exists: `library-load` in Phase 1a; `re-resolve`, `language-features` and `find-references` in Phase 1b; `branch-cold-load` in Phase 2; `keystroke-diagnostics` and `browser-memory` in Phase 3; `commit-delivery` in Phase 4. The 1M-element and 50k-element inputs come from `sysml-synth`.

| Operation | Budget |
|---|---|
| Load the standard libraries (from the pre-resolved snapshot) | < 300 ms server, < 1 s browser |
| Cold-load a 1M-element branch on the server (from a snapshot) | < 30 s |
| Re-resolve after a typical commit (< 100 changed elements) | < 1 s server |
| Keystroke → diagnostics in the browser (working set ≤ 50k elements) | < 100 ms |
| Completion, hover, go-to-definition | < 50 ms local, < 300 ms if the server is needed |
| Find references / impact query over 1M elements | < 2 s |
| Delivering a commit to other clients on the branch | < 1 s |
| Browser memory for a typical working set | < 1 GB |

## 8. Quality strategy

- **Conformance corpus as the acceptance gate.** The KerML and SysML libraries, SimpleVehicleModel (the SysML Annex A example model) and the spec snippets run in CI, on both native and WASM builds. Today `sysml-corpus` checks that the corpus is complete and intact and that the archives follow KerML 10.3 (ADR 0006). Parsing and resolving every source, the WASM run and the spec snippets come in Phase 1a.
- **Implied-relationship golden tests** checked against the KerML 8.4.3/8.4.4 tables, plus **UUID conformance** tests for the library elements.
- **Round-trip tests:** text → model → text, JSON, `.kpar`, and the database (commit, reload, compare).
- **API conformance suite:** the about 37 OCL test cases in API spec Annex A, automated against the REST PSM in CI. An OCL postcondition becomes an assertion on the response.
- **Versioning property tests:** generate random commit graphs and check the `versionedData` algorithm, the delete rules, merge conflict resolution and monotonic timestamps against an executable model of the PIM OCL.
- **Concurrency tests:** simulated multi-user editing, merges, lock contention and reconnects.
- **Performance regression gates** in CI using the synthetic models (§7). CI already runs every benchmark once, so they keep working. Timing gates start with the first measurable budget, on a dedicated runner or with instruction counts, because timings on shared runners vary too much (ADR 0007).
- **Visual regression tests** for each standard view. End-to-end browser tests (Playwright).
- **Security:** dependency audits (`cargo deny`, in CI since Phase 0, covers the RustSec advisories that `cargo audit` checks; `pnpm audit` from Phase 3), a checked CycloneDX SBOM (ADR 0008), permission tests for every endpoint, an air-gapped install test (no network egress allowed) in CI, and static analysis (CodeQL).
- **Toolchain:** CI builds and tests with exactly the declared minimum Rust version, so a dependency update that needs a newer compiler fails at once.
- **AI-generated code gates:** every change goes through CI plus review (the developer, and `/code-review` in Claude Code). No unreviewed generated code is merged. Spec-derived tests are written before the implementation.

  In Phase 0, `/code-review` found at least one important defect in each of the four PRs it reviewed, all fixed before merging. Examples: a commit change without a payload was read as a delete, a synthetic model missed its size target by 33%, the declared minimum Rust version was too low, and the "reproducible" SBOM contained the checkout path. The tests had passed in every case. Review stays mandatory.

## 9. Risks

| Risk | Mitigation |
|---|---|
| Rust is slow to develop for graph-heavy semantics, and the hiring pool is small | Arena/ID-based model (no `Rc<RefCell>` graphs), follow rust-analyzer's proven patterns, hire or train early, keep the language core small and well-specified |
| Implied relationships and semantic constraints take longer than planned | Implement them table by table, with the library corpus as the test, and compare with the OMG Pilot Implementation |
| Incremental resolution is hard to retrofit | Build on salsa from Phase 1a |
| Text projection loses IDs or formatting | Lossless syntax tree, ID matching by structure, explicit refactorings, round-trip tests |
| A merge produces a model that doesn't validate | Validate every merge semantically, reject or flag the result, and offer locks to prevent conflicts in the first place |
| Server memory for many active branches of a 1M-element model | Share immutable data across branches, evict idle branches, shard projects across instances |
| Cost of passing data between WASM and JavaScript, and the bundle size | Coarse-grained calls, the worker owns the model, lazy loading, a cached snapshot |
| Spec revisions. The IRIs are date-versioned: metamodel `20250201`, KerML interchange `20240201`, API records `20230201`. | Generate code from the machine-readable files, keep the IRIs in one configuration point, and check payloads against `SysML.json` rather than the copy of the metamodel embedded in the API. `check_schema_assumptions.py` fails readably when a revision changes the schema layout. |
| **Defects in the OMG files themselves.** Phase 0 found them in `Schema.json`, in the `OpenAPI.json` examples, in the library `.kpar` archives, and a conflict between KerML 10.3 Table 13 and its normative schema. More are likely. | Treat the normative files as data to verify, not as ground truth: schema checks, errata tables whose tests fail when OMG fixes the defect, and an ADR for each interpretation (ADRs 0005, 0006). Report the defects to OMG. |
| A dependency update raises the minimum Rust version unnoticed (it happened in Phase 0: `zip` and `uuid` needed 1.89 while 1.85 was declared) | CI builds and tests with the declared `rust-version` |
| The pilot doesn't implement merge or diff, and the normative REST schema differs from the PIM (`Query` has no `scope`/`orderBy`) | Use the normative files and the PIM document as the contract (the files take precedence). Offer the PIM-only features as `/x/` extensions. Write down every interpretation. |
| Derived Property Full Conformance is costly: commits change derived values of elements they never touched | Compute derived values on read from the incrementally maintained in-memory model, not stored per version. Fall back to Passthrough for exports from historical commits that aren't loaded. |
| Strictly monotonic commit timestamps on a busy branch | Microsecond `timestamptz`, a per-branch serialized commit path, and nudging the clock forward when two commits collide |
| Clients of the standard API bypass our draft, lock and validation flow | Apply locks and permissions to standard commit endpoints too. Validate on commit, and either reject the commit or record a validation report, as a per-project policy. |
| No standard layout interchange | Layouts stored in the database, with auto-layout as the fallback |
| **Bus factor of one.** A solo developer over 20–28 months risks burnout and stalls. | Staged releases that are each useful on their own, a public roadmap, a contributor-friendly repository (good first issues, a design-doc folder), and `CLAUDE.md` plus decision records so anyone can pick up the work |
| **Scope versus capacity.** All views and document generation in 1.0 with one developer. | Releases 0.1–0.8 deliver value early. Review scope at each release, and defer the Geometry View or the DOCX output if the schedule slips. |
| Plausible but wrong code from Claude Code, especially for spec semantics | Spec-first tests (corpus, API Annex A, implied-relationship tables) as the oracle, cited spec clauses in code comments, and mandatory review. In Phase 0, review caught an important defect in every reviewed PR even though the tests passed (§8). |
| Compliance claims (FIPS, NIST mappings) that go beyond what an open-source project can certify | Ship controls and evidence, never claim authorization, and document the operator's responsibilities |
| Redistribution restrictions on the OMG documents and machine-readable files | `docs/specs/` is never committed. Specs are fetched from upstream with checksums. Shipped libraries come from the EPL-2.0 SysML-v2-Release repository, with attribution (§13). |
| Upstream URLs move or change content | The checksum manifest detects changes. A CI artifact cache keeps the last good copy. |

## 10. Open decisions for the project owner

**Decided** (2026-11-19): see §1. Each of these is the owner's decision or the default the owner accepted.

**Decided** (2026-11-20, v0.7): Phase 0's remaining items (the TypeScript side, `wasm-pack`, PostgreSQL in CI) move to Phases 1a, 2 and 3, as listed under Phase 0 in §6.

**Still open:**

| # | Question | Needed by |
|---|---|---|
| 4 | Branching workflow: personal/feature branches versus working directly on main, and whether a pull-request–style review flow is needed | Phase 4 |
| 6 | Identity provider(s) to support first | Phase 2 |
| 7 | Compliance targets: confirm ITAR and FedRAMP (Moderate or High?) and any other requirements (CMMC, DISA STIG, encryption at rest). This decides how far Phase 10 goes. | Phase 2 (design), Phase 10 (evidence) |
| 18 | External tool integrations (DOORS Next, Polarion, Jama, Teamcenter, Windchill, Jira, Simulink) and when | After 1.0 planning |
| 19 | Simulation and analysis: whether it is needed, and when | After 1.0 planning |
| 26 | Commits from outside tools that fail validation: reject them, or accept them with a report (configurable per project, but which default?) | Phase 2 |
| 27 | UI framework (React, Svelte, Solid, …) | Phase 3 |
| 28 | Diagram library (Sprotty, React Flow, JointJS, custom SVG/canvas). A spike is proposed at the start of Phase 5. | Phase 5 |
| 32 | ~~Open-source license~~ **Decided: Apache-2.0** (2026-11-19, ADR 0003) | — |
| 33 | ~~Whether spec files may be committed~~ **Decided:** nothing under `docs/specs/` is committed (2026-11-19). | — |

## 11. Reference material

**Available:**
- SysML v2.0 Part 1 and Part 2 with the metamodel, the transformation and the libraries
- KerML 1.0 with the metamodel, the interchange schema and the libraries
- The SimpleVehicleModel example

- **Systems Modeling API & Services v1.0** (formal/2026-03-04) in `docs/specs/api/omg/`:
  - the specification document
  - normative `OpenAPI.json` and `Schema.json` (REST PSM)
  - `Systems-Modeling-API.xmi` (PIM)
  - the KerML and SysML OSLC vocabularies and shapes
- The informative pilot implementation files in `docs/specs/api/`, from the pilot repository (EPL-2.0). `README-SOURCE.md` records the source commit.

**Optional:**
- The SysML v2 Pilot Implementation, to use as a behavioral reference for the language engine
- The OSLC Core 3.0 specifications, when Phase 10 starts

## 12. Development approach (solo developer plus Claude Code)

- **`CLAUDE.md` in the repository root** records:
  - the architecture and crate boundaries
  - coding conventions
  - how to run the tests and benchmarks
  - the rule that every behavior traceable to a spec cites its clause (for example, `// KerML 8.4.4.3`)
  - where the spec text lives
- **Specs as context.** The per-clause spec text and the normative files sit in the local `docs/specs/` folder, which is fetched and never committed. Claude Code can therefore load exact clauses instead of relying on memory.
- **Test-first from the spec.** For each feature:
  1. Extract the spec rules.
  2. Write failing tests: corpus cases, implied-relationship tables, Annex A API cases.
  3. Implement.
  4. Review.
- **Small, reviewable increments.** One feature branch and PR per issue. CI must pass. Use `/code-review` before merging. Keep architecture decision records in `docs/adr/` (0001–0008 so far). In practice: signed-off commits, review findings fixed in a further commit on the same PR, and a rebase merge that keeps `main` linear.
- **Generated code where possible.** Metaclass types, API types and the API client come from the normative JSON and OpenAPI files, which reduces hand-written surface area.
- **Parallel Claude Code sessions** for independent work (for example, grammar productions in one, API endpoints in another), using git worktrees, with the developer integrating.
- **Recalibration:** throughput is measured at the end of Phase 1a and each release, and §6 is updated.

## 13. Open source and repository

- **Repository:** https://github.com/brandalorian-dev/eureka-sysml (GitHub; Actions for CI; GitHub Releases plus GHCR for signed artifacts).
- **License: Apache-2.0** (owner decision, 2026-11-19; ADR 0003). The options considered are in §13.1. Contributions use a DCO sign-off.
- **`docs/specs/` is excluded from git** (owner decision, 2026-11-19). `.gitignore` contains `docs/specs/`. The specs are fetched with `scripts/fetch_specs.py` (§6, Phase 0).
- **Runtime libraries.** The product has to ship the KerML and SysML standard libraries. Their redistributable source is the **SysML-v2-Release** repository, which GitHub reports as **EPL-2.0**. At build time they are fetched into the build output (not into `docs/specs/`) and shipped unmodified with their EPL-2.0 license and attribution, in `THIRD_PARTY_NOTICES`.
- **Generated code.** The metaclass and API types are generated at build time from the fetched normative JSON and OpenAPI files, not committed. This avoids redistributing material derived from the OMG files inside our source tree. Whether generated code counts as a derivative work is a question for a lawyer; generating it at build time sidesteps it.
- **Community:** a public roadmap (GitHub Projects), a design-docs folder, "good first issue" labels, a discussions forum. Contributors sign off with a DCO (Developer Certificate of Origin).

### 13.1 License options

The license covers **our own code**: the Rust crates, the web UI, tools and documentation. Third-party material (the OMG libraries and dependencies) keeps its own license. *This is an engineering comparison, not legal advice. Get legal review before 1.0, especially for defense customers.*

| License | Type | Patent grant | What derivatives must do | Fit for this project |
|---|---|---|---|---|
| **Apache-2.0** | Permissive | **Yes**, explicit, ends if the licensee sues over patents | Keep the notices and the NOTICE file, and state changes. No obligation to share source. | **Recommended.** Least friction for defense, aerospace and corporate adopters and contributors. Standard in the Rust ecosystem. Compatible with every dependency we expect to use. |
| **MIT OR Apache-2.0** (dual) | Permissive | Through the Apache option | Licensee chooses either | The common Rust convention. Adds compatibility with GPLv2-only projects, which Apache-2.0 alone lacks. A good choice for the **library crates** if they will be published to crates.io. |
| MIT / BSD-3-Clause | Permissive | **No** explicit grant | Keep the notice | Simple, but there is no patent protection. That matters for engineering tools in patent-heavy industries. |
| **EPL-2.0** | Weak copyleft, per file/module | Yes | Changes to EPL-licensed files must be published as source **when distributed**. Separate modules may use any license. | Matches the OMG pilots, SysML-v2-Release, Sprotty and ELK. Keeps changes to our files open while allowing proprietary plugins. GPL-incompatible unless a "Secondary License" is designated. |
| MPL-2.0 | Weak copyleft, per file | Yes | Modified MPL files must be published as source when distributed | Similar to EPL. GPL-compatible by default. Less common in the MBSE/Eclipse world. |
| LGPL-3.0 | Weak copyleft, per library | Yes | Changes to the library must be shared, and users must be able to relink | Awkward for Rust (static linking) and WASM. Not recommended. |
| GPL-3.0 | Strong copyleft | Yes | The whole distributed work must be GPL, with source | Keeps everything open, but deters corporate and defense integration and proprietary plugins. Doesn't cover server-side use. |
| **AGPL-3.0** | Strong copyleft, including network use | Yes | Anyone running a **modified version as a network service** must offer its source to users | Strongest protection against closed hosted forks. **Many corporate and government policies ban AGPL**, which works against adoption in regulated environments, our main market. Enables dual-licensing business models, but only with a CLA. |

**Considerations specific to this project:**
- **Target users** are defense and aerospace organizations running self-hosted, air-gapped deployments. Their legal teams usually approve permissive licenses quickly. Copyleft (especially AGPL) often needs case-by-case review. US DoD policy treats open-source software, including GPL, as commercial software, but individual programs and contractors often restrict copyleft.
- **The product is self-hosted and not sold as SaaS**, so AGPL's main benefit (preventing closed hosted forks) matters less.
- **Plugins and integrations.** Organizations will want proprietary connectors (DOORS, PLM, in-house tools). Permissive or weak-copyleft licenses allow this. GPL/AGPL make it legally murky.
- **Dependency compatibility.** The expected dependencies are:
  - Rust crates: mostly MIT/Apache-2.0
  - Typst: Apache-2.0
  - aws-lc-rs: ISC/Apache-2.0
  - Monaco: MIT
  - Sprotty and ELK.js: EPL-2.0
  - the OMG libraries: EPL-2.0

  An **Apache-2.0** project can use and bundle all of them. Bundled EPL-2.0 components stay under EPL-2.0, and we publish their source unmodified. A GPL project would conflict with the EPL-2.0 components unless they designate a Secondary License.
- **Sustainability options.** Permissive licenses allow support contracts, hosted offerings by anyone (including us), and paid add-ons. A copyleft-plus-commercial dual license would need a **CLA** (Contributor License Agreement) instead of the lighter DCO, and that tends to deter contributors.
- **Contributions:** use a **DCO** (sign-off on each commit) with Apache-2.0. The license itself covers contributors' patent grants, so no CLA is needed.
- **Trademark:** no license grants rights to the name. A short trademark policy for "Eureka SysML" keeps forks from passing themselves off as the official project. Check the name for conflicts before the first public release.

**Recommendation:**
- **Apache-2.0** for the repository, including the server, web UI and tools.
- Optionally, **MIT OR Apache-2.0** for the crates published to crates.io, following the Rust ecosystem convention.
- If keeping changes to our code open matters more to you than adoption, **EPL-2.0** is the best copyleft choice. It aligns with the OMG and Eclipse ecosystem and still allows proprietary plugins.
