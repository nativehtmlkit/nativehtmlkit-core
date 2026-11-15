# Existing Python / PowerShell surface versus Core wire API

This is an **additional source-based inventory**, distinct from the Core public-wire protocol. Python and PowerShell do **not** define the canonical contract; Core does. Neither wrapper-surface count nor string occurrence is equivalent to fully verified functional coverage.

## Python

- Source: `python/ui4me_core/__init__.py`, `application.py`, `client.py`, `launcher.py`, `live_bitmap.py`.
- `__init__.__all__` exports **12** top-level objects: `Application`, `Window`, `Document`, `Node`, `StaleDocumentError`, `StartupError`, `RpcClient`, `RpcError`, `ProtocolError`, `DisconnectedError`, `LiveBitmap`, `host_binary`.
- `application.py` defines high-level classes `Application`, `Window`, `Document`, `Node` (the source inventory includes all public methods, exact lines, and other package classes).
- `client.py` includes a low-level `RpcClient` that sends raw RPC method calls. It supports more dynamic wire operations than any hard-coded high-level convenience method list.
- Python API packaging has no CPython Rust extension; Python client and Windows system integration are implemented with Python and standard-library `ctypes`. The wheel packaging process **currently** builds/bundles the Rust host; that build dependency must be removed in the new SDK repo.

## PowerShell

- Manifest: `powershell/UI4ME.Core/UI4ME.Core.psd1` explicitly exports **83** functions, and all are defined in `UI4ME.Core.psm1` in the inspected source. Complete names and source lines are in [`sdk-source-inventory.json`](sdk-source-inventory.json).
- Native transport and threading include `private/UI4ME.Transport.cs`, compiled by `Add-Type` at runtime. This **does not** require Rust compilation, but means it is not literally PowerShell-only execution.
- Exported `Invoke-UI4MERpc` and `Invoke-UI4MEDocumentCommand` expose low-level RPC and document-command escape hatches; a missing named wrapper does **not** imply that a Rust wire operation is unreachable.
- PowerShell gallery currently repackages host binaries pulled from Python release wheels. In NativeHTMLKit, both clients should consume qualified Core release artifacts directly and independently.

## RPC name occurrence comparison — cautious indicator only

| Host method | Python literal occurrences | PowerShell literal occurrences |
|---|---:|---:|
| `app.activate` | 0 | 0 |
| `app.force_quit` | 0 | 0 |
| `app.ready` | 1 | 1 |
| `app.request_quit` | 1 | 1 |
| `bitmap.close` | 2 | 2 |
| `bitmap.create` | 1 | 1 |
| `bitmap.present` | 2 | 1 |
| `clipboard.read_text` | 1 | 1 |
| `clipboard.write_text` | 1 | 1 |
| `desktop.open_external` | 1 | 1 |
| `desktop.paths` | 0 | 0 |
| `desktop.preferences` | 1 | 1 |
| `desktop.reveal_file` | 1 | 1 |
| `dialog.choose_folder` | 0 | 1 |
| `dialog.open_files` | 0 | 1 |
| `dialog.save_file` | 0 | 1 |
| `document.command` | 1 | 1 |
| `document.get` | 0 | 0 |
| `document.load` | 1 | 2 |
| `document.unload` | 1 | 1 |
| `host.capabilities` | 1 | 1 |
| `host.diagnostics` | 0 | 0 |
| `host.hello` | 1 | 1 |
| `probe.get_text` | 0 | 0 |
| `probe.set_text` | 0 | 0 |
| `request.cancel` | 1 | 1 |
| `window.create` | 1 | 1 |
| `window.get` | 1 | 1 |
| `window.list` | 0 | 1 |
| `window.request_attention` | 1 | 1 |
| `window.request_close` | 1 | 1 |
| `window.set` | 1 | 1 |

The table above counts only quoted method-name literals in the inspected language source. Zero may mean the operation is not directly named, named indirectly, invoked generically, or generated. **Do not use this table as an SDK method-support matrix.** A real matrix needs behavioral tests per method and per SDK.

## Migration implication

The public IDL should be language-neutral and designed from Core. Python and PowerShell may have different ergonomic wrappers, but must pass the **same conformance suite** for all advertised stable methods. Document direct wrapper coverage separately from direct raw RPC reachability, so there is no false feature-parity claim.
