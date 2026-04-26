use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::Utc;

use super::types::{
    ManifestEntry, ScopeCountSummary, ScopeDiff, ScopeExtensionCounts, ScopeManifest,
    VerificationReport,
};

pub fn write_report(report: &VerificationReport) -> io::Result<()> {
    if let Some(parent) = report.report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_report_file(&report.report_path, &report_to_json(report))
}

pub fn default_report_path(command: &str) -> PathBuf {
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%.6fZ");
    Path::new(".omx")
        .join("reports")
        .join("migrations")
        .join(format!("{stamp}-{command}.json"))
}

pub fn report_to_json(report: &VerificationReport) -> String {
    format!(
        concat!(
            "{{",
            "\"version\":{},",
            "\"command\":{},",
            "\"mode\":{},",
            "\"verification_status\":{},",
            "\"approval_status\":{},",
            "\"exit_code\":{},",
            "\"report_path\":{},",
            "\"failed_actions\":{},",
            "\"errors\":{},",
            "\"warnings\":{},",
            "\"scope_counts\":{},",
            "\"scope_extension_counts\":{},",
            "\"expected_stats\":{{",
            "\"expected_new_links\":{},",
            "\"expected_existing_links\":{},",
            "\"plan_conflicts\":{}",
            "}},",
            "\"diffs\":{},",
            "\"before\":{},",
            "\"expected\":{},",
            "\"after\":{}",
            "}}"
        ),
        report.version,
        json_string(&report.command),
        json_string(&report.mode),
        json_string(report.verification_status.as_str()),
        json_string(report.approval_status.as_str()),
        report.exit_code,
        json_string(report.report_path.display().to_string()),
        report.failed_actions,
        json_array(report.errors.iter().map(json_string)),
        json_array(report.warnings.iter().map(json_string)),
        json_array(report.scope_counts.iter().map(scope_count_json)),
        json_array(
            report
                .scope_extension_counts
                .iter()
                .map(scope_extension_counts_json)
        ),
        report.expected_stats.expected_new_links,
        report.expected_stats.expected_existing_links,
        json_array(report.expected_stats.plan_conflicts.iter().map(json_string)),
        json_array(report.diffs.iter().map(scope_diff_json)),
        json_array(report.before.iter().map(scope_manifest_json)),
        json_array(report.expected.iter().map(scope_manifest_json)),
        json_array(report.after.iter().map(scope_manifest_json)),
    )
}

fn scope_count_json(summary: &ScopeCountSummary) -> String {
    format!(
        "{{\"scope\":{},\"before_count\":{},\"expected_count\":{},\"after_count\":{}}}",
        json_string(summary.scope.as_str()),
        summary.before_count,
        summary.expected_count,
        summary.after_count
    )
}

fn scope_extension_counts_json(summary: &ScopeExtensionCounts) -> String {
    format!(
        "{{\"scope\":{},\"before\":{},\"expected\":{},\"after\":{}}}",
        json_string(summary.scope.as_str()),
        extension_counts_json(&summary.before),
        extension_counts_json(&summary.expected),
        extension_counts_json(&summary.after)
    )
}

fn extension_counts_json(values: &[(String, usize)]) -> String {
    let fields = values
        .iter()
        .map(|(extension, count)| format!("{}:{}", json_string(extension), count))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn scope_diff_json(diff: &ScopeDiff) -> String {
    format!(
        "{{\"scope\":{},\"missing_files\":{},\"unexpected_files\":{},\"mismatched_files\":{}}}",
        json_string(diff.scope.as_str()),
        json_array(diff.missing_files.iter().map(json_string)),
        json_array(diff.unexpected_files.iter().map(json_string)),
        json_array(diff.mismatched_files.iter().map(|entry| {
            format!(
                "{{\"relative_path\":{},\"mismatch_fields\":{}}}",
                json_string(&entry.relative_path),
                json_array(entry.mismatch_fields.iter().map(json_string))
            )
        }))
    )
}

fn scope_manifest_json(scope: &ScopeManifest) -> String {
    format!(
        "{{\"scope\":{},\"entries\":{}}}",
        json_string(scope.scope.as_str()),
        json_array(scope.entries.iter().map(manifest_entry_json))
    )
}

fn manifest_entry_json(entry: &ManifestEntry) -> String {
    format!(
        concat!(
            "{{",
            "\"entry_id\":{},",
            "\"scope\":{},",
            "\"relative_path\":{},",
            "\"file_name\":{},",
            "\"extension\":{},",
            "\"size\":{},",
            "\"origin_before_entry_id\":{},",
            "\"origin_before_relative_path\":{},",
            "\"action_ids\":{},",
            "\"link_info\":{{\"link_type\":{},\"source_entry_id\":{}}}",
            "}}"
        ),
        json_string(&entry.entry_id),
        json_string(entry.scope.as_str()),
        json_string(&entry.relative_path),
        json_string(&entry.file_name),
        json_string(&entry.extension),
        entry.size,
        json_optional_string(entry.origin_before_entry_id.as_deref()),
        json_optional_string(entry.origin_before_relative_path.as_deref()),
        json_array(entry.action_ids.iter().map(json_string)),
        json_string(&entry.link_type),
        json_optional_string(entry.link_source_entry_id.as_deref()),
    )
}

fn json_array(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
}

fn json_optional_string(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn json_string(value: impl AsRef<str>) -> String {
    let escaped = value
        .as_ref()
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            other => vec![other],
        })
        .collect::<String>();
    format!("\"{escaped}\"")
}

#[cfg(unix)]
fn write_report_file(path: &Path, contents: &str) -> io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_report_file(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}
