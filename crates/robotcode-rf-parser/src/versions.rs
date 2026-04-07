//! Robot Framework version constants and feature-flag helpers.

/// A Robot Framework version triple `(major, minor, patch)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RfVersion(pub u32, pub u32, pub u32);

impl std::fmt::Display for RfVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Robot Framework 5.0
pub const RF5: RfVersion = RfVersion(5, 0, 0);
/// Robot Framework 6.0
pub const RF6: RfVersion = RfVersion(6, 0, 0);
/// Robot Framework 7.0
pub const RF7: RfVersion = RfVersion(7, 0, 0);

/// Feature flags that differ between RF versions.
#[derive(Debug, Clone)]
pub struct VersionConfig {
    /// RF5+ supports `WHILE` loops.
    pub has_while: bool,
    /// RF5+ supports `TRY`/`EXCEPT`/`FINALLY`.
    pub has_try_except: bool,
    /// RF6+ supports `RETURN` statement (distinct from `[Return]` setting).
    pub has_return_statement: bool,
    /// RF7+ uses `DEFAULT TAGS` instead of `FORCE TAGS`.
    pub has_default_tags: bool,
    /// RF7+ supports `KEYWORD TAGS` section setting.
    pub has_keyword_tags: bool,
    /// RF7+ supports typed variables (`${x: int}`).
    pub has_typed_variables: bool,
}

impl VersionConfig {
    /// Build a `VersionConfig` from an `RfVersion`.
    pub fn from_version(v: &RfVersion) -> Self {
        Self {
            has_while: *v >= RF5,
            has_try_except: *v >= RF5,
            has_return_statement: *v >= RF5,
            has_default_tags: *v >= RF7,
            has_keyword_tags: *v >= RF7,
            has_typed_variables: *v >= RF7,
        }
    }

    /// Config for the latest supported RF version (RF7).
    pub fn latest() -> Self {
        Self::from_version(&RF7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rf7_has_all_features() {
        let cfg = VersionConfig::from_version(&RF7);
        assert!(cfg.has_while);
        assert!(cfg.has_try_except);
        assert!(cfg.has_return_statement);
        assert!(cfg.has_default_tags);
        assert!(cfg.has_keyword_tags);
        assert!(cfg.has_typed_variables);
    }

    #[test]
    fn version_ordering() {
        assert!(RF5 < RF6);
        assert!(RF6 < RF7);
        assert!(RF5 < RF7);
    }
}
