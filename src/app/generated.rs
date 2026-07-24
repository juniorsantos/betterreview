//! Heuristic for detecting generated/vendored files so the UI can de-emphasize
//! them (dimmed marker in the files panel, skipped by unreviewed navigation).

const GENERATED_BASENAMES: &[&str] = &[
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
];

const GENERATED_SUFFIXES: &[&str] = &[".min.js", ".min.css", ".map", ".lock"];

const GENERATED_PATH_COMPONENTS: &[&str] = &[
    "vendor",
    "node_modules",
    "dist",
    "generated",
    "__generated__",
];

/// Returns true when `path` looks like a lockfile, minified/mapped build
/// artifact, or vendored/generated tree entry that shouldn't compete for
/// review attention.
pub fn is_generated(path: &str) -> bool {
    let basename = path.rsplit('/').next().unwrap_or(path);
    if GENERATED_BASENAMES.contains(&basename) {
        return true;
    }
    if GENERATED_SUFFIXES
        .iter()
        .any(|suffix| basename.ends_with(suffix))
    {
        return true;
    }
    path.split('/')
        .any(|component| GENERATED_PATH_COMPONENTS.contains(&component))
}
