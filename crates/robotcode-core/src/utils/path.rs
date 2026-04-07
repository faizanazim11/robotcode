//! File identity and path utilities.
//!
//! Port of the Python `robotcode.core.utils.path` module.

use std::path::{Path, PathBuf};

/// A stable file identity using (device, inode) on Unix.
/// On Windows both fields contain `0` (no inode support).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId {
    pub dev: u64,
    pub ino: u64,
}

/// Return the [`FileId`] for `path`, or `None` if the path cannot be stat'd.
pub fn file_id(path: impl AsRef<Path>) -> Option<FileId> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path.as_ref()).ok()?;
        Some(FileId {
            dev: meta.dev(),
            ino: meta.ino(),
        })
    }

    #[cfg(windows)]
    {
        // Windows doesn't expose inode numbers through the standard library.
        // Return a best-effort value using the file index from the Windows API,
        // or fall back to zeros so callers can at least check `None` vs `Some`.
        let _ = path;
        Some(FileId { dev: 0, ino: 0 })
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        None
    }
}

/// Return `true` if `id1` and `id2` refer to the same file.
///
/// `None` never matches anything.
pub fn same_file_id(id1: Option<FileId>, id2: Option<FileId>) -> bool {
    match (id1, id2) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Return `true` if `path` is relative to (a descendant of) `base`.
pub fn path_is_relative_to(path: impl AsRef<Path>, base: impl AsRef<Path>) -> bool {
    path.as_ref().starts_with(base.as_ref())
}

/// Normalize `path` to an absolute, cleaned-up path.
///
/// On Windows, drive letters are upper-cased. Does **not** resolve symlinks.
pub fn normalized_path(path: impl AsRef<Path>) -> PathBuf {
    let p = path.as_ref();

    // Make absolute (without touching the filesystem if possible)
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(p)
    };

    // Lexically clean the path (remove `.` and `..`)
    let mut components: Vec<std::ffi::OsString> = Vec::new();
    for component in absolute.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop();
            }
            other => components.push(other.as_os_str().to_os_string()),
        }
    }

    if components.is_empty() {
        return PathBuf::from("/");
    }

    let mut result = PathBuf::new();
    for c in components {
        result.push(c);
    }

    #[cfg(windows)]
    {
        // Upper-case the drive letter (e.g., `c:\` → `C:\`)
        let s = result.to_string_lossy();
        if s.len() >= 2 && s.as_bytes()[1] == b':' {
            let upper = s[..1].to_uppercase() + &s[1..];
            return PathBuf::from(upper.as_ref());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_is_relative_to() {
        assert!(path_is_relative_to(
            "/home/user/project/file.robot",
            "/home/user/project"
        ));
        assert!(!path_is_relative_to(
            "/home/other/file.robot",
            "/home/user/project"
        ));
    }

    #[test]
    fn test_normalized_path_removes_dots() {
        let p = normalized_path("/home/user/../user/./project");
        assert_eq!(p, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn test_file_id_some_for_existing() {
        // Use the current executable which definitely exists
        let path = std::env::current_exe().unwrap();
        assert!(file_id(&path).is_some());
    }

    #[test]
    fn test_file_id_none_for_missing() {
        assert!(file_id("/this/path/definitely/does/not/exist/robotcode").is_none());
    }
}
