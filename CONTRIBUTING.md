# Contributing to Eureka SysML

Thank you for your interest! The project is in early development. See the [project plan](docs/proj-mgmt/PROJECT_PLAN.md) for the roadmap.

## Ground rules

- **License:** contributions are accepted under the [Apache License 2.0](LICENSE).
- **Developer Certificate of Origin (DCO):** every commit must be signed off (`git commit -s`). The sign-off certifies that you wrote the change, or otherwise have the right to submit it, as described at https://developercertificate.org/. No CLA is required.
- **Specifications are not committed.** Never add files under `docs/specs/`. Fetch them with `python scripts/fetch_specs.py fetch`.
- **No copied pilot code.** Do not copy code from the OMG pilot implementations or from other projects with incompatible licenses.
- **Cite the spec.** Behavior that implements a spec rule must reference its clause in a comment, for example `// KerML 8.4.4.3`.

## Workflow

1. Open or pick an issue. For larger changes, discuss the design first (or add an ADR in `docs/adr/`).
2. Create a branch, write tests first, then implement.
3. Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` locally.
4. Open a pull request using the template. CI must pass.

## AI-assisted contributions

AI coding assistants are welcome, and the maintainer uses Claude Code. You are responsible for every line you submit: review it, test it, and make sure it matches the spec. See [CLAUDE.md](CLAUDE.md) for the project rules these tools should follow.

## Code of conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).
