//! URI parsing and normalization.
//!
//! Port of the Python `robotcode.core.uri` module.

use std::path::{Path, PathBuf};

use url::Url;

/// Error type for invalid URIs.
#[derive(Debug, thiserror::Error)]
pub enum UriError {
    #[error("Invalid URI: {0}")]
    Parse(#[from] url::ParseError),
    #[error(
        "Invalid URI scheme '{0}': only 'file' and 'untitled' are supported for path conversion"
    )]
    InvalidScheme(String),
    #[error("Cannot convert URI to path: {0}")]
    PathConversion(String),
}

/// A URI wrapper that provides filesystem path conversion and normalization.
///
/// Supports `file://` and `untitled://` schemes (as used by the LSP protocol).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Uri {
    inner: Url,
}

impl Uri {
    /// Parse a URI string.
    pub fn parse(s: &str) -> Result<Self, UriError> {
        let url = Url::parse(s)?;
        Ok(Self { inner: url })
    }

    /// Create a URI from a filesystem path.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, UriError> {
        let url = Url::from_file_path(path.as_ref())
            .map_err(|_| UriError::PathConversion(format!("{}", path.as_ref().display())))?;
        Ok(Self { inner: url })
    }

    /// Convert this URI to a filesystem path.
    ///
    /// Handles UNC paths and Windows drive letters, matching VS Code / LSP conventions.
    pub fn to_path(&self) -> Result<PathBuf, UriError> {
        let scheme = self.inner.scheme();
        if scheme != "file" && scheme != "untitled" {
            return Err(UriError::InvalidScheme(scheme.to_string()));
        }

        self.inner
            .to_file_path()
            .map_err(|_| UriError::PathConversion(self.inner.as_str().to_string()))
    }

    /// Return the scheme of this URI.
    pub fn scheme(&self) -> &str {
        self.inner.scheme()
    }

    /// Return the host/netloc of this URI.
    pub fn host(&self) -> Option<&str> {
        self.inner.host_str()
    }

    /// Return the path component of this URI.
    pub fn path(&self) -> &str {
        self.inner.path()
    }

    /// Return the query string, if any.
    pub fn query(&self) -> Option<&str> {
        self.inner.query()
    }

    /// Return the fragment, if any.
    pub fn fragment(&self) -> Option<&str> {
        self.inner.fragment()
    }

    /// Return the inner [`Url`].
    pub fn as_url(&self) -> &Url {
        &self.inner
    }

    /// Return a normalized version of this URI.
    ///
    /// For `file://` URIs this canonicalizes the path. Falls back to the
    /// original if the path does not exist on disk.
    pub fn normalized(&self) -> Self {
        if self.inner.scheme() == "file" {
            if let Ok(p) = self.to_path() {
                let normalized = crate::utils::path::normalized_path(&p);
                if let Ok(u) = Self::from_path(&normalized) {
                    return u;
                }
            }
        }
        self.clone()
    }

    /// Return a new URI with the given path component replaced.
    pub fn with_path(&self, new_path: &str) -> Result<Self, UriError> {
        let mut url = self.inner.clone();
        url.set_path(new_path);
        Ok(Self { inner: url })
    }
}

impl std::fmt::Display for Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::str::FromStr for Uri {
    type Err = UriError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl serde::Serialize for Uri {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.inner.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Uri {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Uri::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_uri() {
        let uri = Uri::parse("file:///home/user/project/test.robot").unwrap();
        assert_eq!(uri.scheme(), "file");
        assert_eq!(uri.path(), "/home/user/project/test.robot");
    }

    #[test]
    fn test_roundtrip_path() {
        let path = std::env::current_dir().unwrap().join("Cargo.toml");
        // Only test if the file exists
        if path.exists() {
            let uri = Uri::from_path(&path).unwrap();
            let roundtrip = uri.to_path().unwrap();
            assert_eq!(
                path.canonicalize().unwrap(),
                roundtrip.canonicalize().unwrap()
            );
        }
    }

    #[test]
    fn test_display() {
        let uri = Uri::parse("file:///home/user/test.robot").unwrap();
        assert_eq!(uri.to_string(), "file:///home/user/test.robot");
    }

    #[test]
    fn test_equality() {
        let a = Uri::parse("file:///home/user/test.robot").unwrap();
        let b = Uri::parse("file:///home/user/test.robot").unwrap();
        assert_eq!(a, b);
    }
}
