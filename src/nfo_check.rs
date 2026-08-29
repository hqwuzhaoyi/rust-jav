use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::report::{CommandReport, OutputMode};

/// A directory that is missing NFO metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingNfo {
    /// Relative path from the source root (e.g. "UNCENSORED/MIDA-107-U").
    pub rel_path: String,
    /// The likely movie code derived from the directory name.
    pub movie_code: Option<String>,
    /// Files present in the directory (for diagnostics).
    pub files: Vec<String>,
}

/// Check a source directory for movie subdirectories that lack `.nfo` files.
///
/// `max_depth` controls how deep to recurse (1 = immediate children only,
/// 2 = children/grandchildren, etc.).  Directories whose name starts with `.`
/// or matches any entry in `skip` are ignored.
pub fn check_missing_nfos(
    source: &Path,
    max_depth: usize,
    skip: &[String],
) -> io::Result<Vec<MissingNfo>> {
    let mut results = Vec::new();
    scan_dir(source, source, 0, max_depth, skip, &mut results)?;
    results.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(results)
}

fn scan_dir(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    skip: &[String],
    results: &mut Vec<MissingNfo>,
) -> io::Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    let entries: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();

    // Determine if *this* directory is a "movie directory" (has media files but no NFO).
    // We only flag directories that contain at least one media file.
    let media_exts = ["mp4", "mkv", "avi", "wmv", "flv", "ts", "m4v"];
    let has_media = entries.iter().any(|e| {
        e.path().is_file()
            && e.path()
                .extension()
                .map(|ext| {
                    media_exts
                        .iter()
                        .any(|m| m == &ext.to_string_lossy().to_lowercase())
                })
                .unwrap_or(false)
    });
    let has_nfo = entries.iter().any(|e| {
        e.path()
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("nfo"))
            .unwrap_or(false)
    });

    if has_media && !has_nfo && depth > 0 {
        let rel = dir
            .strip_prefix(root)
            .unwrap_or(dir)
            .to_string_lossy()
            .to_string();
        let movie_code = dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        let files: Vec<String> = entries
            .iter()
            .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
            .collect();
        results.push(MissingNfo {
            rel_path: rel,
            movie_code,
            files,
        });
    }

    // Recurse into subdirectories
    for entry in &entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || skip.iter().any(|s| s == &name) {
            continue;
        }
        scan_dir(root, &path, depth + 1, max_depth, skip, results)?;
    }

    Ok(())
}

pub(crate) fn run_nfo_check_command(
    dir: PathBuf,
    max_depth: usize,
    skip: Vec<String>,
) -> CommandReport {
    let mut report = CommandReport::new("nfo-check", OutputMode::Preview, dir.clone(), vec![]);

    match check_missing_nfos(&dir, max_depth, &skip) {
        Ok(missing) => {
            for m in &missing {
                report.actions.push(crate::report::ActionItem {
                    kind: "missing-nfo".to_string(),
                    source: Some(dir.join(&m.rel_path)),
                    target: None,
                    status: crate::report::ActionStatus::Planned,
                    reason: m.movie_code.clone(),
                });
            }
            let total = missing.len();
            report.summary.planned_actions = total;
            if total == 0 {
                report
                    .warnings
                    .push("All directories have NFO files.".to_string());
            } else {
                report
                    .warnings
                    .push(format!("{total} directories missing NFO files."));
            }
        }
        Err(e) => {
            report.errors.push(format!("Failed to scan directory: {e}"));
            report.summary.error_count = 1;
        }
    }

    report
}

pub fn execute_nfo_check_command(
    dir: PathBuf,
    max_depth: usize,
    skip: Vec<String>,
) -> CommandReport {
    crate::application::ApplicationServices::new().nfo().check(
        crate::application::NfoCheckRequest {
            source_dir: dir,
            max_depth,
            skip,
        },
    )
}

/// Return just the movie codes for directories missing NFOs (one per line).
pub fn missing_codes_only(dir: &Path, max_depth: usize, skip: &[String]) -> io::Result<String> {
    let missing = check_missing_nfos(dir, max_depth, skip)?;
    let codes: Vec<&str> = missing
        .iter()
        .filter_map(|m| m.movie_code.as_deref())
        .collect();
    Ok(codes.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_missing_nfo() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        // movie_a has mp4 + nfo → ok
        let movie_a = base.join("movie_a");
        fs::create_dir(&movie_a).unwrap();
        fs::write(movie_a.join("video.mp4"), b"fake").unwrap();
        fs::write(movie_a.join("movie.nfo"), b"<movie/>").unwrap();

        // movie_b has mp4 but no nfo → should be flagged
        let movie_b = base.join("movie_b");
        fs::create_dir(&movie_b).unwrap();
        fs::write(movie_b.join("video.mp4"), b"fake").unwrap();

        // movie_c has no media → should not be flagged
        let movie_c = base.join("movie_c");
        fs::create_dir(&movie_c).unwrap();
        fs::write(movie_c.join("info.txt"), b"nothing").unwrap();

        let missing = check_missing_nfos(base, 1, &[]).unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].rel_path, "movie_b");
        assert_eq!(missing[0].movie_code.as_deref(), Some("movie_b"));
    }

    #[test]
    fn skips_hidden_and_skipped_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        let hidden = base.join(".hidden");
        fs::create_dir(&hidden).unwrap();
        fs::write(hidden.join("video.mp4"), b"fake").unwrap();

        let skip_me = base.join("trailers");
        fs::create_dir(&skip_me).unwrap();
        fs::write(skip_me.join("video.mp4"), b"fake").unwrap();

        let missing = check_missing_nfos(base, 1, &["trailers".into()]).unwrap();
        assert!(missing.is_empty());
    }
}
