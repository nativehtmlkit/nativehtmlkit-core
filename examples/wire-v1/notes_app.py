#!/usr/bin/env python3
"""Quick notes: a complete wire-v1 application written against the raw protocol.

What it teaches
    The whole life of a wire-v1 backend, without an SDK: launching the host with an
    ``app.toml`` manifest, ``host.hello``, ``window.create`` -> ``document.load`` ->
    ``app.ready``, reacting to ``document.event`` notifications (``submit``, ``click``,
    ``invalid``), changing the page with ``document.command`` ops (``get_value``,
    ``set_value``, ``insert_html``, ``set_text``, ``replace_children_html``), escaping
    untrusted text, answering ``app.before_quit`` and exiting on ``app.shutdown``.

How to run
    python3 notes_app.py --host /path/to/nativehtmlkit-host            # real window
    python3 notes_app.py --host /path/to/nativehtmlkit-host --self-test --headless
    python3 notes_app.py --host ... --self-test --headless --transcript session.jsonl

    ``--host`` defaults to $NATIVEHTMLKIT_HOST_BINARY. ``--headless`` (or
    NATIVEHTMLKIT_TEST_MODE=1) runs the host without a desktop. ``--self-test`` makes the
    backend drive its own form (there is no synthetic input in headless mode), check the
    result through the public API and exit with status 0 only if every check passed.
    ``--transcript`` writes every frame (launch token redacted) as JSON lines.

The script has two roles: started by you it is the *launcher*; started by the host (the
launch variables are present) it is the *backend*.
"""
from __future__ import annotations

import argparse
import html
import json
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wire_client  # noqa: E402  (the example client next to this file)

PAGE = """<!doctype html>
<html lang="en"><head><style>
  body { font: 15px system-ui, sans-serif; margin: 0; padding: 16px; color: #1f2328; }
  form { display: flex; gap: 8px; }
  input { flex: 1; padding: 6px 8px; border: 1px solid #8c959f; border-radius: 6px; }
  input:invalid { border-color: #cf222e; }
  button { padding: 6px 12px; border-radius: 6px; border: 1px solid #8c959f; background: #f6f8fa; }
  #error { color: #cf222e; min-height: 1.2em; margin: 6px 0; }
  ul { padding-left: 20px; } li { margin: 4px 0; }
  footer { display: flex; justify-content: space-between; align-items: center; color: #57606a; }
</style></head><body>
  <h1>Quick notes</h1>
  <form id="add">
    <input id="note" name="note" required maxlength="200" placeholder="Write a note" aria-label="Note">
    <button id="save" type="submit">Add</button>
  </form>
  <p id="error" role="alert"></p>
  <ul id="notes"></ul>
  <footer><span id="count">0 notes</span><button id="clear" type="button">Clear all</button></footer>
</body></html>"""


# ---------------------------------------------------------------------------------- backend
class NotesBackend:
    def __init__(self, session: wire_client.Session, doc: wire_client.Document):
        self.session, self.doc, self.notes = session, doc, 0

    def show_count(self) -> None:
        self.doc.command("set_text", element="count", text=f"{self.notes} note" + ("" if self.notes == 1 else "s"))

    def add_note(self) -> None:
        text = self.doc.command("get_value", element="note")["value"].strip()
        if not text:
            self.doc.command("set_text", element="error", text="A note cannot be only spaces.")
            return
        # insert_html parses markup: escape untrusted text. (set_text needs no escaping.)
        self.doc.command("insert_html", element="notes", position="beforeend",
                         html=f"<li>{html.escape(text)}</li>")
        self.notes += 1
        self.doc.command("set_value", element="note", value="")
        self.doc.command("set_text", element="error", text="")
        self.show_count()

    def handle(self, event: dict) -> None:
        kind = event["type"]
        # Match on element OR path: an inner element with its own id becomes `element`.
        targets = {event.get("element"), *event.get("path", [])}
        if kind == "submit" and "add" in targets:
            self.add_note()
        elif kind == "invalid" and event.get("element") == "note":
            self.doc.command("set_text", element="error", text="Please write something first.")
        elif kind == "click" and "clear" in targets:
            self.doc.command("replace_children_html", element="notes", html="")
            self.notes = 0
            self.show_count()

    def run(self) -> None:
        """Event loop: dispatch document events until the host announces shutdown."""
        while True:
            message = self.session.next_message()
            method = message["method"]
            if method == "app.shutdown":  # release resources, exit before deadline_ms
                return
            if method == "document.event" and message["params"]["document_id"] == self.doc.document_id:
                self.handle(message["params"]["event"])
            elif method == "document.error":
                print("document error:", message["params"].get("message"), file=sys.stderr)


def drain_events(backend: NotesBackend) -> None:
    """Self-test helper: handle the events that commands already produced."""
    while backend.session.notifications:
        message = backend.session.notifications.popleft()
        if message["method"] == "document.event":
            backend.handle(message["params"]["event"])


def self_test(backend: NotesBackend) -> list[str]:
    doc, failures = backend.doc, []

    def check(label: str, ok: bool) -> None:
        print(("PASS " if ok else "FAIL ") + label)
        if not ok:
            failures.append(label)

    # 1. An empty required field: request_submit runs constraint validation instead.
    result = doc.command("request_submit", element="add")
    drain_events(backend)
    check("empty form is not submitted", result.get("submitted") is False)
    check("invalid event shows an error", doc.command("get_text", element="error")["text"] == "Please write something first.")
    # 2. Untrusted text is stored as text, not markup.
    doc.command("set_value", element="note", value="<b>milk</b> & eggs")
    result = doc.command("request_submit", element="add")
    drain_events(backend)
    check("valid form is submitted", result.get("submitted") is True)
    check("note is escaped", doc.command("get_text", element="notes")["text"] == "<b>milk</b> & eggs")
    check("no <b> element was injected", doc.command("query_nodes", selector="#notes b")["total"] == 0)
    check("input was cleared", doc.command("get_value", element="note")["value"] == "")
    # 3. Whitespace passes `required` but not our own rule.
    doc.command("set_value", element="note", value="   ")
    doc.command("request_submit", element="add")
    drain_events(backend)
    check("whitespace note rejected", doc.command("query_nodes", selector="#notes li")["total"] == 1)
    doc.command("set_value", element="note", value="call Ana")
    doc.command("request_submit", element="add")
    drain_events(backend)
    check("counter reads 2 notes", doc.command("get_text", element="count")["text"] == "2 notes")
    return failures


def backend_main(args: argparse.Namespace) -> int:
    transcript = open(args.transcript, "w", encoding="utf-8") if args.transcript else None
    token = os.environ.get("UI4ME_TOKEN", "")

    def trace(direction: str, message: dict) -> None:
        line = json.dumps({"dir": direction, "frame": message}, ensure_ascii=False)
        transcript.write(line.replace(token, "<redacted-token>") + "\n")

    session = wire_client.connect_from_environment(trace=trace if transcript else None)
    try:
        window = session.request("window.create", {"title": "Quick notes", "width": 520, "height": 420})
        loaded = session.request("document.load", {"window_id": window["window_id"], "html": PAGE})
        doc = wire_client.Document(session, window["window_id"], loaded["document_id"])
        session.request("app.ready")
        backend = NotesBackend(session, doc)
        if args.self_test:
            failures = self_test(backend)
            print("SELF-TEST " + ("PASS" if not failures else "FAIL"))
            session.request("app.request_quit")  # host asks app.before_quit; we allow it
            backend.run()  # returns at app.shutdown
            return 1 if failures else 0
        backend.run()
        return 0
    except wire_client.Disconnected:
        return 0
    finally:
        session.close()
        if transcript:
            transcript.close()


# --------------------------------------------------------------------------------- launcher
def launcher_main(args: argparse.Namespace) -> int:
    host = args.host or os.environ.get("NATIVEHTMLKIT_HOST_BINARY")
    if not host or not os.path.isfile(host):
        print("Pass --host PATH or set NATIVEHTMLKIT_HOST_BINARY to a nativehtmlkit-host binary.", file=sys.stderr)
        return 2
    headless = args.headless or os.environ.get("NATIVEHTMLKIT_TEST_MODE") == "1"
    backend_args = [os.path.abspath(__file__), "--backend"]
    if args.self_test:
        backend_args.append("--self-test")
    if args.transcript:
        backend_args += ["--transcript", os.path.abspath(args.transcript)]
    with tempfile.TemporaryDirectory(prefix="notes-app-") as folder:
        manifest = wire_client.write_manifest(
            folder, app_id="org.example.QuickNotes", name="Quick notes",
            program=sys.executable, args=backend_args, cwd=os.getcwd(),
            env={"PYTHONUNBUFFERED": "1", "PYTHONIOENCODING": "utf-8"})
        command = [host, "run", "--app", manifest] + (["--headless"] if headless else [])
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stderr=subprocess.PIPE)
        passed = False
        for line in process.stderr:  # backend print() output arrives here as host records
            text = wire_client.relay_host_stderr(line)
            if text is not None:
                print(text, flush=True)
                passed = passed or text == "SELF-TEST PASS"
        status = process.wait()
    if args.self_test:
        return 0 if (status == 0 and passed) else 1
    return status


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--host", help="path to nativehtmlkit-host (default: $NATIVEHTMLKIT_HOST_BINARY)")
    parser.add_argument("--headless", action="store_true", help="run the host without a desktop")
    parser.add_argument("--self-test", action="store_true", help="drive the form and verify the result")
    parser.add_argument("--transcript", help="write every wire frame to this JSON-lines file")
    parser.add_argument("--backend", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    return backend_main(args) if args.backend else launcher_main(args)


if __name__ == "__main__":
    raise SystemExit(main())
