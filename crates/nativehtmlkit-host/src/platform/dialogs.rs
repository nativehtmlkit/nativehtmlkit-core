//! Parented, cancellable native file dialogs. The worker retains the real
//! parent until the native dialog is closed and its resources are released.
//! Completion always runs off the GUI thread; callers marshal it to their
//! event loop. Dropping the handle initiates cancellation without blocking.

use anyhow::{bail, Result};
use std::path::PathBuf;
use std::sync::Arc;
use winit::window::Window;

#[cfg(target_os = "linux")]
#[path = "dialogs_linux.rs"]
mod native;
#[cfg(target_os = "windows")]
#[path = "dialogs_windows.rs"]
mod native;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogKind {
    OpenFiles,
    SaveFile,
    ChooseFolder,
}

#[derive(Clone, Debug)]
pub struct DialogFilter {
    pub name: String,
    /// Literal suffixes without the leading dot, or `*` for all files.
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DialogOptions {
    pub kind: DialogKind,
    /// Allow more than one selected file for an open-file request.
    pub multiple: bool,
    pub title: Option<String>,
    pub directory: Option<PathBuf>,
    pub file_name: Option<String>,
    pub filters: Vec<DialogFilter>,
}

impl DialogOptions {
    fn validate(&self) -> Result<()> {
        if self
            .title
            .as_ref()
            .is_some_and(|v| v.len() > 4096 || v.contains('\0'))
        {
            bail!("dialog title is too long or contains NUL");
        }
        if self.file_name.as_ref().is_some_and(|v| {
            v.len() > 4096 || v.contains(['\0', '/', '\\']) || v == "." || v == ".."
        }) {
            bail!("dialog file_name must be a filename, not a path");
        }
        if self.filters.len() > 64 {
            bail!("at most 64 dialog filters are supported");
        }
        for filter in &self.filters {
            if filter.name.is_empty() || filter.name.len() > 256 || filter.name.contains('\0') {
                bail!("dialog filter name is empty, too long or contains NUL");
            }
            if filter.extensions.is_empty() || filter.extensions.len() > 64 {
                bail!("dialog filter requires between 1 and 64 extensions");
            }
            for extension in &filter.extensions {
                if extension != "*"
                    && (extension.is_empty()
                        || extension.len() > 64
                        || extension.starts_with('.')
                        || !extension
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"._+-".contains(&c)))
                {
                    bail!("dialog extensions must be literal suffixes without a leading dot, or *");
                }
            }
        }
        Ok(())
    }

    fn title(&self) -> &str {
        self.title.as_deref().unwrap_or(match self.kind {
            DialogKind::OpenFiles if self.multiple => "Open files",
            DialogKind::OpenFiles => "Open file",
            DialogKind::SaveFile => "Save file",
            DialogKind::ChooseFolder => "Choose folder",
        })
    }
}

pub struct DialogHandle {
    cancel: Arc<dyn Fn() + Send + Sync>,
}

impl DialogHandle {
    /// Request native cancellation. Completion reports `Ok(None)` once the
    /// dialog is actually closed; cancellation itself never blocks the GUI.
    pub fn cancel(&self) {
        (self.cancel)();
    }

    /// Shared lifetime handle for a native chooser or portal request.
    pub(crate) fn from_cancel(cancel: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self { cancel }
    }
}

impl Drop for DialogHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub type DialogResult = Result<Option<Vec<PathBuf>>>;

pub fn start(
    parent: Arc<dyn Window>,
    options: DialogOptions,
    completion: Box<dyn FnOnce(DialogResult) + Send>,
) -> Result<DialogHandle> {
    options.validate()?;
    // Some platforms (Win32 in winit) only expose the parent's handle on the
    // GUI thread, which is where this function runs.
    let captured = native::capture_parent(&parent)?;
    let state = Arc::new(native::Cancellation::default());
    let worker_state = Arc::clone(&state);
    std::thread::Builder::new()
        .name("nativehtmlkit-dialog".into())
        .spawn(move || {
            // The Arc also makes raw window/display handles valid throughout the
            // platform call, including exporting/unexporting a Wayland surface.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                native::run(&parent, captured, &options, &worker_state)
            }))
            .unwrap_or_else(|_| Err(anyhow::anyhow!("native dialog worker panicked")));
            complete_after_release(parent, result, completion);
        })?;
    Ok(DialogHandle::from_cancel(Arc::new(move || state.cancel())))
}

/// CORE-A-15: release a worker's window reference *before* reporting completion. The event
/// loop may remove the window as soon as it sees the completion; dropping afterwards could make
/// this worker the last owner, tearing the native window (a Wayland surface) down off the
/// event-loop thread.
pub(crate) fn complete_after_release<P, R>(parent: P, result: R, completion: impl FnOnce(R)) {
    drop(parent);
    completion(result);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CORE-A-15: when the completion runs, the worker no longer holds the parent.
    #[test]
    fn worker_releases_the_parent_before_completion() {
        let owner = Arc::new(());
        let worker = owner.clone();
        let probe = owner.clone();
        complete_after_release(worker, 7, move |value| {
            assert_eq!(value, 7);
            assert_eq!(Arc::strong_count(&probe), 2, "worker still held the parent");
        });
        assert_eq!(Arc::strong_count(&owner), 1);
    }
    #[test]
    fn rejects_filter_globs_and_filename_paths() {
        let mut options = DialogOptions {
            kind: DialogKind::OpenFiles,
            multiple: true,
            title: None,
            directory: None,
            file_name: None,
            filters: vec![DialogFilter {
                name: "Images".into(),
                extensions: vec!["png".into(), "tar.gz".into()],
            }],
        };
        assert!(options.validate().is_ok());
        options.filters[0].extensions.push("*;*.exe".into());
        assert!(options.validate().is_err());
        options.filters.clear();
        options.file_name = Some("../outside".into());
        assert!(options.validate().is_err());
    }
}
