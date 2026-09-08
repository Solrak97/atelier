use std::{
    cmp::Ordering,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

/// A directory opened as a Caduceus project.
pub struct Project {
    root: PathBuf,
    entries: Vec<ProjectEntry>,
}

impl Project {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a directory", root.display()),
            ));
        }

        let entries = read_entries(&root)?;
        Ok(Self { root, entries })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }
}

/// One file or directory in a project tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEntry {
    name: String,
    path: PathBuf,
    kind: ProjectEntryKind,
    children: Vec<ProjectEntry>,
}

impl ProjectEntry {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn kind(&self) -> ProjectEntryKind {
        self.kind
    }

    pub fn children(&self) -> &[ProjectEntry] {
        &self.children
    }
}

/// The navigable kind of a project entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectEntryKind {
    Directory,
    File,
}

fn read_entries(directory: &Path) -> io::Result<Vec<ProjectEntry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if is_ignored(&entry.file_name()) {
            continue;
        }

        let path = entry.path();
        let file_type = entry.file_type()?;
        let kind = if file_type.is_dir() {
            ProjectEntryKind::Directory
        } else {
            ProjectEntryKind::File
        };
        let children = if kind == ProjectEntryKind::Directory {
            read_entries(&path)?
        } else {
            Vec::new()
        };

        entries.push(ProjectEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path,
            kind,
            children,
        });
    }

    entries.sort_by(|left, right| match (left.kind, right.kind) {
        (ProjectEntryKind::Directory, ProjectEntryKind::File) => Ordering::Less,
        (ProjectEntryKind::File, ProjectEntryKind::Directory) => Ordering::Greater,
        _ => left
            .name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name)),
    });
    Ok(entries)
}

fn is_ignored(name: &OsStr) -> bool {
    matches!(name.to_str(), Some(".git" | "target"))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TempProject(PathBuf);

    impl TempProject {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("caduceus-project-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn reads_a_sorted_recursive_tree() {
        let temp = TempProject::new();
        fs::create_dir(temp.0.join("src")).unwrap();
        fs::write(temp.0.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(temp.0.join("README.md"), "# project").unwrap();
        fs::write(temp.0.join("alpha.txt"), "alpha").unwrap();

        let project = Project::open(&temp.0).unwrap();

        assert_eq!(project.root(), fs::canonicalize(&temp.0).unwrap());
        assert_eq!(
            project
                .entries()
                .iter()
                .map(ProjectEntry::name)
                .collect::<Vec<_>>(),
            ["src", "alpha.txt", "README.md"]
        );
        assert_eq!(project.entries()[0].kind(), ProjectEntryKind::Directory);
        assert_eq!(project.entries()[0].children()[0].name(), "main.rs");
    }

    #[test]
    fn omits_repository_and_build_internals() {
        let temp = TempProject::new();
        fs::create_dir(temp.0.join(".git")).unwrap();
        fs::create_dir(temp.0.join("target")).unwrap();
        fs::write(temp.0.join(".git/config"), "config").unwrap();
        fs::write(temp.0.join("target/output"), "binary").unwrap();
        fs::write(temp.0.join("visible.txt"), "visible").unwrap();

        let project = Project::open(&temp.0).unwrap();

        assert_eq!(project.entries().len(), 1);
        assert_eq!(project.entries()[0].name(), "visible.txt");
    }

    #[test]
    fn rejects_a_file_as_the_project_root() {
        let temp = TempProject::new();
        let file = temp.0.join("file.txt");
        fs::write(&file, "text").unwrap();

        let error = Project::open(file).err().unwrap();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
