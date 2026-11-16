use anyhow::{bail, Context, Result};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct InstanceLock {
    pub instance_key: String,
    pub primary: bool,
    pub endpoint: String,
    _file: File,
}

impl InstanceLock {
    pub fn acquire(app_id: &str) -> Result<Self> {
        // Preserve the original desktop realm when systemd launches the host.
        let realm = std::env::var("UI4ME_DESKTOP_REALM").unwrap_or_else(|_| desktop_realm());
        let key = super::namespace_hash(format!("{app_id}\0{realm}").as_bytes());
        let directory = runtime_directory()?;
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(directory.join(format!("{key}.lock")))?;
        if file.metadata()?.uid() != unsafe { libc::geteuid() } {
            bail!("instance lock is not owned by this user");
        }
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        let primary = if rc == 0 {
            true
        } else {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::WouldBlock {
                return Err(error.into());
            }
            false
        };
        let endpoint = endpoint_for(&key)?;
        Ok(Self {
            instance_key: key,
            primary,
            endpoint,
            _file: file,
        })
    }
}

fn desktop_realm() -> String {
    let uid = unsafe { libc::geteuid() };
    let base = runtime_base(uid);
    let display = std::env::var_os("WAYLAND_DISPLAY");
    realm_for_compositor(
        uid,
        &base,
        display.as_deref(),
        std::env::var_os("UI4ME_TEST_MODE").is_some(),
    )
}

fn runtime_base(uid: u32) -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("nativehtmlkit-runtime-{uid}")))
}

fn realm_for_compositor(
    uid: u32,
    runtime: &Path,
    display: Option<&OsStr>,
    headless: bool,
) -> String {
    let runtime = runtime
        .canonicalize()
        .unwrap_or_else(|_| runtime.to_owned());
    if headless {
        return format!("uid={uid};headless;runtime={runtime:?}");
    }
    let display = Path::new(display.unwrap_or_else(|| OsStr::new("wayland-0")));
    let path = if display.is_absolute() {
        display.to_owned()
    } else {
        runtime.join(display)
    };
    let path = path.canonicalize().unwrap_or(path);
    let identity = std::fs::metadata(&path)
        .map(|metadata| format!("{}:{}", metadata.dev(), metadata.ino()))
        .unwrap_or_else(|_| "not-present".into());
    // XDG_SESSION_ID may be absent in desktop/systemd activation but present
    // in a terminal attached to the very same compositor. Its socket identity
    // is the stable realm shared by both launch routes; inode changes separate
    // compositor restarts even when WAYLAND_DISPLAY is reused.
    format!("uid={uid};wayland={path:?};identity={identity}")
}

pub fn runtime_directory() -> Result<PathBuf> {
    let uid = unsafe { libc::geteuid() };
    let base = runtime_base(uid);
    if !base.is_absolute() {
        bail!("runtime directory must be an absolute path");
    }
    ensure_private_directory(&base)?;
    let path = base.join("nativehtmlkit-host");
    ensure_private_directory(&path)?;
    Ok(path)
}

/// Where live-bitmap transport files are created (CORE-A-02): the private, owner-checked 0700
/// runtime directory, never the shared root of `$XDG_RUNTIME_DIR`.
pub fn bitmap_directory() -> Result<PathBuf> {
    runtime_directory()
}

/// Whether a process with `pid` currently exists (signal 0; `EPERM` means it exists). A reused
/// pid reads as alive, which only ever keeps a file, never removes a live one.
pub fn process_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: kill with signal 0 performs only the existence/permission check.
    unsafe {
        libc::kill(pid, 0) == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(test)]
thread_local! {
    /// Test probe: the mode a directory had at the instant `ensure_private_directory` created it.
    static CREATED_MODE: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    // CORE-A-09: created with 0700 in the mkdir call itself (umask can only narrow it), so the
    // directory never exists with a wider mode. A pre-existing one is verified below, never
    // repaired: it must be a real directory owned by us with no group/other bits.
    use std::os::unix::fs::DirBuilderExt;
    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {
            #[cfg(test)]
            CREATED_MODE.with(|m| m.set(std::fs::symlink_metadata(path).ok().map(|md| md.mode())));
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.into()),
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        bail!(
            "private runtime directory has unsafe ownership or permissions: {}",
            path.display()
        );
    }
    Ok(())
}

pub fn endpoint_for(instance_key: &str) -> Result<String> {
    if instance_key.is_empty() || !instance_key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid socket instance key");
    }
    let path = runtime_directory()?.join(format!("{instance_key}.sock"));
    let value = path.to_str().context("runtime socket path is not UTF-8")?;
    if value.len() >= 104 {
        bail!("runtime directory is too long for a Unix-domain socket");
    }
    Ok(value.to_owned())
}

pub struct NativeListener {
    listener: UnixListener,
    path: PathBuf,
}

impl NativeListener {
    /// Caller must hold the primary InstanceLock for this endpoint.
    pub fn bind(endpoint: &str) -> Result<Self> {
        let path = PathBuf::from(endpoint);
        if path.exists() {
            let metadata = std::fs::symlink_metadata(&path)?;
            use std::os::unix::fs::FileTypeExt;
            if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
                bail!("refusing to replace a non-owned or non-socket endpoint");
            }
            // A live listener must never be stolen, including accidental misuse
            // of Server::bind by a caller that did not obtain the instance lock.
            match NativeStream::connect_with_retry(endpoint, Duration::from_millis(200), false) {
                Ok(_) => bail!("instance endpoint is already active"),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                    ) => {}
                Err(error) => {
                    return Err(error).context("cannot prove that the old endpoint is stale");
                }
            }
            std::fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, path })
    }

    pub fn accept(&mut self, stop: &AtomicBool) -> io::Result<Option<NativeStream>> {
        while !stop.load(Ordering::Acquire) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    return Ok(Some(NativeStream {
                        stream,
                        deadline: None,
                    }));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    // Wake immediately for a connection; the timeout only
                    // bounds how long close() waits to observe `stop`.
                    let mut poll = libc::pollfd {
                        fd: self.listener.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // SAFETY: one valid pollfd for the listener owned by self.
                    if unsafe { libc::poll(&mut poll, 1, 100) } < 0 {
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::Interrupted {
                            return Err(error);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }
}

impl Drop for NativeListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct NativeStream {
    stream: UnixStream,
    deadline: Option<Instant>,
}

impl NativeStream {
    pub fn connect(endpoint: &str, timeout: Duration) -> io::Result<Self> {
        Self::connect_with_retry(endpoint, timeout, true)
    }
    fn connect_with_retry(
        endpoint: &str,
        timeout: Duration,
        retry_missing: bool,
    ) -> io::Result<Self> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "timeout is too large"))?;
        let bytes = endpoint.as_bytes();
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        if bytes.is_empty() || bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid local socket path",
            ));
        }
        address.sun_family = libc::AF_UNIX as _;
        for (destination, byte) in address.sun_path.iter_mut().zip(bytes) {
            *destination = *byte as _;
        }
        loop {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "local socket connection deadline exceeded",
                ));
            }
            // A blocking UnixStream::connect can hang on a full listener
            // backlog. Use a nonblocking descriptor even for the connection
            // attempt, then restore blocking I/O with explicit deadlines.
            let raw = unsafe {
                libc::socket(
                    libc::AF_UNIX,
                    libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
                    0,
                )
            };
            if raw < 0 {
                return Err(io::Error::last_os_error());
            }
            let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
            let result = unsafe {
                libc::connect(
                    descriptor.as_raw_fd(),
                    (&address as *const libc::sockaddr_un).cast(),
                    std::mem::size_of::<libc::sockaddr_un>() as _,
                )
            };
            let connected = if result == 0 {
                true
            } else {
                let error = io::Error::last_os_error();
                match error.raw_os_error() {
                    Some(libc::EINPROGRESS) | Some(libc::EALREADY) => {
                        let mut poll = libc::pollfd {
                            fd: descriptor.as_raw_fd(),
                            events: libc::POLLOUT,
                            revents: 0,
                        };
                        loop {
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            if remaining.is_zero() {
                                return Err(io::Error::new(
                                    io::ErrorKind::TimedOut,
                                    "local connection timed out",
                                ));
                            }
                            let milliseconds =
                                remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
                            let result = unsafe { libc::poll(&mut poll, 1, milliseconds) };
                            if result == 0 {
                                return Err(io::Error::new(
                                    io::ErrorKind::TimedOut,
                                    "local connection timed out",
                                ));
                            }
                            if result < 0 {
                                let error = io::Error::last_os_error();
                                if error.kind() == io::ErrorKind::Interrupted {
                                    continue;
                                }
                                return Err(error);
                            }
                            let mut code = 0_i32;
                            let mut size = std::mem::size_of::<i32>() as libc::socklen_t;
                            if unsafe {
                                libc::getsockopt(
                                    descriptor.as_raw_fd(),
                                    libc::SOL_SOCKET,
                                    libc::SO_ERROR,
                                    (&mut code as *mut i32).cast(),
                                    &mut size,
                                )
                            } < 0
                            {
                                return Err(io::Error::last_os_error());
                            }
                            if code != 0 {
                                return Err(io::Error::from_raw_os_error(code));
                            }
                            break true;
                        }
                    }
                    // For Unix sockets EAGAIN means the listener backlog is
                    // full; this descriptor has not begun connecting.
                    Some(libc::EAGAIN) => false,
                    Some(libc::ENOENT) | Some(libc::ECONNREFUSED) if retry_missing => false,
                    _ => return Err(error),
                }
            };
            if connected {
                let stream = UnixStream::from(descriptor);
                stream.set_nonblocking(false)?;
                return Ok(Self {
                    stream,
                    deadline: Some(deadline),
                });
            }
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(20)),
            );
        }
    }
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            stream: self.stream.try_clone()?,
            deadline: self.deadline,
        })
    }
    pub fn shutdown(&self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
    pub fn set_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.deadline = timeout
            .map(|duration| {
                Instant::now().checked_add(duration).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "timeout is too large")
                })
            })
            .transpose()?;
        Ok(())
    }
    fn remaining(&self) -> io::Result<Option<Duration>> {
        match self.deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "local I/O deadline exceeded",
                    ));
                }
                Ok(Some(remaining))
            }
            None => Ok(None),
        }
    }
}
impl Read for NativeStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.stream.set_read_timeout(self.remaining()?)?;
        self.stream.read(bytes)
    }
}
impl Write for NativeStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.stream.set_write_timeout(self.remaining()?)?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

/// Re-enter the same binary under a transient user service. No global manager
/// environment is modified. Environment *names*, never secret values, occur in
/// the systemd-run command line. argv expansion is explicitly disabled.
pub fn bootstrap_supervised(args: &[OsString]) -> Result<Option<i32>> {
    if is_systemd_managed()
        || std::env::var_os("UI4ME_TEST_MODE").is_some()
        || std::env::var_os("UI4ME_ALLOW_UNSUPERVISED").is_some()
    {
        return Ok(None);
    }
    let unit = format!(
        "nativehtmlkit-host-{}-{}.service",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    );
    let mut command = Command::new("systemd-run");
    command
        .args([
            "--user",
            "--wait",
            "--pipe",
            "--collect",
            "--quiet",
            "--service-type=exec",
            "--expand-environment=no",
            "--property=ExitType=main",
            "--property=KillMode=control-group",
            "--property=TimeoutStopSec=5s",
            "--same-dir",
        ])
        .arg(format!("--unit={unit}"))
        .env("UI4ME_SYSTEMD_UNIT", &unit)
        .env("UI4ME_DESKTOP_REALM", desktop_realm())
        .env(LAUNCHER_ENV, launcher_identity(std::process::id())?);
    const OWN: [&str; 3] = ["UI4ME_SYSTEMD_UNIT", "UI4ME_DESKTOP_REALM", LAUNCHER_ENV];
    for (key, _) in std::env::vars_os() {
        if !OWN.iter().any(|own| key == OsStr::new(own)) {
            let mut argument = OsString::from("--setenv=");
            argument.push(key);
            command.arg(argument);
        }
    }
    command
        .args(OWN.map(|own| format!("--setenv={own}")))
        .arg("--")
        .arg(std::env::current_exe()?)
        .args(args);
    let status = command.status().context("cannot start systemd user supervision; run doctor, or explicitly choose --allow-unsupervised for diagnostics")?;
    Ok(Some(status.code().unwrap_or(1)))
}

/// The supervised host's lifetime remains bound to the launcher that the
/// desktop, terminal or test harness started. systemd-run --wait never stops
/// the unit when its client dies, so without this a Ctrl+C, closed terminal or
/// killed launcher would leave an orphaned application running.
const LAUNCHER_ENV: &str = "NATIVEHTMLKIT_LAUNCHER";

/// "pid:starttime", where starttime is the kernel's boot-relative process
/// start stamp. It distinguishes the launcher from a later PID reuse.
fn launcher_identity(pid: u32) -> Result<String> {
    Ok(format!("{pid}:{}", process_start_time(pid)?))
}

fn process_start_time(pid: u32) -> Result<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    // The command name is parenthesized and may contain spaces or ')'.
    let rest = stat
        .rsplit_once(')')
        .map(|(_, rest)| rest)
        .context("malformed /proc stat")?;
    // Fields after the name start at field 3 (state); starttime is field 22.
    rest.split_whitespace()
        .nth(19)
        .and_then(|value| value.parse().ok())
        .context("malformed /proc process start time")
}

/// Least time the event loop gets to act on launcher loss, and the margin added
/// to the manifest's shutdown timeout (NHK-CROSS-0001).
const WATCHDOG_MINIMUM: Duration = Duration::from_secs(2);
const WATCHDOG_MARGIN: Duration = Duration::from_secs(1);

fn watchdog_grace(shutdown_timeout: Duration) -> Duration {
    shutdown_timeout
        .saturating_add(WATCHDOG_MARGIN)
        .max(WATCHDOG_MINIMUM)
}

/// Wait on the injected clock until `acknowledged` is set or `grace` has passed.
/// Returns whether the event loop acknowledged in time.
fn await_acknowledgement(
    acknowledged: &AtomicBool,
    grace: Duration,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration),
) -> bool {
    let start = now();
    loop {
        if acknowledged.load(Ordering::SeqCst) {
            return true;
        }
        let elapsed = now().saturating_duration_since(start);
        if elapsed >= grace {
            return acknowledged.load(Ordering::SeqCst);
        }
        sleep((grace - elapsed).min(Duration::from_millis(50)));
    }
}

/// Signal the backend's process group unless the event loop has already reaped the backend
/// (the shared id is then 0): a reaped group id may have been reused (FR-R-13).
fn signal_live_group(group: &std::sync::atomic::AtomicU32, kill: impl FnOnce(i32)) {
    match i32::try_from(group.load(Ordering::SeqCst)) {
        Ok(group) if group > 1 => kill(group),
        _ => {}
    }
}

/// Runs on the watcher thread after launcher loss has been signalled. A host
/// whose event loop is blocked never consumes the flag, so this thread ends the
/// process itself: best-effort SIGKILL of the backend process group, one stderr
/// record, exit 1. Under the systemd unit (KillMode=control-group) the exit then
/// cleans up the rest of the control group. A fully stopped (SIGSTOP) host runs
/// no thread at all; that case is a documented limitation.
fn launcher_watchdog(watchdog: &super::LauncherWatchdog) {
    let grace = watchdog_grace(watchdog.shutdown_timeout);
    if await_acknowledgement(
        &watchdog.acknowledged,
        grace,
        Instant::now,
        std::thread::sleep,
    ) {
        return;
    }
    signal_live_group(&watchdog.backend_group, |group| {
        // SAFETY: plain signal delivery to the backend's own process group
        // (spawn_backend makes the backend a session and group leader).
        unsafe { libc::kill(-group, libc::SIGKILL) };
    });
    let line = crate::logging::emergency_record(&format!(
        "launcher exited and the event loop did not begin shutdown within {} ms; terminating the host",
        grace.as_millis()
    ));
    // stderr may be a full pipe or locked by a blocked log writer: bound the wait.
    let (written, wait) = std::sync::mpsc::sync_channel(1);
    let _ = std::thread::Builder::new()
        .name("nativehtmlkit-watchdog-report".into())
        .spawn(move || {
            let _ = writeln!(io::stderr(), "{line}");
            let _ = written.send(());
        });
    let _ = wait.recv_timeout(Duration::from_millis(500));
    std::process::exit(1);
}

/// Watch the launcher of a supervised host. Returns a flag set once the
/// launcher has exited (or was already gone), after which `wake` is called and
/// the bounded `watchdog` starts. Returns None when this host was not started
/// by the supervising launcher.
pub fn watch_launcher(
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    watchdog: super::LauncherWatchdog,
) -> Option<std::sync::Arc<AtomicBool>> {
    if !is_systemd_managed() {
        return None;
    }
    let value = std::env::var(LAUNCHER_ENV).ok()?;
    let (pid, start) = value.split_once(':')?;
    let (pid, start): (u32, u64) = (pid.parse().ok()?, start.parse().ok()?);
    let gone = std::sync::Arc::new(AtomicBool::new(false));
    // SAFETY: pidfd_open takes a PID and flags and returns a new descriptor.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0) };
    let opened = if raw >= 0 {
        // SAFETY: the successful syscall returned a descriptor we now own.
        Ok(unsafe { OwnedFd::from_raw_fd(raw as i32) })
    } else {
        Err(io::Error::last_os_error().raw_os_error().unwrap_or(0))
    };
    // Check identity after opening the pidfd so a PID reused before the open
    // cannot be mistaken for the launcher.
    let same = process_start_time(pid).ok() == Some(start);
    let pidfd = match launcher_watch(opened, same) {
        LauncherWatch::Pidfd(pidfd) => Some(pidfd),
        LauncherWatch::Poll(errno) => {
            // CORE-A-10: pidfd_open is unavailable (kernel < 5.3: ENOSYS; a seccomp filter:
            // EPERM) or failed for lack of resources; the launcher itself is alive. Poll its
            // identity instead of declaring it gone.
            tracing::warn!(
                errno,
                "pidfd_open failed; watching the launcher by polling its process identity"
            );
            let flag = gone.clone();
            let spawned = std::thread::Builder::new()
                .name("nativehtmlkit-launcher-watch".into())
                .spawn(move || {
                    poll_until_gone(
                        || process_start_time(pid).ok() == Some(start),
                        std::thread::sleep,
                    );
                    tracing::warn!("launcher exited; shutting down the supervised application");
                    flag.store(true, Ordering::SeqCst);
                    wake();
                    launcher_watchdog(&watchdog);
                });
            if let Err(error) = spawned {
                tracing::warn!(%error, "cannot start launcher watch");
            }
            return Some(gone);
        }
        LauncherWatch::Gone => None,
    };
    let Some(pidfd) = pidfd else {
        tracing::warn!("launcher already exited; shutting down the supervised application");
        gone.store(true, Ordering::SeqCst);
        wake();
        if let Err(error) = std::thread::Builder::new()
            .name("nativehtmlkit-launcher-watch".into())
            .spawn(move || launcher_watchdog(&watchdog))
        {
            tracing::warn!(%error, "cannot start launcher watchdog");
        }
        return Some(gone);
    };
    let flag = gone.clone();
    let spawned = std::thread::Builder::new()
        .name("nativehtmlkit-launcher-watch".into())
        .spawn(move || {
            let mut poll = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            loop {
                // SAFETY: one valid pollfd for the descriptor owned by this thread.
                let ready = unsafe { libc::poll(&mut poll, 1, -1) };
                if ready > 0 {
                    break;
                }
                if ready < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    tracing::warn!(error = %io::Error::last_os_error(), "cannot watch launcher");
                    return;
                }
            }
            tracing::warn!("launcher exited; shutting down the supervised application");
            flag.store(true, Ordering::SeqCst);
            wake();
            launcher_watchdog(&watchdog);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "cannot start launcher watch");
    }
    Some(gone)
}

/// How the launcher is watched, from the `pidfd_open` outcome (descriptor or errno) and
/// whether the PID still has the launcher's start time (CORE-A-10).
#[derive(Debug)]
enum LauncherWatch<T> {
    Pidfd(T),
    /// pidfd_open failed with this errno while the launcher is alive: poll its identity.
    Poll(i32),
    Gone,
}

fn launcher_watch<T>(opened: Result<T, i32>, same_identity: bool) -> LauncherWatch<T> {
    match (opened, same_identity) {
        (_, false) | (Err(libc::ESRCH), _) => LauncherWatch::Gone,
        (Ok(pidfd), true) => LauncherWatch::Pidfd(pidfd),
        (Err(errno), true) => LauncherWatch::Poll(errno),
    }
}

/// Interval of the identity poll used when pidfd_open is unavailable.
const LAUNCHER_POLL: Duration = Duration::from_millis(500);

/// Return once `alive` reports false, checking every [`LAUNCHER_POLL`].
fn poll_until_gone(mut alive: impl FnMut() -> bool, mut sleep: impl FnMut(Duration)) {
    while alive() {
        sleep(LAUNCHER_POLL);
    }
}

pub fn is_systemd_managed() -> bool {
    let Ok(unit) = std::env::var("UI4ME_SYSTEMD_UNIT") else {
        return false;
    };
    if !unit.starts_with("nativehtmlkit-host-") || !unit.ends_with(".service") {
        return false;
    }
    std::fs::read_to_string("/proc/self/cgroup")
        .map(|s| s.lines().any(|line| line.ends_with(&unit)))
        .unwrap_or(false)
}

/// Only Windows creates a console window for a GUI launch.
pub fn detach_owned_console() {}

/// What `run` uses, independent of how this diagnostic process was started.
pub fn run_containment_description() -> &'static str {
    "systemd-user-service via systemd-run --user (required; --allow-unsupervised is an explicit diagnostic)"
}

pub fn containment_description() -> &'static str {
    if is_systemd_managed() {
        "systemd-user-service"
    } else {
        "process-group-only (diagnostic/test mode; hard-host-death descendant cleanup is not guaranteed)"
    }
}

pub struct NativeChild {
    child: Option<Child>,
    group: Option<i32>,
    pid: u32,
    status: Option<ExitStatus>,
}

pub struct SpawnedChild {
    pub child: NativeChild,
    pub stdout: Box<dyn Read + Send>,
    pub stderr: Box<dyn Read + Send>,
}

pub fn spawn_backend(
    config: &crate::config::BackendConfig,
    endpoint: &str,
    token: &str,
    app_id: &str,
) -> Result<SpawnedChild> {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(&config.program);
    for key in super::HOST_ONLY_ENV {
        command.env_remove(key);
    }
    command
        .args(&config.args)
        .current_dir(&config.cwd)
        .envs(&config.env)
        .env("UI4ME_ENDPOINT", endpoint)
        .env("UI4ME_TOKEN", token)
        .env("UI4ME_APP_ID", app_id)
        .env(
            "UI4ME_PROTOCOL_VERSION",
            crate::protocol::PROTOCOL_VERSION.to_string(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                // pre_exec runs after fork in a multithreaded host. Avoid
                // allocating an error message while allocator locks may have
                // been inherited in a locked state from vanished threads.
                return Err(io::Error::from_raw_os_error(libc::ECHILD));
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("start backend executable")?;
    let stdout = child.stdout.take().context("backend stdout pipe missing")?;
    let stderr = child.stderr.take().context("backend stderr pipe missing")?;
    let pid = child.id();
    Ok(SpawnedChild {
        child: NativeChild {
            group: Some(pid as i32),
            child: Some(child),
            pid,
            status: None,
        },
        stdout: Box::new(stdout),
        stderr: Box::new(stderr),
    })
}

impl NativeChild {
    pub fn id(&self) -> u32 {
        self.pid
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        let Some(child) = self.child.as_mut() else {
            return Ok(self.status);
        };
        // Observe exit without reaping: the zombie keeps its PID/process-group
        // identity reserved until remaining group members have been killed.
        // Signalling the old group after Child::try_wait reaped the leader
        // would introduce a PID-reuse race against an unrelated process.
        let mut information: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut information,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(None);
            }
            if error.raw_os_error() == Some(libc::ECHILD) {
                self.group = None;
            }
            return Err(error);
        }
        if unsafe { information.si_pid() } == 0 {
            return Ok(None);
        }
        self.signal(libc::SIGKILL)?;
        let status = self
            .child
            .as_mut()
            .expect("child retained until reaping")
            .try_wait()?;
        if let Some(status) = status {
            self.status = Some(status);
            self.group = None;
        }
        Ok(status)
    }
    pub fn terminate(&mut self) -> io::Result<()> {
        self.signal(libc::SIGTERM)
    }
    pub fn kill(&mut self) -> io::Result<()> {
        self.signal(libc::SIGKILL)
    }
    fn signal(&self, signal: i32) -> io::Result<()> {
        let Some(group) = self.group else {
            return Ok(());
        };
        if unsafe { libc::kill(-group, signal) } == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        Ok(())
    }
}
impl Drop for NativeChild {
    fn drop(&mut self) {
        let _ = self.kill();
        if let Some(mut child) = self.child.take() {
            if self.status.is_none() {
                // The GUI must not wait for a process to acknowledge SIGKILL.
                // The reaper owns Child so no exited child becomes a zombie
                // while a host-owned diagnostic window remains open.
                let _ = std::thread::Builder::new()
                    .name("nativehtmlkit-child-reaper".into())
                    .spawn(move || {
                        let _ = child.wait();
                    });
            }
        }
    }
}

pub fn standard_paths(app_id: &str) -> Result<serde_json::Value> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;
    let path = |name, fallback| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(fallback))
            .join(app_id)
    };
    Ok(
        serde_json::json!({"config": path("XDG_CONFIG_HOME", ".config"), "data": path("XDG_DATA_HOME", ".local/share"),
        "cache": path("XDG_CACHE_HOME", ".cache"), "state": path("XDG_STATE_HOME", ".local/state"), "runtime": runtime_directory()?}),
    )
}

pub fn initialize_application(_app_id: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CORE-A-10: a failed pidfd_open of a live launcher (ENOSYS, EPERM, EMFILE ...) falls back
    /// to polling instead of shutting the application down; only a missing or replaced
    /// launcher process counts as gone.
    #[test]
    fn pidfd_failure_of_a_live_launcher_falls_back_to_polling() {
        assert!(matches!(
            launcher_watch(Ok(5), true),
            LauncherWatch::Pidfd(5)
        ));
        for errno in [libc::ENOSYS, libc::EPERM, libc::EMFILE, libc::ENOMEM] {
            assert!(matches!(
                launcher_watch::<i32>(Err(errno), true),
                LauncherWatch::Poll(e) if e == errno
            ));
        }
        assert!(matches!(
            launcher_watch::<i32>(Err(libc::ESRCH), true),
            LauncherWatch::Gone
        ));
        assert!(matches!(launcher_watch(Ok(5), false), LauncherWatch::Gone));
        assert!(matches!(
            launcher_watch::<i32>(Err(libc::ENOSYS), false),
            LauncherWatch::Gone
        ));
        // The poll returns exactly when the identity check first fails.
        let mut checks = 0;
        let mut slept = Duration::ZERO;
        poll_until_gone(
            || {
                checks += 1;
                checks < 4
            },
            |step| slept += step,
        );
        assert_eq!((checks, slept), (4, 3 * LAUNCHER_POLL));
    }

    /// FR-R-13: once the event loop has reaped the backend (and cleared the shared id), the
    /// watchdog sends no signal; a live backend's group is signalled once.
    #[test]
    fn watchdog_never_signals_a_reaped_backend_group() {
        use std::sync::atomic::AtomicU32;
        let group = AtomicU32::new(4242);
        let mut sent = Vec::new();
        signal_live_group(&group, |g| sent.push(g));
        assert_eq!(sent, [4242]);
        group.store(0, Ordering::SeqCst); // reaped by the event loop
        signal_live_group(&group, |g| sent.push(g));
        for invalid in [1, u32::MAX] {
            group.store(invalid, Ordering::SeqCst); // init / not a valid pid_t
            signal_live_group(&group, |g| sent.push(g));
        }
        assert_eq!(sent, [4242]);
    }

    /// NHK-CROSS-0001: the watchdog bound is the shutdown timeout plus a margin, never
    /// below the minimum.
    #[test]
    fn watchdog_grace_extends_the_shutdown_timeout_with_a_minimum() {
        assert_eq!(watchdog_grace(Duration::ZERO), Duration::from_secs(2));
        assert_eq!(
            watchdog_grace(Duration::from_millis(500)),
            Duration::from_secs(2)
        );
        assert_eq!(
            watchdog_grace(Duration::from_secs(5)),
            Duration::from_secs(6)
        );
        assert_eq!(watchdog_grace(Duration::MAX), Duration::MAX);
    }

    /// NHK-CROSS-0001: a fake clock drives the decision; no real time passes.
    #[test]
    fn watchdog_fires_only_when_the_event_loop_does_not_acknowledge_in_time() {
        use std::cell::Cell;
        let base = Instant::now();
        let grace = Duration::from_secs(3);
        let run = |acknowledge_at: Option<Duration>| {
            let flag = AtomicBool::new(false);
            let clock = Cell::new(Duration::ZERO);
            let sleeps = Cell::new(0u32);
            let acknowledged = await_acknowledgement(
                &flag,
                grace,
                || base + clock.get(),
                |step| {
                    assert!(step > Duration::ZERO && step <= Duration::from_millis(50));
                    sleeps.set(sleeps.get() + 1);
                    clock.set(clock.get() + step);
                    if acknowledge_at.is_some_and(|at| clock.get() >= at) {
                        flag.store(true, Ordering::SeqCst);
                    }
                },
            );
            (acknowledged, clock.get(), sleeps.get())
        };
        // Never acknowledged: fires exactly at the bound, polling in bounded steps.
        let (acknowledged, waited, sleeps) = run(None);
        assert!(!acknowledged);
        assert_eq!(waited, grace);
        assert_eq!(sleeps, 60);
        // Acknowledged before the bound: stands down as soon as it is seen.
        let (acknowledged, waited, _) = run(Some(Duration::from_millis(400)));
        assert!(acknowledged);
        assert_eq!(waited, Duration::from_millis(400));
        // Already acknowledged: no wait at all.
        let flag = AtomicBool::new(true);
        assert!(await_acknowledgement(
            &flag,
            grace,
            || base,
            |_| panic!("slept")
        ));
    }

    #[test]
    fn launcher_identity_distinguishes_processes_by_start_time() {
        let _serial = crate::platform::process_test_guard();
        let own = launcher_identity(std::process::id()).unwrap();
        assert_eq!(own, launcher_identity(std::process::id()).unwrap());
        let mut child = Command::new("sleep").arg("5").spawn().unwrap();
        let other = launcher_identity(child.id()).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        let (own_pid, own_start) = own.split_once(':').unwrap();
        let (child_pid, child_start) = other.split_once(':').unwrap();
        assert_ne!(own_pid, child_pid);
        assert!(own_start.parse::<u64>().unwrap() <= child_start.parse::<u64>().unwrap());
        assert!(
            launcher_identity(child.id()).is_err(),
            "reaped process has no identity"
        );
    }

    #[test]
    fn compositor_identity_is_stable_across_launch_routes_and_changes_on_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wayland-0");
        File::create(&path).unwrap();
        let relative =
            realm_for_compositor(1000, directory.path(), Some(OsStr::new("wayland-0")), false);
        let absolute = realm_for_compositor(1000, directory.path(), Some(path.as_os_str()), false);
        assert_eq!(relative, absolute);
        assert_eq!(
            relative,
            realm_for_compositor(1000, directory.path(), None, false)
        );
        fs_rename_keep_identity(&path);
        assert_ne!(
            relative,
            realm_for_compositor(1000, directory.path(), None, false)
        );
        assert_ne!(
            relative,
            realm_for_compositor(1001, directory.path(), None, false)
        );
        assert_eq!(
            realm_for_compositor(1000, directory.path(), None, true),
            realm_for_compositor(
                1000,
                directory.path(),
                Some(OsStr::new("other-display")),
                true
            )
        );
    }

    fn fs_rename_keep_identity(path: &Path) {
        // Keep the old inode allocated so a filesystem cannot reuse it for
        // the replacement while this test checks compositor restarts.
        std::fs::rename(path, path.with_extension("old")).unwrap();
        File::create(path).unwrap();
    }

    #[test]
    fn socket_namespace_cannot_escape_the_private_directory() {
        for invalid in ["", "../escape", "/tmp/escape", "not-hex"] {
            assert!(endpoint_for(invalid).is_err());
        }
    }

    /// CORE-A-09: the directory must never exist with a mode wider than 0700, not even between
    /// creation and a later chmod; a pre-existing directory is accepted only if it is private.
    #[test]
    fn private_directory_is_created_private_and_verified_when_present() {
        let base = tempfile::tempdir().unwrap();
        let fresh = base.path().join("fresh");
        CREATED_MODE.with(|m| m.set(None));
        ensure_private_directory(&fresh).unwrap();
        let at_creation = CREATED_MODE
            .with(|m| m.get())
            .expect("directory was created");
        assert_eq!(at_creation & 0o077, 0, "created with mode {at_creation:o}");
        assert_eq!(std::fs::metadata(&fresh).unwrap().mode() & 0o777, 0o700);
        // Already present and private: accepted.
        ensure_private_directory(&fresh).unwrap();
        // Already present but readable by others: refused, not repaired.
        let open = base.path().join("open");
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(ensure_private_directory(&open).is_err());
        // A symlink to a private directory: refused.
        let link = base.path().join("link");
        std::os::unix::fs::symlink(&fresh, &link).unwrap();
        assert!(ensure_private_directory(&link).is_err());
    }

    #[test]
    fn lock_excludes_other_instance_and_releases() {
        let _serial = crate::platform::process_test_guard();
        let app = format!("org.nativehtmlkit.test.{}", std::process::id());
        let one = InstanceLock::acquire(&app).unwrap();
        assert!(one.primary);
        let two = InstanceLock::acquire(&app).unwrap();
        assert!(!two.primary);
        assert_eq!(one.endpoint, two.endpoint);
        drop(one);
        drop(two);
        assert!(InstanceLock::acquire(&app).unwrap().primary);
    }

    #[test]
    fn deadline_applies_to_all_partial_reads() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        let mut stream = NativeStream {
            stream: reader,
            deadline: None,
        };
        stream
            .set_timeout(Some(Duration::from_millis(180)))
            .unwrap();
        let writer = std::thread::spawn(move || {
            for _ in 0..12 {
                if writer.write_all(b"x").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });
        let start = Instant::now();
        let mut bytes = [0; 12];
        let error = stream.read_exact(&mut bytes).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        // The error (rather than a successful 12-byte read) proves that the
        // per-call deadline was not refreshed. Leave scheduler headroom for
        // overloaded CI machines when checking elapsed wall-clock time.
        assert!(start.elapsed() < Duration::from_secs(2));
        stream.shutdown();
        writer.join().unwrap();
    }

    #[test]
    fn active_listener_cannot_be_replaced_but_stale_socket_can() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.sock");
        let endpoint = path.to_str().unwrap();
        let listener = NativeListener::bind(endpoint).unwrap();
        assert!(NativeListener::bind(endpoint).is_err());
        drop(listener);
        // UnixListener intentionally leaves its filesystem entry behind.
        drop(UnixListener::bind(&path).unwrap());
        assert!(path.exists());
        assert!(NativeListener::bind(endpoint).is_ok());
    }

    /// CORE-A-11: supervision markers of *this* host must not reach the backend (a host the
    /// backend launches would otherwise skip its own supervision and watch the wrong launcher).
    /// `UI4ME_TEST_MODE` stays inherited. The markers are set on a re-executed copy of this test
    /// binary, so no test mutates the shared process environment.
    #[test]
    fn backend_does_not_inherit_supervision_markers() {
        const CHILD: &str = "NATIVEHTMLKIT_TEST_A11_CHILD";
        let _serial = crate::platform::process_test_guard();
        if std::env::var_os(CHILD).is_some() {
            let config = crate::config::BackendConfig {
                program: "/bin/sh".into(),
                args: vec![
                    "-c".into(),
                    r#"printf '%s|%s|%s' "${UI4ME_SYSTEMD_UNIT-unset}" "${NATIVEHTMLKIT_LAUNCHER-unset}" "${UI4ME_TEST_MODE-unset}""#.into(),
                ],
                cwd: std::env::temp_dir(),
                env: Default::default(),
            };
            let mut native =
                spawn_backend(&config, "/unused", "test", "org.nativehtmlkit.test").unwrap();
            let mut out = String::new();
            native.stdout.read_to_string(&mut out).unwrap();
            assert_eq!(out, "unset|unset|1", "backend environment");
            return;
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "platform::linux::tests::backend_does_not_inherit_supervision_markers",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .env("UI4ME_SYSTEMD_UNIT", "outer.service")
            .env("NATIVEHTMLKIT_LAUNCHER", "1:2")
            .env("UI4ME_TEST_MODE", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child test failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout)
        );
    }

    #[test]
    fn exited_child_is_reaped_and_exit_status_is_stable() {
        let _serial = crate::platform::process_test_guard();
        let config = crate::config::BackendConfig {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "exit 7".into()],
            cwd: std::env::temp_dir(),
            env: Default::default(),
        };
        let mut native =
            spawn_backend(&config, "/unused", "test", "org.nativehtmlkit.test").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = native.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(status.code(), Some(7));
        assert_eq!(native.child.try_wait().unwrap().unwrap().code(), Some(7));
        assert!(
            native.child.group.is_none(),
            "a reaped PID must not remain a signal target"
        );
        let mut raw_status = 0;
        assert_eq!(
            unsafe { libc::waitpid(native.child.id() as i32, &mut raw_status, libc::WNOHANG) },
            -1
        );
        assert_eq!(
            io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
    }

    #[test]
    fn dropping_live_child_reaps_without_blocking_the_gui() {
        let _serial = crate::platform::process_test_guard();
        let config = crate::config::BackendConfig {
            program: "/bin/sleep".into(),
            args: vec!["30".into()],
            cwd: std::env::temp_dir(),
            env: Default::default(),
        };
        let native = spawn_backend(&config, "/unused", "test", "org.nativehtmlkit.test").unwrap();
        let pid = native.child.id();
        let start = Instant::now();
        drop(native);
        assert!(start.elapsed() < Duration::from_millis(500));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            // Observe without racing the reaper for ownership of the exit.
            let mut information: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid,
                    &mut information,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
                break;
            }
            assert!(Instant::now() < deadline, "child was not reaped");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
