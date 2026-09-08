use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;

/// User-owned application directories for config, data, and cache.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
}

impl AppPaths {
    /// Resolve and create the standard XDG roots for Atelier.
    pub fn standard() -> io::Result<Self> {
        let dirs = ProjectDirs::from("", "Atelier", "atelier").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not determine Atelier application directories",
            )
        })?;
        Self::from_dirs(dirs.config_dir(), dirs.data_dir(), dirs.cache_dir())
    }

    /// Use explicit roots, creating them if needed. Intended for tests.
    pub fn from_dirs(
        config: impl AsRef<Path>,
        data: impl AsRef<Path>,
        cache: impl AsRef<Path>,
    ) -> io::Result<Self> {
        let config = config.as_ref().to_path_buf();
        let data = data.as_ref().to_path_buf();
        let cache = cache.as_ref().to_path_buf();
        fs::create_dir_all(&config)?;
        fs::create_dir_all(&data)?;
        fs::create_dir_all(&cache)?;
        Ok(Self {
            config,
            data,
            cache,
        })
    }

    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    pub fn projects_file(&self) -> PathBuf {
        self.data.join("projects.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn creates_injected_roots() {
        let temp = TempDir::new("paths");
        let paths = AppPaths::from_dirs(
            temp.path().join("config"),
            temp.path().join("data"),
            temp.path().join("cache"),
        )
        .unwrap();

        assert!(paths.config_dir().is_dir());
        assert!(paths.data_dir().is_dir());
        assert!(paths.cache_dir().is_dir());
        assert_eq!(
            paths.projects_file(),
            paths.data_dir().join("projects.toml")
        );
    }
}
