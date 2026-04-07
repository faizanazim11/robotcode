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
        // Windows doesn't expose stable inode numbers through the standard library.
        // Verify the path can be stat'd so the function contract is consistent
        // across platforms (returns None for non-existent paths).
        let _meta = std::fs::metadata(path.as_ref()).ok()?;
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

    // Lexically clean the path (remove `.` and `..`).
    // Track a minimum depth to avoid popping past the root or drive prefix.
    let mut components: Vec<std::ffi::OsString> = Vec::new();
    let mut min_depth: usize = 0;

    for component in absolute.components() {
        use std::path::Component;
        match component {
            // Root and prefix components (e.g. `/` on Unix, `C:` + `\` on Windows)
            // are never poppable — record how many we have pushed.
            Component::RootDir | Component::Prefix(_) => {
                components.push(component.as_os_str().to_os_string());
                min_depth += 1;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                // Only pop a normal component; never go past the root/prefix.
                if components.len() > min_depth {
                    components.pop();
                }
            }
            Component::Normal(_) => {
                components.push(component.as_os_str().to_os_string());
            }
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
            let upper: String = s[..1].to_uppercase() + &s[1..];
            return PathBuf::from(upper);
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
    fn test_normalized_path_does_not_escape_root() {
        // `..` at the filesystem root must not produce a relative path.
        let p = normalized_path("/../a");
        assert_eq!(p, PathBuf::from("/a"));
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
