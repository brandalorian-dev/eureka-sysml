# ADR 0004: Specification files are fetched, never committed

- Status: Accepted (owner decision)
- Date: 2026-11-19

## Context

The build, the tests and Claude Code need the OMG specification documents and machine-readable files (metamodel JSON/XMI, libraries, OpenAPI). These may not be freely redistributable in a public repository.

## Decision

- `docs/specs/` is git-ignored, and CI fails if anything under it is tracked.
- `specs.lock.json` (committed) lists every required file with its pinned upstream URL (omg.org, or the GitHub repository at a fixed commit), size and SHA-256. `scripts/fetch_specs.py` downloads and verifies them.
- Code generated from spec files is generated at build time and not committed.
- Libraries shipped in releases are taken from the EPL-2.0 SysML-v2-Release repository and listed in `THIRD_PARTY_NOTICES.md`.

## Consequences

Builds need network access, or a primed cache or air-gapped mirror of `docs/specs/`, which can be checked with `fetch_specs.py verify`. Upstream changes are detected by checksum.
