#!/usr/bin/env python3
"""Stage a locally built Core host + protocol contract; publish only with --publish + confirmation.

Staging (default for local qualification) never touches GitHub. The archive is verified from a
clean extraction: every file against SHA256SUMS, and the extracted host's --version.

Provenance: the manifest records the HEAD commit, whether the tree is dirty, a reproducible
`source_tree` hash of the working tree (built in a temporary index: the real index and working
tree are never modified), the SHA-256 of the diff from HEAD to that tree when dirty, the host
SHA-256, the build target triple and the rustc/cargo versions. Git failures abort; nothing is
ever reported clean on error.

Licences: `licenses/THIRD-PARTY-LICENSES.txt` is generated from `cargo metadata --locked` for the
normal-dependency tree of `nativehtmlkit-host` on the build target, with each crate's licence and
notice files. Staging fails if a crate has neither a licence text nor a declared licence.

The zip is deterministic: sorted entries, the HEAD commit time as every entry's timestamp, fixed
permissions. The host binary itself is not bit-reproducible.

Binary/source binding (FR-R-04): the host embeds the source identity of the tree it was built
from (crates/nativehtmlkit-host/build.rs; `nativehtmlkit-host --version --verbose`). Staging and
publication refuse a binary whose embedded commit, tree hash and dirty flag differ from the
checkout being staged, or whose identity is unknown. A source export without git carries
`SOURCE_IDENTITY.json` (written by `--write-source-identity DIR` from a clean checkout); there the
listed files are re-hashed and must match both the file and the binary. Staging also refuses a
binary that embeds the local home, Cargo home or source directory path; `--build` builds the
release host with the required `--remap-path-prefix` flags (used by build-local.sh/.ps1).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO = "nativehtmlkit/nativehtmlkit-core"
REPO_URL = "https://github.com/" + REPO
HOST_PACKAGE = "nativehtmlkit-host"
RENDERER = ROOT / "crates/nativehtmlkit-renderer/src"
# Files copied verbatim into the stage (published path -> source).
STATIC_FILES = {
    "LICENSE": ROOT / "LICENSE",
    "THIRD_PARTY_NOTICES.md": ROOT / "THIRD_PARTY_NOTICES.md",
    "SOURCE_PROVENANCE.json": ROOT / "SOURCE_PROVENANCE.json",
    # The host embeds Noto Sans once (renderer last resort, also used by `probe` since 1.0.3).
    "licenses/NotoSans-OFL.txt": ROOT / "crates/nativehtmlkit-assets/assets/last-resort/OFL.txt",
    # Hyphenation patterns embedded by the renderer (MIT, BSD-3-Clause, notice-preservation terms).
    "licenses/hyphenation-NOTICES.txt": RENDERER / "text/data/hyphenation/NOTICES.txt",
    # UA stylesheets embedded by the renderer: source, attribution and the list of changes (CC-BY-4.0 3(a)(1)(B)).
    "licenses/UA-STYLESHEETS-PROVENANCE.md": RENDERER / "style/ua/PROVENANCE.md",
    # Full MPL-2.0 text, once, for the Stylo family, cssparser and the other MPL-2.0 crates.
    "licenses/MPL-2.0.txt": ROOT / "patches/licenses/MPL-2.0.txt",
}
UA_NOTICE = """\
User-agent stylesheets embedded in nativehtmlkit-host (html.css, forms.css, quirks.css)

Derived from the WHATWG HTML Living Standard, section 15 "Rendering",
https://html.spec.whatwg.org/multipage/rendering.html
(snapshot whatwg/html @ 6d7788bc56a4b39d955a9a7fdb501c7c7d27999a).
Copyright (c) WHATWG (Apple, Google, Mozilla, Microsoft).
Licensed under the Creative Commons Attribution 4.0 International License (CC-BY-4.0),
https://creativecommons.org/licenses/by/4.0/
The material was modified; the rules taken and every modification (M1-M21) are listed in
UA-STYLESHEETS-PROVENANCE.md next to this file. Block 2 of forms.css is NativeHTMLKit-owned.

LICENCE STATUS: CC-BY-4.0 for these stylesheets (decision OD-26) was APPROVED BY THE PROJECT
OWNER on 2026-11-15 (see crates/nativehtmlkit-renderer/src/style/ua/PROVENANCE.md).
"""
LICENCE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)", re.I)
STYLO_UPSTREAM = "https://github.com/servo/stylo @ 37c1ed3b707c8f333304de2107c13b68fbc1057c"
TAFFY_UPSTREAM = "https://github.com/DioxusLabs/taffy @ fca167cf659b8bf48019d0dea90624109a78103f"


class ReleaseError(SystemExit):
    def __init__(self, message: str) -> None:
        super().__init__(f"release-local.py: error: {message}")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256(path: pathlib.Path) -> str:
    return sha256_bytes(path.read_bytes())


def run(args: list[str], *, env: dict | None = None, text: bool = True) -> str | bytes:
    """Run a command in ROOT; any failure (missing tool or non-zero exit) aborts the release."""
    try:
        done = subprocess.run(args, cwd=ROOT, capture_output=True, text=text, env=env,
                              encoding="utf-8" if text else None, errors="replace" if text else None)
    except OSError as error:
        raise ReleaseError(f"cannot run {args[0]}: {error}")
    if done.returncode != 0:
        stderr = done.stderr if text else done.stderr.decode(errors="replace")
        raise ReleaseError(f"{' '.join(args)} failed ({done.returncode}): {stderr.strip()}")
    return done.stdout.strip() if text else done.stdout


def git(*args: str, env: dict | None = None) -> str:
    # GIT_OPTIONAL_LOCKS=0: `git status` must not refresh (rewrite) the real index.
    return run(["git", *args], env=dict(env or os.environ, GIT_OPTIONAL_LOCKS="0"))


IDENTITY_FILE = "SOURCE_IDENTITY.json"
IDENTITY_SCHEMA = "nativehtmlkit-core-source-identity/1"


def files_digest(root: pathlib.Path, files: list[str]) -> str:
    """SHA-256 over `"<sha256>  <path>\n"` lines, in list order (same as host build.rs)."""
    total = hashlib.sha256()
    for name in files:
        total.update(f"{sha256(root / name)}  {name}\n".encode("utf-8"))
    return total.hexdigest()


def is_git_checkout() -> bool:
    if shutil.which("git") is None:
        return False
    done = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=ROOT, capture_output=True, text=True,
                          env=dict(os.environ, GIT_OPTIONAL_LOCKS="0"))
    return done.returncode == 0 and pathlib.Path(done.stdout.strip()).resolve() == ROOT


def write_source_identity(export: pathlib.Path) -> None:
    """Write SOURCE_IDENTITY.json for a `git archive HEAD` export of this clean checkout."""
    source = source_identity()
    if source["source_dirty"]:
        raise ReleaseError("a source identity file can only describe a clean commit")
    files = sorted(git("ls-tree", "-r", "--name-only", "HEAD").splitlines())
    bad = [f for f in files if any(c in f for c in ',"\\]') or f.startswith("/")]
    if bad:
        raise ReleaseError(f"file names unsupported by the identity format: {bad[:5]}")
    missing = [f for f in files if not (export / f).is_file()]
    if missing:
        raise ReleaseError(f"{export} lacks {len(missing)} tracked files, e.g. {missing[:5]}")
    digest = files_digest(ROOT, files)
    if files_digest(export, files) != digest:
        raise ReleaseError(f"{export} differs from HEAD (line endings or edits); export with `git archive HEAD`")
    record = {
        "schema_name": IDENTITY_SCHEMA,
        "source_commit": source["source_commit"],
        "source_commit_time": source["source_commit_time"],
        "source_tree": source["source_tree"],
        "files_sha256": digest,
        "files": files,
    }
    (export / IDENTITY_FILE).write_text(json.dumps(record, indent=1, ensure_ascii=False) + "\n",
                                        encoding="utf-8", newline="\n")
    print(f"wrote {export / IDENTITY_FILE}: commit {source['source_commit']}, {len(files)} files")


def export_identity() -> dict:
    """The identity of a non-git source export, verified against its SOURCE_IDENTITY.json."""
    path = pathlib.Path(os.environ.get("NATIVEHTMLKIT_SOURCE_IDENTITY") or ROOT / IDENTITY_FILE)
    try:
        record = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise ReleaseError(f"not a git checkout and no readable {IDENTITY_FILE} ({error}); "
                           "source provenance cannot be established")
    if record.get("schema_name") != IDENTITY_SCHEMA or not isinstance(record.get("files"), list):
        raise ReleaseError(f"{path} is not a {IDENTITY_SCHEMA} record")
    digest = files_digest(ROOT, record["files"])
    if digest != record.get("files_sha256"):
        raise ReleaseError(f"the source export differs from {path} (listed files changed)")
    return {
        "source_commit": record["source_commit"],
        "source_commit_time": int(record["source_commit_time"]),
        "source_dirty": False,
        "source_tree": record["source_tree"],
        "source_tree_method": f"from {IDENTITY_FILE} of a source export; its {len(record['files'])} listed files verified by SHA-256",
        "source_files_sha256": digest,
        "source_identity": "file",
    }


def checkout_identity() -> dict:
    if is_git_checkout():
        return {**source_identity(), "source_identity": "git"}
    return export_identity()


def embedded_identity(binary: pathlib.Path) -> dict:
    done = subprocess.run([str(binary), "--version", "--verbose"], capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=15)
    try:
        value = json.loads(done.stdout)
    except ValueError:
        value = None
    if done.returncode != 0 or not isinstance(value, dict) or "source_identity" not in value:
        raise ReleaseError(f"{binary} embeds no source identity (`--version --verbose` failed); "
                           "it predates FR-R-04 or is not a nativehtmlkit-host 1.0.3+ build")
    return value


def identity_mismatch(embedded: dict, checkout: dict) -> list[str]:
    """Differences between the binary's embedded identity and the checkout (empty: bound)."""
    if embedded.get("source_identity") not in ("git", "file"):
        return [f"the binary's source identity is unknown ({embedded.get('source_identity_note')})"]
    keys = ["source_identity", "source_commit", "source_tree", "source_dirty"]
    if checkout.get("source_identity") == "file":
        keys.append("source_files_sha256")
    return [f"{k}: binary {embedded.get(k)!r}, checkout {checkout.get(k)!r}" for k in keys if embedded.get(k) != checkout.get(k)]


def leaked_paths(binary: pathlib.Path) -> list[str]:
    """Local paths (home, Cargo home, source root) embedded in the binary (FR-R-06)."""
    data = binary.read_bytes()
    home = pathlib.Path.home()
    cargo_home = pathlib.Path(os.environ.get("CARGO_HOME") or home / ".cargo")
    found = []
    for path in {home, cargo_home, ROOT}:
        for text in {str(path), path.as_posix()}:
            if len(text) > 3 and text.encode("utf-8") in data:
                found.append(text)
    return sorted(set(found))


def remap_rustflags() -> list[str]:
    """`--remap-path-prefix` flags for the release build: the most specific prefix comes last
    (rustc applies the last matching remapping)."""
    home = pathlib.Path.home()
    cargo_home = pathlib.Path(os.environ.get("CARGO_HOME") or home / ".cargo")
    flags = [f"--remap-path-prefix={home}=/home", f"--remap-path-prefix={cargo_home}=/cargo",
             f"--remap-path-prefix={ROOT}=/nativehtmlkit-core"]
    if os.name == "nt":
        # The PE debug directory names the PDB by absolute path; keep only its file name.
        flags.append("-Clink-arg=/PDBALTPATH:%_PDB%")
    return flags


def build_release_host() -> None:
    env = dict(os.environ, CARGO_ENCODED_RUSTFLAGS="\x1f".join(remap_rustflags()))
    env.pop("RUSTFLAGS", None)
    done = subprocess.run(["cargo", "build", "--locked", "--release", "-p", HOST_PACKAGE], cwd=ROOT, env=env)
    if done.returncode != 0:
        raise ReleaseError(f"release build failed ({done.returncode})")


def source_identity() -> dict:
    if shutil.which("git") is None:
        raise ReleaseError("git is required to record source provenance")
    if pathlib.Path(git("rev-parse", "--show-toplevel")).resolve() != ROOT:
        raise ReleaseError(f"{ROOT} is not the top of a git checkout")
    commit = git("rev-parse", "--verify", "HEAD^{commit}")
    head_tree = git("rev-parse", "HEAD^{tree}")
    status = git("status", "--porcelain=v1", "--untracked-files=all")
    # Hash the working tree as git would commit it (tracked + untracked, honouring .gitignore)
    # through a temporary index. The real index and the working tree are not touched; only
    # loose objects are added to the object store.
    with tempfile.TemporaryDirectory(prefix="nhk-core-index-") as tmp:
        index = pathlib.Path(tmp) / "index"
        env = dict(os.environ, GIT_INDEX_FILE=str(index))
        real_index = ROOT / git("rev-parse", "--git-path", "index")
        if real_index.is_file():
            shutil.copyfile(real_index, index)  # reuse its stat cache; `add -A` makes it match the tree
        else:
            git("read-tree", "HEAD", env=env)
        git("add", "-A", "--", ".", env=env)
        tree = git("write-tree", env=env)
    dirty = bool(status) or tree != head_tree
    identity = {
        "source_commit": commit,
        "source_commit_time": int(git("show", "-s", "--format=%ct", "HEAD")),
        "source_dirty": dirty,
        "source_tree": tree,
        "source_tree_method": "git write-tree of a temporary index after `git add -A` (equals HEAD^{tree} when clean)",
    }
    if dirty:
        diff = run(["git", "diff", "--binary", "--full-index", head_tree, tree], text=False,
                   env=dict(os.environ, GIT_OPTIONAL_LOCKS="0"))
        identity["source_diff_sha256"] = sha256_bytes(diff)
        identity["source_diff_method"] = f"sha256 of `git diff --binary --full-index HEAD^{{tree}} {tree}` (tracked and untracked changes)"
        identity["source_dirty_files"] = git("diff", "--name-status", head_tree, tree).splitlines()
    return identity


def toolchain() -> dict:
    verbose = run(["rustc", "-vV"])
    host = next((line.split(":", 1)[1].strip() for line in verbose.splitlines() if line.startswith("host:")), None)
    if not host:
        raise ReleaseError("cannot determine the rustc host triple")
    return {"rustc": run(["rustc", "--version"]), "cargo": run(["cargo", "--version"]), "host_triple": host}


def _licence_texts(directory: pathlib.Path, license_file: str | None) -> list[tuple[str, str]]:
    found: dict[str, pathlib.Path] = {}
    if directory.is_dir():
        for entry in directory.iterdir():
            if entry.is_file() and LICENCE_FILE.match(entry.name):
                found[entry.name] = entry
    if license_file:
        path = (directory / license_file).resolve()
        if path.is_file():
            found.setdefault(license_file, path)
    return [(name, found[name].read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n")) for name in sorted(found)]


def _source_location(pkg: dict) -> str:
    source = pkg.get("source") or ""
    if source.startswith("registry+"):
        return f"crates.io package {pkg['name']} {pkg['version']}: https://crates.io/crates/{pkg['name']}/{pkg['version']}"
    if not source:
        manifest = pathlib.Path(pkg["manifest_path"]).resolve()
        try:
            rel = manifest.parent.relative_to(ROOT).as_posix()
        except ValueError:
            raise ReleaseError(f"path dependency outside the repository: {manifest}")
        where = f"{REPO_URL} source repository, directory {rel}/"
        if rel.startswith("patches/stylo/"):
            where += f"; upstream {STYLO_UPSTREAM} with the patches in patches/stylo/ (P-006-*.patch)"
        elif rel.startswith("patches/taffy/"):
            where += f"; upstream {TAFFY_UPSTREAM} with the patches in patches/taffy/ (P-008-*.patch)"
        elif rel.startswith("third_party/"):
            where += "; the crates.io package of the same name and version with the changes in third_party/README.md"
        return where
    return source


def third_party_licences(target: str) -> tuple[str, dict]:
    meta = json.loads(run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", target]))
    workspace = set(meta["workspace_members"])
    by_key: dict[tuple[str, str], dict] = {}
    for pkg in meta["packages"]:
        if pkg["id"] not in workspace:
            by_key[(pkg["name"], pkg["version"])] = pkg
    # `cargo metadata` unifies features across all targets (it has no resolver-2 per-target
    # feature split), so its graph over-approximates, e.g. macOS-only objc2 crates on Linux.
    # The exact normal-dependency set for the build target comes from `cargo tree`, the
    # per-crate details (licence, repository, source directory) from the metadata.
    tree = run(["cargo", "tree", "--locked", "--offline", "-e", "normal", "-p", HOST_PACKAGE,
                "--target", target, "--prefix", "none", "--format", "{p}"])
    keys = set()
    for line in tree.splitlines():
        m = re.match(r"^(\S+) v(\S+)", line.strip())
        if m:
            keys.add((m.group(1), m.group(2)))
    workspace_names = {p["name"] for p in meta["packages"] if p["id"] in workspace}
    keys = {k for k in keys if k[0] not in workspace_names}
    unknown = sorted(f"{n} {v}" for n, v in keys - by_key.keys())
    if unknown or not keys:
        raise ReleaseError(f"cargo tree and cargo metadata disagree: {unknown or 'empty tree'}")
    crates = sorted((by_key[k] for k in keys), key=lambda p: (p["name"], p["version"]))
    missing, declared_only, mpl = [], [], []
    sections = []
    for pkg in crates:
        texts = _licence_texts(pathlib.Path(pkg["manifest_path"]).parent, pkg.get("license_file"))
        licence = pkg.get("license") or ""
        if not texts and not licence:
            missing.append(f"{pkg['name']} {pkg['version']}")
            continue
        if not texts:
            declared_only.append(f"{pkg['name']} {pkg['version']}")
        if "MPL" in licence:
            mpl.append(pkg)
        lines = [
            "=" * 78,
            f"{pkg['name']} {pkg['version']}",
            f"Declared licence: {licence or '(none; see licence text below)'}",
            f"Repository: {pkg.get('repository') or '(not declared)'}",
            f"Source: {_source_location(pkg)}",
        ]
        if texts:
            for name, text in texts:
                lines += ["", f"--- {name} ---", text.rstrip("\n")]
        else:
            lines += ["", "(The package ships no licence or notice file; its declared SPDX licence applies.)"]
        sections.append("\n".join(lines))
    if missing:
        raise ReleaseError("crates without licence text or declared licence: " + ", ".join(missing))
    header = [
        "Third-party software in nativehtmlkit-host",
        f"Build target: {target}. Generated by scripts/release-local.py: crate set from",
        f"`cargo tree --locked -e normal -p {HOST_PACKAGE} --target {target}`, details from",
        "`cargo metadata --locked`. First-party workspace crates are covered by LICENSE.",
        f"Crates: {len(crates)}; with licence/notice files: {len(crates) - len(declared_only)};",
        f"declared licence only (no file in the package): {len(declared_only)}.",
        "",
        "Embedded data (see the files next to this one):",
        "  NotoSans-OFL.txt              Noto Sans Regular (SIL OFL 1.1), embedded once",
        "  hyphenation-NOTICES.txt       TeX hyphenation patterns (MIT, BSD-3-Clause, notice-preservation)",
        "  UA-STYLESHEETS-NOTICE.txt     WHATWG-derived UA stylesheets (CC-BY-4.0, owner-approved 2026-11-15)",
        "",
        "MPL-2.0: the full licence text is MPL-2.0.txt next to this file. Source code form of the",
        "MPL-2.0 covered crates in this binary is available at:",
        *[f"  {p['name']} {p['version']}: {_source_location(p)}" for p in mpl],
        "",
    ]
    if declared_only:
        header += ["Crates that ship no licence file (declared licence applies):", *[f"  {c}" for c in declared_only], ""]
    summary = {"crates": len(crates), "declared_only": len(declared_only), "mpl": len(mpl), "target": target}
    return "\n".join(header) + "\n" + "\n\n".join(sections) + "\n", summary


def deterministic_zip(stage: pathlib.Path, archive: pathlib.Path, timestamp: int, executable: str) -> None:
    # Zip timestamps cannot precede 1980; use the HEAD commit time (UTC) for every entry.
    date_time = time.gmtime(max(timestamp, 315532800))[:6]
    tmp = archive.with_suffix(".zip.partial")
    with zipfile.ZipFile(tmp, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for f in sorted((x for x in stage.rglob("*") if x.is_file()), key=lambda x: x.relative_to(stage).as_posix()):
            name = f.relative_to(stage).as_posix()
            info = zipfile.ZipInfo(name, date_time=date_time)
            info.create_system = 3  # Unix attributes on every platform
            info.external_attr = (0o100755 if name == executable else 0o100644) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, f.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    os.replace(tmp, archive)


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--version", default=None)
    p.add_argument("--stage-only", action="store_true")
    p.add_argument("--publish", action="store_true")
    p.add_argument("--confirm", default="")
    p.add_argument("--binary", type=pathlib.Path, default=None)
    p.add_argument("--restage", action="store_true",
                   help="replace an existing stage of the same version even if its host SHA-256 differs")
    p.add_argument("--build", action="store_true",
                   help="build the release host with --remap-path-prefix first (then stage/publish if requested)")
    p.add_argument("--write-source-identity", type=pathlib.Path, metavar="EXPORT_DIR",
                   help="write SOURCE_IDENTITY.json for a `git archive HEAD` export of this clean checkout and exit")
    a = p.parse_args()
    if a.write_source_identity:
        write_source_identity(a.write_source_identity.resolve())
        return 0
    if a.build:
        build_release_host()
        if not (a.stage_only or a.publish):
            return 0
    if a.publish and a.stage_only:
        p.error("Choose --stage-only or --publish")
    version = a.version or tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    if a.publish and a.confirm != "publish-" + version:
        p.error("Explicit --confirm publish-" + version + " required")
    exe = "nativehtmlkit-host.exe" if os.name == "nt" else "nativehtmlkit-host"
    binary = a.binary or ROOT / "target" / "release" / exe
    if not binary.is_file():
        p.error(f"Build nativehtmlkit-host locally first: {binary}")
    magic = binary.read_bytes()[:4]
    if os.name == "nt" and not magic.startswith(b"MZ"):
        p.error("Expected a Windows PE host executable")
    if os.name != "nt" and magic != b"\x7fELF":
        p.error("Expected a Linux ELF host executable")
    reported = subprocess.run([str(binary), "--version"], capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=15, check=True).stdout.strip()
    if reported != f"nativehtmlkit-host {version}":
        p.error(f"host --version {reported!r} does not match release version {version}")
    subprocess.run([sys.executable, str(ROOT / "scripts/generate-protocol-contract.py"), "--check"], check=True)
    source = checkout_identity()
    embedded = embedded_identity(binary)
    mismatch = identity_mismatch(embedded, source)
    if mismatch:
        p.error("the host binary was not built from this checkout (FR-R-04):\n  " + "\n  ".join(mismatch)
                + "\nRebuild it from this tree (python3 scripts/release-local.py --build); if the tree is unchanged "
                  "since a build, touch crates/nativehtmlkit-host/build.rs to refresh the embedded identity.")
    leaks = leaked_paths(binary)
    if leaks:
        p.error(f"the host binary embeds local paths {leaks} (FR-R-06); build it with "
                "`python3 scripts/release-local.py --build` (remaps them with --remap-path-prefix)")
    tools = toolchain()
    host_sha = sha256(binary)
    if a.publish:
        if source["source_identity"] != "git":
            p.error("Publication requires a git checkout (the tag is verified against HEAD)")
        if source["source_dirty"]:
            p.error("Clean commit required before publication")
        tag = "v" + version
        tagged = run(["git", "tag", "--list", tag])
        if tagged != tag:
            p.error(f"Tag {tag} does not exist; tag the release commit first")
        if git("rev-parse", "--verify", tag + "^{commit}") != source["source_commit"]:
            p.error(f"Tag {tag} does not point at HEAD {source['source_commit']}")
        if shutil.which("gh") is None:
            p.error("gh CLI is required for --publish")
        if subprocess.run(["gh", "release", "view", tag, "--repo", REPO], cwd=ROOT, capture_output=True).returncode == 0:
            p.error(f"GitHub release {tag} already exists; release versions are immutable")

    label = "windows-x64" if os.name == "nt" else "linux-x64"
    stage = ROOT / "dist" / f"nativehtmlkit-core-v{version}-{label}"
    previous = stage / "release-manifest.json"
    if stage.exists():
        try:
            old = json.loads(previous.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            old = {}
        if old.get("host_sha256") != host_sha and not a.restage:
            p.error(f"{stage.name} already exists with host SHA-256 {old.get('host_sha256', '<unknown>')}, "
                    f"not {host_sha}; bump the version or pass --restage to replace it")
        print(f"replacing existing stage {stage.name} (previous host {old.get('host_sha256', '<unknown>')})")
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    shutil.copy2(binary, stage / exe)
    if sha256(stage / exe) != host_sha:
        p.error("host binary changed while staging")
    shutil.copytree(ROOT / "protocol", stage / "protocol")
    for name, src in STATIC_FILES.items():
        (stage / name).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, stage / name)
    (stage / "licenses/UA-STYLESHEETS-NOTICE.txt").write_text(UA_NOTICE, encoding="utf-8", newline="\n")
    licences, licence_summary = third_party_licences(tools["host_triple"])
    (stage / "licenses/THIRD-PARTY-LICENSES.txt").write_text(licences, encoding="utf-8", newline="\n")
    manifest = {
        "schema": 2,
        "component": "nativehtmlkit-core",
        "version": version,
        "platform": label,
        "build_target": tools["host_triple"],
        "host_executable": exe,
        "host_sha256": host_sha,
        "host_version_output": reported,
        "host_reproducible": False,
        "protocol_version": 1,
        "protocol_status": "1.0: public-stable wire contract since host 1.0.3 (see protocol/COMPATIBILITY.md)",
        **{k: v for k, v in source.items() if k != "source_commit_time"},
        "host_embedded_source": {k: embedded.get(k) for k in ("source_identity", "source_commit", "source_tree", "source_dirty", "source_files_sha256")},
        "toolchain": tools["rustc"],
        "cargo": tools["cargo"],
        "third_party_licences": licence_summary,
        # Written at stage time, before any upload: a stage-only archive is never published by
        # this run; a --publish archive records the intended tag, and the GitHub release itself
        # (plus the `.published.json` sidecar written after a successful upload) is the record.
        "published": False if not a.publish else None,
        **({"publication": {"tag": "v" + version, "repository": REPO, "state": "prepared; tag verified == HEAD"}} if a.publish else {}),
    }
    (stage / "release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
    files = sorted((x for x in stage.rglob("*") if x.is_file()), key=lambda x: x.relative_to(stage).as_posix())
    (stage / "SHA256SUMS").write_text("".join(f"{sha256(x)}  {x.relative_to(stage).as_posix()}\n" for x in files),
                                      encoding="utf-8", newline="\n")
    archive = stage.parent / (stage.name + ".zip")
    deterministic_zip(stage, archive, source["source_commit_time"], exe)

    # Verify from a clean extraction, not from the staging tree.
    with tempfile.TemporaryDirectory(prefix="nhk-core-verify-") as tmp:
        with zipfile.ZipFile(archive) as z:
            z.extractall(tmp)
        out = pathlib.Path(tmp)
        listed = set()
        for line in (out / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
            digest, name = line.split("  ", 1)
            listed.add(name)
            if sha256(out / name) != digest:
                p.error(f"archive verification failed for {name}")
        extracted = {x.relative_to(out).as_posix() for x in out.rglob("*") if x.is_file()} - {"SHA256SUMS"}
        if extracted != listed:
            p.error(f"archive and SHA256SUMS differ: {sorted(extracted ^ listed)}")
        host = out / exe
        if os.name != "nt":
            host.chmod(0o755)
        if subprocess.run([str(host), "--version"], capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=15).stdout.strip() != reported:
            p.error("extracted host --version differs")
        if identity_mismatch(embedded_identity(host), source):
            p.error("extracted host embeds a different source identity")
    (stage.parent / (archive.name + ".sha256")).write_text(f"{sha256(archive)}  {archive.name}\n", encoding="utf-8", newline="\n")
    print(f"staged and verified: {archive}\n  sha256 {sha256(archive)}\n  host   {host_sha}\n"
          f"  commit {source['source_commit']}{' (dirty, diff ' + source['source_diff_sha256'][:12] + ')' if source['source_dirty'] else ''}\n"
          f"  tree   {source['source_tree']}\n  licences: {licence_summary['crates']} crates "
          f"({licence_summary['declared_only']} declared-only, {licence_summary['mpl']} MPL-2.0)")
    if a.publish:
        staged = json.loads((stage / "release-manifest.json").read_text(encoding="utf-8"))
        if staged["source_commit"] != git("rev-parse", "--verify", "HEAD^{commit}") or staged["source_dirty"] is not False:
            p.error("staged manifest is not from a clean HEAD; refusing to publish")
        if identity_mismatch(embedded_identity(stage / exe), source_identity() | {"source_identity": "git"}):
            p.error("staged host was not built from the clean HEAD; refusing to publish")
        # gh release create does not trigger any Actions in this repository: no workflows shipped.
        subprocess.run(["gh", "release", "create", "v" + version, str(archive), "--repo", REPO, "--verify-tag",
                        "--title", "NativeHTMLKit Core v" + version, "--notes", "Locally built and tested."], check=True)
        sidecar = stage.parent / (archive.name + ".published.json")
        sidecar.write_text(json.dumps({"published": True, "tag": "v" + version, "repository": REPO,
                                       "archive_sha256": sha256(archive), "source_commit": source["source_commit"]}, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
