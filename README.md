# NativeHTMLKit Core

Rust-native HTML/CSS desktop engine, local native host, and the future **public, language-independent protocol contract**.

**Status: migration placeholder. No Rust Core source is present here yet.** The original tested implementation remains at [ui4me/ui4me-core](https://github.com/ui4me/ui4me-core). Its current working code must be audited again when migration begins; the research snapshot here is historical.

## Ownership and proposed structure

| Folder | Responsibility | Current state |
| --- | --- | --- |
| `crates/` | Rust workspace, renderer, host, document, assets, types | Placeholder only |
| `protocol/` | Canonical versioned protocol, IDL/JSON Schema and SDK conformance fixtures | Placeholder; not frozen |
| `release/` | Independent native binaries, SHA-256/provenance/licenses, manual workflows | Placeholder only |
| `tests/` | Rust/protocol integration and Linux Wayland/interactive Windows GUI verification | Placeholder only |
| `docs/Contract-Review/` | **Historical source audit** and **proposed** public contract, risks and roadmap | Research snapshot included |

Start at [Contract Review report](docs/Contract-Review/REPORT.md), [source-audit inventory](docs/Contract-Review/01-implementation/IMPLEMENTATION-AUDIT.md), and [adversarial review](docs/Contract-Review/03-critical-review/adversarial-review.md). **Proposed schemas and methods are not yet certified against a migrated host.**

The Core release should publish qualified host binaries with checksums and a tested public wire contract. CLI, Python and PowerShell SDKs should consume that release independently; Core must not depend on their source.

See [Core agent rules](AGENTS.md). Global policy resides in the private Project repository `nativehtmlkit-project`; external contributors may work with this repository's self-contained instructions.
