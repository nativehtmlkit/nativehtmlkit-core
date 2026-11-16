//! Bounded document-local raw bitmap transport. Data pixels never enter JSON,
//! a PNG decoder or the CSS resource cache. A trusted backend maps the private
//! host-created file for writes and calls `bitmap.present` after completing a
//! slot. The owner thread snapshots one slot before returning the acknowledgement.
use super::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tempfile::TempPath;

pub(super) const MAX_BITMAP_BYTES: u64 = 64 * 1024 * 1024;
const MAX_BITMAPS: usize = 8;
/// CORE-A-02: all live-bitmap transports of this host together (tmpfs plus host copies). Without
/// it, 16 documents x 64 MiB could pin 1 GiB of the user's runtime filesystem.
const MAX_HOST_BITMAP_BYTES: u64 = 256 * 1024 * 1024;
/// Transport file names are `ui4me-bitmap-<host pid>-<random>` (the legacy prefix is kept so
/// existing leak checks still match).
const TRANSPORT_PREFIX: &str = "ui4me-bitmap-";

/// CORE-A-02: remove transport files left behind by hosts that died before their backend's first
/// `bitmap.present` (SIGKILL, crash). Only files whose name carries the pid of a process that no
/// longer exists are removed; files of live hosts, legacy unnamed files and anything that is not a
/// regular file are left alone. Returns the number of files removed.
///
/// FR-R-15: on Linux a live host holds a shared `flock` on each transport file it created,
/// and a file is removed only when an exclusive lock can be taken, so a host in another PID namespace
/// that shares the runtime directory keeps its files. On Windows the files live in a dedicated
/// `%TEMP%\nativehtmlkit-bitmaps` directory, so the sweep never scans all of `%TEMP%`.
pub(super) fn sweep_stale_transports() -> usize {
    match platform::bitmap_directory() {
        Ok(dir) => sweep_stale_transports_in(&dir, platform::process_alive),
        Err(_) => 0,
    }
}

fn sweep_stale_transports_in(dir: &std::path::Path, alive: impl Fn(u32) -> bool) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let own = std::process::id();
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|n| n.strip_prefix(TRANSPORT_PREFIX))
            .and_then(|rest| rest.split_once('-'))
            .filter(|(pid, _)| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|(pid, _)| pid.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own || alive(pid) {
            continue;
        }
        let path = entry.path();
        let regular = std::fs::symlink_metadata(&path)
            .map(|m| m.file_type().is_file())
            .unwrap_or(false);
        if !regular {
            continue;
        }
        #[cfg(target_os = "linux")]
        let _released = match unheld(&path) {
            Some(file) => file,
            None => continue,
        };
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Probe a transport file with an exclusive `flock` (FR-R-15): it succeeds only when no live
/// host and no backend holds the file. `false` when a lock is held elsewhere or the file system
/// does not support it (the caller then keeps the file).
#[cfg(target_os = "linux")]
fn try_lock(file: &File) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: flock on a descriptor this function borrows; no memory is passed.
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

/// The lock a live host keeps on its own transport file. It is SHARED (NHK-CROSS-0008): a
/// backend must be able to open and map the path, and runtimes such as .NET take a shared
/// advisory `flock` on every file they open, which an exclusive host lock would refuse. A
/// shared lock still makes the sweep's exclusive probe fail, so liveness detection is unchanged.
#[cfg(target_os = "linux")]
fn hold_lock(file: &File) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: flock on a descriptor this function borrows; no memory is passed.
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) == 0 }
}

/// The transport file opened and locked, proving no live host holds it; `None` otherwise.
/// Opened without following a final symlink.
#[cfg(target_os = "linux")]
fn unheld(path: &std::path::Path) -> Option<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .ok()?;
    try_lock(&file).then_some(file)
}

pub(super) struct Bitmap {
    file: File,
    /// The private path the backend maps. It is unlinked as soon as the
    /// backend proves it has mapped the file (its first `bitmap.present`), so
    /// a killed host cannot leave tmpfs memory behind; dropping the bitmap
    /// before that point removes it as well.
    path: Option<TempPath>,
    /// A path whose early unlink failed (Windows: a producer handle that does not
    /// share deletion). Removed when the bitmap is dropped, after the producer
    /// closed its handle, instead of leaking the file.
    retained: Option<TempPath>,
    width: u32,
    height: u32,
    stride: u32,
    slots: u8,
    slot_bytes: u64,
    last_sequence: u64,
}
impl std::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bitmap")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("slots", &self.slots)
            .field("last_sequence", &self.last_sequence)
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    window_id: String,
    document_id: String,
    bitmap_id: String,
    width: u32,
    height: u32,
    #[serde(default = "two")]
    slots: u8,
    #[serde(default = "rgba")]
    pixel_format: String,
}
fn two() -> u8 {
    2
}
fn rgba() -> String {
    "RGBA8".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Present {
    window_id: String,
    document_id: String,
    bitmap_id: String,
    sequence: u64,
    slot: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Close {
    window_id: String,
    document_id: String,
    bitmap_id: String,
}

impl Bitmap {
    fn new(request: &Create) -> Result<Self, RpcError> {
        if request.pixel_format != "RGBA8" {
            return Err(RpcError::invalid_params("Only RGBA8 sRGB is supported"));
        }
        if request.bitmap_id.is_empty()
            || request.bitmap_id.len() > 64
            || !request
                .bitmap_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(RpcError::invalid_params(
                "bitmap_id must contain 1..64 ASCII letters/digits/_/-",
            ));
        }
        if !(2..=3).contains(&request.slots)
            || request.width == 0
            || request.height == 0
            || request.width > 4096
            || request.height > 4096
        {
            return Err(RpcError::invalid_params(
                "Bitmap limits: 1..4096 pixels per side and 2..3 slots",
            ));
        }
        let stride = request
            .width
            .checked_mul(4)
            .ok_or_else(|| RpcError::busy("Bitmap stride overflow"))?;
        let slot_bytes = u64::from(stride) * u64::from(request.height);
        let total = slot_bytes * u64::from(request.slots);
        if total > MAX_BITMAP_BYTES {
            return Err(RpcError::busy("Bitmap transport allocation exceeds 64 MiB"));
        }
        let dir = platform::bitmap_directory().map_err(|e| RpcError::unavailable(e.to_string()))?;
        let prefix = format!("{TRANSPORT_PREFIX}{}-", std::process::id());
        let file = tempfile::Builder::new()
            .prefix(&prefix)
            .tempfile_in(dir)
            .map_err(|e| RpcError::unavailable(e.to_string()))?;
        file.as_file()
            .set_len(total)
            .map_err(|e| RpcError::unavailable(e.to_string()))?;
        // Held for the bitmap's lifetime: the startup sweep of any host leaves it alone.
        #[cfg(target_os = "linux")]
        if !hold_lock(file.as_file()) {
            tracing::debug!("bitmap transport file cannot be locked; a sweep keeps it anyway only while the creating pid is alive");
        }
        let (file, path) = file.into_parts();
        Ok(Self {
            file,
            path: Some(path),
            retained: None,
            width: request.width,
            height: request.height,
            stride,
            slots: request.slots,
            slot_bytes,
            last_sequence: 0,
        })
    }

    fn read(&mut self, slot: u8, sequence: u64) -> Result<Option<std::sync::Arc<[u8]>>, RpcError> {
        if slot >= self.slots {
            return Err(RpcError::invalid_params("Bitmap slot is out of bounds"));
        }
        if sequence == 0 {
            return Err(RpcError::invalid_params("Bitmap sequence must be positive"));
        }
        if sequence <= self.last_sequence {
            return Ok(None);
        }
        // Copy from the shared file exactly once. No GUI-thread wait for the
        // producer: the contract requires the producer to finish the mmap write
        // before sending `bitmap.present` and to wait for acknowledgement before
        // reusing the slot. A fast producer can drop pending older frames.
        let file: &mut File = &mut self.file;
        file.seek(SeekFrom::Start(u64::from(slot) * self.slot_bytes))
            .map_err(|e| RpcError::unavailable(e.to_string()))?;
        let mut bytes = vec![0u8; self.slot_bytes as usize];
        file.read_exact(&mut bytes)
            .map_err(|e| RpcError::unavailable(e.to_string()))?;
        // Rendering consumes premultiplied RGBA. The public API accepts ordinary
        // straight-alpha RGBA8 and normalizes it here (sRGB alpha multiplication).
        for rgba in bytes.as_chunks_mut::<4>().0 {
            let a = u32::from(rgba[3]);
            if a < 255 {
                for channel in &mut rgba[..3] {
                    *channel = ((u32::from(*channel) * a + 127) / 255) as u8;
                }
            }
        }
        self.last_sequence = sequence;
        if let Some(path) = self.path.take() {
            // The producer has mapped the file before presenting; both sides
            // keep using their open descriptors and mappings. On Windows this
            // needs every handle to share deletion (the Python client opens it
            // delete-on-close); otherwise keep the path for removal at drop.
            match std::fs::remove_file(&path) {
                Ok(()) => drop(path),
                Err(error) => {
                    tracing::debug!(%error, "cannot unlink bitmap transport file yet");
                    self.retained = Some(path);
                }
            }
        }
        Ok(Some(bytes.into()))
    }
}

pub(super) type Bitmaps = HashMap<String, Bitmap>;

impl Host {
    pub(super) fn dispatch_bitmap(
        &mut self,
        method: &str,
        params: Value,
        event_loop: Option<&dyn ActiveEventLoop>,
    ) -> Result<Value, RpcError> {
        match method {
            "bitmap.create" => {
                let req: Create = serde_json::from_value(params)
                    .map_err(|e| RpcError::invalid_params(e.to_string()))?;
                let host_existing: u64 = self
                    .windows
                    .values()
                    .filter_map(|w| w.document.as_ref())
                    .flat_map(|d| d.bitmaps.values())
                    .map(|v| v.slot_bytes * u64::from(v.slots))
                    .sum();
                let window = self
                    .windows
                    .get_mut(&req.window_id)
                    .ok_or_else(|| RpcError::closed("Window does not exist"))?;
                let doc = window
                    .document
                    .as_mut()
                    .filter(|d| d.id == req.document_id)
                    .ok_or_else(|| RpcError::closed("Document not loaded or stale"))?;
                if doc.bitmaps.contains_key(&req.bitmap_id) {
                    return Err(RpcError::invalid_state("Bitmap already exists"));
                }
                if doc.bitmaps.len() >= MAX_BITMAPS {
                    return Err(RpcError::busy("Maximum eight bitmaps per document"));
                }
                let bitmap = Bitmap::new(&req)?;
                let total_existing: u64 = doc
                    .bitmaps
                    .values()
                    .map(|v| v.slot_bytes * u64::from(v.slots))
                    .sum();
                let total_new = bitmap.slot_bytes * u64::from(bitmap.slots);
                if total_existing + total_new > MAX_BITMAP_BYTES {
                    return Err(RpcError::busy(
                        "Document live bitmap transport budget exceeded",
                    ));
                }
                if host_existing + total_new > MAX_HOST_BITMAP_BYTES {
                    return Err(RpcError::busy(
                        "Host live bitmap transport budget (256 MiB) exceeded",
                    ));
                }
                // Register transparent pixels immediately so the intrinsic size
                // is available for the first HTML/CSS layout.
                let empty = vec![0u8; bitmap.slot_bytes as usize];
                doc.engine
                    .set_live_bitmap(&req.bitmap_id, req.width, req.height, empty.into())?;
                let path = bitmap
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let result = json!({"bitmap_id":req.bitmap_id,"width":bitmap.width,"height":bitmap.height,
                    "stride":bitmap.stride,"pixel_format":"RGBA8","slots":bitmap.slots,
                    "slot_bytes":bitmap.slot_bytes,"path":path,"owner":"backend-writes;host-reads-after-present"});
                doc.bitmaps.insert(req.bitmap_id, bitmap);
                self.document_changed(&req.window_id, event_loop);
                Ok(result)
            }
            "bitmap.present" => {
                let req: Present = serde_json::from_value(params)
                    .map_err(|e| RpcError::invalid_params(e.to_string()))?;
                let window = self
                    .windows
                    .get_mut(&req.window_id)
                    .ok_or_else(|| RpcError::closed("Window does not exist"))?;
                let doc = window
                    .document
                    .as_mut()
                    .filter(|d| d.id == req.document_id)
                    .ok_or_else(|| RpcError::closed("Document not loaded or stale"))?;
                let bitmap = doc
                    .bitmaps
                    .get_mut(&req.bitmap_id)
                    .ok_or_else(|| RpcError::closed("Bitmap not registered"))?;
                let frame = bitmap.read(req.slot, req.sequence)?;
                let applied = frame.is_some();
                if let Some(bytes) = frame {
                    doc.engine.set_live_bitmap(
                        &req.bitmap_id,
                        bitmap.width,
                        bitmap.height,
                        bytes,
                    )?;
                }
                if applied {
                    // A same-size frame changes pixels only: no DOM events,
                    // semantics or geometry, so skip `document_changed` (which
                    // republishes the accessibility tree) and just schedule a
                    // paint. Creation and close keep the full refresh path.
                    if let Some(native) = window.native.as_ref() {
                        native.window.request_redraw();
                    }
                }
                Ok(
                    json!({"sequence":req.sequence,"presented_to_engine":applied,"slot_released":true}),
                )
            }
            "bitmap.close" => {
                let req: Close = serde_json::from_value(params)
                    .map_err(|e| RpcError::invalid_params(e.to_string()))?;
                let window = self
                    .windows
                    .get_mut(&req.window_id)
                    .ok_or_else(|| RpcError::closed("Window does not exist"))?;
                let doc = window
                    .document
                    .as_mut()
                    .filter(|d| d.id == req.document_id)
                    .ok_or_else(|| RpcError::closed("Document not loaded or stale"))?;
                let removed = doc.bitmaps.remove(&req.bitmap_id).is_some();
                if removed {
                    doc.engine.remove_live_bitmap(&req.bitmap_id)?;
                }
                if removed {
                    self.document_changed(&req.window_id, event_loop);
                }
                Ok(json!({"closed":removed}))
            }
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-R-15: the pid in a transport name is only meaningful in the creating host's PID
    /// namespace. A file whose pid looks dead here but which a live host still holds (its
    /// lock; e.g. a host in a container sharing `$XDG_RUNTIME_DIR`) is kept; once that host
    /// is gone (lock released) the file is swept.
    #[cfg(target_os = "linux")]
    #[test]
    fn sweep_keeps_a_transport_still_held_by_a_host_in_another_pid_namespace() {
        use std::os::fd::AsRawFd;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("{TRANSPORT_PREFIX}999999999-abc"));
        let held = File::create(&path).unwrap();
        // What a live host does with its transport file while it exists.
        assert!(hold_lock(&held));
        // NHK-CROSS-0008: a backend runtime that takes its own shared advisory lock on open
        // (.NET FileStream / MemoryMappedFile do) must still be able to open the file.
        let backend = File::open(&path).unwrap();
        assert_eq!(
            unsafe { libc::flock(backend.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) },
            0,
            "a backend's shared lock was refused while the host holds the file"
        );
        drop(backend);
        assert_eq!(sweep_stale_transports_in(dir.path(), |_| false), 0);
        assert!(path.exists(), "a held transport was removed");
        drop(held);
        assert_eq!(sweep_stale_transports_in(dir.path(), |_| false), 1);
        assert!(!path.exists());
    }
    #[test]
    fn reject_unbounded_or_unsupported_descriptions() {
        let req = Create {
            window_id: "w".into(),
            document_id: "d".into(),
            bitmap_id: "x".into(),
            width: 4096,
            height: 4096,
            slots: 3,
            pixel_format: "RGBA8".into(),
        };
        assert!(Bitmap::new(&req).is_err());
        let req = Create {
            width: 8,
            height: 4,
            slots: 2,
            ..req
        };
        let mut b = Bitmap::new(&req).unwrap();
        assert_eq!(b.slot_bytes, 128);
        assert!(b.read(2, 1).is_err());
        assert!(b.read(0, 0).is_err());
    }

    /// CORE-A-02: the transport lives in the host's private directory (Linux: the owner-checked
    /// 0700 `$XDG_RUNTIME_DIR/nativehtmlkit-host`) and its name carries the host pid, so a later
    /// host can tell a leaked file of a dead host from a live one.
    #[test]
    fn transport_file_is_private_and_names_the_host_pid() {
        let req = Create {
            window_id: "w".into(),
            document_id: "d".into(),
            bitmap_id: "x".into(),
            width: 2,
            height: 2,
            slots: 2,
            pixel_format: "RGBA8".into(),
        };
        let b = Bitmap::new(&req).unwrap();
        let path = b.path.as_ref().unwrap().to_path_buf();
        #[cfg(target_os = "linux")]
        assert_eq!(
            path.parent().unwrap(),
            crate::platform::runtime_directory().unwrap(),
            "transport must be inside the private runtime directory"
        );
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let prefix = format!("ui4me-bitmap-{}-", std::process::id());
        assert!(name.starts_with(&prefix), "{name} lacks {prefix}");
    }

    /// CORE-A-02: the startup sweep removes only regular files of dead hosts.
    #[test]
    fn startup_sweep_removes_only_files_of_dead_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let own = std::process::id();
        let names = [
            ("ui4me-bitmap-4000001-AbC123".to_owned(), false), // dead host: removed
            (format!("ui4me-bitmap-{own}-AbC123"), true),      // this host
            ("ui4me-bitmap-4000002-AbC123".to_owned(), true),  // live host
            ("ui4me-bitmap-ISPGaH".to_owned(), true),          // legacy name, owner unknown
            ("ui4me-bitmap--x".to_owned(), true),
            ("ui4me-bitmap-12a-x".to_owned(), true),
            ("unrelated-4000001-x".to_owned(), true),
        ];
        for (name, _) in &names {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("ui4me-bitmap-4000001-dir")).unwrap();
        let alive = |pid: u32| pid == 4_000_002;
        assert_eq!(sweep_stale_transports_in(dir.path(), alive), 1);
        for (name, kept) in &names {
            assert_eq!(dir.path().join(name).exists(), *kept, "{name}");
        }
        assert!(dir.path().join("ui4me-bitmap-4000001-dir").is_dir());
    }

    #[test]
    fn process_liveness_probe() {
        let _serial = platform::process_test_guard();
        assert!(platform::process_alive(std::process::id()));
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(!platform::process_alive(pid), "a reaped child is not alive");
    }

    /// A killed host must not leave its tmpfs transport behind: the path is
    /// unlinked once the producer has presented (and therefore mapped) it.
    #[test]
    fn transport_file_is_unlinked_after_the_first_present() {
        let req = Create {
            window_id: "w".into(),
            document_id: "d".into(),
            bitmap_id: "x".into(),
            width: 2,
            height: 2,
            slots: 2,
            pixel_format: "RGBA8".into(),
        };
        let mut b = Bitmap::new(&req).unwrap();
        let path = b.path.as_ref().unwrap().to_path_buf();
        assert!(path.exists());
        // A stale or rejected present proves nothing about the mapping.
        assert!(b.read(5, 1).is_err());
        assert!(path.exists());
        assert_eq!(b.read(0, 1).unwrap().unwrap().len(), 16);
        assert!(!path.exists(), "mapped transport must be unlinked");
        // The open descriptor still serves later frames.
        assert!(b.read(1, 2).unwrap().is_some());
        // Dropping an unpresented bitmap removes its file too.
        let unpresented = Bitmap::new(&req).unwrap();
        let path = unpresented.path.as_ref().unwrap().to_path_buf();
        drop(unpresented);
        assert!(!path.exists());
    }

    /// Windows: a producer handle without delete sharing (the CRT default)
    /// blocks the early unlink. The file must then go when the bitmap is
    /// closed, after the producer released it; it previously leaked.
    #[cfg(windows)]
    #[test]
    fn windows_transport_without_delete_sharing_is_removed_at_close() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        const FILE_SHARE_WRITE: u32 = 2;
        let req = Create {
            window_id: "w".into(),
            document_id: "d".into(),
            bitmap_id: "x".into(),
            width: 2,
            height: 2,
            slots: 2,
            pixel_format: "RGBA8".into(),
        };
        let mut b = Bitmap::new(&req).unwrap();
        let path = b.path.as_ref().unwrap().to_path_buf();
        let producer = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&path)
            .unwrap();
        assert!(b.read(0, 1).unwrap().is_some());
        assert!(path.exists(), "the producer's handle forbids deletion");
        drop(producer);
        drop(b);
        assert!(!path.exists(), "removed once both handles are closed");
    }

    /// Windows: a delete-on-close producer handle with delete sharing (how the
    /// Python client opens it) lets the transport disappear even if neither
    /// side cleans up explicitly.
    #[cfg(windows)]
    #[test]
    fn windows_delete_on_close_transport_leaves_nothing_behind() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_ALL: u32 = 1 | 2 | 4;
        const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
        const DELETE: u32 = 0x0001_0000;
        let req = Create {
            window_id: "w".into(),
            document_id: "d".into(),
            bitmap_id: "x".into(),
            width: 2,
            height: 2,
            slots: 2,
            pixel_format: "RGBA8".into(),
        };
        let b = Bitmap::new(&req).unwrap();
        let path = b.path.as_ref().unwrap().to_path_buf();
        let producer = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .access_mode(0xC000_0000 | DELETE)
            .share_mode(FILE_SHARE_ALL)
            .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
            .open(&path)
            .unwrap();
        // The host is "killed": its handle closes and its TempPath is never run.
        let Bitmap {
            file, path: tmp, ..
        } = b;
        drop(file);
        let _ = tmp.map(|t| t.keep());
        assert!(path.exists() || std::fs::metadata(&path).is_err());
        drop(producer);
        assert!(
            !path.exists(),
            "delete-on-close removes the orphaned transport"
        );
    }
}
