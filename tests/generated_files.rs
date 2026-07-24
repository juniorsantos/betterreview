use betterreview::app::is_generated;

#[test]
fn is_generated_matches_known_lockfile_basenames() {
    let cases = [
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "Cargo.lock",
        "composer.lock",
        "Gemfile.lock",
        "go.sum",
        "poetry.lock",
        "uv.lock",
        "bun.lockb",
        "sub/dir/package-lock.json",
        "sub/dir/Cargo.lock",
    ];
    for path in cases {
        assert!(is_generated(path), "expected {path} to be generated");
    }
}

#[test]
fn is_generated_matches_minified_and_map_and_lock_suffixes() {
    let cases = [
        "assets/app.min.js",
        "assets/app.min.css",
        "assets/app.js.map",
        "custom.lock",
        "src/vendor.lock",
    ];
    for path in cases {
        assert!(is_generated(path), "expected {path} to be generated");
    }
}

#[test]
fn is_generated_matches_generated_path_components() {
    let cases = [
        "vendor/lib.rs",
        "src/vendor/lib.rs",
        "node_modules/pkg/index.js",
        "dist/bundle.js",
        "generated/code.go",
        "__generated__/thing.ts",
    ];
    for path in cases {
        assert!(is_generated(path), "expected {path} to be generated");
    }
}

#[test]
fn is_generated_rejects_ordinary_source_files() {
    let cases = [
        "src/main.rs",
        "README.md",
        "package.json",
        "lockfile.txt",
        "vendors/foo.rs",
        "unlock.rs",
        "app.min.jsx",
    ];
    for path in cases {
        assert!(!is_generated(path), "expected {path} to not be generated");
    }
}
