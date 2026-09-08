use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::paths::AppPaths;

const MAX_RECENT_PROJECTS: usize = 20;

/// A folder previously opened as a Atelier project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecentProject {
    pub path: PathBuf,
    pub name: String,
    pub opened_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct RegistryFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_open: Option<PathBuf>,
    #[serde(default)]
    projects: Vec<RecentProject>,
}

/// Durable list of recently opened projects.
#[derive(Clone, Debug)]
pub struct ProjectRegistry {
    file: PathBuf,
    last_open: Option<PathBuf>,
    projects: Vec<RecentProject>,
}

impl ProjectRegistry {
    pub fn load(paths: &AppPaths) -> io::Result<Self> {
        let file = paths.projects_file();
        let (last_open, projects) = if file.exists() {
            let contents = fs::read_to_string(&file)?;
            let parsed = toml::from_str::<RegistryFile>(&contents)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            (parsed.last_open, parsed.projects)
        } else {
            (None, Vec::new())
        };
        Ok(Self {
            file,
            last_open,
            projects,
        })
    }

    pub fn empty(paths: &AppPaths) -> Self {
        Self {
            file: paths.projects_file(),
            last_open: None,
            projects: Vec::new(),
        }
    }

    pub fn projects(&self) -> &[RecentProject] {
        &self.projects
    }

    pub fn last_open(&self) -> Option<&Path> {
        self.last_open.as_deref()
    }

    pub fn forget_last_open(&mut self) -> io::Result<()> {
        if self.last_open.is_none() {
            return Ok(());
        }
        self.last_open = None;
        self.save()
    }

    pub fn record(&mut self, root: &Path) -> io::Result<&RecentProject> {
        let path = fs::canonicalize(root)?;
        let name = path
            .file_name()
            .unwrap_or_else(|| path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let opened_at = unix_now();
        self.projects.retain(|project| project.path != path);
        self.projects.insert(
            0,
            RecentProject {
                path,
                name,
                opened_at,
            },
        );
        self.projects.truncate(MAX_RECENT_PROJECTS);
        self.last_open = Some(self.projects[0].path.clone());
        self.save()?;
        Ok(&self.projects[0])
    }

    pub fn remove(&mut self, root: &Path) -> io::Result<bool> {
        let path = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let before = self.projects.len();
        self.projects
            .retain(|project| project.path != path && project.path != root);
        let dropped_last_open = self.last_open.as_ref().is_some_and(|last| {
            last == &path || last == root
        });
        if dropped_last_open {
            self.last_open = None;
        }
        if self.projects.len() == before && !dropped_last_open {
            return Ok(false);
        }
        self.save()?;
        Ok(self.projects.len() != before)
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(&RegistryFile {
            last_open: self.last_open.clone(),
            projects: self.projects.clone(),
        })
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(&self.file, contents)
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn registry_at(temp: &TempDir) -> (AppPaths, ProjectRegistry) {
        let paths = AppPaths::from_dirs(
            temp.path().join("config"),
            temp.path().join("data"),
            temp.path().join("cache"),
        )
        .unwrap();
        (paths.clone(), ProjectRegistry::load(&paths).unwrap())
    }

    #[test]
    fn missing_file_starts_empty() {
        let temp = TempDir::new("registry-empty");
        let (_, registry) = registry_at(&temp);
        assert!(registry.projects().is_empty());
    }

    #[test]
    fn records_deduplicates_and_caps_recent_projects() {
        let temp = TempDir::new("registry-record");
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let (paths, mut registry) = registry_at(&temp);

        registry.record(&first).unwrap();
        registry.record(&second).unwrap();
        registry.record(&first).unwrap();

        assert_eq!(registry.projects().len(), 2);
        assert_eq!(
            registry.projects()[0].path,
            fs::canonicalize(&first).unwrap()
        );
        assert_eq!(registry.projects()[0].name, "first");
        assert_eq!(
            registry.projects()[1].path,
            fs::canonicalize(&second).unwrap()
        );

        for index in 0..MAX_RECENT_PROJECTS {
            let extra = temp.path().join(format!("project-{index}"));
            fs::create_dir(&extra).unwrap();
            registry.record(&extra).unwrap();
        }
        assert_eq!(registry.projects().len(), MAX_RECENT_PROJECTS);

        let reloaded = ProjectRegistry::load(&paths).unwrap();
        assert_eq!(reloaded.projects().len(), MAX_RECENT_PROJECTS);
        assert_eq!(
            reloaded.projects()[0].name,
            format!("project-{}", MAX_RECENT_PROJECTS - 1)
        );
    }

    #[test]
    fn last_open_follows_record_and_can_be_forgotten() {
        let temp = TempDir::new("registry-last-open");
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let (paths, mut registry) = registry_at(&temp);

        registry.record(&first).unwrap();
        assert_eq!(
            registry.last_open(),
            Some(fs::canonicalize(&first).unwrap().as_path())
        );

        registry.record(&second).unwrap();
        assert_eq!(
            registry.last_open(),
            Some(fs::canonicalize(&second).unwrap().as_path())
        );

        let reloaded = ProjectRegistry::load(&paths).unwrap();
        assert_eq!(reloaded.last_open(), registry.last_open());

        registry.forget_last_open().unwrap();
        assert_eq!(registry.last_open(), None);
        assert_eq!(registry.projects().len(), 2);

        let forgotten = ProjectRegistry::load(&paths).unwrap();
        assert_eq!(forgotten.last_open(), None);
        assert_eq!(forgotten.projects().len(), 2);
    }

    #[test]
    fn remove_drops_a_missing_or_canonical_path() {
        let temp = TempDir::new("registry-remove");
        let project = temp.path().join("gone");
        fs::create_dir(&project).unwrap();
        let (_, mut registry) = registry_at(&temp);
        registry.record(&project).unwrap();

        assert!(registry.remove(&project).unwrap());
        assert!(registry.projects().is_empty());
        assert_eq!(registry.last_open(), None);
        assert!(!registry.remove(&project).unwrap());
    }

    #[test]
    fn rejects_invalid_toml() {
        let temp = TempDir::new("registry-invalid");
        let paths = AppPaths::from_dirs(
            temp.path().join("config"),
            temp.path().join("data"),
            temp.path().join("cache"),
        )
        .unwrap();
        fs::write(paths.projects_file(), "this is not toml {").unwrap();

        let error = ProjectRegistry::load(&paths).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
