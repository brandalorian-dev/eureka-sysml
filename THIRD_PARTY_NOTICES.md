# Third-party notices

This file lists third-party material that is **bundled with Eureka SysML releases**, together with its license. Source dependencies that are only linked (Rust crates, npm packages) are reported per release in the SBOM (CycloneDX). Their licenses are checked against an allowlist in CI (`deny.toml`).

| Component | Source | License | How it is used |
|---|---|---|---|
| KerML standard libraries (Semantic, Data Type, Function) | https://github.com/Systems-Modeling/SysML-v2-Release | EPL-2.0 | Shipped **unmodified** in releases as the standard libraries every model uses. Not stored in this repository. They are fetched at build time. |
| SysML standard libraries (Systems Library and the domain libraries) | https://github.com/Systems-Modeling/SysML-v2-Release | EPL-2.0 | Same as above |

The EPL-2.0 source of these libraries is available at the URLs above. Each release links the exact upstream commit it used.

*This list grows as components are bundled. Each entry must be added in the same change that starts bundling the component.*
