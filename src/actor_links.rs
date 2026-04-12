use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::report::{ActionItem, ActionStatus, CommandReport, OutputMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorLinkPlan {
    pub nfo_path: PathBuf,
    pub movie_code: String,
    pub actors: Vec<String>,
    pub actions: Vec<ActionItem>,
    pub warnings: Vec<String>,
}

pub fn parse_actor_names(nfo_contents: &str) -> Vec<String> {
    let cleaned = nfo_contents.trim_start_matches('\u{feff}');
    let mut actors = Vec::new();
    let mut cursor = cleaned;

    while let Some(start) = cursor.find("<actor>") {
        let after_open = &cursor[start + "<actor>".len()..];
        let Some(end) = after_open.find("</actor>") else {
            break;
        };
        let actor_block = &after_open[..end];
        if let Some(name) = extract_tag(actor_block, "name") {
            let trimmed = name.trim();
            if !trimmed.is_empty() && !actors.iter().any(|existing| existing == trimmed) {
                actors.push(trimmed.to_string());
            }
        }
        cursor = &after_open[end + "</actor>".len()..];
    }

    actors
}

pub fn plan_actor_links(
    source_dir: &Path,
    actors_root: &Path,
    exclude_dirs: &[PathBuf],
) -> io::Result<Vec<ActorLinkPlan>> {
    // Build effective exclude list: user-provided + auto-detect actors_root inside source
    let mut excludes: Vec<PathBuf> = exclude_dirs.to_vec();
    if let Ok(canonical_source) = source_dir.canonicalize() {
        if let Ok(canonical_actors) = actors_root.canonicalize() {
            if canonical_actors.starts_with(&canonical_source)
                && !excludes.iter().any(|e| {
                    e.canonicalize().ok().as_ref() == Some(&canonical_actors)
                })
            {
                excludes.push(canonical_actors);
            }
        }
    }
    let nfo_files = collect_nfo_files(source_dir, &excludes)?;
    let mut plans = Vec::new();

    for nfo_path in nfo_files {
        let contents = fs::read_to_string(&nfo_path)?;
        let actors = parse_actor_names(&contents);
        let stem = nfo_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        // When NFO is named "movie.nfo" (nested structure), derive movie_code
        // from the directory name. If inside a "movie/" subdirectory
        // (e.g. IPZZ-408/movie/movie.nfo), use the grandparent name ("IPZZ-408").
        let movie_code = if stem.eq_ignore_ascii_case("movie") {
            let parent = nfo_path.parent();
            let parent_name = parent
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if parent_name.eq_ignore_ascii_case("movie") {
                parent
                    .and_then(|p| p.parent())
                    .and_then(|gp| gp.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or(stem)
                    .to_string()
            } else {
                parent_name.to_string()
            }
        } else {
            stem.to_string()
        };

        let mut warnings = Vec::new();
        if actors.is_empty() {
            warnings.push(format!("No actors found in {}", nfo_path.display()));
        }

        let mut related_files = collect_related_files(&nfo_path)?;
        // Ensure the NFO itself is always included (it may live in a subdirectory
        // like movie/ and not appear in the grandparent scan).
        if !related_files.iter().any(|(src, _)| src == &nfo_path) {
            let relative = nfo_path
                .file_name()
                .map(|n| PathBuf::from(n))
                .unwrap_or_else(|| nfo_path.clone());
            related_files.push((nfo_path.clone(), relative));
            related_files.sort_by(|a, b| a.1.cmp(&b.1));
        }
        let mut actions = Vec::new();

        // Movies without actors go into "未分类" folder
        let actor_list: Vec<String> = if actors.is_empty() {
            vec!["未分类".to_string()]
        } else {
            actors.clone()
        };

        for actor in &actor_list {
            let actor_dir_name = sanitize_path_component(actor);
            for (source, relative) in &related_files {
                let target = actors_root
                    .join(&actor_dir_name)
                    .join(&movie_code)
                    .join(relative);
                actions.push(ActionItem {
                    kind: "hard-link".to_string(),
                    source: Some(source.clone()),
                    target: Some(target),
                    status: ActionStatus::Planned,
                    reason: None,
                });
            }
        }

        plans.push(ActorLinkPlan {
            nfo_path,
            movie_code,
            actors,
            actions,
            warnings,
        });
    }

    Ok(plans)
}

pub fn execute_actor_links_command(
    source_dir: PathBuf,
    actors_root: PathBuf,
    exclude_dirs: Vec<PathBuf>,
    apply: bool,
) -> io::Result<CommandReport> {
    let mode = if apply {
        OutputMode::Apply
    } else {
        OutputMode::Preview
    };
    let mut report = CommandReport::new(
        "actor-links",
        mode,
        source_dir.clone(),
        vec!["actor-links".to_string()],
    );
    let plans = plan_actor_links(&source_dir, &actors_root, &exclude_dirs)?;

    for plan in plans {
        report.warnings.extend(plan.warnings);
        if !apply {
            report.actions.extend(plan.actions);
            continue;
        }

        for action in plan.actions {
            let mut applied_action = action.clone();
            applied_action.status = ActionStatus::Applied;
            let Some(source) = action.source.as_ref() else {
                applied_action.status = ActionStatus::Failed;
                applied_action.reason = Some("missing source file".to_string());
                report.actions.push(applied_action);
                continue;
            };
            let Some(target) = action.target.as_ref() else {
                applied_action.status = ActionStatus::Failed;
                applied_action.reason = Some("missing target path".to_string());
                report.actions.push(applied_action);
                continue;
            };

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }

            match fs::hard_link(source, target) {
                Ok(_) => report.actions.push(applied_action),
                Err(error) if target.exists() => {
                    applied_action.status = ActionStatus::Skipped;
                    applied_action.reason =
                        Some(format!("target already exists: {}", target.display()));
                    report.actions.push(applied_action);
                    report.warnings.push(format!(
                        "Skipped existing hard link target {}: {}",
                        target.display(),
                        error
                    ));
                }
                Err(error) => {
                    applied_action.status = ActionStatus::Failed;
                    applied_action.reason = Some(error.to_string());
                    report.errors.push(format!(
                        "Failed to hard-link {} -> {}: {}",
                        source.display(),
                        target.display(),
                        error
                    ));
                    report.actions.push(applied_action);
                }
            }
        }
    }

    report.finalize();
    Ok(report)
}

fn extract_tag(contents: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = contents.find(&open)?;
    let rest = &contents[start + open.len()..];
    let end = rest.find(&close)?;
    Some(rest[..end].to_string())
}

fn collect_nfo_files(source_dir: &Path, excludes: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_with_extension(source_dir, "nfo", excludes, &mut files)?;
    Ok(files)
}

fn should_exclude(path: &Path, excludes: &[PathBuf]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    if let Ok(canonical) = path.canonicalize() {
        excludes.iter().any(|e| {
            e.canonicalize().ok().as_ref() == Some(&canonical)
        })
    } else {
        false
    }
}

fn collect_files_with_extension(
    dir: &Path,
    extension: &str,
    excludes: &[PathBuf],
    results: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip excluded directories and hidden directories
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with('.') || should_exclude(&path, excludes) {
                continue;
            }
            collect_files_with_extension(&path, extension, excludes, results)?;
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
        {
            results.push(path);
        }
    }
    Ok(())
}

/// Collect related media files for a given NFO.
/// Returns (source_path, relative_path_from_scan_dir) pairs so subdirectory
/// structure (trickplay/, trailers/) is preserved in the target path.
fn collect_related_files(nfo_path: &Path) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    let parent = nfo_path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "NFO file has no parent"))?;
    let stem = nfo_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "NFO file has no stem"))?;

    let is_nested = stem.eq_ignore_ascii_case("movie");

    // When movie.nfo is inside a "movie/" subdirectory (e.g. IPZZ-408/movie/movie.nfo),
    // the actual media files are in the grandparent directory (IPZZ-408/).
    let scan_dir = if is_nested {
        let parent_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if parent_name.eq_ignore_ascii_case("movie") {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        }
    } else {
        parent
    };

    let mut files = Vec::new();
    collect_files_recursive(scan_dir, scan_dir, is_nested, stem, &mut files)?;
    files.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(files)
}

fn collect_files_recursive(
    dir: &Path,
    scan_root: &Path,
    is_nested: bool,
    stem: &str,
    results: &mut Vec<(PathBuf, PathBuf)>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Skip hidden/system files and trickplay/trailer index files
        if file_name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            // Recurse into subdirectories (trickplay/, trailers/, etc.)
            collect_files_recursive(&path, scan_root, is_nested, stem, results)?;
        } else {
            let relative = path.strip_prefix(scan_root).unwrap_or(&path).to_path_buf();
            // For flat structure: only include files matching the stem prefix
            // For nested structure: include all files
            if is_nested || file_name.starts_with(stem) {
                results.push((path, relative));
            }
        }
    }
    Ok(())
}

fn sanitize_path_component(component: &str) -> String {
    component
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_actor_names;

    #[test]
    fn parses_actor_names_from_nfo() {
        let fixture = include_str!("../REBD-615.nfo");
        let actors = parse_actor_names(fixture);
        assert_eq!(actors, vec!["miru".to_string()]);
    }

    #[test]
    fn parses_multiple_actors() {
        let fixture = r#"
            <movie>
              <actor><name>miru</name></actor>
              <actor><name>Alice</name></actor>
            </movie>
        "#;
        let actors = parse_actor_names(fixture);
        assert_eq!(actors, vec!["miru".to_string(), "Alice".to_string()]);
    }
}
