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
    /// Recursive regular-file paths in this derived Actor Folder.
    pub derived_file_count: usize,
    /// Compatibility alias for `derived_file_count`.
    pub hard_link_count: usize,
    /// Distinct regular-file inodes referenced by those paths.
    pub unique_inode_count: usize,
    pub logical_size: u64,
    pub reclaimable_space: u64,
    pub poster_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorFolderDetail {
    pub folder: ActorFolder,
    pub file_identities: Vec<FileIdentity>,
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
        #[cfg(unix)]
        let unique_inodes = files
            .iter()
            .map(|(_, metadata)| ((metadata.dev(), metadata.ino()), metadata))
            .collect::<HashMap<_, _>>();
        #[cfg(unix)]
        let logical_size = unique_inodes.values().map(|metadata| metadata.len()).sum();
        #[cfg(unix)]
        let reclaimable_space = unique_inodes
            .iter()
            .filter(|(inode, metadata)| metadata.nlink() == inode_occurrences[inode])
            .map(|(_, metadata)| metadata.len())
            .sum();
        #[cfg(unix)]
        let unique_inode_count = unique_inodes.len();
        #[cfg(not(unix))]
        let (logical_size, reclaimable_space, unique_inode_count) = (
            files.iter().map(|(_, metadata)| metadata.len()).sum(),
            0,
            files.len(),
        );
        let poster_path = files
            .iter()
            .map(|(path, _)| path)
            .find(|candidate| candidate.parent() == Some(path.as_path()) && is_poster(candidate))
            .cloned();
        let movie_count = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .filter(|item| item.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .count();
        folders.push(ActorFolder {
            name: entry.file_name().to_string_lossy().into_owned(),
            path,
            movie_count,
            derived_file_count: files.len(),
            #[cfg(unix)]
            hard_link_count: files.len(),
            #[cfg(not(unix))]
            hard_link_count: files.len(),
            unique_inode_count,
            logical_size,
            reclaimable_space,
            poster_path,
        });
    }
    folders.sort_by_key(|folder| folder.name.to_lowercase());
    Ok(folders)
}

pub fn actor_folder_detail(
    actors_root: &Path,
    actor_name: &str,
) -> io::Result<Option<ActorFolderDetail>> {
    let Some(path) = resolve_actor_folder(actors_root, actor_name)? else {
        return Ok(None);
    };
    let Some(folder) = browse_actor_folders(actors_root)?
        .into_iter()
        .find(|folder| folder.name == actor_name)
    else {
        return Ok(None);
    };
    let mut files = Vec::new();
    collect_regular_files(&path, &mut files)?;
    #[cfg(unix)]
    let file_identities = files
        .into_iter()
        .map(|(_, metadata)| FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    #[cfg(not(unix))]
    let file_identities = Vec::new();
    Ok(Some(ActorFolderDetail {
        folder,
        file_identities,
    }))
}

pub fn remove_actor_folder(
    actors_root: &Path,
    actor_name: &str,
) -> io::Result<Vec<RemovalOutcome>> {
    let canonical = resolve_actor_folder(actors_root, actor_name)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Actor Folder does not exist"))?;
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

fn resolve_actor_folder(actors_root: &Path, actor_name: &str) -> io::Result<Option<PathBuf>> {
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
    let metadata = match fs::symlink_metadata(&folder) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
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
    Ok(Some(canonical))
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
