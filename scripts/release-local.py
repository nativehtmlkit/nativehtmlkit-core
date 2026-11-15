#!/usr/bin/env python3
"""Stage a locally built Core host + protocol contract; publish only with --publish + confirmation.

Staging (default for local qualification) never touches GitHub. The archive is verified from a
clean extraction: every file against SHA256SUMS, and the extracted host's --version.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import tomllib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO = "nativehtmlkit/nativehtmlkit-core"
LICENSES = {
    "LICENSE": ROOT / "LICENSE",
    "THIRD_PARTY_NOTICES.md": ROOT / "THIRD_PARTY_NOTICES.md",
    # The host embeds the Noto Sans last-resort font; its licence ships with every distributable.
    "licenses/NotoSans-OFL.txt": ROOT / "crates/nativehtmlkit-assets/assets/last-resort/OFL.txt",
    "SOURCE_PROVENANCE.json": ROOT / "SOURCE_PROVENANCE.json",
}


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True).stdout.strip()


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--version", default=None)
    p.add_argument("--stage-only", action="store_true")
    p.add_argument("--publish", action="store_true")
    p.add_argument("--confirm", default="")
    p.add_argument("--binary", type=pathlib.Path, default=None)
    a = p.parse_args()
    if a.publish and a.stage_only:
        p.error("Choose --stage-only or --publish")
    version = a.version or tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    exe = "nativehtmlkit-host.exe" if os.name == "nt" else "nativehtmlkit-host"
    binary = a.binary or ROOT / "target" / "release" / exe
    if not binary.is_file():
        p.error(f"Build nativehtmlkit-host locally first: {binary}")
    magic = binary.read_bytes()[:4]
    if os.name == "nt" and not magic.startswith(b"MZ"):
        p.error("Expected a Windows PE host executable")
    if os.name != "nt" and magic != b"\x7fELF":
        p.error("Expected a Linux ELF host executable")
    reported = subprocess.run([str(binary), "--version"], capture_output=True, text=True, timeout=15, check=True).stdout.strip()
    if reported != f"nativehtmlkit-host {version}":
        p.error(f"host --version {reported!r} does not match release version {version}")
    subprocess.run([sys.executable, str(ROOT / "scripts/generate-protocol-contract.py"), "--check"], check=True)
    dirty = bool(git("status", "--porcelain"))
    if a.publish:
        if a.confirm != "publish-" + version:
            p.error("Explicit --confirm publish-" + version + " required")
        if dirty:
            p.error("Clean commit required before publication")

    label = "windows-x64" if os.name == "nt" else "linux-x64"
    stage = ROOT / "dist" / f"nativehtmlkit-core-v{version}-{label}"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    shutil.copy2(binary, stage / exe)
    shutil.copytree(ROOT / "protocol", stage / "protocol")
    for name, src in LICENSES.items():
        (stage / name).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, stage / name)
    manifest = {
        "schema": 1,
        "component": "nativehtmlkit-core",
        "version": version,
        "platform": label,
        "host_executable": exe,
        "host_sha256": sha256(stage / exe),
        "host_version_output": reported,
        "protocol_version": 1,
        "protocol_status": "1.0-rc: wire contract locked by runtime golden; public stability withheld (see protocol/CONFORMANCE-ROADMAP.md)",
        "source_commit": git("rev-parse", "HEAD") or "uncommitted",
        "source_dirty": dirty,
        "toolchain": subprocess.run(["rustc", "--version"], cwd=ROOT, capture_output=True, text=True).stdout.strip(),
        "published": False,
    }
    (stage / "release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    files = sorted(x for x in stage.rglob("*") if x.is_file())
    (stage / "SHA256SUMS").write_text("".join(f"{sha256(x)}  {x.relative_to(stage).as_posix()}\n" for x in files))
    archive = stage.parent / (stage.name + ".zip")
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for f in sorted(x for x in stage.rglob("*") if x.is_file()):
            info = zipfile.ZipInfo.from_file(f, f.relative_to(stage).as_posix())
            with f.open("rb") as handle:
                z.writestr(info, handle.read(), compress_type=zipfile.ZIP_DEFLATED)

    # Verify from a clean extraction, not from the staging tree.
    with tempfile.TemporaryDirectory(prefix="nhk-core-verify-") as tmp:
        with zipfile.ZipFile(archive) as z:
            z.extractall(tmp)
        out = pathlib.Path(tmp)
        for line in (out / "SHA256SUMS").read_text().splitlines():
            digest, name = line.split("  ", 1)
            if sha256(out / name) != digest:
                p.error(f"archive verification failed for {name}")
        host = out / exe
        if os.name != "nt":
            host.chmod(0o755)
        if subprocess.run([str(host), "--version"], capture_output=True, text=True, timeout=15).stdout.strip() != reported:
            p.error("extracted host --version differs")
    (stage.parent / (archive.name + ".sha256")).write_text(f"{sha256(archive)}  {archive.name}\n")
    print(f"staged and verified: {archive}\n  sha256 {sha256(archive)}\n  host   {manifest['host_sha256']}\n  commit {manifest['source_commit']}{' (dirty)' if dirty else ''}")
    if a.publish:
        # gh release create does not trigger any Actions in this repository: no workflows shipped.
        subprocess.run(["gh", "release", "create", "v" + version, str(archive), "--repo", REPO, "--verify-tag",
                        "--title", "NativeHTMLKit Core v" + version, "--notes", "Locally built and tested."], check=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
