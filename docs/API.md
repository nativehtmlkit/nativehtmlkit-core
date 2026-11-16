# Core API

NativeHTMLKit Core has exactly one public programming interface: the **wire-v1 host protocol**
spoken between `nativehtmlkit-host` and a backend process. Everything else in this repository — the
Rust crates, their modules and types — is private implementation and may change in any release.
Consumers (SDKs, the CLI, independent clients) must not link to or depend on Core's Rust code.

## Public surfaces

| Surface | Stability | Reference |
|---|---|---|
| Wire protocol v1 (methods, notifications, `document.command` ops, error codes, framing, launch variables) | Stable since 1.0.3; additive changes only within v1 | [Protocol reference](../protocol/README.md), [generated wire tables](../protocol/reference/wire-tables.md), [op reference](../protocol/reference/document-commands.md), [`protocol/v1/`](../protocol/v1/) |
| Application manifest (`app.toml`) and `nativehtmlkit-host run` | Stable with v1 | [Protocol reference §1](../protocol/README.md#1-launch-and-transport) |
| Hosted HTML/CSS profile `ui4me-desktop-1.0` | Versioned by its id; query it with `css_profile` | [HTML and CSS support](guides/html-css-support.md), [generated profile](reference/html-css-profile.md) |
| Event types and fields | Part of v1; new types and members may be added | [Events guide](guides/events.md) |
| Markup attributes `data-nativehtmlkit-{drag,no-drag,resize,drop}` and URL schemes `nativehtmlkit-res://`, `nativehtmlkit-bitmap://` (legacy `ui4me-*` spellings stay valid) | Part of v1 | [Protocol reference §9](../protocol/README.md#9-markup-and-resource-names) |
| `nativehtmlkit-host render`, `doctor`, `probe`, `--version --verbose` | Diagnostic tools; output may gain members | [Protocol reference §1](../protocol/README.md#1-launch-and-transport) |
| Host stderr log records, `NATIVEHTMLKIT_DIAGNOSTICS` | Diagnostics, **not** an API | [Protocol reference §10](../protocol/README.md#10-host-diagnostics-output-not-part-of-the-wire-contract) |

## Domains of the protocol

| Domain | Methods and ops | Typical SDK feature |
|---|---|---|
| Session | `host.hello`, `host.capabilities`, `host.diagnostics` | connect, capability checks, support bundles |
| Application lifecycle | `app.ready`, `app.request_quit`, `app.force_quit`; host requests `app.before_quit`; notifications `app.shutdown`, `app.activated` | `run()`, quit confirmation, single-instance hand-off |
| Windows | `window.create`, `window.get`, `window.list`, `window.set`, `window.request_attention`, `window.request_close`; host request `window.before_close`; `window.event` | window objects, close confirmation |
| Documents | `document.load`, `document.get`, `document.unload`, `document.command` (81 ops); `document.event`, `document.error` | DOM-like document API, event handlers |
| Live pixels | `bitmap.create`, `bitmap.present`, `bitmap.close` | live bitmap / canvas-like widgets |
| Desktop services | `dialog.*`, `clipboard.*`, `desktop.open_external`, `desktop.reveal_file`, `desktop.paths`, `desktop.preferences`; `desktop.preferences_changed` | file pickers, clipboard, open in browser, app data folders, theme following |
| Cancellation | `request.cancel`; `request.cancelled` | cancellable async calls |
| Diagnostics | `css_profile`, `explain`, `get_diagnostics`, `get_accessibility_tree`, `get_computed_style`, `get_resource_stats` ops | developer tools, automated accessibility tests |

## Choosing an entry point

- **Building an app:** use the [Python SDK](https://github.com/nativehtmlkit/nativehtmlkit-python),
  the [PowerShell SDK](https://github.com/nativehtmlkit/nativehtmlkit-powershell) or the
  [CLI](https://github.com/nativehtmlkit/nativehtmlkit-cli). They implement the protocol, pin and
  verify the host binary and bind its lifetime correctly.
- **Writing a new SDK or client:** start with the
  [tutorial](tutorials/first-wire-client.md), then follow the
  [SDK authoring guide](../protocol/SDK-AUTHORING.md).
- **Changing the protocol:** new wire operations or semantics need an approved Core feature, an
  additive change that regenerates `protocol/v1/` (`scripts/generate-protocol-contract.py`) and
  re-blesses the golden, and conformance tests in every consumer. See
  [`COMPATIBILITY.md`](../protocol/COMPATIBILITY.md).
