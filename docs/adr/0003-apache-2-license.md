# ADR 0003: License the project under Apache-2.0

- Status: Accepted (owner decision)
- Date: 2026-11-19

## Context

The project is free and open-source software. The main users are defense and aerospace organizations running self-hosted deployments. The options compared are in plan §13.1.

## Decision

All original code and documentation are licensed under **Apache-2.0**. Contributions use the DCO (sign-off), not a CLA. Bundled third-party material keeps its own license and is listed in `THIRD_PARTY_NOTICES.md`. Dependency licenses are enforced by `deny.toml`.

## Consequences

- Legal approval is quick for corporate and government adopters. There is an explicit patent grant, and proprietary plugins and integrations are allowed.
- Others may ship closed derivatives. This is accepted.
