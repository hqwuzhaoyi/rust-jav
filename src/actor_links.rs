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

pub fn plan_actor_links(source_dir: &Path, actors_root: &Path) -> io::Result<Vec<ActorLinkPlan>> {
    let nfo_files = collect_nfo_files(source_dir)?;
    let mut plans = Vec::new();

    for nfo_path in nfo_files {
        let contents = fs::read_to_string(&nfo_path)?;
        let actors = parse_actor_names(&contents);
        let movie_code = nfo_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut warnings = Vec::new();
        if actors.is_empty() {
            warnings.push(format!("No actors found in {}", nfo_path.display()));
        }

        let related_files = collect_related_files(&nfo_path)?;
        let mut actions = Vec::new();

        for actor in &actors {
            let actor_dir_name = sanitize_path_component(actor);
            for file in &related_files {
                let target = actors_root
                    .join(&actor_dir_name)
                    .join(&movie_code)
                    .join(file.file_name().unwrap());
                actions.push(ActionItem {
                    kind: "hard-link".to_string(),
                    source: Some(file.clone()),
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
    let plans = plan_actor_links(&source_dir, &actors_root)?;

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

fn collect_nfo_files(source_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_with_extension(source_dir, "nfo", &mut files)?;
    Ok(files)
}

fn collect_files_with_extension(
    dir: &Path,
    extension: &str,
    results: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, results)?;
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

fn collect_related_files(nfo_path: &Path) -> io::Result<Vec<PathBuf>> {
    let parent = nfo_path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "NFO file has no parent"))?;
    let movie_code = nfo_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "NFO file has no stem"))?;

    let mut files = Vec::new();
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with(movie_code) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
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
