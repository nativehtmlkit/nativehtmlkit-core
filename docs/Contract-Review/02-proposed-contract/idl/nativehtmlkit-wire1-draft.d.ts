/** Source-derived structural IDL from ui4me-core-main(6).zip.
 * Contract status: PROPOSED; NOT CERTIFIED.
 * Protocol version: wire 1. Host version at capture: 1.0.1.
 * This intentionally preserves current wire method names and UI4ME env identifiers.
 * Runtime checks and semantics are additional to these types.
 */
export type JsonScalar = null | boolean | number | string;
export type Json = JsonScalar | Json[] | { [key: string]: Json };
export interface RpcRequest<M extends string, P> {jsonrpc: "2.0"; id: string; method: M; params: P;}
export interface RpcNotification<M extends string, P> {jsonrpc: "2.0"; method: M; params: P;}
export interface RpcError {code: number; message: string; data?: Json;}
export type RpcResponse<R = Json> =
    | {jsonrpc:"2.0"; id: string; result: R}
    | {jsonrpc:"2.0"; id: string | null; error: RpcError};
export interface ResourceEntry {path: string; data_url: string;}

export type BatchOp =
    {
      op: "set_text";
      element: string;
      text: string;
  }
  | {
      op: "set_attribute";
      element: string;
      name: string;
      value: string;
  }
  | {
      op: "remove_attribute";
      element: string;
      name: string;
  }
  | {
      op: "add_class";
      element: string;
      name: string;
  }
  | {
      op: "remove_class";
      element: string;
      name: string;
  }
  | {
      op: "insert_html";
      element: string;
      position: string;
      html: string;
  }
  | {
      op: "replace_children_html";
      element: string;
      html: string;
  }
  | {
      op: "remove_element";
      element: string;
  }
  | {
      op: "move_element";
      element: string;
      target: string;
      position: string;
  }
  | {
      op: "append_text";
      element: string;
      text: string;
  }
  | {
      op: "trim_children";
      element: string;
      keep: number;
      from?: string;
  };

export type NodeBatchOp =
    {
      op: "set_text";
      node: string;
      text: string;
  }
  | {
      op: "set_attribute";
      node: string;
      name: string;
      value: string;
  }
  | {
      op: "remove_attribute";
      node: string;
      name: string;
  }
  | {
      op: "add_class";
      node: string;
      name: string;
  }
  | {
      op: "remove_class";
      node: string;
      name: string;
  }
  | {
      op: "insert_html";
      node: string;
      position: string;
      html: string;
  }
  | {
      op: "replace_children_html";
      node: string;
      html: string;
  }
  | {
      op: "remove_element";
      node: string;
  }
  | {
      op: "move_element";
      node: string;
      target_node: string;
      position: string;
  }
  | {
      op: "append_text";
      node: string;
      text: string;
  }
  | {
      op: "trim_children";
      node: string;
      keep: number;
      from?: string;
  };

export type DocumentCommand =
    {
      op: "register_resource_bundle";
      name: string;
      entries: Array<ResourceEntry>;
  }
  | {
      op: "unregister_resource_bundle";
      name: string;
  }
  | {
      op: "get_resource_stats";
  }
  | {
      op: "purge_unused_resources";
      url?: string | null;
  }
  | {
      op: "element_at_point";
      x: number;
      y: number;
  }
  | {
      op: "get_focused_element";
  }
  | {
      op: "blur";
      element?: string | null;
      node?: string | null;
  }
  | {
      op: "get_control_selection";
      element?: string | null;
      node?: string | null;
  }
  | {
      op: "set_control_selection";
      element?: string | null;
      node?: string | null;
      start: number;
      end: number;
      direction?: string;
  }
  | {
      op: "replace_control_range";
      element?: string | null;
      node?: string | null;
      replacement: string;
      start?: number | null;
      end?: number | null;
      selection_mode?: string;
  }
  | {
      op: "edit_command";
      command: string;
      element?: string | null;
      node?: string | null;
  }
  | {
      op: "reset_form";
      element?: string | null;
      node?: string | null;
  }
  | {
      op: "request_submit";
      element?: string | null;
      node?: string | null;
      submitter?: string | null;
      submitter_node?: string | null;
  }
  | {
      op: "get_form_data";
      element?: string | null;
      node?: string | null;
      submitter?: string | null;
      submitter_node?: string | null;
  }
  | {
      op: "set_custom_validity";
      element?: string | null;
      node?: string | null;
      message: string;
  }
  | {
      op: "query_nodes";
      selector: string;
      offset?: number;
      limit?: number;
  }
  | {
      op: "node_batch";
      ops: Array<NodeBatchOp>;
  }
  | {
      op: "set_text";
      element: string;
      text: string;
  }
  | {
      op: "get_value";
      element: string;
  }
  | {
      op: "set_value";
      element: string;
      value: string;
  }
  | {
      op: "get_checked";
      element: string;
  }
  | {
      op: "set_checked";
      element: string;
      checked: boolean;
  }
  | {
      op: "set_attribute";
      element: string;
      name: string;
      value: string;
  }
  | {
      op: "remove_attribute";
      element: string;
      name: string;
  }
  | {
      op: "add_class";
      element: string;
      name: string;
  }
  | {
      op: "remove_class";
      element: string;
      name: string;
  }
  | {
      op: "focus";
      element?: string | null;
      node?: string | null;
  }
  | {
      op: "replace_children_html";
      element: string;
      html: string;
  }
  | {
      op: "insert_html";
      element: string;
      position: string;
      html: string;
  }
  | {
      op: "remove_element";
      element: string;
  }
  | {
      op: "move_element";
      element: string;
      target: string;
      position: string;
  }
  | {
      op: "get_html";
      element?: string | null;
      node?: string | null;
      outer?: boolean;
      offset?: number;
      limit?: number | null;
  }
  | {
      op: "text_rects";
      element?: string | null;
      node?: string | null;
      start?: number | null;
      end?: number | null;
  }
  | {
      op: "text_at_point";
      x: number;
      y: number;
  }
  | {
      op: "get_files";
      element: string;
  }
  | {
      op: "get_computed_style";
      element?: string | null;
      node?: string | null;
      properties: Array<string>;
  }
  | {
      op: "add_stylesheet";
      css: string;
  }
  | {
      op: "replace_stylesheet";
      sheet: number;
      css: string;
  }
  | {
      op: "remove_stylesheet";
      sheet: number;
  }
  | {
      op: "list_stylesheets";
  }
  | {
      op: "node_of";
      element?: string | null;
  }
  | {
      op: "get_node";
      node: string;
  }
  | {
      op: "node_children";
      node: string;
      offset?: number;
      limit?: number;
  }
  | {
      op: "get_text";
      element: string;
  }
  | {
      op: "get_attribute";
      element: string;
      name: string;
  }
  | {
      op: "get_attributes";
      element: string;
  }
  | {
      op: "has_element";
      element: string;
  }
  | {
      op: "query";
      selector: string;
      limit?: number;
      offset?: number;
  }
  | {
      op: "batch";
      ops: Array<BatchOp>;
  }
  | {
      op: "set_default_action";
      element: string;
      event: string;
      key?: string | null;
      shift?: boolean | null;
      ctrl?: boolean | null;
      alt?: boolean | null;
      meta?: boolean | null;
      prevent?: boolean;
  }
  | {
      op: "get_validity";
      element: string;
  }
  | {
      op: "check_validity";
      element: string;
  }
  | {
      op: "get_scroll";
      element?: string | null;
  }
  | {
      op: "scroll_to";
      element?: string | null;
      x?: number | null;
      y?: number | null;
      behavior?: string | null;
  }
  | {
      op: "scroll_by";
      element?: string | null;
      dx?: number;
      dy?: number;
      behavior?: string | null;
  }
  | {
      op: "scroll_into_view";
      element: string;
      block?: string;
      inline?: string;
      behavior?: string | null;
  }
  | {
      op: "get_rect";
      element: string;
  }
  | {
      op: "set_environment";
      color_scheme?: string | null;
      reduced_motion?: string | null;
  }
  | {
      op: "get_environment";
  }
  | {
      op: "set_scroll_follow";
      element?: string | null;
      follow?: boolean;
  }
  | {
      op: "get_selection";
  }
  | {
      op: "clear_selection";
  }
  | {
      op: "select_text";
      element?: string | null;
  }
  | {
      op: "copy_selection";
  }
  | {
      op: "append_text";
      element: string;
      text: string;
  }
  | {
      op: "trim_children";
      element: string;
      keep: number;
      from?: string;
  }
  | {
      op: "show_dialog";
      element: string;
      modal?: boolean;
  }
  | {
      op: "close_dialog";
      element: string;
      return_value?: string | null;
  }
  | {
      op: "get_dialog";
      element: string;
  }
  | {
      op: "show_popover";
      element: string;
  }
  | {
      op: "hide_popover";
      element: string;
  }
  | {
      op: "toggle_popover";
      element: string;
  }
  | {
      op: "get_popover";
      element: string;
  }
  | {
      op: "set_pointer_capture";
      element: string;
      pointer_id?: number;
      capture?: boolean;
  }
  | {
      op: "get_pointer_capture";
  }
  | {
      op: "get_options";
      element: string;
  }
  | {
      op: "set_selected_values";
      element: string;
      values: Array<string>;
  }
  | {
      op: "get_accessibility_tree";
  }
  | {
      op: "get_diagnostics";
  }
  | {
      op: "css_profile";
  }
  | {
      op: "explain";
      element?: string | null;
      node?: string | null;
      property?: string | null;
  };

/** All 32 host-callable method identifiers observed in Rust dispatch. */
export type HostMethod =
   "app.activate"
  | "app.force_quit"
  | "app.ready"
  | "app.request_quit"
  | "bitmap.close"
  | "bitmap.create"
  | "bitmap.present"
  | "clipboard.read_text"
  | "clipboard.write_text"
  | "desktop.open_external"
  | "desktop.paths"
  | "desktop.preferences"
  | "desktop.reveal_file"
  | "dialog.choose_folder"
  | "dialog.open_files"
  | "dialog.save_file"
  | "document.command"
  | "document.get"
  | "document.load"
  | "document.unload"
  | "host.capabilities"
  | "host.diagnostics"
  | "host.hello"
  | "probe.get_text"
  | "probe.set_text"
  | "request.cancel"
  | "window.create"
  | "window.get"
  | "window.list"
  | "window.request_attention"
  | "window.request_close"
  | "window.set";

export interface HostHelloParams {token: string; protocol_version: 1; client?: {name?:string;version?:string};}
export interface HostHelloResult {protocol_version:1; host_version:string; app_id:string; capabilities:Record<string,Json>;}
export interface DocumentCommandParams {window_id:string;document_id:string;command:DocumentCommand;}
/** The following method-specific result shapes must be extracted, tested and frozen before stable publication. */
export type UncertifiedResult = Json;

/** Source-oriented method input mapping; semantic constraints require runtime validation. */
export interface HostMethodParams {
  "app.activate": { args?: Array<string>; cwd?: string; activation_token?: string | null; };
  "app.force_quit": { [key:string]: Json; };
  "app.ready": { [key:string]: Json; };
  "app.request_quit": { [key:string]: Json; };
  "bitmap.close": { window_id: string; document_id: string; bitmap_id: string; };
  "bitmap.create": { window_id: string; document_id: string; bitmap_id: string; width: number; height: number; slots?: number; pixel_format?: string; };
  "bitmap.present": { window_id: string; document_id: string; bitmap_id: string; sequence: number; slot: number; };
  "clipboard.read_text": { [key:string]: Json; };
  "clipboard.write_text": { text: string; };
  "desktop.open_external": { target: string; window_id?: string | null; };
  "desktop.paths": { [key:string]: Json; };
  "desktop.preferences": { [key:string]: Json; };
  "desktop.reveal_file": { path: string; window_id?: string | null; };
  "dialog.choose_folder": { window_id: string; title?: string | null; directory?: string | null; file_name?: string | null; filters?: Array<{ name: string; extensions: Array<string>; }>; multiple?: boolean; };
  "dialog.open_files": { window_id: string; title?: string | null; directory?: string | null; file_name?: string | null; filters?: Array<{ name: string; extensions: Array<string>; }>; multiple?: boolean; };
  "dialog.save_file": { window_id: string; title?: string | null; directory?: string | null; file_name?: string | null; filters?: Array<{ name: string; extensions: Array<string>; }>; multiple?: boolean; };
  "document.command": { window_id: string; document_id: string; command: DocumentCommand; };
  "document.get": { window_id: string; };
  "document.load": { window_id: string; html?: string | null; path?: string | null; asset_root?: string | null; };
  "document.unload": { window_id: string; document_id: string; };
  "host.capabilities": { [key:string]: Json; };
  "host.diagnostics": { [key:string]: Json; };
  "host.hello": { token: string; protocol_version: 1; client?: { name?: string; version?: string; }; };
  "probe.get_text": { window_id: string; };
  "probe.set_text": { window_id: string; text: string; };
  "request.cancel": { id: string; };
  "window.create": { title?: string; width?: number; height?: number; visible?: boolean; resizable?: boolean; decorations?: boolean; min_width?: number | null; min_height?: number | null; max_width?: number | null; max_height?: number | null; };
  "window.get": { window_id: string; };
  "window.list": { [key:string]: Json; };
  "window.request_attention": { window_id: string; kind?: string | null; };
  "window.request_close": { window_id: string; };
  "window.set": { window_id: string; title?: string | null; width?: number | null; height?: number | null; visible?: boolean | null; resizable?: boolean | null; decorations?: boolean | null; minimized?: boolean | null; maximized?: boolean | null; fullscreen?: boolean | null; focus?: boolean | null; x?: number | null; y?: number | null; min_width?: number | null; min_height?: number | null; max_width?: number | null; max_height?: number | null; };
}
