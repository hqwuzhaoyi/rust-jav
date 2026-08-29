use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActorFolder {
    pub name: String,
    pub path: PathBuf,
    pub movie_count: usize,
    pub hard_link_count: usize,
    pub logical_size: u64,
    pub reclaimable_space: u64,
    pub poster_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalOutcome {
    pub kind: String,
    pub path: PathBuf,
    pub status: String,
    pub message: Option<String>,
}

pub fn browse_actor_folders(actors_root: &Path) -> io::Result<Vec<ActorFolder>> {
    if !actors_root.exists() {
        return Ok(Vec::new());
    }
    let mut folders = Vec::new();
    for entry in fs::read_dir(actors_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let mut files = Vec::new();
        collect_regular_files(&path, &mut files)?;
        let mut inode_occurrences = HashMap::new();
        for (_, metadata) in &files {
            #[cfg(unix)]
            {
                *inode_occurrences
                    .entry((metadata.dev(), metadata.ino()))
                    .or_insert(0u64) += 1;
            }
        }
        let logical_size = files.iter().map(|(_, metadata)| metadata.len()).sum();
        #[cfg(unix)]
        let mut reclaimable_inodes = HashSet::new();
        let reclaimable_space = files
            .iter()
            .filter(|(_, metadata)| {
                metadata.nlink() <= inode_occurrences[&(metadata.dev(), metadata.ino())]
                    && reclaimable_inodes.insert((metadata.dev(), metadata.ino()))
            })
            .map(|(_, metadata)| metadata.len())
            .sum();
        #[cfg(not(unix))]
        let reclaimable_space = 0;
        let poster_path = files
            .iter()
            .map(|(path, _)| path)
            .find(|path| is_poster(path))
            .cloned();
        let movie_count = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .filter(|item| item.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .count();
        folders.push(ActorFolder {
            name: entry.file_name().to_string_lossy().into_owned(),
            path,
            movie_count,
            #[cfg(unix)]
            hard_link_count: files
                .iter()
                .filter(|(_, metadata)| metadata.nlink() > 1)
                .count(),
            #[cfg(not(unix))]
            hard_link_count: files.len(),
            logical_size,
            reclaimable_space,
            poster_path,
        });
    }
    folders.sort_by_key(|folder| folder.name.to_lowercase());
    Ok(folders)
}

pub fn remove_actor_folder(
    actors_root: &Path,
    actor_name: &str,
) -> io::Result<Vec<RemovalOutcome>> {
    if actor_name.is_empty()
        || Path::new(actor_name).components().count() != 1
        || !matches!(
            Path::new(actor_name).components().next(),
            Some(Component::Normal(_))
        )
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Actor Folder name must be one safe path component",
        ));
    }
    let root = fs::canonicalize(actors_root)?;
    let folder = root.join(actor_name);
    let metadata = fs::symlink_metadata(&folder)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Actor Folder must be a real directory inside Actor View root",
        ));
    }
    let canonical = fs::canonicalize(&folder)?;
    if canonical.parent() != Some(root.as_path()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Actor Folder escapes Actor View root",
        ));
    }
    let mut files = Vec::new();
    collect_regular_files(&canonical, &mut files)?;
    let mut outcomes = Vec::new();
    for (path, metadata) in files {
        fs::remove_file(&path)?;
        outcomes.push(RemovalOutcome {
            #[cfg(unix)]
            kind: if metadata.nlink() > 1 {
                "unlink-derived-hard-link"
            } else {
                "unlink-derived-path"
            }
            .into(),
            #[cfg(not(unix))]
            kind: "unlink-derived-path".into(),
            path,
            status: "applied".into(),
            message: Some("derived Actor View path removed; source Media Asset untouched".into()),
        });
    }
    remove_empty_tree(&canonical, &mut outcomes)?;
    Ok(outcomes)
}

#[cfg(unix)]
pub fn validate_hard_link_compatibility(media_root: &Path, actors_root: &Path) -> io::Result<()> {
    let media = fs::metadata(media_root)?;
    let actors = fs::metadata(actors_root)?;
    validate_device_ids(media.dev(), actors.dev())
}

#[cfg(not(unix))]
pub fn validate_hard_link_compatibility(_: &Path, _: &Path) -> io::Result<()> {
    Ok(())
}

fn validate_device_ids(media_device: u64, actor_device: u64) -> io::Result<()> {
    if media_device != actor_device {
        Err(io::Error::new(io::ErrorKind::CrossesDevices, "Media Root and Actor View root are on different filesystems or ZFS datasets; hard links require the same filesystem/dataset (copy and symlink fallback are disabled)"))
    } else {
        Ok(())
    }
}

fn collect_regular_files(dir: &Path, files: &mut Vec<(PathBuf, fs::Metadata)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_regular_files(&entry.path(), files)?;
        } else if metadata.is_file() {
            files.push((entry.path(), metadata));
        }
    }
    Ok(())
}

fn remove_empty_tree(dir: &Path, outcomes: &mut Vec<RemovalOutcome>) -> io::Result<()> {
    let mut dirs = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    for child in dirs.drain(..) {
        remove_empty_tree(&child, outcomes)?;
    }
    fs::remove_dir(dir)?;
    outcomes.push(RemovalOutcome {
        kind: "remove-derived-directory".into(),
        path: dir.to_owned(),
        status: "applied".into(),
        message: None,
    });
    Ok(())
}

fn is_poster(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    (name.contains("poster")
        || name.contains("portrait")
        || name == "folder.jpg"
        || name == "folder.jpeg"
        || name == "folder.png"
        || name == "folder.webp")
        && ["jpg", "jpeg", "png", "webp"].iter().any(|ext| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
        })
}

#[cfg(test)]
mod tests {
    use super::validate_device_ids;
    #[test]
    fn different_filesystem_or_zfs_dataset_is_an_explicit_failure() {
        let error = validate_device_ids(41, 42).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::CrossesDevices);
        assert!(error
            .to_string()
            .contains("different filesystems or ZFS datasets"));
        assert!(error.to_string().contains("fallback are disabled"));
    }
}
