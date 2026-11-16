//! owner: 007-resources (T011, T039) — the file guard.
//!
//! Check, open, then verify: root membership is first decided without opening anything (CORE-A-01:
//! the path as written or its resolved existing prefix; UNC only under a UNC root). On Windows a
//! path outside every root is refused on its lexically normalised string and never resolved
//! (FR-R-12: resolving opens a handle, even on a mapped network drive); on Unix the resolution of
//! the existing prefix uses `lstat`/`readlink` on the path's components, which can trigger an
//! automount but never opens the target. The file is then
//! opened normally (links are followed), then the **opened handle's**
//! final path is compared with the canonical roots by path components, then `fstat` must report a
//! regular file and the size is checked against the kind's limit before reading. Path-string checks
//! alone are never trusted, so a symlink or junction swapped between check and open cannot escape.
//!
//! Nothing here writes, creates, deletes or lists: the only operations are open (read-only) and read.

#[cfg(unix)]
pub mod unix;
pub mod windows;

use nativehtmlkit_engine_types::codes_generated::Code;
use nativehtmlkit_engine_types::stage::Stage;
use nativehtmlkit_engine_types::status::Status;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Content identity of a file for the cache key: size and modification time (data-model).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    pub size: u64,
    pub mtime_ns: u128,
}

/// A file that passed the guard.
#[derive(Debug)]
pub struct VerifiedFile {
    pub file: File,
    /// The opened handle's final path (never the requested string).
    pub final_path: PathBuf,
    pub identity: FileIdentity,
}

fn status(code: Code, msg: impl Into<String>) -> Status {
    Status::new(code, Stage::Resource).with_message(msg)
}

fn io_code(e: &std::io::Error) -> Code {
    if e.kind() == std::io::ErrorKind::NotFound {
        Code::FileNotFound
    } else {
        Code::ResourceDenied
    }
}

/// Whether `path` lies inside `root`, comparing components (never string prefixes, so `/app2` is
/// not inside `/app`); case-insensitively when `case_insensitive` (Windows).
pub fn path_within(path: &Path, root: &Path, case_insensitive: bool) -> bool {
    let mut p = path.components();
    for rc in root.components() {
        match p.next() {
            Some(pc) if comp_eq(&pc, &rc, case_insensitive) => {}
            _ => return false,
        }
    }
    true
}

fn comp_eq(a: &Component, b: &Component, ci: bool) -> bool {
    if ci {
        a.as_os_str().to_string_lossy().to_lowercase()
            == b.as_os_str().to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

fn identity_of(md: &std::fs::Metadata) -> FileIdentity {
    let mtime_ns = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    FileIdentity {
        size: md.len(),
        mtime_ns,
    }
}

fn finish(file: File, final_path: PathBuf, max_bytes: u64) -> Result<VerifiedFile, Status> {
    let md = file
        .metadata()
        .map_err(|e| status(io_code(&e), "fstat failed"))?;
    if !md.is_file() {
        return Err(status(Code::ResourceDenied, "not a regular file"));
    }
    if md.len() > max_bytes {
        return Err(status(
            Code::SizeLimit,
            format!("file is {} bytes; the limit is {max_bytes}", md.len()),
        ));
    }
    Ok(VerifiedFile {
        file,
        final_path,
        identity: identity_of(&md),
    })
}

/// Open `path` for a document request and verify it against `roots` (open-then-verify).
///
/// * outside every root (decided before the open; existing or not): `RESOURCE_DENIED` with one
///   uniform message; inside a root but not existing: `FILE_NOT_FOUND`; unreadable, resolving
///   outside every root after the open, or not a regular file: `RESOURCE_DENIED`; larger than
///   `max_bytes`: `SIZE_LIMIT`.
pub fn open_verified(
    path: &Path,
    roots: &[PathBuf],
    max_bytes: u64,
) -> Result<VerifiedFile, Status> {
    if path.as_os_str().is_empty() || path.to_string_lossy().contains('\0') {
        return Err(status(Code::ResourceDenied, "invalid path"));
    }
    let ci = cfg!(windows);
    // CORE-A-01: decide root membership *before* anything is opened, so a path outside every root
    // (a device node, a FIFO, or on Windows an SMB share) is never opened; on Windows it is not
    // resolved either (FR-R-12).
    // One message for every outside-root case: the answer must not reveal what exists beyond the
    // roots (decision D-007-02).
    if !within_roots_lexically(path, roots, ci, cfg!(windows)) {
        return Err(status(Code::ResourceDenied, OUTSIDE_ROOTS));
    }
    // Defence in depth (symlink swapped after the check, TOCTOU): verify the opened handle.
    let (file, final_path) = match platform_open(path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(status(Code::FileNotFound, "file not found"));
        }
        Err(e) => return Err(status(io_code(&e), format!("open failed: {e}"))),
    };
    if !roots.iter().any(|r| path_within(&final_path, r, ci)) {
        return Err(status(Code::ResourceDenied, OUTSIDE_ROOTS));
    }
    finish(file, final_path, max_bytes)
}

const OUTSIDE_ROOTS: &str = "path is outside every configured root";

/// The pre-open membership decision.
///
/// * Unix: the path's longest existing prefix is resolved (`realpath`: `lstat`/`readlink` only,
///   nothing is opened) and compared with the roots, so a symlink inside a root that points
///   outside is refused before the open as well.
/// * Windows: `canonicalize` itself opens a (zero-access) handle, so a path that lies under a
///   root as written (no `.`/`..` components) is admitted without resolving it. Any other path is
///   first normalised lexically (`.` dropped, `..` applied to the string); when that is outside
///   every root it is refused without touching the file system (FR-R-12; an 8.3 short-name
///   spelling of a root is therefore refused), otherwise it is resolved. A UNC path is admitted
///   only under a UNC root, judged on the string alone (resolving it would already contact the
///   server).
///
/// `windows` selects the Windows rules (a parameter so both rule sets are unit-tested anywhere).
fn within_roots_lexically(path: &Path, roots: &[PathBuf], ci: bool, windows: bool) -> bool {
    if windows {
        let p = path.to_string_lossy();
        let rs: Vec<String> = roots
            .iter()
            .map(|r| r.to_string_lossy().into_owned())
            .collect();
        let rs: Vec<&str> = rs.iter().map(String::as_str).collect();
        if !windows::unc_permitted(&p, &rs) {
            return false;
        }
        if windows::is_unc(&p) {
            // Covered by a UNC root lexically; the post-open check verifies the final path.
            return true;
        }
    }
    let plain = windows
        && path
            .components()
            .all(|c| !matches!(c, Component::CurDir | Component::ParentDir));
    if plain && roots.iter().any(|r| path_within(path, r, ci)) {
        return true;
    }
    if windows {
        let normalised = lexically_normalised(path);
        if !roots.iter().any(|r| path_within(&normalised, r, ci)) {
            return false;
        }
    }
    let resolved = resolve_existing_prefix(path);
    roots.iter().any(|r| path_within(&resolved, r, ci))
}

/// `path` with `.` components dropped and each `..` removing the preceding normal component,
/// on the string only (a `..` at the root stays at the root, as the OS resolves it).
fn lexically_normalised(path: &Path) -> PathBuf {
    let mut out: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.last(), Some(Component::Normal(_))) {
                    out.pop();
                } else if out.is_empty() {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out.iter().collect()
}

/// Canonicalise the longest existing prefix of `path` and re-append the missing tail, so a
/// not-yet-existing file is judged where it would live (symlinked ancestors resolved).
fn resolve_existing_prefix(path: &Path) -> PathBuf {
    #[cfg(test)]
    RESOLVED.with(|r| r.borrow_mut().push(path.to_path_buf()));
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path.to_path_buf();
    loop {
        if let Ok(c) = std::fs::canonicalize(&cur) {
            let mut out = c;
            for t in tail.iter().rev() {
                out.push(t);
            }
            // `canonicalize` yields `\\?\` paths on Windows; roots are stored without the prefix.
            return crate::policy::strip_verbatim(out);
        }
        match (
            cur.file_name().map(|n| n.to_owned()),
            cur.parent().map(|p| p.to_path_buf()),
        ) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                cur = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Open a host-supplied path (trusted declaration, Clarification Q2): no root check; the file must
/// exist, be a regular file and be within the size limit.
pub fn open_trusted(path: &Path, max_bytes: u64) -> Result<VerifiedFile, Status> {
    if path.to_string_lossy().contains('\0') {
        return Err(status(Code::ResourceDenied, "invalid path"));
    }
    let (file, final_path) =
        platform_open(path).map_err(|e| status(io_code(&e), format!("open failed: {e}")))?;
    finish(file, final_path, max_bytes)
}

#[cfg(test)]
thread_local! {
    /// Every path handed to `platform_open` on this thread (test-only probe: proves which paths
    /// the guard actually opened, not just which it reported as denied).
    static OPENED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Every path the pre-open check resolved (FR-R-12 probe: resolving touches the path).
    static RESOLVED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn platform_open(path: &Path) -> std::io::Result<(File, PathBuf)> {
    #[cfg(test)]
    OPENED.with(|o| o.borrow_mut().push(path.to_path_buf()));
    #[cfg(unix)]
    {
        unix::open_with_final_path(path)
    }
    #[cfg(windows)]
    {
        windows::open_with_final_path(path)
    }
}

/// Read the whole file, never more than `max_bytes` (the file may grow after `fstat`).
pub fn read_limited(file: &File, max_bytes: u64) -> Result<Vec<u8>, Status> {
    let mut buf = Vec::new();
    let mut limited = file.take(max_bytes.saturating_add(1));
    limited
        .read_to_end(&mut buf)
        .map_err(|e| status(io_code(&e), format!("read failed: {e}")))?;
    if buf.len() as u64 > max_bytes {
        return Err(status(
            Code::SizeLimit,
            format!("file exceeds {max_bytes} bytes"),
        ));
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_opened() -> Vec<PathBuf> {
        OPENED.with(|o| std::mem::take(&mut *o.borrow_mut()))
    }

    fn take_resolved() -> Vec<PathBuf> {
        RESOLVED.with(|r| std::mem::take(&mut *r.borrow_mut()))
    }

    /// FR-R-12: under the Windows rules a path outside every root (as written, or after `..`
    /// is applied to the string) is refused without being resolved; a path whose normalised
    /// form is inside a root is still resolved (it may cross a junction). The Unix rules keep
    /// resolving the existing prefix.
    #[test]
    fn windows_rules_never_resolve_a_path_outside_every_root() {
        let root = PathBuf::from("/nhk-fr-r-12/root");
        let roots = [root.clone()];
        take_resolved();
        for outside in [
            "/nhk-fr-r-12/elsewhere/file.png",
            "/nhk-fr-r-12/root/../elsewhere/file.png",
            "/nhk-fr-r-12/root/./../../x",
        ] {
            assert!(
                !within_roots_lexically(Path::new(outside), &roots, false, true),
                "{outside}"
            );
            assert_eq!(
                take_resolved(),
                Vec::<PathBuf>::new(),
                "{outside} was resolved"
            );
        }
        // Plain and inside: admitted without resolving.
        assert!(within_roots_lexically(
            Path::new("/nhk-fr-r-12/root/a.png"),
            &roots,
            false,
            true
        ));
        assert!(take_resolved().is_empty());
        // `..` that stays inside: resolved (the junction/symlink check needs the real path).
        let _ = within_roots_lexically(
            Path::new("/nhk-fr-r-12/root/sub/../a.png"),
            &roots,
            false,
            true,
        );
        assert_eq!(
            take_resolved(),
            [PathBuf::from("/nhk-fr-r-12/root/sub/../a.png")]
        );
        // Unix rules: an outside path is resolved (lstat/readlink only) before being refused.
        assert!(!within_roots_lexically(
            Path::new("/nhk-fr-r-12/elsewhere/x"),
            &roots,
            false,
            false
        ));
        assert_eq!(take_resolved().len(), 1);
        assert_eq!(
            lexically_normalised(Path::new("/a/./b/../../../c")),
            PathBuf::from("/c")
        );
    }

    /// A fresh `<tmp>/nhk-fsguard-<pid>-<n>/{root,outside}` tree.
    #[cfg(unix)]
    fn scratch() -> (PathBuf, PathBuf, PathBuf) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir().join(format!(
            "nhk-fsguard-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("root")).unwrap();
        std::fs::create_dir_all(base.join("outside")).unwrap();
        let root = std::fs::canonicalize(base.join("root")).unwrap();
        let outside = std::fs::canonicalize(base.join("outside")).unwrap();
        (base, root, outside)
    }

    /// CORE-A-01: a file outside every root must be denied without ever being opened, and the
    /// denial must not depend on whether the file exists.
    #[cfg(unix)]
    #[test]
    fn outside_root_is_denied_without_opening() {
        let (base, root, outside) = scratch();
        let secret = outside.join("secret.png");
        std::fs::write(&secret, b"x").unwrap();
        take_opened();
        let existing = open_verified(&secret, std::slice::from_ref(&root), 1024).unwrap_err();
        assert!(take_opened().is_empty(), "an outside-root file was opened");
        let missing = open_verified(
            &outside.join("missing.png"),
            std::slice::from_ref(&root),
            1024,
        )
        .unwrap_err();
        assert!(take_opened().is_empty(), "an outside-root path was opened");
        assert_eq!(existing.code, Code::ResourceDenied);
        assert_eq!(missing.code, Code::ResourceDenied);
        assert_eq!(existing.message, missing.message, "existence oracle");
        // A FIFO outside the root: opening it would be a side effect on a foreign object.
        let fifo = outside.join("pipe.png");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status();
        if made.map(|s| s.success()).unwrap_or(false) {
            let e = open_verified(&fifo, std::slice::from_ref(&root), 1024).unwrap_err();
            assert_eq!(e.code, Code::ResourceDenied);
            assert_eq!(e.message, existing.message);
            assert!(take_opened().is_empty(), "an outside-root FIFO was opened");
        }
        let _ = std::fs::remove_dir_all(base);
    }

    /// A symlink inside the root that points outside stays refused (Unix: already by the
    /// resolved pre-open check; the opened handle's final path is still verified as defence in
    /// depth against a swap after the check), while ordinary in-root and aliased paths still work.
    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_still_rejected() {
        let (base, root, outside) = scratch();
        std::fs::write(outside.join("secret.png"), b"x").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.png"), root.join("link.png")).unwrap();
        std::fs::write(root.join("ok.png"), b"ok").unwrap();
        take_opened();
        let e =
            open_verified(&root.join("link.png"), std::slice::from_ref(&root), 1024).unwrap_err();
        assert_eq!(e.code, Code::ResourceDenied);
        assert_eq!(e.message, OUTSIDE_ROOTS);
        assert!(take_opened().is_empty(), "the escaping symlink was opened");
        let ok = open_verified(&root.join("ok.png"), std::slice::from_ref(&root), 1024).unwrap();
        assert_eq!(ok.final_path, root.join("ok.png"));
        // A path that reaches the root through a symlinked ancestor is still inside.
        std::os::unix::fs::symlink(&root, base.join("alias")).unwrap();
        assert!(
            open_verified(
                &base.join("alias/ok.png"),
                std::slice::from_ref(&root),
                1024
            )
            .is_ok()
        );
        let missing =
            open_verified(&root.join("nope.png"), std::slice::from_ref(&root), 1024).unwrap_err();
        assert_eq!(missing.code, Code::FileNotFound);
        let _ = std::fs::remove_dir_all(base);
    }

    /// CORE-A-01 (Windows): a UNC path is denied without an open (no SMB connection) unless a
    /// configured root is itself UNC and lexically covers it.
    #[cfg(windows)]
    #[test]
    fn unc_is_denied_without_opening() {
        let root = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = crate::policy::strip_verbatim(root);
        take_opened();
        for p in [r"\\192.0.2.10\s\a.png", r"//192.0.2.10/s/a.png"] {
            let e = open_verified(Path::new(p), std::slice::from_ref(&root), 1024).unwrap_err();
            assert_eq!(e.code, Code::ResourceDenied, "{p}");
        }
        let unc_root = PathBuf::from(r"\\192.0.2.10\other");
        let e = open_verified(
            Path::new(r"\\192.0.2.10\s\a.png"),
            std::slice::from_ref(&unc_root),
            1024,
        )
        .unwrap_err();
        assert_eq!(e.code, Code::ResourceDenied);
        assert!(take_opened().is_empty(), "a UNC path was opened");
    }

    #[test]
    fn components_not_string_prefixes() {
        assert!(path_within(
            Path::new("/app/ui/a.png"),
            Path::new("/app"),
            false
        ));
        assert!(!path_within(
            Path::new("/app2/a.png"),
            Path::new("/app"),
            false
        ));
        assert!(!path_within(Path::new("/app"), Path::new("/app/ui"), false));
        assert!(path_within(Path::new("C:\\App\\a.png"), Path::new("c:\\app"), true) || cfg!(unix));
    }

    #[test]
    fn case_insensitive_comparison() {
        assert!(path_within(
            Path::new("/App/A.png"),
            Path::new("/app"),
            true
        ));
        assert!(!path_within(
            Path::new("/App/A.png"),
            Path::new("/app"),
            false
        ));
    }
}
