#[path = "../build_support/frontend_assets.rs"]
mod frontend_assets;

use std::fs;

#[test]
fn embedded_asset_gate_rejects_a_replaced_bundle_with_unchanged_provenance() {
    let frontend = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend");
    frontend_assets::verify_frontend_assets(&frontend, &frontend.join("dist")).unwrap();

    let temporary = tempfile::tempdir().unwrap();
    let copied_dist = temporary.path().join("dist");
    fs::create_dir_all(copied_dist.join("assets")).unwrap();
    for relative in [
        "index.html",
        "assets/asset-manifest.json",
        "assets/app.js",
        "assets/app.css",
    ] {
        fs::copy(
            frontend.join("dist").join(relative),
            copied_dist.join(relative),
        )
        .unwrap();
    }
    fs::write(
        copied_dist.join("assets/app.js"),
        b"console.log('replaced older application bundle');\n",
    )
    .unwrap();

    let error = frontend_assets::verify_frontend_assets(&frontend, &copied_dist).unwrap_err();
    assert!(
        error.contains("app.js content hash does not match"),
        "{error}"
    );
}

#[test]
fn embedded_asset_gate_rejects_tampered_shell_body_with_unchanged_provenance() {
    let frontend = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend");
    let temporary = tempfile::tempdir().unwrap();
    let copied_dist = temporary.path().join("dist");
    fs::create_dir_all(copied_dist.join("assets")).unwrap();
    for relative in [
        "index.html",
        "assets/asset-manifest.json",
        "assets/app.js",
        "assets/app.css",
    ] {
        fs::copy(
            frontend.join("dist").join(relative),
            copied_dist.join(relative),
        )
        .unwrap();
    }
    let shell_path = copied_dist.join("index.html");
    let shell = fs::read_to_string(&shell_path).unwrap().replace(
        "<title>rust-jav Management</title>",
        "<title>tampered older shell</title>",
    );
    fs::write(shell_path, shell).unwrap();

    let error = frontend_assets::verify_frontend_assets(&frontend, &copied_dist).unwrap_err();
    assert!(
        error.contains("normalized index hash does not match"),
        "{error}"
    );
}
