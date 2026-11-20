# ADR 0002: Rust core (native + WASM), TypeScript UI, PostgreSQL

- Status: Accepted
- Date: 2026-11-19

## Context

The tool must handle models of 100k–1M elements with 10–50 concurrent users through a web application. The options compared were TypeScript only (Langium), Java with Xtext/EMF, Kotlin, C#, Python, C++/Qt, and Rust with a TypeScript UI (plan §3).

## Decision

- **The language engine, semantics and versioning are written in Rust.** They compile natively for the server and to WASM for the browser, so both always validate identically.
- **The UI is TypeScript in the browser.**
- **PostgreSQL** is the system of record. Element versions are stored as JSONB in the KerML 10.4 JSON format, following the API PIM 7.1.2 versioning model.

## Consequences

- Good performance and memory control at scale, and one engine on both sides.
- Development is slower than with TypeScript or Langium, and more of the language tooling has to be hand-built.
- Two languages in the repository: Rust and TypeScript. Python is used only for developer scripts.
