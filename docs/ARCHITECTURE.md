# Core architecture

```mermaid
flowchart LR
  PY[Python SDK] -->|local authenticated IPC
wire v1| HOST[Native Rust host]
  PS[PowerShell SDK] -->|wire v1| HOST
  CLI[Native CLI] -->|wire v1| HOST
  HOST --> DOC[HTML parser + DOM]
  DOC --> STYLE[CSS/style and layout]
  STYLE --> RENDER[Raster/compositing]
  HOST --> OS[Wayland / Win32 integration]
  HOST --> A11Y[Accessibility / native dialogs]
```

The native host owns window creation, document generation, layout, hit testing and device integration. Backends communicate over local authenticated transport using the versioned public protocol. A document handle becomes stale after unload or replacement. Bitmap frames use a bounded file-mapping resource rather than new network transport.

The Core repository contains vendored and patched third-party crates; changing them requires upstream/provenance review, not an SDK-level workaround. Accessibility and multi-monitor behavior are operating-system integration responsibilities and need interactive native tests.
