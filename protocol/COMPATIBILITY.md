# Protocol versioning and compatibility policy

Applies to the NativeHTMLKit host wire protocol. Host releases, CLI releases and SDK releases are
versioned independently of the protocol version.

## Protocol version 1

- **Status: public-stable since host 1.0.3** (2026-11-16). Every rule below is binding
  for all later 1.x hosts; the behaviours host 1.0.3 introduced (protocol `README.md` §3, §7, §8,
  §10: id-correlated `document.load` responses, output backpressure, deferred first presentation,
  initial placement, the Windows bitmap directory, backend-output relay) are part of the stable
  contract.
- `protocol_version` is the integer in `host.hello`. Hosts in the 1.x line speak exactly `1`.
- **Never changed within v1:** framing (§2 of `README.md`), envelope rules (§3), authentication and
  launch variables (`UI4ME_ENDPOINT`, `UI4ME_TOKEN`, `UI4ME_APP_ID`, `UI4ME_PROTOCOL_VERSION`),
  error codes and their meaning, existing method/op names, existing parameter names and types,
  existing result members, lifecycle ordering and the `{allow}` decision contract, and the legacy
  markup/resource spellings (`ui4me-res://`, `ui4me-bitmap://`, `data-ui4me-*`) and wire-visible
  identifiers (`ui4me-desktop-1.0`, `ui4me-css/1`, `ui4me-diagnostics`).
- **Allowed additive changes in a v1 host:** new methods, new document ops, new *optional*
  parameters, new result members, new notification types, new `capabilities`/`features` keys, new
  aliases (as done for `nativehtmlkit-*`). Clients MUST ignore unknown result members, unknown
  notifications and unknown capability keys, and MUST answer unknown host *requests* with an error.
- Every additive change regenerates `v1/method-matrix.json` and schemas and re-blesses
  `v1/conformance/golden.json`; a review must confirm the golden diff only *adds* cases or members.
  A removed case, a changed error code or a removed member is a breaking change.
- Quirks locked as v1 behaviour (they are documented, not bugs to silently "fix"): no `-32700`
  responses; dropped method-less frames on the authenticated connection; last-key-wins duplicates;
  parameterless ops ignoring extra members; `document.load` oversize returning `-32005`.

## Breaking changes → protocol version 2

A v2 must be negotiated (a v2 host keeps accepting `protocol_version: 1` for at least one major host
release, or the host refuses with `-32010` and the release notes say so). Candidate v2 items:
`NATIVEHTMLKIT_*` launch variables (never exported alongside the `UI4ME_*` ones, so a credential is
not duplicated into children), strict unknown-member rejection for parameterless ops, `-32700`
responses before closing, and removal of legacy spellings.

## Host / client compatibility matrix

Each consumer release records the exact host version, host SHA-256 and protocol version it was
qualified with (CLI: `manifest.json` in its archive; workspace: `nativehtmlkit-project`
compatibility record). A consumer may run with a newer 1.x host of the same protocol version, but
only the recorded host is "qualified".

## Platforms

Qualified targets: Linux x86-64 KDE Plasma Wayland, Windows x86-64 interactive desktop (Server 2022
tested). Other Unix targets compile only as best effort and are not qualified.
