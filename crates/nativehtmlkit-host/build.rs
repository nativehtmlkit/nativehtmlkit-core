//! Windows resources, and the source identity embedded in the host (FR-R-04).
//!
//! Source identity: in a git checkout, the HEAD commit and the hash of the working tree as git
//! would commit it (`git add -A` into a temporary index, then `git write-tree`): the same
//! `source_tree` that `scripts/release-local.py` records, equal to `HEAD^{tree}` when clean. New
//! objects go to a temporary object directory, so the build never writes into `.git`.
//! In a source export without git, the identity comes from `SOURCE_IDENTITY.json` at the source
//! root (or the file named by `NATIVEHTMLKIT_SOURCE_IDENTITY`), written by
//! `scripts/release-local.py --write-source-identity`; the digest of the listed files is
//! recomputed here, so an export edited before the build is recorded as dirty. Without either,
//! the identity is `unknown` and `release-local.py` refuses to stage the binary (fail closed).
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=host.rc");
    println!("cargo:rerun-if-changed=host.manifest");
    println!("cargo:rerun-if-changed=assets/nativehtmlkit.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("host.rc", embed_resource::NONE)
            .manifest_required()
            .expect("Windows resource compilation failed");
    }
    let root =
        canonical(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).expect("source root");
    let identity = identity(&root);
    for (key, value) in [
        ("NHK_BUILD_SOURCE_IDENTITY", identity.kind.as_str()),
        ("NHK_BUILD_SOURCE_COMMIT", identity.commit.as_str()),
        ("NHK_BUILD_SOURCE_TREE", identity.tree.as_str()),
        (
            "NHK_BUILD_SOURCE_DIRTY",
            if identity.dirty { "true" } else { "false" },
        ),
        ("NHK_BUILD_SOURCE_FILES_SHA256", identity.files.as_str()),
        ("NHK_BUILD_SOURCE_NOTE", identity.note.as_str()),
    ] {
        println!("cargo:rustc-env={key}={}", value.replace(['\n', '\r'], " "));
    }
}

/// `canonicalize` without the Windows verbatim prefix (`\\?\C:\...` becomes `C:\...`), so
/// paths compare equal to git's output and stay usable as git's working directory.
fn canonical(path: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => Some(PathBuf::from(rest)),
        _ => Some(path),
    }
}

#[derive(Default)]
struct Identity {
    kind: String,
    commit: String,
    tree: String,
    dirty: bool,
    files: String,
    note: String,
}

fn identity(root: &Path) -> Identity {
    println!("cargo:rerun-if-env-changed=NATIVEHTMLKIT_SOURCE_IDENTITY");
    let file = std::env::var_os("NATIVEHTMLKIT_SOURCE_IDENTITY").map(PathBuf::from);
    if file.is_none() {
        match git_identity(root) {
            Ok(Some(identity)) => return identity,
            Ok(None) => {}
            Err(error) => {
                return Identity {
                    kind: "unknown".into(),
                    note: format!("git identity failed: {error}"),
                    ..Default::default()
                }
            }
        }
    }
    let file = file.unwrap_or_else(|| root.join("SOURCE_IDENTITY.json"));
    println!("cargo:rerun-if-changed={}", file.display());
    if !file.is_file() {
        return Identity {
            kind: "unknown".into(),
            note: "not a git checkout and no SOURCE_IDENTITY.json".into(),
            ..Default::default()
        };
    }
    match file_identity(root, &file) {
        Ok(identity) => identity,
        Err(error) => Identity {
            kind: "unknown".into(),
            note: format!("{}: {error}", file.display()),
            ..Default::default()
        },
    }
}

fn git(root: &Path, args: &[&str], env: &[(&str, &Path)]) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0");
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// `Ok(None)`: not the top of a git checkout (or git is not installed).
fn git_identity(root: &Path) -> Result<Option<Identity>, String> {
    let Ok(top) = git(root, &["rev-parse", "--show-toplevel"], &[]) else {
        return Ok(None);
    };
    if canonical(Path::new(&top)).as_deref() != Some(root) {
        return Ok(None);
    }
    let path = |name: &str| -> Result<PathBuf, String> {
        let p = PathBuf::from(git(root, &["rev-parse", "--git-path", name], &[])?);
        Ok(if p.is_absolute() { p } else { root.join(p) })
    };
    // Rerun when HEAD moves or any source file changes. The index is not watched: `git status`
    // refreshes it, and staging alone does not change the `git add -A` tree.
    for name in ["HEAD", "packed-refs", "refs/heads", "refs/tags"] {
        println!("cargo:rerun-if-changed={}", path(name)?.display());
    }
    let ignored = ["target", "dist", "verification", ".venv", ".git"];
    let entries = std::fs::read_dir(root).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !ignored.contains(&name.as_ref()) && !name.ends_with(".log") {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
    let commit = git(root, &["rev-parse", "--verify", "HEAD^{commit}"], &[])?;
    let head_tree = git(root, &["rev-parse", "HEAD^{tree}"], &[])?;
    let objects = path("objects")?;
    let real_index = path("index")?;
    let scratch = tempdir()?;
    let index = scratch.join("index");
    let object_dir = scratch.join("objects");
    std::fs::create_dir_all(&object_dir).map_err(|e| e.to_string())?;
    let env = [
        ("GIT_INDEX_FILE", index.as_path()),
        ("GIT_OBJECT_DIRECTORY", object_dir.as_path()),
        ("GIT_ALTERNATE_OBJECT_DIRECTORIES", objects.as_path()),
    ];
    let result = (|| {
        if real_index.is_file() {
            std::fs::copy(&real_index, &index).map_err(|e| e.to_string())?;
        } else {
            git(root, &["read-tree", "HEAD"], &env)?;
        }
        git(root, &["add", "-A", "--", "."], &env)?;
        git(root, &["write-tree"], &env)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    let tree = result?;
    Ok(Some(Identity {
        kind: "git".into(),
        dirty: tree != head_tree,
        commit,
        tree,
        files: String::new(),
        note: String::new(),
    }))
}

fn tempdir() -> Result<PathBuf, String> {
    let base = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    let dir = base.join(format!("nhk-source-identity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// The digest `release-local.py` also computes: SHA-256 over `"<sha256>  <path>\n"` lines of
/// the listed files in the listed order.
fn files_digest(root: &Path, files: &[String]) -> Result<String, String> {
    let mut all = Sha256::new();
    for name in files {
        if name.is_empty() || name.starts_with('/') || name.split('/').any(|c| c == "..") {
            return Err(format!("invalid listed path {name:?}"));
        }
        let path = root.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = std::fs::read(&path).map_err(|e| format!("{name}: {e}"))?;
        all.update(format!("{}  {name}\n", hex(&Sha256::digest(&bytes))).as_bytes());
    }
    Ok(hex(&all.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn file_identity(root: &Path, file: &Path) -> Result<Identity, String> {
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    // A deliberately small reader for the fixed format written by release-local.py: string
    // members and one array of strings. Anything unexpected fails closed.
    let string = |key: &str| -> Result<String, String> {
        let needle = format!("\"{key}\": \"");
        let start = text.find(&needle).ok_or_else(|| format!("missing {key}"))? + needle.len();
        let end = text[start..]
            .find('"')
            .ok_or_else(|| format!("unterminated {key}"))?;
        let value = &text[start..start + end];
        if value.contains('\\') {
            return Err(format!("unsupported escape in {key}"));
        }
        Ok(value.to_owned())
    };
    if string("schema_name")? != "nativehtmlkit-core-source-identity/1" {
        return Err("unknown schema_name".into());
    }
    let commit = string("source_commit")?;
    let tree = string("source_tree")?;
    let recorded = string("files_sha256")?;
    let list_start = text.find("\"files\": [").ok_or("missing files")? + "\"files\": [".len();
    let list_end = list_start + text[list_start..].find(']').ok_or("unterminated files")?;
    let files: Vec<String> = text[list_start..list_end]
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .filter(|s| !s.contains('\\') && !s.contains('"'))
                .map(str::to_owned)
                .ok_or_else(|| format!("invalid files entry {s}"))
        })
        .collect::<Result<_, _>>()?;
    if files.is_empty() {
        return Err("empty files list".into());
    }
    let digest = files_digest(root, &files)?;
    Ok(Identity {
        kind: "file".into(),
        dirty: digest != recorded,
        commit,
        tree,
        files: digest,
        note: String::new(),
    })
}
