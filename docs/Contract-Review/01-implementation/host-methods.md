# Implemented host method inventory — source only

**Evidence:** names extracted from Rust dispatch matches in `crates/ui4me-host/src/host/{requests.rs,documents.rs,bitmap.rs,mod.rs}`. This is not a live-host verification.

**Count:** 32 host-callable method identifiers, including `host.hello` and the special activation path.

| Wire method | Implemented source | Notes / stability issue |
|---|---|---|
| `app.activate` | `crates/ui4me-host/src/host/mod.rs:732` | Special pre-auth second-instance request; forwards activation payload to running backend, bounded at 32 KiB. Not general multi-client control. |
| `app.force_quit` | `crates/ui4me-host/src/host/requests.rs:263` | Force shutdown; bypasses normal cooperative decision. |
| `app.ready` | `crates/ui4me-host/src/host/requests.rs:248` | Requires at least one window; transitions Initializing → Running. |
| `app.request_quit` | `crates/ui4me-host/src/host/requests.rs:259` | Requests quit; host calls `app.before_quit` and waits for reverse response. |
| `bitmap.close` | `crates/ui4me-host/src/host/bitmap.rs:284` | Release bitmap and resource lifecycle. |
| `bitmap.create` | `crates/ui4me-host/src/host/bitmap.rs:198` | Two/three-slot memory-mapped RGBA8 protocol, file path returned; 4096x4096 per bitmap, 64 MiB per document. |
| `bitmap.present` | `crates/ui4me-host/src/host/bitmap.rs:245` | Sequence monotonic; read from mapped slot; ack after copy. |
| `clipboard.read_text` | `crates/ui4me-host/src/host/requests.rs:362` | Asynchronous transfer; text only. |
| `clipboard.write_text` | `crates/ui4me-host/src/host/requests.rs:354` | Text ≤1 MiB. |
| `desktop.open_external` | `crates/ui4me-host/src/host/requests.rs:317` | Asynchronous desktop service; OS may accept without guaranteeing presentation. |
| `desktop.paths` | `crates/ui4me-host/src/host/requests.rs:314` | App-specific standard paths; currently ignores additional params. |
| `desktop.preferences` | `crates/ui4me-host/src/host/requests.rs:316` | Observed preferences, optional/null availability; currently ignores additional params. |
| `desktop.reveal_file` | `crates/ui4me-host/src/host/requests.rs:332` | Validated absolute path and platform service, asynchronous. |
| `dialog.choose_folder` | `crates/ui4me-host/src/host/requests.rs:369` | Asynchronous folder picker; OS-dependent. |
| `dialog.open_files` | `crates/ui4me-host/src/host/requests.rs:369` | Asynchronous native file picker; one native dialog per window. |
| `dialog.save_file` | `crates/ui4me-host/src/host/requests.rs:369` | Asynchronous save file picker; no guarantee of content creation. |
| `document.command` | `crates/ui4me-host/src/host/documents.rs:201` | Nested tagged command enum. 81 variants in source; all are host-dispatched, not all recommended as frozen public surface. |
| `document.get` | `crates/ui4me-host/src/host/documents.rs:179` | Loaded document descriptor or loaded:false. |
| `document.load` | `crates/ui4me-host/src/host/documents.rs:178` | Exactly one of html or path; max 1 MiB source; optional asset_root; one doc per window. |
| `document.unload` | `crates/ui4me-host/src/host/documents.rs:190` | Document IDs become stale; unload releases document and bitmap state. |
| `host.capabilities` | `crates/ui4me-host/src/host/requests.rs:241` | Replies with platform-dependent capability object, but currently no schema/versioned taxonomy. |
| `host.diagnostics` | `crates/ui4me-host/src/host/requests.rs:242` | Diagnostics; not suitable as stable public application API by default. |
| `host.hello` | `crates/ui4me-host/src/host/mod.rs:723` | Exact protocol_version=1; 32-byte token; must precede authenticated operations. Host does not validate `client` metadata or require an ID. |
| `probe.get_text` | `crates/ui4me-host/src/host/requests.rs:305` | Diagnostic probe surface; candidates for nonstable developer-only namespace. |
| `probe.set_text` | `crates/ui4me-host/src/host/requests.rs:292` | Diagnostic probe surface; candidates for nonstable developer-only namespace. |
| `request.cancel` | `crates/ui4me-host/src/host/requests.rs:373` | Cancels pending native jobs/clipboard transfers. Does not promise cancellation of every method. |
| `window.create` | `crates/ui4me-host/src/host/requests.rs:267` | Native top-level window; Wayland denies hidden windows; logical size limits 1..8192. |
| `window.get` | `crates/ui4me-host/src/host/requests.rs:277` | Returns requested and observed state, with null for unobservable OS state. |
| `window.list` | `crates/ui4me-host/src/host/requests.rs:281` | Returns window descriptors; currently ignores extra params. |
| `window.request_attention` | `crates/ui4me-host/src/host/requests.rs:276` | Platform-dependent. Linux cannot clear attention. |
| `window.request_close` | `crates/ui4me-host/src/host/requests.rs:287` | Asynchronous close negotiation `window.before_close`. |
| `window.set` | `crates/ui4me-host/src/host/requests.rs:275` | Window patches; explicit null distinguishes removal of min/max constraints; OS may deny requested states. |

**Counting rule:** multiple alternatives on one match arm count as separate wire method names. Both pre-auth special paths count; notifications and reverse requests are listed separately.

**Caution:** `host.hello` `client` metadata currently ignored; methods that do not deserialize parameters can accept extra keys. The published contract must define whether this permissiveness remains legal.
