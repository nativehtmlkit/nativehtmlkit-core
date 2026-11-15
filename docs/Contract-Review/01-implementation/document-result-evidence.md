# Document operation result evidence — source excerpts

Each excerpt begins at the Rust `DocumentCommand::execute` dispatch arm for the operation. **This is not a parsed result schema**; it is the exact implementation evidence to use during the per-operation response schema extraction and live verification.

## `register_resource_bundle`

Source: `crates/ui4me-host/src/host/document_commands.rs:700`

```rust
            Self::RegisterResourceBundle { name, entries } => {
                document.register_resource_bundle(&name, decode_resource_entries(entries)?)?
            }
```

## `unregister_resource_bundle`

Source: `crates/ui4me-host/src/host/document_commands.rs:703`

```rust
            Self::UnregisterResourceBundle { name } => {
                document.unregister_resource_bundle(&name)?
            }
```

## `get_resource_stats`

Source: `crates/ui4me-host/src/host/document_commands.rs:706`

```rust
            Self::GetResourceStats => document.resource_stats()?,
```

## `purge_unused_resources`

Source: `crates/ui4me-host/src/host/document_commands.rs:707`

```rust
            Self::PurgeUnusedResources { url } => {
                document.purge_unused_resources(url.as_deref())?
            }
```

## `element_at_point`

Source: `crates/ui4me-host/src/host/document_commands.rs:710`

```rust
            Self::ElementAtPoint { x, y } => {
                if !x.is_finite() || !y.is_finite() {
                    return Err(RpcError::invalid_params("Coordinates must be finite"));
                }
                document.element_at_point(x, y)?
            }
```

## `get_focused_element`

Source: `crates/ui4me-host/src/host/document_commands.rs:716`

```rust
            Self::GetFocusedElement => document.focused_element(),
```

## `blur`

Source: `crates/ui4me-host/src/host/document_commands.rs:717`

```rust
            Self::Blur { element, node } => {
                changed(document.blur(element.as_deref(), node.as_deref()))?
            }
```

## `get_control_selection`

Source: `crates/ui4me-host/src/host/document_commands.rs:720`

```rust
            Self::GetControlSelection { element, node } => {
                document.control_selection(element.as_deref(), node.as_deref())?
            }
```

## `set_control_selection`

Source: `crates/ui4me-host/src/host/document_commands.rs:723`

```rust
            Self::SetControlSelection {
                element,
                node,
                start,
                end,
                direction,
            } => document.set_control_selection(
                element.as_deref(),
                node.as_deref(),
                start,
                end,
                &direction,
            )?,
```

## `replace_control_range`

Source: `crates/ui4me-host/src/host/document_commands.rs:736`

```rust
            Self::ReplaceControlRange {
                element,
                node,
                replacement,
                start,
                end,
                selection_mode,
            } => document.replace_control_range(
                element.as_deref(),
                node.as_deref(),
                &replacement,
                start,
                end,
                &selection_mode,
            )?,
```

## `edit_command`

Source: `crates/ui4me-host/src/host/document_commands.rs:751`

```rust
            Self::EditCommand {
                command,
                element,
                node,
            } => document.edit_command(&command, element.as_deref(), node.as_deref())?,
```

## `reset_form`

Source: `crates/ui4me-host/src/host/document_commands.rs:756`

```rust
            Self::ResetForm { element, node } => {
                document.reset_form(element.as_deref(), node.as_deref())?
            }
```

## `request_submit`

Source: `crates/ui4me-host/src/host/document_commands.rs:759`

```rust
            Self::RequestSubmit {
                element,
                node,
                submitter,
                submitter_node,
            } => document.request_submit(
                element.as_deref(),
                node.as_deref(),
                submitter.as_deref(),
                submitter_node.as_deref(),
            )?,
```

## `get_form_data`

Source: `crates/ui4me-host/src/host/document_commands.rs:770`

```rust
            Self::GetFormData {
                element,
                node,
                submitter,
                submitter_node,
            } => document.form_data(
                element.as_deref(),
                node.as_deref(),
                submitter.as_deref(),
                submitter_node.as_deref(),
            )?,
```

## `set_custom_validity`

Source: `crates/ui4me-host/src/host/document_commands.rs:781`

```rust
            Self::SetCustomValidity {
                element,
                node,
                message,
            } => document.set_custom_validity(element.as_deref(), node.as_deref(), &message)?,
```

## `query_nodes`

Source: `crates/ui4me-host/src/host/document_commands.rs:786`

```rust
            Self::QueryNodes {
                selector,
                offset,
                limit,
            } => {
                if limit == 0 || limit > 4096 {
                    return Err(RpcError::invalid_params("query limit must be 1..4096"));
                }
                document.query_nodes(&selector, offset, limit)?
            }
```

## `node_batch`

Source: `crates/ui4me-host/src/host/document_commands.rs:796`

```rust
            Self::NodeBatch { ops } => {
                if ops.len() > 1024 {
                    return Err(RpcError::busy(
                        "A node batch permits at most 1024 operations",
                    ));
                }
                let ops = ops
                    .into_iter()
                    .map(NodeBatchOp::into_engine)
                    .collect::<Result<Vec<_>, _>>()?;
                changed(document.apply_node_batch(&ops))?
            }
```

## `set_text`

Source: `crates/ui4me-host/src/host/document_commands.rs:808`

```rust
            Self::SetText { element, text } => changed(document.set_text(&element, &text))?,
```

## `get_text`

Source: `crates/ui4me-host/src/host/document_commands.rs:809`

```rust
            Self::GetText { element } => json!({"text":document.get_text(&element)?}),
```

## `get_value`

Source: `crates/ui4me-host/src/host/document_commands.rs:810`

```rust
            Self::GetValue { element } => json!({"value":document.get_value(&element)?}),
```

## `set_value`

Source: `crates/ui4me-host/src/host/document_commands.rs:811`

```rust
            Self::SetValue { element, value } => changed(document.set_value(&element, &value))?,
```

## `get_checked`

Source: `crates/ui4me-host/src/host/document_commands.rs:812`

```rust
            Self::GetChecked { element } => json!({"checked":document.get_checked(&element)?}),
```

## `set_checked`

Source: `crates/ui4me-host/src/host/document_commands.rs:813`

```rust
            Self::SetChecked { element, checked } => {
                changed(document.set_checked(&element, checked))?
            }
```

## `set_attribute`

Source: `crates/ui4me-host/src/host/document_commands.rs:816`

```rust
            Self::SetAttribute {
                element,
                name,
                value,
            } => changed(document.set_attribute(&element, &name, Some(&value)))?,
```

## `remove_attribute`

Source: `crates/ui4me-host/src/host/document_commands.rs:821`

```rust
            Self::RemoveAttribute { element, name } => {
                changed(document.set_attribute(&element, &name, None))?
            }
```

## `add_class`

Source: `crates/ui4me-host/src/host/document_commands.rs:824`

```rust
            Self::AddClass { element, name } => changed(document.add_class(&element, &name))?,
```

## `remove_class`

Source: `crates/ui4me-host/src/host/document_commands.rs:825`

```rust
            Self::RemoveClass { element, name } => changed(document.remove_class(&element, &name))?,
```

## `get_attribute`

Source: `crates/ui4me-host/src/host/document_commands.rs:826`

```rust
            Self::GetAttribute { element, name } => {
                json!({"value":document.get_attribute(&element, &name)?})
            }
```

## `get_attributes`

Source: `crates/ui4me-host/src/host/document_commands.rs:829`

```rust
            Self::GetAttributes { element } => {
                let attributes: serde_json::Map<String, Value> = document
                    .get_attributes(&element)?
                    .into_iter()
                    .map(|(name, value)| (name, Value::String(value)))
                    .collect();
                json!({"attributes":attributes})
            }
```

## `has_element`

Source: `crates/ui4me-host/src/host/document_commands.rs:837`

```rust
            Self::HasElement { element } => json!({"exists":document.has_element(&element)}),
```

## `query`

Source: `crates/ui4me-host/src/host/document_commands.rs:838`

```rust
            Self::Query {
                selector,
                offset,
                limit,
            } => {
                if limit == 0 || limit > 4096 {
                    return Err(RpcError::invalid_params("query limit must be 1..4096"));
                }
                // The model itself caps all nodes at 65,536. Query all identified
                // matches once so pagination counts do not confuse anonymous nodes
                // with truncated identified matches.
                let (ids, matched_nodes) = document.query_ids(&selector, 65_536)?;
                let total = ids.len();
                let selected: Vec<_> = ids.into_iter().skip(offset).take(limit).collect();
                let next = offset.saturating_add(selected.len());
                json!({"ids":selected,"total":total,"matched_nodes":matched_nodes,
                    "offset":offset,"limit":limit,"truncated":next < total})
            }
```

## `focus`

Source: `crates/ui4me-host/src/host/document_commands.rs:856`

```rust
            Self::Focus { element, node } => {
                changed(document.focus_target(element.as_deref(), node.as_deref()))?
            }
```

## `replace_children_html`

Source: `crates/ui4me-host/src/host/document_commands.rs:859`

```rust
            Self::ReplaceChildrenHtml { element, html } => {
                changed(document.replace_children_html(&element, &html))?
            }
```

## `insert_html`

Source: `crates/ui4me-host/src/host/document_commands.rs:862`

```rust
            Self::InsertHtml {
                element,
                position,
                html,
            } => changed(document.insert_html(&element, &position, &html))?,
```

## `remove_element`

Source: `crates/ui4me-host/src/host/document_commands.rs:867`

```rust
            Self::RemoveElement { element } => changed(document.remove_element(&element))?,
```

## `move_element`

Source: `crates/ui4me-host/src/host/document_commands.rs:868`

```rust
            Self::MoveElement {
                element,
                target,
                position,
            } => changed(document.move_element(&element, &target, &position))?,
```

## `append_text`

Source: `crates/ui4me-host/src/host/document_commands.rs:873`

```rust
            Self::AppendText { element, text } => changed(document.append_text(&element, &text))?,
```

## `trim_children`

Source: `crates/ui4me-host/src/host/document_commands.rs:874`

```rust
            Self::TrimChildren {
                element,
                keep,
                from,
            } => changed(document.trim_children(&element, keep, trim_from_start(&from)?))?,
```

## `batch`

Source: `crates/ui4me-host/src/host/document_commands.rs:879`

```rust
            Self::Batch { ops } => {
                if ops.len() > 1024 {
                    return Err(RpcError::busy(
                        "A document batch permits at most 1024 operations",
                    ));
                }
                let operations = ops
                    .into_iter()
                    .map(BatchOp::into_engine)
                    .collect::<Result<Vec<_>, _>>()?;
                changed(document.apply_batch(&operations))?
            }
```

## `get_html`

Source: `crates/ui4me-host/src/host/document_commands.rs:891`

```rust
            Self::GetHtml {
                element,
                node,
                outer,
                offset,
                limit,
            } => {
                if limit.is_some_and(|limit| !(4..=262_144).contains(&limit)) {
                    return Err(RpcError::invalid_params(
                        "HTML byte limit must be 4..262144",
                    ));
                }
                let html = document.get_html(element.as_deref(), node.as_deref(), outer)?;
                if !html.is_char_boundary(offset) {
                    return Err(RpcError::invalid_params("HTML offset must be a UTF-8 character boundary within the serialized document"));
                }
                let mut end = limit.map_or(html.len(), |limit| {
                    offset.saturating_add(limit).min(html.len())
                });
                while !html.is_char_boundary(end) {
                    end -= 1;
                }
                json!({"html":&html[offset..end],"total_bytes":html.len(),"offset":offset,
                    "next_offset":(end < html.len()).then_some(end),"truncated":end < html.len()})
            }
```

## `node_of`

Source: `crates/ui4me-host/src/host/document_commands.rs:916`

```rust
            Self::NodeOf { element } => json!({"node":document.node_of(element.as_deref())?}),
```

## `get_node`

Source: `crates/ui4me-host/src/host/document_commands.rs:917`

```rust
            Self::GetNode { node } => document.get_node(&node)?,
```

## `node_children`

Source: `crates/ui4me-host/src/host/document_commands.rs:918`

```rust
            Self::NodeChildren {
                node,
                offset,
                limit,
            } => {
                if limit == 0 || limit > 4096 {
                    return Err(RpcError::invalid_params("children limit must be 1..4096"));
                }
                document.node_children(&node, offset, limit)?
            }
```

## `add_stylesheet`

Source: `crates/ui4me-host/src/host/document_commands.rs:928`

```rust
            Self::AddStylesheet { css } => document.add_stylesheet(&css)?,
```

## `replace_stylesheet`

Source: `crates/ui4me-host/src/host/document_commands.rs:929`

```rust
            Self::ReplaceStylesheet { sheet, css } => document.replace_stylesheet(sheet, &css)?,
```

## `remove_stylesheet`

Source: `crates/ui4me-host/src/host/document_commands.rs:930`

```rust
            Self::RemoveStylesheet { sheet } => changed(document.remove_stylesheet(sheet))?,
```

## `list_stylesheets`

Source: `crates/ui4me-host/src/host/document_commands.rs:931`

```rust
            Self::ListStylesheets => json!({"sheets":document.list_stylesheets()}),
```

## `get_computed_style`

Source: `crates/ui4me-host/src/host/document_commands.rs:932`

```rust
            Self::GetComputedStyle {
                element,
                node,
                properties,
            } => document.computed_style(element.as_deref(), node.as_deref(), &properties)?,
```

## `get_rect`

Source: `crates/ui4me-host/src/host/document_commands.rs:937`

```rust
            Self::GetRect { element } => json!({"rect":document.element_rect(&element)?.map(|r|
                json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height}))}),
```

## `text_rects`

Source: `crates/ui4me-host/src/host/document_commands.rs:939`

```rust
            Self::TextRects {
                element,
                node,
                start,
                end,
            } => document.text_rects(element.as_deref(), node.as_deref(), start, end)?,
```

## `text_at_point`

Source: `crates/ui4me-host/src/host/document_commands.rs:945`

```rust
            Self::TextAtPoint { x, y } => {
                if !x.is_finite() || !y.is_finite() {
                    return Err(RpcError::invalid_params("Text coordinates must be finite"));
                }
                json!({"hit":document.text_at_point(x, y)?})
            }
```

## `get_scroll`

Source: `crates/ui4me-host/src/host/document_commands.rs:951`

```rust
            Self::GetScroll { element } => scroll_json(document.scroll_state(element.as_deref())?),
```

## `scroll_to`

Source: `crates/ui4me-host/src/host/document_commands.rs:952`

```rust
            Self::ScrollTo {
                element,
                x,
                y,
                behavior,
            } => scroll_json(document.scroll_to_behavior(
                element.as_deref(),
                x,
                y,
                behavior.as_deref().unwrap_or("auto"),
            )?),
```

## `scroll_by`

Source: `crates/ui4me-host/src/host/document_commands.rs:963`

```rust
            Self::ScrollBy {
                element,
                dx,
                dy,
                behavior,
            } => scroll_json(document.scroll_by_behavior(
                element.as_deref(),
                dx,
                dy,
                behavior.as_deref().unwrap_or("auto"),
            )?),
```

## `scroll_into_view`

Source: `crates/ui4me-host/src/host/document_commands.rs:974`

```rust
            Self::ScrollIntoView {
                element,
                block,
                inline,
                behavior,
            } => changed(document.scroll_into_view_behavior(
                &element,
                &block,
                &inline,
                behavior.as_deref().unwrap_or("auto"),
            ))?,
```

## `set_scroll_follow`

Source: `crates/ui4me-host/src/host/document_commands.rs:985`

```rust
            Self::SetScrollFollow { element, follow } => {
                changed(document.set_scroll_follow(element.as_deref(), follow))?
            }
```

## `get_selection`

Source: `crates/ui4me-host/src/host/document_commands.rs:988`

```rust
            Self::GetSelection => json!({"text":document.selection_text()?}),
```

## `clear_selection`

Source: `crates/ui4me-host/src/host/document_commands.rs:989`

```rust
            Self::ClearSelection => {
                document.clear_selection();
                json!({"updated":true})
            }
```

## `select_text`

Source: `crates/ui4me-host/src/host/document_commands.rs:993`

```rust
            Self::SelectText { element } => changed(document.select_text(element.as_deref()))?,
```

## `copy_selection`

Source: `crates/ui4me-host/src/host/document_commands.rs:994`

```rust
            Self::CopySelection => {
                let text = document.selection_text()?;
                if text.len() > 1024 * 1024 {
                    return Err(RpcError::busy(
                        "Copied selection exceeds the 1 MiB native clipboard limit",
                    ));
                }
                // Admit the encoded reply before queuing the side effect. A
                // failed response-size check must not still write the clipboard.
                let result = bounded_result(json!({"text": &text}))?;
                if !text.is_empty() {
                    document.set_clipboard_text(&text)?;
                }
                result
            }
```

## `get_validity`

Source: `crates/ui4me-host/src/host/document_commands.rs:1009`

```rust
            Self::GetValidity { element } => document.get_validity(&element)?,
```

## `check_validity`

Source: `crates/ui4me-host/src/host/document_commands.rs:1010`

```rust
            Self::CheckValidity { element } => document.check_validity(&element)?,
```

## `get_files`

Source: `crates/ui4me-host/src/host/document_commands.rs:1011`

```rust
            Self::GetFiles { element } => json!({"files":document.get_files(&element)?}),
```

## `get_options`

Source: `crates/ui4me-host/src/host/document_commands.rs:1012`

```rust
            Self::GetOptions { element } => {
                let options: Vec<_> = document.select_options(&element)?.into_iter()
                    .map(|(value, label, selected, disabled)| json!({"value":value,"label":label,"selected":selected,"disabled":disabled})).collect();
                json!({"options":options})
            }
```

## `set_selected_values`

Source: `crates/ui4me-host/src/host/document_commands.rs:1017`

```rust
            Self::SetSelectedValues { element, values } => {
                changed(document.set_selected_values(&element, &values))?
            }
```

## `show_dialog`

Source: `crates/ui4me-host/src/host/document_commands.rs:1020`

```rust
            Self::ShowDialog { element, modal } => changed(document.show_dialog(&element, modal))?,
```

## `close_dialog`

Source: `crates/ui4me-host/src/host/document_commands.rs:1021`

```rust
            Self::CloseDialog {
                element,
                return_value,
            } => changed(document.close_dialog(&element, return_value.as_deref()))?,
```

## `get_dialog`

Source: `crates/ui4me-host/src/host/document_commands.rs:1025`

```rust
            Self::GetDialog { element } => {
                let (open, modal, return_value) = document.dialog_state(&element)?;
                json!({"open":open,"modal":modal,"return_value":return_value})
            }
```

## `show_popover`

Source: `crates/ui4me-host/src/host/document_commands.rs:1029`

```rust
            Self::ShowPopover { element } => changed(document.show_popover(&element))?,
```

## `hide_popover`

Source: `crates/ui4me-host/src/host/document_commands.rs:1030`

```rust
            Self::HidePopover { element } => changed(document.hide_popover(&element))?,
```

## `toggle_popover`

Source: `crates/ui4me-host/src/host/document_commands.rs:1031`

```rust
            Self::TogglePopover { element } => json!({"open":document.toggle_popover(&element)?}),
```

## `get_popover`

Source: `crates/ui4me-host/src/host/document_commands.rs:1032`

```rust
            Self::GetPopover { element } => {
                let (open, kind) = document.popover_state(&element)?;
                json!({"open":open,"kind":kind})
            }
```

## `set_default_action`

Source: `crates/ui4me-host/src/host/document_commands.rs:1036`

```rust
            Self::SetDefaultAction {
                element,
                event,
                key,
                shift,
                ctrl,
                alt,
                meta,
                prevent,
            } => changed(document.set_default_action(
                ui4me_renderer::DefaultActionRule {
                    element,
                    event,
                    key,
                    shift,
                    ctrl,
                    alt,
                    meta,
                },
                prevent,
            ))?,
```

## `set_pointer_capture`

Source: `crates/ui4me-host/src/host/document_commands.rs:1057`

```rust
            Self::SetPointerCapture {
                element,
                pointer_id,
                capture,
            } => json!({"accepted":document.set_pointer_capture(&element, pointer_id, capture)?}),
```

## `get_pointer_capture`

Source: `crates/ui4me-host/src/host/document_commands.rs:1062`

```rust
            Self::GetPointerCapture => json!({"element":document.pointer_capture()}),
```

## `get_environment`

Source: `crates/ui4me-host/src/host/document_commands.rs:1063`

```rust
            Self::GetEnvironment => document.environment(),
```

## `set_environment`

Source: `crates/ui4me-host/src/host/document_commands.rs:1064`

```rust
            Self::SetEnvironment {
                color_scheme,
                reduced_motion,
            } => {
                let dark = match color_scheme.as_deref() {
                    None => None,
                    Some("light") => Some(Some(false)),
                    Some("dark") => Some(Some(true)),
                    Some("system") => Some(None),
                    Some(_) => {
                        return Err(RpcError::invalid_params(
                            "color_scheme must be light, dark or system",
                        ))
                    }
                };
                let reduce = match reduced_motion.as_deref() {
                    None => None,
                    Some("reduce") => Some(Some(true)),
                    Some("no-preference") => Some(Some(false)),
                    Some("system") => Some(None),
                    Some(_) => {
                        return Err(RpcError::invalid_params(
                            "reduced_motion must be reduce, no-preference or system",
                        ))
                    }
                };
                document.set_app_environment(dark, reduce)?;
                document.environment()
```

## `get_accessibility_tree`

Source: `crates/ui4me-host/src/host/document_commands.rs:1093`

```rust
            Self::GetAccessibilityTree => json!({"tree":document.accessibility_json()?}),
```

## `get_diagnostics`

Source: `crates/ui4me-host/src/host/document_commands.rs:1094`

```rust
            Self::GetDiagnostics => {
                document.refresh_for_diagnostics();
                json!({"diagnostics":document.diagnostics(),"css_profile_version":ui4me_renderer::css_profile().0})
            }
```

## `css_profile`

Source: `crates/ui4me-host/src/host/document_commands.rs:1098`

```rust
            Self::CssProfile => {
                let (version, features) = ui4me_renderer::css_profile();
                let features: Vec<_> = features
                    .into_iter()
                    .map(|(id, state)| json!({"id":id,"state":state}))
                    .collect();
                json!({"version":version,"features":features})
            }
```

## `explain`

Source: `crates/ui4me-host/src/host/document_commands.rs:1106`

```rust
            Self::Explain {
                element,
                node,
```
