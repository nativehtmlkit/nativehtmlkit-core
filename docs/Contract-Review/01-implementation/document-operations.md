# Source-derived document operation catalogue

**Count:** 81 enum operations selected by `document.command.params.command.op`. Rust source: `crates/ui4me-host/src/host/document_commands.rs`.

The fields below are decoded with serde. `?` means omitted input is accepted (a Rust `Option<T>` or `#[serde(default)]`). Other fields are required. Dynamic validation inside the renderer still applies.

| Operation | Fields (Rust input types) | Evidence |
|---|---|---|
| `register_resource_bundle` | `name: String`, `entries: Vec<ResourceEntry>` | `crates/ui4me-host/src/host/document_commands.rs:34` |
| `unregister_resource_bundle` | `name: String` | `crates/ui4me-host/src/host/document_commands.rs:38` |
| `get_resource_stats` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:41` |
| `purge_unused_resources` | `url?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:42` |
| `element_at_point` | `x: f64`, `y: f64` | `crates/ui4me-host/src/host/document_commands.rs:46` |
| `get_focused_element` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:50` |
| `blur` | `element?: Option<String>`, `node?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:51` |
| `get_control_selection` | `element?: Option<String>`, `node?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:57` |
| `set_control_selection` | `element?: Option<String>`, `node?: Option<String>`, `start: u32`, `end: u32`, `direction?: String` | `crates/ui4me-host/src/host/document_commands.rs:63` |
| `replace_control_range` | `element?: Option<String>`, `node?: Option<String>`, `replacement: String`, `start?: Option<u32>`, `end?: Option<u32>`, `selection_mode?: String` | `crates/ui4me-host/src/host/document_commands.rs:73` |
| `edit_command` | `command: String`, `element?: Option<String>`, `node?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:86` |
| `reset_form` | `element?: Option<String>`, `node?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:93` |
| `request_submit` | `element?: Option<String>`, `node?: Option<String>`, `submitter?: Option<String>`, `submitter_node?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:99` |
| `get_form_data` | `element?: Option<String>`, `node?: Option<String>`, `submitter?: Option<String>`, `submitter_node?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:109` |
| `set_custom_validity` | `element?: Option<String>`, `node?: Option<String>`, `message: String` | `crates/ui4me-host/src/host/document_commands.rs:119` |
| `query_nodes` | `selector: String`, `offset?: usize`, `limit?: usize` | `crates/ui4me-host/src/host/document_commands.rs:126` |
| `node_batch` | `ops: Vec<NodeBatchOp>` | `crates/ui4me-host/src/host/document_commands.rs:133` |
| `set_text` | `element: String`, `text: String` | `crates/ui4me-host/src/host/document_commands.rs:136` |
| `get_value` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:140` |
| `set_value` | `element: String`, `value: String` | `crates/ui4me-host/src/host/document_commands.rs:143` |
| `get_checked` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:147` |
| `set_checked` | `element: String`, `checked: bool` | `crates/ui4me-host/src/host/document_commands.rs:150` |
| `set_attribute` | `element: String`, `name: String`, `value: String` | `crates/ui4me-host/src/host/document_commands.rs:154` |
| `remove_attribute` | `element: String`, `name: String` | `crates/ui4me-host/src/host/document_commands.rs:159` |
| `add_class` | `element: String`, `name: String` | `crates/ui4me-host/src/host/document_commands.rs:163` |
| `remove_class` | `element: String`, `name: String` | `crates/ui4me-host/src/host/document_commands.rs:167` |
| `focus` | `element?: Option<String>`, `node?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:171` |
| `replace_children_html` | `element: String`, `html: String` | `crates/ui4me-host/src/host/document_commands.rs:177` |
| `insert_html` | `element: String`, `position: String`, `html: String` | `crates/ui4me-host/src/host/document_commands.rs:181` |
| `remove_element` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:186` |
| `move_element` | `element: String`, `target: String`, `position: String` | `crates/ui4me-host/src/host/document_commands.rs:189` |
| `get_html` | `element?: Option<String>`, `node?: Option<String>`, `outer?: bool`, `offset?: usize`, `limit?: Option<usize>` | `crates/ui4me-host/src/host/document_commands.rs:194` |
| `text_rects` | `element?: Option<String>`, `node?: Option<String>`, `start?: Option<u32>`, `end?: Option<u32>` | `crates/ui4me-host/src/host/document_commands.rs:208` |
| `text_at_point` | `x: f64`, `y: f64` | `crates/ui4me-host/src/host/document_commands.rs:218` |
| `get_files` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:222` |
| `get_computed_style` | `element?: Option<String>`, `node?: Option<String>`, `properties: Vec<String>` | `crates/ui4me-host/src/host/document_commands.rs:225` |
| `add_stylesheet` | `css: String` | `crates/ui4me-host/src/host/document_commands.rs:232` |
| `replace_stylesheet` | `sheet: u64`, `css: String` | `crates/ui4me-host/src/host/document_commands.rs:235` |
| `remove_stylesheet` | `sheet: u64` | `crates/ui4me-host/src/host/document_commands.rs:239` |
| `list_stylesheets` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:242` |
| `node_of` | `element?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:243` |
| `get_node` | `node: String` | `crates/ui4me-host/src/host/document_commands.rs:247` |
| `node_children` | `node: String`, `offset?: usize`, `limit?: usize` | `crates/ui4me-host/src/host/document_commands.rs:250` |
| `get_text` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:257` |
| `get_attribute` | `element: String`, `name: String` | `crates/ui4me-host/src/host/document_commands.rs:260` |
| `get_attributes` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:264` |
| `has_element` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:267` |
| `query` | `selector: String`, `limit?: usize`, `offset?: usize` | `crates/ui4me-host/src/host/document_commands.rs:270` |
| `batch` | `ops: Vec<BatchOp>` | `crates/ui4me-host/src/host/document_commands.rs:277` |
| `set_default_action` | `element: String`, `event: String`, `key?: Option<String>`, `shift?: Option<bool>`, `ctrl?: Option<bool>`, `alt?: Option<bool>`, `meta?: Option<bool>`, `prevent?: bool` | `crates/ui4me-host/src/host/document_commands.rs:280` |
| `get_validity` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:296` |
| `check_validity` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:299` |
| `get_scroll` | `element?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:302` |
| `scroll_to` | `element?: Option<String>`, `x?: Option<f64>`, `y?: Option<f64>`, `behavior?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:306` |
| `scroll_by` | `element?: Option<String>`, `dx?: f64`, `dy?: f64`, `behavior?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:317` |
| `scroll_into_view` | `element: String`, `block?: String`, `inline?: String`, `behavior?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:327` |
| `get_rect` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:336` |
| `set_environment` | `color_scheme?: Option<String>`, `reduced_motion?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:339` |
| `get_environment` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:345` |
| `set_scroll_follow` | `element?: Option<String>`, `follow?: bool` | `crates/ui4me-host/src/host/document_commands.rs:346` |
| `get_selection` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:352` |
| `clear_selection` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:353` |
| `select_text` | `element?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:354` |
| `copy_selection` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:358` |
| `append_text` | `element: String`, `text: String` | `crates/ui4me-host/src/host/document_commands.rs:359` |
| `trim_children` | `element: String`, `keep: usize`, `from?: String` | `crates/ui4me-host/src/host/document_commands.rs:363` |
| `show_dialog` | `element: String`, `modal?: bool` | `crates/ui4me-host/src/host/document_commands.rs:369` |
| `close_dialog` | `element: String`, `return_value?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:374` |
| `get_dialog` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:379` |
| `show_popover` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:382` |
| `hide_popover` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:385` |
| `toggle_popover` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:388` |
| `get_popover` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:391` |
| `set_pointer_capture` | `element: String`, `pointer_id?: u32`, `capture?: bool` | `crates/ui4me-host/src/host/document_commands.rs:394` |
| `get_pointer_capture` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:401` |
| `get_options` | `element: String` | `crates/ui4me-host/src/host/document_commands.rs:402` |
| `set_selected_values` | `element: String`, `values: Vec<String>` | `crates/ui4me-host/src/host/document_commands.rs:405` |
| `get_accessibility_tree` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:409` |
| `get_diagnostics` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:411` |
| `css_profile` | no additional fields | `crates/ui4me-host/src/host/document_commands.rs:412` |
| `explain` | `element?: Option<String>`, `node?: Option<String>`, `property?: Option<String>` | `crates/ui4me-host/src/host/document_commands.rs:413` |

## Nested batches

The Rust `BatchOp` and `NodeBatchOp` enums have 11 variants each. Both variants have independent format and are checked against a maximum 1,024 operations per batch in the dispatch.

| Batch variant | `batch` | `node_batch` |
|---|---|---|
| `set_text` | `crates/ui4me-host/src/host/document_commands.rs:572` | `crates/ui4me-host/src/host/document_commands.rs:472` |
| `set_attribute` | `crates/ui4me-host/src/host/document_commands.rs:576` | `crates/ui4me-host/src/host/document_commands.rs:476` |
| `remove_attribute` | `crates/ui4me-host/src/host/document_commands.rs:581` | `crates/ui4me-host/src/host/document_commands.rs:481` |
| `add_class` | `crates/ui4me-host/src/host/document_commands.rs:585` | `crates/ui4me-host/src/host/document_commands.rs:485` |
| `remove_class` | `crates/ui4me-host/src/host/document_commands.rs:589` | `crates/ui4me-host/src/host/document_commands.rs:489` |
| `insert_html` | `crates/ui4me-host/src/host/document_commands.rs:593` | `crates/ui4me-host/src/host/document_commands.rs:493` |
| `replace_children_html` | `crates/ui4me-host/src/host/document_commands.rs:598` | `crates/ui4me-host/src/host/document_commands.rs:498` |
| `remove_element` | `crates/ui4me-host/src/host/document_commands.rs:602` | `crates/ui4me-host/src/host/document_commands.rs:502` |
| `move_element` | `crates/ui4me-host/src/host/document_commands.rs:605` | `crates/ui4me-host/src/host/document_commands.rs:505` |
| `append_text` | `crates/ui4me-host/src/host/document_commands.rs:610` | `crates/ui4me-host/src/host/document_commands.rs:510` |
| `trim_children` | `crates/ui4me-host/src/host/document_commands.rs:614` | `crates/ui4me-host/src/host/document_commands.rs:514` |

## Limitations of this extraction

- This catalogue is extracted from a tagged Rust enum and does **not** prove that every operation has stable semantics, a response schema, or equivalent SDK coverage.
- `Option` fields can accept null as well as omission; serde defaults are not always equivalent to nullable types.
- Operations dispatch into `ui4me-renderer`, which applies additional state, size and validity checks; they must be tested separately.
- The generated `document-command.schema.json` encodes structural input types, not runtime state transitions or all semantic bounds.
