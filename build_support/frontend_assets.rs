use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const FIXED_INPUTS: &[&str] = &[
    "index.html",
    "package-lock.json",
    "package.json",
    "source-digest.ts",
    "tsconfig.json",
    "vite.config.ts",
];

fn source_inputs(root: &Path, directory: &Path, inputs: &mut Vec<String>) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to enumerate {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            source_inputs(root, &path, inputs)?;
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.contains(".test.") || name == "test-setup.ts" || name == "test-css.ts" {
            continue;
        }
        if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("css" | "ts" | "tsx")
        ) {
            inputs.push(
                path.strip_prefix(root)
                    .map_err(|_| {
                        format!(
                            "frontend source {} is outside {}",
                            path.display(),
                            root.display()
                        )
                    })?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

pub fn frontend_production_inputs(frontend_root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut inputs = FIXED_INPUTS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<Vec<_>>();
    source_inputs(frontend_root, &frontend_root.join("src"), &mut inputs)?;
    inputs.sort();
    Ok(inputs
        .into_iter()
        .map(|relative| {
            let path = frontend_root.join(&relative);
            (relative, path)
        })
        .collect())
}

fn expected_source_digest(frontend_root: &Path) -> Result<String, String> {
    let mut digest = Sha256::new();
    for (relative, path) in frontend_production_inputs(frontend_root)? {
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(fs::read(&path).map_err(|error| {
            format!(
                "failed to read frontend production input {}: {error}",
                path.display()
            )
        })?);
        digest.update([0]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

const PROVENANCE_META: &[&str] = &[
    "rust-jav-source-digest",
    "rust-jav-asset-manifest",
    "rust-jav-index-sha256",
    "rust-jav-app-js-sha256",
    "rust-jav-app-css-sha256",
];

fn meta_content<'a>(shell: &'a str, name: &str) -> Result<&'a str, String> {
    let prefix = format!("<meta name=\"{name}\" content=\"");
    if shell.matches(&prefix).count() != 1 {
        return Err(format!(
            "embedded frontend shell must contain exactly one {name} metadata entry"
        ));
    }
    shell
        .split_once(&prefix)
        .map(|(_, remainder)| remainder)
        .and_then(|remainder| remainder.split_once('\"').map(|(value, _)| value))
        .ok_or_else(|| format!("embedded frontend shell is missing {name} metadata"))
}

fn normalized_shell(shell: &str) -> Result<String, String> {
    let mut normalized = shell.to_owned();
    for name in PROVENANCE_META {
        let prefix = format!("<meta name=\"{name}\" content=\"");
        if normalized.matches(&prefix).count() != 1 {
            return Err(format!(
                "embedded frontend shell must contain exactly one {name} metadata entry"
            ));
        }
        let value_start = normalized
            .find(&prefix)
            .expect("checked provenance metadata must exist")
            + prefix.len();
        let value_end = normalized[value_start..]
            .find('\"')
            .map(|offset| value_start + offset)
            .ok_or_else(|| format!("embedded frontend shell has malformed {name} metadata"))?;
        normalized.replace_range(value_start..value_end, "");
    }
    Ok(normalized)
}

fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn verify_asset(
    manifest: &serde_json::Value,
    shell: &str,
    dist_root: &Path,
    key: &str,
    expected_path: &str,
    html_attribute: &str,
    hash_meta_name: &str,
) -> Result<(), String> {
    let record = &manifest["assets"][key];
    let path = record["path"]
        .as_str()
        .ok_or_else(|| format!("frontend asset manifest has no {key} path"))?;
    if path != expected_path {
        return Err(format!(
            "frontend asset manifest {key} path is {path}, expected {expected_path}"
        ));
    }
    let reference = format!("{html_attribute}=\"{path}\"");
    if shell.matches(&reference).count() != 1 {
        return Err(format!(
            "embedded frontend shell must reference manifest {key} path {path} exactly once"
        ));
    }
    let recorded_hash = record["sha256"]
        .as_str()
        .ok_or_else(|| format!("frontend asset manifest has no {key} sha256"))?;
    if meta_content(shell, hash_meta_name)? != recorded_hash {
        return Err(format!(
            "embedded frontend shell {key} hash does not match the manifest"
        ));
    }
    let relative = path
        .strip_prefix('/')
        .ok_or_else(|| format!("frontend asset path must be root-relative: {path}"))?;
    let bytes = fs::read(dist_root.join(relative))
        .map_err(|error| format!("failed to read frontend asset {path}: {error}"))?;
    if content_hash(&bytes) != recorded_hash {
        return Err(format!(
            "{} content hash does not match tracked provenance",
            Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
        ));
    }
    Ok(())
}

pub fn verify_frontend_assets(frontend_root: &Path, dist_root: &Path) -> Result<(), String> {
    let shell_path = dist_root.join("index.html");
    let shell = fs::read_to_string(&shell_path).map_err(|error| {
        format!(
            "failed to read embedded frontend shell {}: {error}",
            shell_path.display()
        )
    })?;
    let expected_source = expected_source_digest(frontend_root)?;
    if meta_content(&shell, "rust-jav-source-digest")? != expected_source {
        return Err(format!(
            "frontend/dist is stale relative to production source (expected digest {expected_source})"
        ));
    }
    let manifest_path = meta_content(&shell, "rust-jav-asset-manifest")?;
    if manifest_path != "/assets/asset-manifest.json" {
        return Err(format!(
            "embedded frontend shell references unexpected manifest {manifest_path}"
        ));
    }
    let manifest_bytes = fs::read(dist_root.join("assets/asset-manifest.json"))
        .map_err(|error| format!("failed to read tracked frontend asset manifest: {error}"))?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("tracked frontend asset manifest is invalid: {error}"))?;
    if manifest["version"] != 1 {
        return Err("tracked frontend asset manifest has an unsupported version".to_string());
    }
    if manifest["source_digest"].as_str() != Some(expected_source.as_str()) {
        return Err("tracked frontend asset manifest source digest is stale".to_string());
    }
    if manifest["index"]["path"].as_str() != Some("/index.html") {
        return Err("tracked frontend asset manifest has an unexpected index path".to_string());
    }
    let normalized_index_hash =
        manifest["index"]["normalized_sha256"]
            .as_str()
            .ok_or_else(|| {
                "tracked frontend asset manifest has no normalized index hash".to_string()
            })?;
    if meta_content(&shell, "rust-jav-index-sha256")? != normalized_index_hash {
        return Err("embedded frontend shell index hash does not match the manifest".to_string());
    }
    if content_hash(normalized_shell(&shell)?.as_bytes()) != normalized_index_hash {
        return Err("normalized index hash does not match tracked provenance".to_string());
    }
    verify_asset(
        &manifest,
        &shell,
        dist_root,
        "javascript",
        "/assets/app.js",
        "src",
        "rust-jav-app-js-sha256",
    )?;
    verify_asset(
        &manifest,
        &shell,
        dist_root,
        "stylesheet",
        "/assets/app.css",
        "href",
        "rust-jav-app-css-sha256",
    )?;
    Ok(())
}
