use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::types::{ManifestEntry, MigrationScope, ScopeManifest};

pub fn scan_scope(
    root_dir: &Path,
    scope: MigrationScope,
    allow_missing: bool,
) -> io::Result<ScopeManifest> {
    let root_metadata = match fs::symlink_metadata(root_dir) {
        Ok(metadata) => metadata,
        Err(error) if allow_missing && error.kind() == io::ErrorKind::NotFound => {
            return Ok(ScopeManifest {
                scope,
                root_dir: root_dir.to_path_buf(),
                entries: Vec::new(),
            });
        }
        Err(error) => return Err(error),
    };
    if root_metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("symlink roots are not allowed: {}", root_dir.display()),
        ));
    }

    let mut files = Vec::new();
    collect_files(root_dir, root_dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let entries = files
        .into_iter()
        .enumerate()
        .map(|(index, (relative_path, size, file_identity, link_type))| {
            let prefix = match scope {
                MigrationScope::Source => "src",
                MigrationScope::ActorsRoot => "actor",
            };
            let mut entry = ManifestEntry::from_scanned(
                format!("{prefix}-{index:06}"),
                scope,
                relative_path,
                size,
            );
            entry.file_identity = file_identity;
            entry.link_type = link_type.to_string();
            entry
        })
        .collect();

    Ok(ScopeManifest {
        scope,
        root_dir: root_dir.to_path_buf(),
        entries,
    })
}

fn collect_files(
    root: &Path,
    current: &Path,
    out: &mut Vec<(String, u64, Option<String>, &'static str)>,
) -> io::Result<()> {
    let mut paths = fs::read_dir(current)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<PathBuf>>();
    paths.sort();

    for path in paths {
        let metadata = fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push((
            relative,
            metadata.len(),
            file_identity(&metadata),
            link_type(&metadata),
        ));
    }

    Ok(())
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;

    Some(format!("{}:{}", metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn file_identity(_: &fs::Metadata) -> Option<String> {
    None
}

#[cfg(unix)]
fn link_type(metadata: &fs::Metadata) -> &'static str {
    use std::os::unix::fs::MetadataExt;

    if metadata.nlink() > 1 {
        "hardlink"
    } else {
        "none"
    }
}

#[cfg(not(unix))]
fn link_type(_: &fs::Metadata) -> &'static str {
    "none"
}
