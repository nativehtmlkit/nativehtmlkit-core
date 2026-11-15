# Capabilities: exposed, internal, missing, future

| Capability | Observed status | Contract decision |
|---|---|---|
| Native host, IPC, handshake, windows and lifecycle | **Implemented wire** | Publish and stabilize after conformance |
| Document load, HTML/CSS, DOM text/classes/attributes/forms | **Implemented wire** | Prioritize initial CLI qualification |
| Advanced node queries/batches, CSS introspection, resource bundles | **Implemented wire** | Extended or diagnostic classification until fully verified |
| File dialogs, clipboard, desktop services and preferences | **Implemented wire; platform conditional** | Require explicit capability negotiation and native qualification |
| Live RGBA bitmap shared slots | **Implemented wire + markup scheme** | Experimental until cross-language memory mapping contract validated |
| Native accessibility tree and input events | **Implemented internally with snapshot/events exposed** | Preserve; qualify actual AT-SPI/UIA behavior separately |
| Renderer `render_into`, host input dispatch, low-level raster/window control | **Internal Rust APIs** | Keep private; don't accidentally expose |
| `ui4me-host render` PNG | **Host CLI diagnostic** | Not a general RPC screenshot/export API |
| Generic application result streaming to shell | **Not host RPC** | CLI launcher design, no Core protocol change for v1 |
| Reattach after backend crash | **Missing** | Not v1; future session protocol |
| Multiple authenticated clients per host | **Missing** | Not v1; future authorizations/coordination design |
| Remote HTTP/TCP, MCP endpoint, A2A | **Missing** | Future *adapters*, keep local host default |
| Bounded event replay / sequence IDs | **Missing** | v1 resync on error; future protocol enhancement |
| JS/Web APIs, browser network engine, GPU WebGL/canvas | **Not implemented / outside design** | Do not promise |
| Video/audio playback | **Not implemented** | Later separately researched media stack |
| Public machine-readable per-method request/results | **Incomplete** | Stable-contract blocker |
| Cross-repo Core/SDK integration CI | **Not implemented** | Manual workflow + native acceptance |
| Pure PowerShell transport | **No; PowerShell includes C# Add-Type** | Preserve reliable existing helper unless separately justified |

## Recommendations by stage

**P0 (foundational):** Freeze current wire inventory as a tracked baseline; complete schemas/results; rectify ambiguity; publish compatibility policy; add source drift gate and conformance runner.

**P1 (CLI foundation):** `nativehtmlkit-cli` uses qualified Core host binary and wire v1; ship `nativehtmlkit` and `nhk` command aliases; `select` pipeline acceptance on Linux and Windows.

**P2 (CLI usefulness):** Viewer, select/table, searchable forms, CSV/JSON/TSV conversion, input limits, bounded pagination and exact stdout/exit behaviors.

**P3 (SDK migration):** Python and PowerShell independent releases, pinned artifacts, compatibility tests against old and new host; gradual full names for public APIs.

**P4 (separate future proposals):** Reconnect, multi-client collaboration, MCP, headless/offscreen API, video/audio, more powerful data streaming. These are explicitly not blockers for wire v1 or CLI MVP.
