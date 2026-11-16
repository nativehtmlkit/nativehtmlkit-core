#!/usr/bin/env python3
"""Verify Core's documentation against the contract and a real host.

    python3 scripts/docs/check-docs.py --host PATH/TO/nativehtmlkit-host

Checks, each reported PASS/FAIL (SKIP when it needs a host and none was given):
  * protocol/v1 contract is unchanged relative to the Rust source (generate-protocol-contract --check)
  * protocol/reference/wire-tables.md matches protocol/v1 (generate-wire-tables --check)
  * docs/reference/html-css-profile.md matches the live host (generate-html-css-reference --check)
  * every example under examples/wire-v1 passes its headless --self-test
  * no host process started by this run is left behind
  * relative Markdown links in README.md, docs/, protocol/ and examples/ resolve
Exit status 0 only if nothing failed.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXAMPLES = ["examples/wire-v1/notes_app.py", "examples/wire-v1/live_bitmap.py"]


def host_processes(host: str) -> set[int]:
    """PIDs whose executable is this host binary (Linux /proc; empty elsewhere)."""
    found = set()
    proc = Path("/proc")
    if not proc.is_dir():
        return found
    target = os.path.realpath(host)
    for entry in proc.iterdir():
        if entry.name.isdigit():
            try:
                if os.path.realpath(entry / "exe") == target:
                    found.add(int(entry.name))
            except OSError:
                continue
    return found


def broken_links() -> list[str]:
    problems = []
    files = [ROOT / "README.md", *ROOT.glob("docs/**/*.md"), *ROOT.glob("protocol/*.md"),
             *ROOT.glob("protocol/reference/*.md"), *ROOT.glob("examples/**/*.md")]
    for file in files:
        if "Contract-Review" in file.parts:
            continue
        for target in re.findall(r"\]\(([^)\s]+)\)", file.read_text(encoding="utf-8")):
            if re.match(r"^[a-z]+:", target) or target.startswith("#"):
                continue
            path = (file.parent / target.split("#", 1)[0]).resolve()
            if not path.exists():
                problems.append(f"{file.relative_to(ROOT)} -> {target}")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--host", default=os.environ.get("NATIVEHTMLKIT_HOST_BINARY"))
    args = parser.parse_args()
    results: list[tuple[str, str]] = []

    def run(label: str, command: list[str], needs_host: bool = False) -> None:
        if needs_host and not args.host:
            results.append(("SKIP", label))
            return
        process = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=600)
        status = "PASS" if process.returncode == 0 else "FAIL"
        results.append((status, label))
        if status == "FAIL":
            print(process.stdout[-4000:], process.stderr[-4000:], sep="\n", file=sys.stderr)

    py = sys.executable
    before = host_processes(args.host) if args.host else set()
    run("protocol/v1 contract matches the Rust source", [py, "scripts/generate-protocol-contract.py", "--check"])
    run("wire tables match protocol/v1", [py, "scripts/docs/generate-wire-tables.py", "--check"])
    run("HTML/CSS profile matches the live host",
        [py, "scripts/docs/generate-html-css-reference.py", "--check", "--host", args.host or ""], needs_host=True)
    for example in EXAMPLES:
        run(f"{example} --self-test (headless)",
            [py, example, "--host", args.host or "", "--headless", "--self-test"], needs_host=True)
    if args.host:
        leftover = host_processes(args.host) - before
        results.append(("PASS" if not leftover else "FAIL", f"no leftover host processes {sorted(leftover) or ''}".strip()))
    links = broken_links()
    results.append(("PASS" if not links else "FAIL", "relative Markdown links resolve"))
    for link in links:
        print("broken link:", link, file=sys.stderr)
    for status, label in results:
        print(f"{status} {label}")
    counts = {s: sum(1 for r, _ in results if r == s) for s in ("PASS", "FAIL", "SKIP")}
    print(f"PASS={counts['PASS']} FAIL={counts['FAIL']} SKIP={counts['SKIP']}")
    return 1 if counts["FAIL"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
