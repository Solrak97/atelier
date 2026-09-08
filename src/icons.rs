use std::path::Path;

use gpui::{Hsla, IntoElement, div, prelude::*, px, rgb, svg};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExplorerIcon {
    ChevronDown,
    ChevronRight,
    File,
    Folder,
    FolderOpen,
    Lock,
    Markdown,
    Rust,
    Toml,
}

impl ExplorerIcon {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::ChevronDown => include_bytes!("../assets/icons/explorer/chevron-down.svg"),
            Self::ChevronRight => include_bytes!("../assets/icons/explorer/chevron-right.svg"),
            Self::File => include_bytes!("../assets/icons/explorer/file.svg"),
            Self::Folder => include_bytes!("../assets/icons/explorer/folder.svg"),
            Self::FolderOpen => include_bytes!("../assets/icons/explorer/folder-open.svg"),
            Self::Lock => include_bytes!("../assets/icons/explorer/lock.svg"),
            Self::Markdown => include_bytes!("../assets/icons/explorer/markdown.svg"),
            Self::Rust => include_bytes!("../assets/icons/explorer/rust.svg"),
            Self::Toml => include_bytes!("../assets/icons/explorer/toml.svg"),
        }
    }

    pub(crate) fn paint(self, size: f32, color: impl Into<Hsla>) -> impl IntoElement {
        svg()
            .size(px(size))
            .flex_none()
            .text_color(color)
            .data(self.bytes())
    }
}

pub(crate) fn file_icon(path: &Path) -> (ExplorerIcon, Hsla) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name.ends_with(".lock") || name == "lock" {
        return (ExplorerIcon::Lock, rgb(0x8f96a3).into());
    }

    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("rs") => (ExplorerIcon::Rust, rgb(0xdea584).into()),
        Some("toml") => (ExplorerIcon::Toml, rgb(0x98c379).into()),
        Some("md" | "markdown") => (ExplorerIcon::Markdown, rgb(0x61afef).into()),
        _ => (ExplorerIcon::File, rgb(0xaeb4bf).into()),
    }
}

pub(crate) fn folder_icon(expanded: bool) -> (ExplorerIcon, Hsla) {
    let icon = if expanded {
        ExplorerIcon::FolderOpen
    } else {
        ExplorerIcon::Folder
    };
    (icon, rgb(0xc4a574).into())
}

pub(crate) fn chevron(expanded: bool) -> impl IntoElement {
    let icon = if expanded {
        ExplorerIcon::ChevronDown
    } else {
        ExplorerIcon::ChevronRight
    };
    icon.paint(10.0, rgb(0x8f96a3))
}

pub(crate) fn chevron_slot() -> impl IntoElement {
    div().w(px(10.0)).h(px(10.0)).flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_icons_from_file_names() {
        assert_eq!(file_icon(Path::new("src/main.rs")).0, ExplorerIcon::Rust);
        assert_eq!(file_icon(Path::new("Cargo.toml")).0, ExplorerIcon::Toml);
        assert_eq!(file_icon(Path::new("README.md")).0, ExplorerIcon::Markdown);
        assert_eq!(file_icon(Path::new("Cargo.lock")).0, ExplorerIcon::Lock);
        assert_eq!(file_icon(Path::new("notes.txt")).0, ExplorerIcon::File);
    }

    #[test]
    fn explorer_svgs_have_a_view_box() {
        for icon in [
            ExplorerIcon::ChevronDown,
            ExplorerIcon::ChevronRight,
            ExplorerIcon::File,
            ExplorerIcon::Folder,
            ExplorerIcon::FolderOpen,
            ExplorerIcon::Lock,
            ExplorerIcon::Markdown,
            ExplorerIcon::Rust,
            ExplorerIcon::Toml,
        ] {
            let svg = String::from_utf8(icon.bytes().to_vec()).unwrap();
            assert!(
                svg.contains("viewBox=\"0 0 16 16\""),
                "{icon:?} is missing a 16x16 viewBox"
            );
        }
    }
}
