#!/usr/bin/env python3
"""Generate the protocol v1 method/op matrix and JSON Schemas from the Rust host source.

The Rust serde types are the authority. This script parses the request structs and the
`DocumentCommand` enum (field names, types, `#[serde(default)]`, `deny_unknown_fields`) and
emits draft 2020-12 schemas plus a machine-readable method matrix. Behavior that the type
system cannot express (results, error codes, lifecycle) is locked separately by the golden
runtime conformance test `crates/nativehtmlkit-host/tests/protocol_conformance.rs`.

Usage: python3 scripts/generate-protocol-contract.py [--check]
  --check  fail if the committed files differ from freshly generated output.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
HOST = ROOT / "crates/nativehtmlkit-host/src"
OUT = ROOT / "protocol/v1"

SOURCES = {
    "requests": HOST / "host/requests.rs",
    "documents": HOST / "host/documents.rs",
    "bitmap": HOST / "host/bitmap.rs",
    "commands": HOST / "host/document_commands.rs",
}

# Client -> host methods and the serde type each handler decodes (verified against the
# dispatch code; `None` means the handler does not decode params, so any object is accepted).
CLIENT_METHODS = {
    "host.hello": ("handshake", "hand-validated: token (string), protocol_version (1); other members ignored"),
    "host.capabilities": (None, None),
    "host.diagnostics": (None, None),
    "app.ready": (None, None),
    "app.request_quit": (None, None),
    "app.force_quit": (None, None),
    "window.create": ("requests", "CreateWindow"),
    "window.set": ("requests", "SetWindow"),
    "window.request_attention": ("requests", "Attention"),
    "window.get": ("requests", "WindowKey"),
    "window.list": (None, None),
    "window.request_close": ("requests", "WindowKey"),
    "probe.set_text": ("requests", "SetText"),
    "probe.get_text": ("requests", "WindowKey"),
    "desktop.paths": (None, None),
    "desktop.preferences": (None, None),
    "desktop.open_external": ("requests", "Target"),
    "desktop.reveal_file": ("requests", "RevealFile"),
    "clipboard.write_text": ("requests", "Text"),
    "clipboard.read_text": (None, None),
    "dialog.open_files": ("requests", "DialogParams"),
    "dialog.save_file": ("requests", "DialogParams"),
    "dialog.choose_folder": ("requests", "DialogParams"),
    "request.cancel": ("requests", "Cancel"),
    "document.load": ("documents", "LoadDocument"),
    "document.get": ("documents", "WindowDocument"),
    "document.unload": ("documents", "DocumentKey"),
    "document.command": ("documents", "CommandRequest"),
    "bitmap.create": ("bitmap", "Create"),
    "bitmap.present": ("bitmap", "Present"),
    "bitmap.close": ("bitmap", "Close"),
}
# Sent by a *secondary* launcher over the private endpoint; never by the authenticated backend.
SECONDARY_METHODS = {"app.activate": "any JSON object up to 32 KiB, forwarded verbatim as app.activated params"}
HOST_REQUESTS = {
    "window.before_close": {"params": ["window_id", "reason"], "reply": {"allow": "boolean (missing/false/non-boolean = veto)"}},
    "app.before_quit": {"params": ["reason"], "reply": {"allow": "boolean (missing/false/non-boolean = veto)"}},
}
HOST_NOTIFICATIONS = {
    "app.shutdown": ["deadline_ms"],
    "app.activated": ["<secondary launcher payload>"],
    "window.event": ["window_id", "type", "..."],
    "document.event": ["window_id", "document_id", "event"],
    "document.error": ["window_id", "document_id", "message"],
    "request.cancelled": ["id", "reason"],
    "desktop.preferences_changed": ["<native preferences snapshot>"],
}

SCALARS = {
    "String": {"type": "string"},
    "PathBuf": {"type": "string", "description": "file system path"},
    "bool": {"type": "boolean"},
    "f64": {"type": "number"},
    "f32": {"type": "number"},
    "u8": {"type": "integer", "minimum": 0, "maximum": 255},
    "u32": {"type": "integer", "minimum": 0, "maximum": 4294967295},
    "u64": {"type": "integer", "minimum": 0},
    "usize": {"type": "integer", "minimum": 0},
    "i32": {"type": "integer", "minimum": -2147483648, "maximum": 2147483647},
}


def strip_comments(text: str) -> str:
    return re.sub(r"//[^\n]*", "", text)


def block(text: str, header_regex: str) -> str:
    m = re.search(header_regex, text)
    if not m:
        raise SystemExit(f"source changed: cannot find {header_regex!r}")
    start = text.index("{", m.end() - 1)
    depth = 0
    for i in range(start, len(text)):
        depth += {"{": 1, "}": -1}.get(text[i], 0)
        if depth == 0:
            return text[start + 1 : i]
    raise SystemExit("unbalanced braces")


def split_top(body: str) -> list[str]:
    """Split on commas that are not nested in <> {} () []."""
    parts, depth, cur = [], 0, ""
    for ch in body:
        if ch in "<{([":
            depth += 1
        elif ch in ">})]":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    if cur.strip():
        parts.append(cur)
    return [p.strip() for p in parts if p.strip()]


def type_schema(rust: str, nested: dict) -> dict:
    rust = rust.strip()
    m = re.fullmatch(r"Option<(.+)>", rust)
    if m:
        inner = type_schema(m.group(1), nested)
        return {"anyOf": [inner, {"type": "null"}]}
    m = re.fullmatch(r"Vec<(.+)>", rust)
    if m:
        return {"type": "array", "items": type_schema(m.group(1), nested)}
    if rust in SCALARS:
        return dict(SCALARS[rust])
    if rust in nested:
        return {"$ref": f"#/$defs/{rust}"}
    if rust.startswith("commands::"):
        return {"$ref": "document-command.schema.json"}
    raise SystemExit(f"unmapped Rust type {rust!r}; extend SCALARS or nested types")


def fields(body: str, nested: dict) -> tuple[dict, list[str], dict]:
    props, required, defaults = {}, [], {}
    for raw in split_top(body):
        attrs = re.findall(r"#\[serde\(([^)]*)\)\]", raw)
        decl = re.sub(r"#\[[^\]]*\]", "", raw).strip()
        decl = re.sub(r"^pub(\([^)]*\))?\s+", "", decl)
        if ":" not in decl:
            raise SystemExit(f"unexpected field {decl!r}")
        name, rust = (x.strip() for x in decl.split(":", 1))
        schema = type_schema(rust, nested)
        has_default = any(a.strip().startswith("default") for a in attrs)
        optional_type = rust.startswith("Option<")
        dm = next((re.search(r'default\s*=\s*"([^"]+)"', a) for a in attrs if "default" in a), None)
        if dm:
            defaults[name] = dm.group(1) + "()"
        elif has_default:
            defaults[name] = "Default::default()"
        props[name] = schema
        # serde treats a missing Option<T> as None even without #[serde(default)].
        if not has_default and not optional_type:
            required.append(name)
    return props, required, defaults


def outer_attributes(text: str, item_start: int) -> str:
    """The contiguous `#[...]` attribute lines directly above an item."""
    lines = text[:item_start].rstrip().splitlines()
    attrs = []
    for line in reversed(lines):
        if line.strip().startswith("#["):
            attrs.append(line.strip())
        else:
            break
    return " ".join(reversed(attrs))


def parse_struct(text: str, name: str, nested: dict) -> dict:
    head = re.search(r"^[ \t]*(pub(\([^)]*\))?\s+)?struct\s+" + name + r"\s*\{", text, re.M)
    if not head:
        raise SystemExit(f"struct {name} not found")
    deny = "deny_unknown_fields" in outer_attributes(text, head.start())
    body = block(text, r"struct\s+" + name + r"\s*\{")
    props, required, defaults = fields(body, nested)
    schema = {"type": "object", "properties": props, "required": required}
    if deny:
        schema["additionalProperties"] = False
    if defaults:
        schema["x-rust-defaults"] = defaults
    return schema


def parse_tagged_enum(text: str, name: str, nested: dict) -> dict[str, dict]:
    body = block(text, r"enum\s+" + name + r"\s*\{")
    variants: dict[str, dict] = {}
    i = 0
    tokens = body
    pattern = re.compile(r"([A-Z][A-Za-z0-9]*)\s*(\{|,|$)")
    pos = 0
    while True:
        m = pattern.search(tokens, pos)
        if not m:
            break
        vname = m.group(1)
        if m.group(2) == "{":
            start = m.end() - 1
            depth = 0
            for j in range(start, len(tokens)):
                depth += {"{": 1, "}": -1}.get(tokens[j], 0)
                if depth == 0:
                    inner = tokens[start + 1 : j]
                    pos = j + 1
                    break
            props, required, defaults = fields(inner, nested)
        else:
            props, required, defaults = {}, [], {}
            pos = m.end()
        op = re.sub(r"(?<!^)(?=[A-Z])", "_", vname).lower()
        schema = {
            "type": "object",
            "properties": {"op": {"const": op}, **props},
            "required": ["op", *required],
        }
        if m.group(2) == "{":
            schema["additionalProperties"] = False
        else:
            # serde does not apply deny_unknown_fields to unit variants of an internally
            # tagged enum: extra members are ignored (locked by the runtime golden).
            schema["x-unknown-members"] = "ignored (protocol v1 behavior of parameterless ops)"
        if defaults:
            schema["x-rust-defaults"] = defaults
        variants[op] = schema
    return variants


def generate() -> dict[pathlib.Path, str]:
    # Normalise CRLF (Windows checkouts with core.autocrlf) before parsing.
    src = {k: strip_comments(p.read_text(encoding="utf-8").replace("\r\n", "\n")) for k, p in SOURCES.items()}
    cmd = src["commands"]
    nested_names = ["ResourceEntry"]
    nested: dict[str, dict] = {n: {} for n in nested_names + ["NodeBatchOp", "BatchOp"]}
    for n in nested_names:
        nested[n] = parse_struct(cmd, n, nested)
    for n in ["NodeBatchOp", "BatchOp"]:
        nested[n] = {"oneOf": list(parse_tagged_enum(cmd, n, nested).values())}
    ops = parse_tagged_enum(cmd, "DocumentCommand", nested)
    files: dict[pathlib.Path, str] = {}
    doc_schema = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://nativehtmlkit.dev/protocol/v1/document-command.schema.json",
        "title": "NativeHTMLKit protocol v1 document.command `command` object",
        "description": "Generated from DocumentCommand (serde tag=op, deny_unknown_fields). Do not edit.",
        "oneOf": [{"$ref": f"#/$defs/op_{k}"} for k in sorted(ops)],
        "$defs": {**{f"op_{k}": v for k, v in sorted(ops.items())}, **nested},
    }
    files[OUT / "schemas/document-command.schema.json"] = json.dumps(doc_schema, indent=2, ensure_ascii=False) + "\n"
    methods = {}
    for method, (where, struct) in sorted(CLIENT_METHODS.items()):
        entry: dict = {"direction": "client->host", "kind": "request"}
        if where is None:
            entry["params"] = {"decoded": False, "note": "params are not decoded; any JSON object (or omitted params) is accepted"}
        elif where == "handshake":
            entry["params"] = {"decoded": "manual", "note": struct}
            schema = {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "title": "host.hello params", "type": "object",
                "properties": {"token": {"type": "string"}, "protocol_version": {"const": 1},
                               "client": {"description": "optional, informational, not validated"}},
                "required": ["token", "protocol_version"],
            }
            files[OUT / f"schemas/methods/{method}.params.schema.json"] = json.dumps(schema, indent=2) + "\n"
            entry["params"]["schema"] = f"schemas/methods/{method}.params.schema.json"
        else:
            local = dict(nested)
            if struct == "DialogParams":
                local["DialogFilter"] = parse_struct(src[where], "DialogFilter", nested)
            schema = parse_struct(src[where], struct, local)
            schema = {"$schema": "https://json-schema.org/draft/2020-12/schema",
                      "title": f"{method} params (Rust {struct})", **schema}
            if struct == "CommandRequest":
                schema["properties"]["command"] = {"$ref": "../document-command.schema.json"}
            if struct == "DialogParams":
                schema["$defs"] = {"DialogFilter": local["DialogFilter"]}
            files[OUT / f"schemas/methods/{method}.params.schema.json"] = json.dumps(schema, indent=2) + "\n"
            entry["params"] = {"decoded": True, "rust_type": struct, "schema": f"schemas/methods/{method}.params.schema.json",
                               "unknown_members": "rejected (-32602)" if schema.get("additionalProperties") is False else "ignored"}
        methods[method] = entry
    for method, note in SECONDARY_METHODS.items():
        methods[method] = {"direction": "secondary-launcher->host", "kind": "request", "params": {"decoded": False, "note": note}}
    for method, spec in HOST_REQUESTS.items():
        methods[method] = {"direction": "host->client", "kind": "request", **spec}
    for method, params in HOST_NOTIFICATIONS.items():
        methods[method] = {"direction": "host->client", "kind": "notification", "params": params}
    matrix = {
        "schema": 1,
        "protocol_version": 1,
        "generated_from": {k: p.relative_to(ROOT).as_posix() for k, p in SOURCES.items()},
        "generator": "scripts/generate-protocol-contract.py",
        "runtime_golden": "protocol/v1/conformance/golden.json (crates/nativehtmlkit-host/tests/protocol_conformance.rs)",
        "methods": dict(sorted(methods.items())),
        "document_command_ops": sorted(ops),
    }
    files[OUT / "method-matrix.json"] = json.dumps(matrix, indent=2) + "\n"
    return files


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    files = generate()
    stale = []
    for path, text in files.items():
        if args.check:
            if not path.is_file() or path.read_text(encoding="utf-8").replace("\r\n", "\n") != text:
                stale.append(path.relative_to(ROOT).as_posix())
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8", newline="\n")
    if args.check and stale:
        print("protocol contract is stale; run scripts/generate-protocol-contract.py:\n  " + "\n  ".join(stale))
        return 1
    print(f"{'checked' if args.check else 'wrote'} {len(files)} files; "
          f"{len(CLIENT_METHODS)} client methods, {len(json.loads(files[OUT / 'method-matrix.json'])['document_command_ops'])} document ops")
    return 0


if __name__ == "__main__":
    sys.exit(main())
