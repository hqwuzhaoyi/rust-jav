#[path = "build_support/frontend_assets.rs"]
mod frontend_assets;

use std::path::PathBuf;

fn main() {
    let frontend_root = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR"),
    )
    .join("frontend");
    for (_, path) in frontend_assets::frontend_production_inputs(&frontend_root)
        .unwrap_or_else(|error| panic!("cannot enumerate frontend production inputs: {error}"))
    {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    for relative in [
        "index.html",
        "assets/asset-manifest.json",
        "assets/app.js",
        "assets/app.css",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            frontend_root.join("dist").join(relative).display()
        );
    }
    frontend_assets::verify_frontend_assets(&frontend_root, &frontend_root.join("dist"))
        .unwrap_or_else(|error| {
            panic!("{error}; run `cd frontend && npm run build`");
        });
}
