# Provenance, scope, and references

## Primary source (authoritative for AS-IS)

User-provided archive: `ui4me-core-main(6).zip` (7.8 MB compressed, 1,706 zip entries, top folder `ui4me-core-main/`). SHA-256: `2ec944224758994253675165671ad2858c7f6bde9c7c4e80af528a833a2e2bd5`.

The archive does not contain `.git` metadata. Its `Cargo.toml` declares workspace version 1.0.1. Exact Git commit identity for **this specific ZIP** cannot be established from the ZIP alone; the previous live GitHub source discussed in the conversation was around the 1.0.1 qualification period, but this report does **not** assume that its commit SHA is identical to the upload.

## Specific Rust code inspected

| Subject | Relative source path |
|---|---|
| Framing, IDs, error codes, request/response parsers | `crates/ui4me-host/src/protocol.rs` |
| Listener, connection limits, reader/writer, backpressure | `crates/ui4me-host/src/transport.rs` |
| Host handshake, capability object, events, pre-auth activation | `crates/ui4me-host/src/host/mod.rs` |
| Main host method dispatcher and native service params | `crates/ui4me-host/src/host/requests.rs` |
| Document load/replace/stale IDs | `crates/ui4me-host/src/host/documents.rs` |
| 81 tagged document operations and results | `crates/ui4me-host/src/host/document_commands.rs` |
| Native document events and drop-on-size handling | `crates/ui4me-host/src/host/document_native.rs` |
| Shared live bitmap mechanics | `crates/ui4me-host/src/host/bitmap.rs` |
| Host TOML manifest, env reservations | `crates/ui4me-host/src/config.rs` |
| Host commands | `crates/ui4me-host/src/main.rs` |
| Application lifecycle and close decisions | `crates/ui4me-host/src/lifecycle.rs` |
| Backend process/stdout-stderr log capture | `crates/ui4me-host/src/backend.rs` |
| Linux spawn and process env | `crates/ui4me-host/src/platform/linux.rs` |
| Windows spawn, job object and process env | `crates/ui4me-host/src/platform/windows.rs` |
| Renderer API, supported internal method capabilities | `crates/ui4me-renderer/src/document.rs` |
| Existing `ui4me-res://` namespace | `crates/ui4me-assets/src/bundles.rs`, `uri.rs` |
| Bitmap markup scheme and internal resource loader | `crates/ui4me-renderer/src/pipeline/resources.rs` |
| Browser-independent UI hooks | `crates/ui4me-renderer/src/document/application_api.rs` |
| Python transport/launcher | `python/ui4me_core/client.py`, `application.py`, `launcher.py`, `_bootstrap.py` |
| PowerShell transport + cmdlets | `powershell/UI4ME.Core/UI4ME.Core.psm1`, `private/UI4ME.Transport.cs` |
| Existing packaging/CI | `setup.py`, `pyproject.toml`, `tools/release/build_psgallery.py`, `.github/workflows/` |

## Public standards used as design comparison only

These are **not** evidence for what the UI4ME source implements. They inform the proposed architecture:

- [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification) — request/response/notification semantics and server-specific extension codes. NativeHTMLKit's legacy wire profile is **more restrictive** than standard JSON-RPC.
- [JSON Schema specification, Draft 2020-12](https://json-schema.org/specification) — chosen vocabulary for candidate validation artifacts.
- [Semantic Versioning 2.0.0](https://semver.org/) — different host, wire-contract and SDK version concerns.
- [GitHub Docs: manually running workflows](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow) — `workflow_dispatch` contract.

## Verification actually performed for this report

- ZIP inspected and extracted without executing uploaded code.
- Host method names, document command variants, two batch enums and host-emitted messages mechanically enumerated from Rust source.
- Dispatch entries cross-checked against selected Rust handler code and existing tests.
- Enumerated Python top-level public API exports and PowerShell functions exported from the module manifest, separately from the wire method list.
- Generated **seven** candidate JSON Schema files and one TypeScript IDL file.
- Verified all seven JSON Schema documents' structural validity and **12** positive/negative candidate fixture expectations using Python `jsonschema`.
- Executed manual-only GitHub Actions trigger checker against **six source workflows**; found exactly **two** automatic-trigger policy violations: `ci.yml` and `test-powershell.yaml`.

**Not performed:** Cargo check/test/Clippy; running Rust host, UI render, Wayland UI, Windows UI, PowerShell Pester, actual IPC conformance traffic, security penetration testing, asset/package reproducibility or release publication. Source tests present in the repository are not newly executed evidence.

## Known provisional artifacts

- `02-proposed-contract/schemas/*.schema.json` — envelope and input shapes, **not full output schema coverage**. `$id` URLs currently use a provisional `nativehtmlkit.dev` base and must be replaced by a real stable location when registered/published.
- `02-proposed-contract/idl/*.d.ts` — structural types, not guaranteed executable TypeScript or final public SDK surface.
- `02-proposed-contract/examples/sdk-handshake-and-window.json` — illustrative order and shortened responses, not exact conformance trace.
- `01-implementation/document-result-evidence.md` — source excerpts, not proof of semantic stability.
- This package contains **no actual modified Core source** and does not constitute a stable protocol release.
