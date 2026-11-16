# Events: what the host tells your application

Everything the user does in a NativeHTMLKit window is handled by the native host first; the host then
reports what happened to the backend as **notifications**. This page lists every notification and
event type that Core 1.0.3 sends, with their fields. It is written for both audiences:

- **App developers** receive these events through an SDK's event registration API (see the
  [Python](https://github.com/nativehtmlkit/nativehtmlkit-python) or
  [PowerShell](https://github.com/nativehtmlkit/nativehtmlkit-powershell) SDK reference for the exact
  calls). The `type` names and fields below are what the SDK hands you.
- **Client authors** receive them as raw wire-v1 frames ([protocol reference §8](../../protocol/README.md#8-lifecycle-host-requests-and-notifications)).

Sources: the host's dispatch code and the renderer's event serializer in this repository, plus
frames captured from a headless 1.0.3 host. Events that need real input (pointer, keyboard, IME,
native drops) cannot be produced in headless mode and were not captured there; their field lists
come from the source.

## Notifications at a glance

| Method | Params | Sent when |
|---|---|---|
| `document.event` | `{window_id, document_id, event}` | a DOM event fired in a document (user input, or a command such as `focus`, `request_submit`, `show_dialog`) |
| `document.error` | `{window_id, document_id, message, fatal?}` | a document-level problem the backend should know about (an event too large for one frame, a failed native drop, a renderer failure — `fatal: true` means the document was removed) |
| `window.event` | `{window_id, type, data}` | native window changes (below) |
| `app.shutdown` | `{deadline_ms}` | the application is ending; exit before the deadline |
| `app.activated` | `{args, cwd, activation_token}` | the user launched a `single`-instance app again; `args` is the second launch's command line |
| `desktop.preferences_changed` | preferences snapshot | the desktop colour scheme, reduced-motion or high-contrast setting changed |
| `request.cancelled` | `{id, reason}` | a pending host decision (`window.before_close`) was superseded, e.g. by an app-wide quit |

Two **host requests** also need an answer (`{"allow": true|false}`): `window.before_close
{window_id, reason}` and `app.before_quit {reason}`. SDKs expose them as close/quit handlers.

Clients must ignore notification types and members they do not know: new ones may be added within v1.

## `document.event`

### Common members

Every DOM event object carries:

| Member | Meaning |
|---|---|
| `type` | Event type (tables below). |
| `element` | `id` of the nearest element with an id on the event path (the target itself if it has one); `""` if none. |
| `path` | Ids of the identified elements on the propagation path, innermost first (only the first one for events that do not bubble, such as `focus`). |
| `node` | Generation-checked node reference of the actual target (usable with `node`-addressed ops). |
| `detail` | Click count for `click`/`dblclick`, else 0. |
| `cancelable` | Whether the event has a default action. |
| `default_prevented` | Present when cancelable: whether a `set_default_action` rule prevented it. |
| `shift`, `ctrl`, `alt`, `meta` | Modifier state. |

**Match targets on `element` *or* `path`.** If a click lands on `<span id="icon">` inside
`<button id="save">`, `element` is `"icon"` and `path` is `["icon", "save", …]`.
Members whose names start with `_` (for example `_target`, `_element`) are internal bookkeeping:
ignore them; they are not part of the contract.

Depending on the target and type, these members are added:

| Member | When |
|---|---|
| `value` | target is a form control (text of `input`/`textarea`, value of `select`) |
| `checked` | target is a checkbox or radio |
| `selection {start, end, direction}` | target is a text control with a selection |
| `files` | target is an `<input type=file>` with chosen files |
| `related_element`, `related_node` | focus and mouse over/out/enter/leave events |
| `key`, `code`, `repeat`, `is_composing` | keyboard events |
| `data`, `input_type` | `beforeinput`, `input`, composition events |
| `x`, `y`, `button`, `buttons` | pointer, mouse and wheel events (viewport CSS px) |
| `pointer_id`, `pointer_type`, `is_primary`, `pressure` | pointer events |
| `delta_x`, `delta_y` | `wheel` |
| `old_state`, `new_state` | `toggle`, `beforetoggle` (`"open"`/`"closed"`) |
| `submitter` | `submit` |
| `validity` | `invalid` (same shape as `get_validity`) |
| `return_value` | `close` of a `<dialog>` |
| `property_name`, `pseudo_element`, `elapsed_time` | transition events |
| `animation_name`, `pseudo_element`, `elapsed_time` | animation events |

### DOM event types

| Group | Types |
|---|---|
| Pointer and mouse | `click`, `dblclick`, `auxclick`, `contextmenu`, `mousedown`, `mouseup`, `mousemove`, `mouseover`, `mouseout`, `mouseenter`, `mouseleave`, `pointerdown`, `pointerup`, `pointermove`, `pointercancel`, `gotpointercapture`, `lostpointercapture`, `wheel` |
| Scrolling | `scroll` |
| Keyboard and text editing | `keydown`, `keyup`, `beforeinput`, `input`, `change`, `select`, `selectionchange`, `copy`, `cut`, `paste` |
| IME composition | `compositionstart`, `compositionupdate`, `compositionend` |
| Focus | `focus`, `blur`, `focusin`, `focusout` |
| Forms | `submit`, `reset`, `invalid` |
| Dialogs, popovers, `<details>` | `toggle`, `beforetoggle`, `cancel` (Escape on a modal dialog), `close` |
| CSS transitions | `transitionrun`, `transitionstart`, `transitionend`, `transitioncancel` |
| CSS animations | `animationstart`, `animationiteration`, `animationend`, `animationcancel` |

### Host-generated event types

These are not DOM events; they have their own members:

| Type | Members | Meaning |
|---|---|---|
| `link_activated` | `element`, `href`, `url` (resolved, or `null`), `node` | the user activated an `<a href>`. **The host never navigates**; open it yourself (for example `desktop.open_external`) if you want to. |
| `dragstart` | `element` (source id), `source`, `x`, `y` | in-page drag of an element with `draggable="true"` began (after 4 CSS px of movement) |
| `dragover` | `element` (target id), `source`, `x`, `y` | the element under the pointer changed during the drag |
| `drop` | `element` (target), `source` | released over a target |
| `dragend` | `element`, `source`, `dropped`, `target` | always last; Escape or focus loss cancels |
| `drag_feedback` | `phase`, `accepted`, `kind` (`files`/`text`), `element`, `node`, `x`, `y`, `external: true` | a drag **from another application** entered, moved over a new target or left (transitions only) |
| `drop` (external) | `element`, `node`, `x`, `y`, `paths` **or** `text`, `external: true` | files (≤1024 local paths) or text dropped from another application onto an element that opted in with `data-nativehtmlkit-drop="files text"` |
| `environment_changed` | `environment` (as `get_environment`) | the desktop preferences changed what the document's media queries see |

### Default actions

Events reach the backend **after** the host has already acted on the input, so a handler cannot
cancel anything. To stop a default action (a shortcut key, form submission, the context menu, a
dialog's Escape `cancel`, a popover toggle), register a rule beforehand with the
`set_default_action` op; the SDKs wrap it. The event then arrives
with `default_prevented: true`. Example, captured from a headless host:

```text
→ document.command {op: "set_default_action", element: "f", event: "submit"}   ← {updated: true}
→ document.command {op: "request_submit", element: "f"}
← document.event {type: "submit", element: "f", path: ["f"], cancelable: true, default_prevented: true, …}
← {submitted: true, default_prevented: true}
```

### What a command emits

Commands that behave like user actions emit the matching events before their response; plain state
setters do not. Captured from Core 1.0.3:

| Command | Events (in order) |
|---|---|
| `focus {element: "i"}` | `focus`, `focusin` (and `blur`, `focusout` on the previously focused element) |
| `request_submit` on a valid form | `submit` |
| `request_submit` with an empty `required` field | `invalid` (with `validity`), then `focus`, `focusin` on that control |
| `reset_form` | `reset` |
| `check_validity` with failures | `invalid` per failing control |
| `show_dialog {modal: true}` | `beforetoggle`, focus changes, `toggle` |
| `close_dialog {return_value: "ok"}` | `beforetoggle`, `close` (with `return_value`), `toggle` |
| `show_popover` / `hide_popover` / `toggle_popover` | `beforetoggle`, `toggle` |
| `set_attribute {name: "open"}` on `<details>` | `toggle` |
| `scroll_to` that moves | `scroll` |
| `set_control_selection` | `select`, `selectionchange` |
| `replace_control_range` | `beforeinput`, `input`, `selectionchange` |
| `edit_command {command: "copy"}` | `copy` |
| `select_text` / `clear_selection` | `selectionchange` |
| `set_value`, `set_checked`, `set_selected_values`, `set_text`, `set_environment` | none |

A typical captured frame:

```json
{"jsonrpc":"2.0","method":"document.event","params":{"window_id":"w1","document_id":"d1",
 "event":{"type":"invalid","element":"note","path":["note"],"node":"node:1:13:1","detail":0,
  "cancelable":true,"default_prevented":false,"value":"","shift":false,"ctrl":false,"alt":false,
  "meta":false,"validity":{"valid":false,"flags":{"valueMissing":true,…},"message":"…"}}}}
```

### Delivery guarantees

- Events of a document are delivered in order. Events of a document that has since been replaced or
  unloaded are dropped, so `document_id` in an event always names the document that produced it.
- A document queues at most 1024 undelivered events; an event that would not fit in one 2 MiB frame
  is replaced by a `document.error`.
- If the backend stops reading, the host drops `document.event` and `window.event` notifications
  (never responses or host requests) and counts them in `host.diagnostics.dropped_notifications`.
  Keep reading while you work, or move slow work off the reading thread.

## `window.event`

`{window_id, type, data}`. Types sent by Core 1.0.3:

| `type` | `data` | When |
|---|---|---|
| `close_requested` | `{reason}` | a close was requested (title-bar button, OS, or `window.request_close` with `reason: "backend_request"`); a `window.before_close` decision follows after `app.ready` |
| `closed` | `{}` | the window was destroyed (not sent for the last window when its close starts shutdown) |
| `focused` | `{focused}` | the native window gained or lost keyboard focus |
| `resized` | `{physical_width, physical_height, scale_factor}` | the surface size changed |
| `scale_changed` | `{scale_factor}` | the display scale changed (moved to another monitor, settings) |
| `key` | `{key, state: "pressed"/"released", repeat, physical_key}` | raw native key event (DOM keyboard events are sent as well when a document is loaded) |
| `dropped_files` / `dropped_text` / `text` | `{paths}` / `{text}` / `{text, cursor, anchor}` | only for the built-in probe surface of a window **without** a document |

None of these are produced in headless mode except `close_requested` and `closed` (from
`window.request_close`).

## Application lifecycle

```text
backend                                   host
  host.hello ────────────────────────────▶
  window.create / document.load ─────────▶
  app.ready ─────────────────────────────▶          phase: running
           ◀──────────────────────────── document.event / window.event …
  app.request_quit ──────────────────────▶          (or the user closes the last window)
           ◀──────────────────────────── app.before_quit {reason}   (request)
  {"allow": true} ───────────────────────▶
           ◀──────────────────────────── app.shutdown {deadline_ms}
  exit before deadline_ms
```

- `app.before_quit` reasons: `backend_request` (from `app.request_quit`) and `last_window` (the user
  closed the last window). Answer `{"allow": false}` to keep running, e.g. to ask about unsaved work.
- `window.before_close` reasons: `backend_request` (`window.request_close`) and `native_close` (the
  title-bar button or another OS close request).
- There is no host-side deadline for these answers; always answer promptly.
- `app.shutdown` makes every window and document handle stale. It is also sent when startup is
  cancelled (the user closed the window before `app.ready`).
- `app.force_quit` from the backend, or Ctrl+Shift+Q pressed by the user in a document window, ends
  the session without asking; the backend is killed and the host exits with status 1.
- A second launch of a `single`-instance app arrives as `app.activated`; the first instance's window
  is focused where the OS allows it.
