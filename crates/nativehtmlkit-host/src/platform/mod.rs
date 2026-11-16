//! Native facilities whose lifetime is independent of the presentation engine.
//!
//! Only this module chooses the operating-system implementation. The same
//! transport and backend state machines are used on both supported platforms.

pub mod dialogs;
pub mod external;
pub mod preferences;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("nativehtmlkit-host 0.1 implements Linux and Windows; this target is not qualified");

/// Host-supervision markers that describe *this* host process and must not be inherited by the
/// backend (CORE-A-11): a host launched by the backend would otherwise believe it is already
/// supervised and watch the outer launcher. `UI4ME_TEST_MODE` is deliberately still inherited.
/// A manifest's explicit `[backend.env]` value is still applied.
pub(crate) const HOST_ONLY_ENV: [&str; 2] = ["UI4ME_SYSTEMD_UNIT", "NATIVEHTMLKIT_LAUNCHER"];

/// What a launcher watch does when the event loop does not act on launcher loss
/// (NHK-CROSS-0001). Unused where the host is not bound to a launcher (Windows).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct LauncherWatchdog {
    /// Set by the event loop once it has consumed the launcher-gone flag.
    pub acknowledged: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The manifest's `shutdown_timeout_ms`.
    pub shutdown_timeout: std::time::Duration,
    /// Backend process (group leader on Linux), terminated best-effort before exiting.
    /// 0 = no backend, or the event loop has reaped it (FR-R-13: its PID, and so the
    /// process-group id, may then belong to an unrelated process group).
    pub backend_group: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

/// Stable, non-secret namespace hash. This is not an authentication primitive.
pub(crate) fn namespace_hash(value: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// Test-only serialisation (CORE-A-06). A child forked by one test shares every open file
/// description of the test process until it execs, including an `flock`ed instance lock, so a test
/// that drops and re-acquires that lock in-process must not overlap any test that spawns a
/// process. Every such test holds this guard; poisoning by a failed test is ignored.
#[cfg(test)]
pub(crate) fn process_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GUARD.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaces_are_stable_and_distinguish_inputs() {
        assert_eq!(namespace_hash(b"hello"), "a430d84680aabd0b");
        assert_ne!(
            namespace_hash(b"app/session-a"),
            namespace_hash(b"app/session-b")
        );
    }
}
