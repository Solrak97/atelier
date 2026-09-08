use std::{
    cmp::Ordering,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use crate::Document;

/// Directory created at a project root for Atelier-owned project files.
pub const PROJECT_METADATA_DIR: &str = ".atelier";

/// A directory opened as a Atelier project.
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

    /// Open a directory, or a file's parent directory with that file loaded.
    pub fn open_from_path(path: impl AsRef<Path>) -> io::Result<(Self, Option<Document>)> {
        let path = path.as_ref();
        if path.is_dir() {
            return Ok((Self::open(path)?, None));
        }

        let document = Document::open(path)?;
        let root = document
            .path()
            .and_then(Path::parent)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file has no parent"))?;
        Ok((Self::open(root)?, Some(document)))
    }

    pub fn ensure_metadata_dir(&self) -> io::Result<PathBuf> {
        let directory = self.root.join(PROJECT_METADATA_DIR);
        fs::create_dir_all(&directory)?;
        Ok(directory)
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
    matches!(
        name.to_str(),
        Some(".git" | "target" | PROJECT_METADATA_DIR)
    )
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
                .join(format!("atelier-project-{}-{unique}", std::process::id()));
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
    fn creates_hidden_metadata_directory() {
        let temp = TempProject::new();
        fs::write(temp.0.join("visible.txt"), "visible").unwrap();
        let project = Project::open(&temp.0).unwrap();
        let metadata = project.ensure_metadata_dir().unwrap();
        fs::write(metadata.join("scratch"), "local").unwrap();

        let reopened = Project::open(&temp.0).unwrap();
        assert!(metadata.is_dir());
        assert_eq!(reopened.entries().len(), 1);
        assert_eq!(reopened.entries()[0].name(), "visible.txt");
    }

    #[test]
    fn opens_a_file_through_its_parent_project() {
        let temp = TempProject::new();
        let file = temp.0.join("notes.txt");
        fs::write(&file, "hello").unwrap();

        let (project, document) = Project::open_from_path(&file).unwrap();
        assert_eq!(project.root(), fs::canonicalize(&temp.0).unwrap());
        assert_eq!(document.unwrap().text(), "hello");
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
