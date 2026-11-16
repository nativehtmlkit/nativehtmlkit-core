#!/usr/bin/env python3
"""Live chart: stream RGBA frames into an <img> through the wire-v1 bitmap resource.

What it teaches
    The bitmap lifecycle a client author must implement: ``bitmap.create`` returns a private
    file path plus ``slots``/``slot_bytes``/``stride``; the backend maps the file, writes
    straight-alpha RGBA8 pixels into one slot, calls ``bitmap.present {slot, sequence}`` and
    reuses the slot only after the acknowledgement. Markup refers to the bitmap as
    ``<img src="nativehtmlkit-bitmap://<bitmap_id>">``. Pixels never travel through JSON.

How to run
    python3 live_bitmap.py --host /path/to/nativehtmlkit-host                 # real window, ~10 s
    python3 live_bitmap.py --host /path/to/nativehtmlkit-host --headless --self-test

    ``--self-test`` presents a fixed number of frames, checks the acknowledgements and the
    image's layout size, then quits (exit status 0 = every check passed).
"""
from __future__ import annotations

import argparse
import math
import mmap
import os
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wire_client  # noqa: E402

WIDTH, HEIGHT = 320, 120
PAGE = f"""<!doctype html><html><head><style>
  body {{ font: 14px system-ui, sans-serif; margin: 16px; background: #0d1117; color: #e6edf3; }}
  img {{ display: block; border: 1px solid #30363d; border-radius: 6px; width: {WIDTH}px; height: {HEIGHT}px; }}
</style></head><body>
  <h2>Signal</h2>
  <img id="chart" alt="Live signal chart" src="nativehtmlkit-bitmap://chart">
  <p id="status">waiting for the first frame</p>
</body></html>"""


def draw(buffer: memoryview, offset: int, stride: int, phase: float) -> None:
    """Render one frame: dark background, grid lines and a moving sine trace."""
    row_bg = bytes((13, 17, 23, 255)) * WIDTH
    grid = bytes((48, 54, 61, 255)) * WIDTH
    for y in range(HEIGHT):
        start = offset + y * stride
        buffer[start:start + WIDTH * 4] = grid if y % 30 == 0 else row_bg
    previous = None
    for x in range(WIDTH):
        value = math.sin(phase + x / 25.0) * 0.6 + math.sin(phase * 0.7 + x / 9.0) * 0.25
        y = int((1 - (value + 1) / 2) * (HEIGHT - 1))
        low, high = (y, y) if previous is None else (min(previous, y), max(previous, y))
        for yy in range(low, high + 1):
            pixel = offset + yy * stride + x * 4
            buffer[pixel:pixel + 4] = bytes((88, 166, 255, 255))
        previous = y


def backend_main(self_test: bool) -> int:
    session = wire_client.connect_from_environment()
    failures: list[str] = []

    def check(label: str, ok: bool) -> None:
        print(("PASS " if ok else "FAIL ") + label)
        if not ok:
            failures.append(label)

    try:
        window = session.request("window.create", {"title": "Live chart", "width": 380, "height": 260})
        loaded = session.request("document.load", {"window_id": window["window_id"], "html": PAGE})
        ids = {"window_id": window["window_id"], "document_id": loaded["document_id"]}
        doc = wire_client.Document(session, **ids)
        info = session.request("bitmap.create", {**ids, "bitmap_id": "chart", "width": WIDTH, "height": HEIGHT, "slots": 2})
        check("bitmap.create returns RGBA8 with stride width*4",
              info["pixel_format"] == "RGBA8" and info["stride"] == WIDTH * 4)
        session.request("app.ready")
        with open(info["path"], "r+b") as handle:  # map before the first present
            mapping = mmap.mmap(handle.fileno(), info["slot_bytes"] * info["slots"])
        view = memoryview(mapping)
        frames = 12 if self_test else 300
        acknowledged = 0
        for sequence in range(1, frames + 1):
            slot = sequence % info["slots"]
            draw(view, slot * info["slot_bytes"], info["stride"], sequence / 6.0)
            ack = session.request("bitmap.present", {**ids, "bitmap_id": "chart", "slot": slot, "sequence": sequence})
            acknowledged += bool(ack["presented_to_engine"] and ack["slot_released"])
            if sequence % 30 == 0 or sequence == frames:
                doc.command("set_text", element="status", text=f"frame {sequence}")
            if not self_test:
                time.sleep(1 / 30)
        stale = session.request("bitmap.present", {**ids, "bitmap_id": "chart", "slot": 0, "sequence": 1})
        if self_test:
            check(f"{frames} frames presented", acknowledged == frames)
            check("an old sequence is not presented again", stale["presented_to_engine"] is False)
            rect = doc.command("get_rect", element="chart")["rect"]
            check("img lays out at the bitmap size", (rect["width"], rect["height"]) == (WIDTH + 2, HEIGHT + 2))
            check("status text updated", doc.command("get_text", element="status")["text"] == f"frame {frames}")
        view.release()
        mapping.close()
        check("bitmap.close releases it", session.request("bitmap.close", {**ids, "bitmap_id": "chart"})["closed"] is True)
        if self_test:
            print("SELF-TEST " + ("PASS" if not failures else "FAIL"))
        session.request("app.request_quit")
        while session.next_message()["method"] != "app.shutdown":
            pass
        return 1 if failures else 0
    except wire_client.Disconnected:
        return 0
    finally:
        session.close()


def launcher_main(args: argparse.Namespace) -> int:
    host = args.host or os.environ.get("NATIVEHTMLKIT_HOST_BINARY")
    if not host or not os.path.isfile(host):
        print("Pass --host PATH or set NATIVEHTMLKIT_HOST_BINARY.", file=sys.stderr)
        return 2
    headless = args.headless or os.environ.get("NATIVEHTMLKIT_TEST_MODE") == "1"
    backend_args = [os.path.abspath(__file__), "--backend"] + (["--self-test"] if args.self_test else [])
    with tempfile.TemporaryDirectory(prefix="live-chart-") as folder:
        manifest = wire_client.write_manifest(folder, app_id="org.example.LiveChart", name="Live chart",
                                              program=sys.executable, args=backend_args,
                                              env={"PYTHONUNBUFFERED": "1"})
        command = [host, "run", "--app", manifest] + (["--headless"] if headless else [])
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stderr=subprocess.PIPE)
        passed = False
        for line in process.stderr:
            text = wire_client.relay_host_stderr(line)
            if text is not None:
                print(text, flush=True)
                passed = passed or text == "SELF-TEST PASS"
        status = process.wait()
    return (0 if status == 0 and passed else 1) if args.self_test else status


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--host")
    parser.add_argument("--headless", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--backend", action="store_true", help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    raise SystemExit(backend_main(arguments.self_test) if arguments.backend else launcher_main(arguments))
