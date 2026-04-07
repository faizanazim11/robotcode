//! Multi-root workspace model.
//!
//! Port of the Python `robotcode.core.workspace` module.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::uri::Uri;

/// A single root folder inside the workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceFolder {
    /// The human-readable name of the folder.
    pub name: String,
    /// The URI of the folder root.
    pub uri: Uri,
}

impl WorkspaceFolder {
    pub fn new(name: impl Into<String>, uri: Uri) -> Self {
        Self {
            name: name.into(),
            uri,
        }
    }

    /// Return the filesystem path of this folder.
    pub fn path(&self) -> Option<PathBuf> {
        self.uri.to_path().ok()
    }
}

/// Multi-root workspace state.
///
/// Holds the root URI, workspace folders, and arbitrary JSON settings.
#[derive(Debug)]
pub struct Workspace {
    /// The primary root URI (may be `None` for untitled workspaces).
    pub root_uri: Option<Uri>,
    folders: RwLock<Vec<WorkspaceFolder>>,
    settings: RwLock<serde_json::Value>,
}

impl Workspace {
    /// Create a new workspace.
    pub fn new(
        root_uri: Option<Uri>,
        folders: Vec<WorkspaceFolder>,
        settings: serde_json::Value,
    ) -> Arc<Self> {
        Arc::new(Self {
            root_uri,
            folders: RwLock::new(folders),
            settings: RwLock::new(settings),
        })
    }

    /// Return a snapshot of the workspace folders.
    pub fn folders(&self) -> Vec<WorkspaceFolder> {
        self.folders.read().unwrap().clone()
    }

    /// Replace the workspace folders.
    pub fn set_folders(&self, new_folders: Vec<WorkspaceFolder>) {
        *self.folders.write().unwrap() = new_folders;
    }

    /// Return the current settings.
    pub fn settings(&self) -> serde_json::Value {
        self.settings.read().unwrap().clone()
    }

    /// Replace the settings.
    pub fn set_settings(&self, new_settings: serde_json::Value) {
        *self.settings.write().unwrap() = new_settings;
    }

    /// Find the innermost workspace folder that contains `uri`.
    ///
    /// If multiple folders match, the one with the longest URI wins (most
    /// specific match).
    pub fn folder_for_uri(&self, uri: &Uri) -> Option<WorkspaceFolder> {
        let target_path = uri.to_path().ok()?;
        let folders = self.folders.read().unwrap();

        folders
            .iter()
            .filter_map(|f| {
                let folder_path = f.path()?;
                if target_path.starts_with(&folder_path) {
                    Some((f.clone(), folder_path.as_os_str().len()))
                } else {
                    None
                }
            })
            .max_by_key(|(_, len)| *len)
            .map(|(f, _)| f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_folder_for_uri() {
        // Use drive-letter URIs on Windows; plain unix paths on other platforms.
        #[cfg(unix)]
        let (folder_uri_str, file_uri_str) = (
            "file:///home/user/project",
            "file:///home/user/project/tests/test.robot",
        );
        #[cfg(windows)]
        let (folder_uri_str, file_uri_str) = (
            "file:///C:/home/user/project",
            "file:///C:/home/user/project/tests/test.robot",
        );

        let folder_uri = Uri::parse(folder_uri_str).unwrap();
        let folder = WorkspaceFolder::new("project", folder_uri);
        let ws = Workspace::new(None, vec![folder], serde_json::Value::Null);

        let file_uri = Uri::parse(file_uri_str).unwrap();
        let found = ws.folder_for_uri(&file_uri);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "project");
    }

    #[test]
    fn test_workspace_no_matching_folder() {
        let folder_uri = Uri::parse("file:///home/user/project").unwrap();
        let folder = WorkspaceFolder::new("project", folder_uri);
        let ws = Workspace::new(None, vec![folder], serde_json::Value::Null);

        let file_uri = Uri::parse("file:///home/other/file.robot").unwrap();
        let found = ws.folder_for_uri(&file_uri);
        assert!(found.is_none());
    }
}
