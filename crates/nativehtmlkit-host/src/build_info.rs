//! The source identity embedded at build time by `build.rs` (FR-R-04). Additive diagnostics:
//! `nativehtmlkit-host --version --verbose` prints it as one JSON object and `doctor` reports it
//! as `build`. `scripts/release-local.py` compares it with the checkout it stages.

/// `git` (computed from the checkout), `file` (from `SOURCE_IDENTITY.json` of a source export)
/// or `unknown` (neither; such a binary is never staged).
pub const SOURCE_IDENTITY: &str = env!("NHK_BUILD_SOURCE_IDENTITY");
pub const SOURCE_COMMIT: &str = env!("NHK_BUILD_SOURCE_COMMIT");
pub const SOURCE_TREE: &str = env!("NHK_BUILD_SOURCE_TREE");
pub const SOURCE_DIRTY: &str = env!("NHK_BUILD_SOURCE_DIRTY");
pub const SOURCE_FILES_SHA256: &str = env!("NHK_BUILD_SOURCE_FILES_SHA256");
const SOURCE_NOTE: &str = env!("NHK_BUILD_SOURCE_NOTE");

fn optional(value: &str) -> serde_json::Value {
    if value.is_empty() {
        serde_json::Value::Null
    } else {
        value.into()
    }
}

pub fn source() -> serde_json::Value {
    serde_json::json!({
        "source_identity": SOURCE_IDENTITY,
        "source_commit": optional(SOURCE_COMMIT),
        "source_tree": optional(SOURCE_TREE),
        "source_dirty": SOURCE_DIRTY == "true",
        "source_files_sha256": optional(SOURCE_FILES_SHA256),
        "source_identity_note": optional(SOURCE_NOTE),
    })
}

/// The `--version --verbose` object: name and version (as `--version` prints them) plus the
/// embedded source identity.
pub fn verbose_version() -> serde_json::Value {
    let mut value = source();
    value["name"] = "nativehtmlkit-host".into();
    value["version"] = env!("CARGO_PKG_VERSION").into();
    value
}

/// Whether the arguments (without argv\[0\]) are exactly `--version --verbose` in either order.
pub fn wants_verbose_version<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> bool {
    let args: Vec<_> = args.iter().map(|a| a.as_ref()).collect();
    args.len() == 2
        && args.contains(&std::ffi::OsStr::new("--version"))
        && args.contains(&std::ffi::OsStr::new("--verbose"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbose_version_reports_a_consistent_embedded_identity() {
        let v = verbose_version();
        assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
        match SOURCE_IDENTITY {
            "git" | "file" => {
                let hex40 = |s: &serde_json::Value| {
                    s.as_str()
                        .is_some_and(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                };
                assert!(hex40(&v["source_commit"]), "{v}");
                assert!(hex40(&v["source_tree"]), "{v}");
                assert!(matches!(SOURCE_DIRTY, "true" | "false"));
            }
            "unknown" => assert!(v["source_identity_note"].is_string(), "{v}"),
            other => panic!("unexpected identity kind {other}"),
        }
    }

    #[test]
    fn only_the_exact_pair_selects_the_verbose_version() {
        assert!(wants_verbose_version(&["--version", "--verbose"]));
        assert!(wants_verbose_version(&["--verbose", "--version"]));
        assert!(!wants_verbose_version(&["--version"]));
        assert!(!wants_verbose_version(&["--version", "--verbose", "x"]));
        assert!(!wants_verbose_version(&["doctor", "--verbose"]));
    }
}
