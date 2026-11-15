# NativeHTMLKit Contract Audit & Public Contract Design

This package intentionally contains **two different things**:

1. **`01-implementation/` — AS-IS:** source-derived inventory of the uploaded UI4ME Core 1.0.1 implementation. It is evidence of code paths, *not* a promise of a stable external API.
   - Includes [`01-implementation/SDK-SURFACE-COMPARISON.md`](01-implementation/SDK-SURFACE-COMPARISON.md) and [`01-implementation/sdk-source-inventory.json`](01-implementation/sdk-source-inventory.json) to distinguish Python/PowerShell public wrappers from the wire.
2. **`02-proposed-contract/` — TO-BE:** proposed architecture and draft contract for the NativeHTMLKit Core release ecosystem. All candidate schemas and IDL are **not production-certified**.

Additionally:

- `REPORT.md` — executive architecture findings and decision memo (read first).
- `03-critical-review/` — adversarial review and gap/risk register.
- `04-roadmap/` — implementation stages, compatibility gates, [release artifact contract](04-roadmap/RELEASE-ARTIFACT-CONTRACT.md), verification and local agent prompt.
- `META/` — evidence coverage and source/provenance.

**Review baseline:** 15 November 2026; user's uploaded `ui4me-core-main(6).zip`; SHA-256 `2ec944224758994253675165671ad2858c7f6bde9c7c4e80af528a833a2e2bd5`.

**Scope limitation:** static inspection of source; we did not compile the host, run native GUI tests, or validate every operation against a running host. The auto-generated structural JSON Schema was tested with Python `jsonschema` against illustrative positive/negative fixtures, not against the Rust binary. The `nativehtmlkit.dev` schema `$id` base is a **provisional identifier, not a claim of domain ownership or a live schema endpoint**.

**Manual-only policy:** No automated workflow triggers; all GitHub Actions in the new repositories must use only `workflow_dispatch`. The source snapshot still has push/PR triggers that must be removed in migration.

**Recommended placement:** Copy the audit and decisions to `nativehtmlkit-project/architecture/` and `archive/`, and the reviewed/accepted canonical contract files to `repositories/nativehtmlkit-core/protocol/` only after verification. Do not treat a draft artifact as a published stable protocol.
