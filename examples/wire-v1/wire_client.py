"""A minimal, single-threaded NativeHTMLKit wire-v1 client using only the Python standard library.

What it teaches
    How a backend talks to ``nativehtmlkit-host`` without an SDK: reading the four launch
    variables, connecting to the private endpoint, length-prefixed JSON framing, the
    ``host.hello`` handshake, correlating responses by ``id``, answering host-originated
    ``{allow}`` requests and queueing notifications. It is the reference used by
    ``notes_app.py``, ``live_bitmap.py`` and ``scripts/docs/generate-html-css-reference.py``.

Design
    One thread does all I/O. ``request()`` writes a request and then reads frames until the
    matching response arrives; notifications read on the way are queued, host requests are
    answered immediately. ``next_message()`` blocks for the next notification. Because only one
    thread touches the stream, a plain synchronous Windows named-pipe handle is enough (with a
    reader thread and a writer thread you would need overlapped I/O on Windows).

This module is an example, not a supported SDK. Apps should use the Python or PowerShell SDK.
"""
from __future__ import annotations

import collections
import json
import os
import socket
import struct
import time
from typing import Any, Callable

PROTOCOL_VERSION = 1
MAX_FRAME_BYTES = 2 * 1024 * 1024  # protocol limit for every frame, both directions
LAUNCH_VARIABLES = ("UI4ME_ENDPOINT", "UI4ME_TOKEN", "UI4ME_APP_ID", "UI4ME_PROTOCOL_VERSION")


class WireError(Exception):
    """A JSON-RPC error response. Branch on ``code`` only; ``message`` is for humans."""

    def __init__(self, code: int, message: str, data: Any = None):
        super().__init__(f"{message} (RPC {code})")
        self.code, self.message, self.data = code, message, data


class Disconnected(Exception):
    """The host closed the connection (normal after ``app.shutdown``)."""


def _connect(endpoint: str, timeout: float = 10.0):
    """Return a binary stream object with ``read``/``write`` for the host endpoint."""
    deadline = time.monotonic() + timeout
    if os.name == "nt":  # \\.\pipe\nativehtmlkit-host-<key>
        while True:
            try:
                return open(endpoint, "r+b", buffering=0)
            except OSError:  # ERROR_PIPE_BUSY / not yet created: retry briefly
                if time.monotonic() > deadline:
                    raise
                time.sleep(0.05)
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    sock.connect(endpoint)
    sock.settimeout(None)
    return sock.makefile("rwb", buffering=0)


def _read_exact(stream, size: int) -> bytes:
    data = bytearray()
    while len(data) < size:
        chunk = stream.read(size - len(data))
        if not chunk:
            raise Disconnected("host closed the connection")
        data += chunk
    return bytes(data)


class Session:
    """One authenticated wire-v1 connection."""

    def __init__(self, stream, *, decide: Callable[[str, dict], bool] | None = None,
                 trace: Callable[[str, dict], None] | None = None):
        self._stream = stream
        self._next_id = 0
        self._responses: dict[str, dict] = {}
        self.notifications: collections.deque[dict] = collections.deque()
        # Called for host->client requests (window.before_close, app.before_quit).
        self.decide = decide or (lambda method, params: True)
        self._trace = trace
        self.hello: dict = {}

    # -- framing -------------------------------------------------------------------------
    def _send(self, message: dict) -> None:
        body = json.dumps(message, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if not 0 < len(body) <= MAX_FRAME_BYTES:
            raise ValueError("frame exceeds the 2 MiB protocol limit")
        if self._trace:
            self._trace("send", message)
        self._stream.write(struct.pack(">I", len(body)) + body)

    def _receive(self) -> dict:
        (length,) = struct.unpack(">I", _read_exact(self._stream, 4))
        if not 0 < length <= MAX_FRAME_BYTES:  # check before allocating
            raise Disconnected(f"invalid frame length {length}")
        message = json.loads(_read_exact(self._stream, length).decode("utf-8"))
        if self._trace:
            self._trace("receive", message)
        return message

    def _dispatch_one(self) -> None:
        """Read one frame and route it: host request, notification or response."""
        message = self._receive()
        if "method" in message and "id" in message:  # host -> client request
            method, params = message["method"], message.get("params", {})
            if method in ("window.before_close", "app.before_quit"):
                allow = bool(self.decide(method, params))
                self._send({"jsonrpc": "2.0", "id": message["id"], "result": {"allow": allow}})
            else:  # clients must answer unknown host requests with an error
                self._send({"jsonrpc": "2.0", "id": message["id"],
                            "error": {"code": -32601, "message": "Unknown method"}})
        elif "method" in message:  # notification
            self.notifications.append(message)
        elif message.get("id") is None:  # uncorrelated error: a client protocol bug
            raise WireError(message["error"]["code"], message["error"]["message"])
        else:
            self._responses[message["id"]] = message

    # -- API -----------------------------------------------------------------------------
    def request(self, method: str, params: dict | None = None) -> Any:
        """Send one request and wait for its response (correlated by id, not by order)."""
        self._next_id += 1
        request_id = f"c{self._next_id}"
        self._send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params or {}})
        while request_id not in self._responses:
            self._dispatch_one()
        response = self._responses.pop(request_id)
        if "error" in response:
            error = response["error"]
            raise WireError(error["code"], error["message"], error.get("data"))
        return response["result"]

    def next_message(self) -> dict:
        """Block until the next notification (document.event, window.event, app.shutdown...)."""
        while not self.notifications:
            self._dispatch_one()
        return self.notifications.popleft()

    def close(self) -> None:
        try:
            self._stream.close()
        except OSError:
            pass


def connect_from_environment(**options) -> Session:
    """Read and remove the launch variables, connect and perform ``host.hello``."""
    values = {name: os.environ.pop(name, None) for name in LAUNCH_VARIABLES}
    endpoint, token = values["UI4ME_ENDPOINT"], values["UI4ME_TOKEN"]
    if not endpoint or not token:
        raise RuntimeError("not launched by nativehtmlkit-host (UI4ME_ENDPOINT/UI4ME_TOKEN missing)")
    if len(token) != 64 or any(c not in "0123456789abcdefABCDEF" for c in token):
        raise RuntimeError("malformed launch token")
    if values["UI4ME_PROTOCOL_VERSION"] != str(PROTOCOL_VERSION):
        raise RuntimeError(f"host offers protocol {values['UI4ME_PROTOCOL_VERSION']}, not 1")
    session = Session(_connect(endpoint), **options)
    session.hello = session.request("host.hello", {"token": token, "protocol_version": PROTOCOL_VERSION})
    if session.hello.get("protocol_version") != PROTOCOL_VERSION:
        raise RuntimeError("host did not confirm protocol version 1")
    return session


class Document:
    """Convenience wrapper: ``document.command`` for one (window_id, document_id) pair."""

    def __init__(self, session: Session, window_id: str, document_id: str):
        self.session, self.window_id, self.document_id = session, window_id, document_id

    def command(self, op: str, **fields) -> Any:
        return self.session.request("document.command", {
            "window_id": self.window_id, "document_id": self.document_id,
            "command": {"op": op, **fields}})


def write_manifest(directory: str, *, app_id: str, name: str, program: str, args: list[str],
                   cwd: str | None = None, env: dict[str, str] | None = None,
                   instance: str = "multiple") -> str:
    """Write an ``app.toml`` for ``nativehtmlkit-host run --app``. JSON strings are valid TOML
    basic strings, so ``json.dumps`` quotes every value safely."""
    q = lambda value: json.dumps(value, ensure_ascii=False)  # noqa: E731
    lines = ["[app]", f"id = {q(app_id)}", f"name = {q(name)}", "",
             "[backend]", f"program = {q(program)}",
             "args = [" + ", ".join(q(a) for a in args) + "]",
             f"cwd = {q(cwd or directory)}", "", "[backend.env]"]
    lines += [f"{q(k)} = {q(v)}" for k, v in sorted((env or {}).items())]
    lines += ["", "[host]", f"instance = {q(instance)}", ""]
    path = os.path.join(directory, "app.toml")
    with open(path, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
    return path


def relay_host_stderr(line: bytes, *, verbose: bool = False) -> str | None:
    """The host writes JSON log records to stderr; backend stdout/stderr lines arrive as records
    with ``fields.source == "backend"``. Return the text worth showing, or None."""
    text = line.decode("utf-8", "replace").rstrip("\r\n")
    try:
        record = json.loads(text)
    except ValueError:
        return text or None
    fields = record.get("fields", {}) if isinstance(record, dict) else {}
    if fields.get("source") == "backend":
        return fields.get("text")
    if verbose or str(record.get("level", "")).upper() in ("WARN", "ERROR"):
        return f"host {record.get('level', '').lower()}: {fields.get('message', text)}"
    return None
