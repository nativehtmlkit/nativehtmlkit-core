# NativeHTMLKit Core

**The native Rust host for browserless HTML/CSS desktop windows.** Core owns the renderer, native windows, input, accessibility plumbing, desktop services and the host protocol. SDKs and the CLI are separate clients.

- Current shipped Core version: **1.0.3**, stable wire protocol **v1**.
- Target desktops: **Linux KDE Wayland** and **Windows**. macOS is not currently a supported release target.
- The UI libraries never import Core internals. Apps should normally use a language SDK or the CLI rather than speak the host protocol themselves.

## Where to begin

| Goal | Read |
|---|---|
| Create an application | [Python SDK](https://github.com/nativehtmlkit/nativehtmlkit-python), [PowerShell SDK](https://github.com/nativehtmlkit/nativehtmlkit-powershell), or [CLI](https://github.com/nativehtmlkit/nativehtmlkit-cli) |
| Understand the process and data flow | [Architecture](docs/ARCHITECTURE.md) |
| Explore wire methods and compatibility | [Protocol](protocol/README.md) and [API ownership](docs/API.md) |
| Build or test the host | [Build & verification](docs/BUILD.md) |
| Explore demos and capabilities | [Public documentation](https://github.com/nativehtmlkit/nativehtmlkit) |

## Known limitation

The Linux Orca select-value announcement issue is tracked as NHK-CROSS-0010. It is not described as fixed by this documentation update. Native accessibility and scaling must be requalified for the exact host binary and OS.

## Project navigation

- [Overview and tutorials](https://github.com/nativehtmlkit/nativehtmlkit) — public developer landing page.
- [Capability and platform matrix](https://github.com/nativehtmlkit/nativehtmlkit/blob/main/docs/capabilities/matrix.md) — which API works where.
- [Known limitations](https://github.com/nativehtmlkit/nativehtmlkit/blob/main/docs/capabilities/limitations.md) — known defects and qualification limits.
- Active private development work is tracked in `nativehtmlkit-project`, not in this repository's documentation.
