# NativeHTMLKit Core

**The native Rust host for browserless HTML/CSS desktop windows.** Core owns the renderer, native windows, input, accessibility plumbing, desktop services and the host protocol. SDKs and the CLI are separate clients.

- Current shipped Core version: **1.0.3**, stable wire protocol **v1**.
- Target desktops: **Linux KDE Wayland** and **Windows**. macOS is not currently a supported release target.
- No JavaScript, no network, no browser engine: your backend process (Python, PowerShell, …) owns the logic and talks to the host over a private, authenticated local channel.
- The UI libraries never import Core internals. Apps should normally use a language SDK or the CLI rather than speak the host protocol themselves.

## Where to begin

| Goal | Read |
|---|---|
| Create an application | [Python SDK](https://github.com/nativehtmlkit/nativehtmlkit-python), [PowerShell SDK](https://github.com/nativehtmlkit/nativehtmlkit-powershell), or [CLI](https://github.com/nativehtmlkit/nativehtmlkit-cli) |
| Know which HTML, CSS and form controls work | [HTML and CSS support](docs/guides/html-css-support.md) and the [generated profile](docs/reference/html-css-profile.md) |
| Know which events your app receives | [Events](docs/guides/events.md) |
| Write an SDK or another client | [Tutorial: first wire-v1 client](docs/tutorials/first-wire-client.md), [SDK authoring guide](protocol/SDK-AUTHORING.md), [examples](examples/README.md) |
| Look up a method, op or error code | [Protocol reference](protocol/README.md), [document ops](protocol/reference/document-commands.md), [wire tables](protocol/reference/wire-tables.md) |
| Understand the process, rendering and security model | [Architecture](docs/ARCHITECTURE.md) and [API surface](docs/API.md) |
| Build or test the host | [Build & verification](docs/BUILD.md) |
| Everything, by kind of document | [Documentation index](docs/README.md) |

## Known limitation

The Linux Orca select-value announcement issue is tracked as NHK-CROSS-0010. It is not described as fixed by this documentation update. Native accessibility and scaling must be requalified for the exact host binary and OS.

## Project navigation

- [Overview and tutorials](https://github.com/nativehtmlkit/nativehtmlkit) — public developer landing page.
- [Capability and platform matrix](https://github.com/nativehtmlkit/nativehtmlkit/blob/main/docs/capabilities/matrix.md) — which API works where.
- [Known limitations](https://github.com/nativehtmlkit/nativehtmlkit/blob/main/docs/capabilities/limitations.md) — known defects and qualification limits.
- Active private development work is tracked in `nativehtmlkit-project`, not in this repository's documentation.
