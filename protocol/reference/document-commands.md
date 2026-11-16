# `document.command` op reference (wire v1)

A `document.command` request runs one op on one document:

```json
{"jsonrpc":"2.0","id":"c7","method":"document.command",
 "params":{"window_id":"w1","document_id":"d1","command":{"op":"set_text","element":"status","text":"Saved"}}}
```

This page explains what each of the 81 ops does and what it returns. Exact field types, defaults
and observed result members are generated in the [wire tables](wire-tables.md); the JSON Schema
[`../v1/schemas/document-command.schema.json`](../v1/schemas/document-command.schema.json) is
authoritative. Result examples below were captured from a headless Core 1.0.3 host.

## Addressing elements and nodes

- **`element`** is the value of an element's `id` attribute. Ids must be unique and contain no
  whitespace (≤256 bytes); a document violating that does not load. An unknown id is `-32602`
  (`Unknown element ID: …`).
- **`node`** is an opaque, generation-checked reference such as `"node:1:13:1"`, returned by
  `query_nodes`, `node_of`, `get_node`, `node_children`, `element_at_point`, `get_focused_element`
  and by every `document.event` (`event.node`). It lets you address elements without an `id`. A node
  that was removed is never reused: using it fails with `-32004` (or a `stale_reference` answer from
  `explain`). Do not parse node references.
- Ops that accept `element` **or** `node` take exactly one of them.
- Mutating ops answer `{"updated": true}` unless stated otherwise.
- Ops may emit `document.event` notifications *before* their response (for example `focus` emits
  `focus`/`focusin`, `request_submit` emits `submit` or `invalid`). Programmatic value changes
  (`set_value`, `set_checked`, `set_selected_values`) do **not** emit `input`/`change`.
- Text arguments are limited to 16 KiB per update (`set_text`, `set_value`, `append_text`);
  inserted HTML and whole documents to 1 MiB; a document to 65,536 nodes and nesting depth 256.
- A result larger than one 2 MiB frame fails with `-32002`; page with `offset`/`limit`.

## DOM content

| Op | What it does | Result |
|---|---|---|
| `set_text {element, text}` | Replace the element's children with one text node. Text is never parsed as markup. Not for form controls (use `set_value`). | `updated` |
| `append_text {element, text}` | Append text (extends the last text node while small, so streaming logs stay cheap). | `updated` |
| `get_text {element}` | Text content. | `{text}` |
| `get_html {element? \| node?, outer?, offset?, limit?}` | Serialized inner (or outer) HTML of an element, or of the whole document when neither is given; paged by byte `offset`/`limit`. | `{html, offset, next_offset, total_bytes, truncated}` |
| `insert_html {element, position, html}` | Parse trusted HTML and insert it at `beforebegin`, `afterbegin`, `beforeend` or `afterend`. The fragment is validated like a document (no scripts, unique ids). **Escape untrusted text.** | `updated` |
| `replace_children_html {element, html}` | Replace the children with parsed HTML. | `updated` |
| `remove_element {element}` | Remove an element (not `html`, `head`, `body`). | `updated` |
| `move_element {element, target, position}` | Move an element relative to `target` (same positions). | `updated` |
| `trim_children {element, keep, from?}` | Keep only `keep` children; `from: "start"` (default) removes the oldest children first, `"end"` the newest. Typical for bounded logs. | `updated` |
| `batch {ops}` | Apply up to 1024 `BatchOp`s (`set_text`, `set_attribute`, `remove_attribute`, `add_class`, `remove_class`, `insert_html`, `replace_children_html`, `remove_element`, `move_element`, `append_text`, `trim_children`) as one update. | `updated` |

## Attributes and classes

| Op | What it does | Result |
|---|---|---|
| `get_attribute {element, name}` | One attribute. | `{value}` |
| `get_attributes {element}` | All attributes. | `{attributes: {name: value}}` |
| `set_attribute {element, name, value}` / `remove_attribute {element, name}` | Change an attribute; `on*` and `srcdoc` are rejected. Setting `open` on `<details>` emits `toggle`. | `updated` |
| `add_class` / `remove_class {element, name}` | Edit `class`. | `updated` |
| `has_element {element}` | Whether an id exists. | `{exists}` |

## Query and node handles

| Op | What it does | Result |
|---|---|---|
| `query {selector, offset?, limit?}` | CSS selector query returning only matches **that have an id**. | `{ids, total, matched_nodes, offset, limit, truncated}` |
| `query_nodes {selector, offset?, limit?}` | Selector query returning node references for every match (limit default 1024). | `{nodes: [{node, element}], total, offset}` |
| `node_of {element?}` | Node reference of an id (or of the document). | `{node}` |
| `get_node {node}` | Describe a node: `type` (`element`, `text`, …), `name`, `id`, `attributes`, `text`, `parent`, `previous`, `next`, `child_count`. | object |
| `node_children {node, offset?, limit?}` | Child references (limit default 256, max 4096). | `{children, total}` |
| `node_batch {ops}` | Like `batch`, but every op addresses a `node` (`MoveElement` uses `target_node`). Stale nodes fail the batch. | `updated` |
| `element_at_point {x, y}` | Hit test in viewport CSS px (`null` outside the viewport). | `{node, element}` or `null` |
| `text_at_point {x, y}` | Text position under a point. | `{hit: {block, element, offset}}` |

## Form controls and validation

| Op | What it does | Result |
|---|---|---|
| `get_value` / `set_value {element, value?}` | Value of `input`, `textarea`, `select`. | `{value}` / `updated` |
| `get_checked` / `set_checked {element, checked?}` | Checkbox/radio state. | `{checked}` / `updated` |
| `get_options {element}` | Options of a `select`. | `{options: [{value, label, selected, disabled}]}` |
| `set_selected_values {element, values}` | Select options by value (several for `multiple`). | `updated` |
| `get_files {element}` | Files chosen in an `<input type=file>` (paths only, never contents). | `{files}` |
| `get_form_data {element \| node, submitter?, submitter_node?}` | Form entries in document order, as a submission would build them. | `{entries: [{name, kind, value}], submitter}` (file entries carry `path`/`filename`) |
| `request_submit {element \| node, submitter?, submitter_node?}` | Run constraint validation; if valid fire a cancelable `submit` event, otherwise `invalid` events and focus the first invalid control. Never navigates. | `{submitted, default_prevented}` |
| `reset_form {element \| node}` | Fire `reset` and restore defaults. | `{reset, default_prevented}` |
| `check_validity {element}` | Validate a control or a whole form; fires `invalid` on failures. | `{valid, count, invalid: [ids]}` |
| `get_validity {element}` | Validity state with the standard flags (`valueMissing`, `typeMismatch`, `patternMismatch`, `tooLong`, `tooShort`, `rangeUnderflow`, `rangeOverflow`, `stepMismatch`, `badInput`, `customError`) and an English `message`. | `{valid, will_validate, flags, message}` |
| `set_custom_validity {element \| node, message}` | Set (non-empty) or clear (`""`) a custom error. | validity state |

## Focus, selection and editing

| Op | What it does | Result |
|---|---|---|
| `focus {element \| node}` / `blur {element? \| node?}` | Move focus (emits `focus`/`focusin`, and `blur`/`focusout` on the old element). `blur` without a target blurs the focused element. | `updated` |
| `get_focused_element` | Focused element. | `{element, node}` or `null` |
| `get_control_selection {element \| node}` | Caret/selection inside a text control (UTF-16 offsets). | `{start, end, direction, length, node}` |
| `set_control_selection {…, start, end, direction?}` | Select in a control (`forward`, `backward`, `none`); emits `select`/`selectionchange`. | selection |
| `replace_control_range {…, replacement, start?, end?, selection_mode?}` | Replace a range (default: the selection) as user editing would (emits `beforeinput`, `input`); `selection_mode` `preserve` (default), `select`, `start`, `end`. | selection + `changed` |
| `edit_command {command, element? \| node?}` | `undo`, `redo`, `cut`, `copy`, `paste`, `select_all`, `delete` on a text control (`copy`/`cut`/`paste` also on the document selection). | `{accepted, clipboard_pending}` |
| `select_text {element?}` | Select an element's text (or the whole document). | `updated` |
| `get_selection` / `copy_selection` / `clear_selection` | Document text selection (not control selections); `copy_selection` returns the text it copies. | `{text}` / `{text}` / `updated` |
| `set_pointer_capture {element, pointer_id?, capture?}` / `get_pointer_capture` | Capture the active pointer for drag interactions; only accepted while that pointer is pressed. | `{accepted}` / `{element}` |

## Dialogs and popovers

| Op | What it does | Result |
|---|---|---|
| `show_dialog {element, modal?}` | Open a `<dialog>` (modal by default: top layer, rest inert). Emits `beforetoggle`, focus events, `toggle`. | `updated` |
| `close_dialog {element, return_value?}` | Close it; emits `close` with `return_value`. | `updated` |
| `get_dialog {element}` | `{open, modal, return_value}`. | object |
| `show_popover` / `hide_popover` / `toggle_popover {element}` | Popover state of an element with `popover`. Emits `beforetoggle`/`toggle`. | `updated` / `{open}` |
| `get_popover {element}` | `{open, kind}` (`auto` or `manual`). | object |

## Scrolling and geometry

| Op | What it does | Result |
|---|---|---|
| `scroll_to {element?, x?, y?, behavior?}` | Scroll a container (or the viewport) to a position; `behavior` `auto`, `instant` or `smooth`. Emits `scroll`. | scroll state |
| `scroll_by {element?, dx?, dy?, behavior?}` | Relative scroll. | scroll state |
| `scroll_into_view {element, block?, inline?, behavior?}` | Align `start`, `center`, `end` or `nearest` (default). | `updated` |
| `get_scroll {element?}` | `{x, y, min_x, min_y, max_x, max_y, width, height, scrollable, animating}`. | object |
| `set_scroll_follow {element?, follow?}` | Keep a container pinned to its end while content is appended (log views). | `updated` |
| `get_rect {element}` | Border box in viewport CSS px. | `{rect: {x, y, width, height}}` |
| `text_rects {element? \| node?, start?, end?}` | Line boxes of an element's text. | `{text, length, rects: [{x, y, width, height, visible}]}` |

## Styles, environment and behaviour

| Op | What it does | Result |
|---|---|---|
| `add_stylesheet {css}` | Add a runtime stylesheet (≤1 MiB, ≤64 sheets). | `{sheet, diagnostics}` (parse problems such as unknown properties, with line/column) |
| `replace_stylesheet {sheet, css}` / `remove_stylesheet {sheet}` | Change a runtime sheet. | `{sheet, diagnostics}` / `updated` |
| `list_stylesheets` | Document `<style>`/`<link>` and runtime sheets. | `{sheets: [{sheet, kind, node, element, href, media, length}]}` |
| `get_computed_style {element \| node, properties}` | 1..64 property names (custom properties allowed): `{status: "supported", value, inherited}`, `{status: "unsupported", feature}` (profile-excluded) or `{status: "invalid", reason}`. | `{schema: 1, properties}` |
| `set_environment {color_scheme?, reduced_motion?}` | App override for `prefers-color-scheme` (`light`, `dark`, `system`) and `prefers-reduced-motion` (`reduce`, `no-preference`, `system`). | environment |
| `get_environment` | `{color_scheme, reduced_motion, high_contrast, scale, …_source, system_…}`; sources are `app`, `system` or `default`. | object |
| `set_default_action {element, event, key?, shift?, ctrl?, alt?, meta?, prevent?}` | Declare ahead of time that the host prevents an event's default action on that element (and descendants), optionally only for one key and modifier combination. Events: `keydown`, `click`, `dblclick`, `auxclick`, `contextmenu`, `beforeinput`, `copy`, `cut`, `paste`, `mousedown`, `pointerdown`, `wheel`, `submit`, `reset`, `cancel`, `beforetoggle`. `prevent: false` removes the rule. ≤256 rules. The delivered event then has `default_prevented: true`. | `updated` |

There is no way to prevent a default action *from an event handler*: events reach the backend after
the host handled the input. Declare rules with `set_default_action` instead.

## Resources and diagnostics

| Op | What it does | Result |
|---|---|---|
| `register_resource_bundle {name, entries: [{path, data_url}]}` | In-memory files served as `nativehtmlkit-res://<name>/<path>` (≤256 entries and 1 MiB decoded per request; at most 64 bundles and 16 MiB of registered data in total). Re-registering replaces the bundle. | `{name, replaced}` |
| `unregister_resource_bundle {name}` | Remove a bundle. | `{name, removed}` |
| `get_resource_stats` / `purge_unused_resources {url?}` | Cache, bundle and SVG raster counters / drop unreferenced cache entries. | stats / `{purged, stats}` |
| `get_accessibility_tree` | The semantic tree the native bridge publishes: nodes with `id`, `role`, `name`, `html_id`, `bounds`, `states`, `relations`, `structure`, `actions`, `children`, and the tree `focus`. | `{tree}` |
| `get_diagnostics` | Bounded engine diagnostics (`code`, `stage`, `feature`, `reason`, `message`), e.g. `UNSUPPORTED_FEATURE` for dropped CSS or `RESOURCE_DENIED` for refused URLs. | `{css_profile_version, diagnostics}` |
| `css_profile` | The CSS feature profile: `{version, features: [{id, state}]}`; see the [generated profile](../../docs/reference/html-css-profile.md). | object |
| `explain {element \| node, property?}` | Why a property has (no) effect: `status` `applied`, `not_applicable` (e.g. `display: none` ancestor, inline box, static position, not a flex/grid item), `overridden`, `unsupported` (profile), `unknown_property` or `stale_reference`, with a human `reason`. Without `property`, explains the element and its inline-style declarations. | `{schema: 1, status, reason, …}` |
