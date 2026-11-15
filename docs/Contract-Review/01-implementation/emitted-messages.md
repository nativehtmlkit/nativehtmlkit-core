# Host → backend messages: actual emitters

| Message | Kind | Source | Contract significance |
|---|---|---|
| `app.activated` | notification | `crates/ui4me-host/src/host/mod.rs:754` | Special secondary-launch activation; treat forwarded args as untrusted. |
| `app.before_quit` | reverse_request | `crates/ui4me-host/src/host/mod.rs:987` | Backend must reply promptly; close/quit can be cancelled. |
| `app.shutdown` | notification | `crates/ui4me-host/src/host/mod.rs:1002` | Cooperative shutdown with deadline_ms. |
| `desktop.preferences_changed` | notification | `crates/ui4me-host/src/host/document_native.rs:197` | Preferences changed; values may remain unknown/null. |
| `document.error` | notification | `crates/ui4me-host/src/host/native.rs:571` | Async document/resource failures including oversized event notification. |
| `document.event` | notification | `crates/ui4me-host/src/host/document_native.rs:240` | Event payload produced by renderer; can be suppressed if encoded size exceeds frame limit. |
| `request.cancelled` | notification | `crates/ui4me-host/src/host/mod.rs:982` | Superseded pending close decision; late responses must be ignored. |
| `window.before_close` | reverse_request | `crates/ui4me-host/src/host/mod.rs:944` | Backend must reply `{"allow":boolean}`. |
| `window.event` | notification | `crates/ui4me-host/src/host/mod.rs:481` | Window lifecycle, focus/size/input notifications; platform dependent. |

**Reverse requests are not notifications.** `window.before_close` and `app.before_quit` carry request IDs and require responses from the SDK. A client that only implements outgoing requests is incomplete.

Events are not guaranteed durable or replayable. The current host sends over bounded queues and may drop oversized notifications or disconnect slow clients. The proposal must define the correct client recovery behavior.
