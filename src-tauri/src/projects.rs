//! Finds regenerable project folders (node_modules, Rust target, Pods, venvs…)
//! with a parallel directory walk.
//!
//! A folder only counts when the project marker sits next to it, e.g.
//! `node_modules` next to `package.json`, `target` next to `Cargo.toml`.
//! Hidden folders, ~/Library and app bundles are never entered, so global
//! installs and editor extensions are left alone.

use crate::{
    size::{path_size, Inodes},
    ticker,
};
use rayon::prelude::*;
use serde::Serialize;
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        Mutex,
    },
    time::UNIX_EPOCH,
};

#[derive(Serialize, Clone)]
pub struct Artifact {
    pub id: String,
    pub kind: &'static str,
    pub path: String,
    pub project: String,
    pub size: u64,
    /// Unix seconds the project manifest was last modified.
    pub modified: u64,
}

struct Ctx {
    seen: Inodes,
    visited: AtomicU64,
    out: Mutex<Vec<Artifact>>,
    root_is_home: bool,
}

const MAX_DEPTH: u32 = 18;
const PKG: &str = "package.json";
const GRADLE: &[&str] = &["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"];
const BUNDLE_EXT: &[&str] = &[".app", ".photoslibrary", ".musiclibrary", ".tvlibrary", ".bundle", ".framework", ".xcarchive"];
const HOME_SKIP: &[&str] = &["Library", "Applications", "Pictures", "Music", "Movies", "go"];

/// Returns (kind, file whose mtime says when the project was last touched).
fn classify(name: &str, files: &HashSet<OsString>, dir: &Path, path: &Path) -> Option<(&'static str, PathBuf)> {
    let has = |f: &str| files.contains(OsStr::new(f));
    let with = |kind, marker: &str| Some((kind, dir.join(marker)));
    match name {
        "node_modules" if has(PKG) => with("node", PKG),
        "target" if has("Cargo.toml") => with("rust", "Cargo.toml"),
        ".next" | ".nuxt" | ".svelte-kit" | ".turbo" | ".parcel-cache" | ".angular" | ".expo" if has(PKG) => with("web", PKG),
        "Pods" if has("Podfile") => with("pods", "Podfile"),
        "build" | ".dart_tool" if has("pubspec.yaml") => with("flutter", "pubspec.yaml"),
        "build" | ".gradle" => GRADLE.iter().find(|m| has(m)).and_then(|m| with("gradle", m)),
        ".venv" | "venv" | "env" | ".env" => {
            let cfg = path.join("pyvenv.cfg");
            cfg.is_file().then_some(("python", cfg))
        }
        _ => None,
    }
}

fn skip(name: &str, depth: u32, ctx: &Ctx) -> bool {
    name.starts_with('.')
        || name == "node_modules"
        || BUNDLE_EXT.iter().any(|e| name.ends_with(e))
        || (depth == 0 && ctx.root_is_home && HOME_SKIP.contains(&name))
}

fn mtime(p: &Path) -> u64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

fn walk(dir: &Path, depth: u32, ctx: &Ctx) {
    ctx.visited.fetch_add(1, Relaxed);
    let Ok(rd) = fs::read_dir(dir) else { return };

    let mut files = HashSet::new();
    let mut dirs = Vec::new();
    for e in rd.flatten() {
        match e.file_type() {
            Ok(ft) if ft.is_dir() => dirs.push(e),
            Ok(_) => {
                files.insert(e.file_name());
            }
            Err(_) => {}
        }
    }

    dirs.par_iter().for_each(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy();
        let path = e.path();
        if let Some((kind, marker)) = classify(&name, &files, dir, &path) {
            let a = Artifact {
                id: format!("p:{}", path.display()),
                kind,
                size: path_size(&path, &ctx.seen),
                modified: mtime(&marker),
                project: dir.to_string_lossy().into_owned(),
                path: path.to_string_lossy().into_owned(),
            };
            ctx.out.lock().unwrap().push(a);
        } else if depth < MAX_DEPTH && !skip(&name, depth, ctx) {
            walk(&path, depth + 1, ctx);
        }
    });
}

/// Scan `root`, calling `progress(folders_visited)` a few times per second.
pub fn scan(root: &Path, home: &Path, progress: impl Fn(u64) + Sync) -> Vec<Artifact> {
    let ctx = Ctx {
        seen: Inodes::default(),
        visited: AtomicU64::new(0),
        out: Mutex::new(Vec::new()),
        root_is_home: root == home,
    };
    ticker::every(150, || progress(ctx.visited.load(Relaxed)), || walk(root, 0, &ctx));

    let mut out = ctx.out.into_inner().unwrap();
    out.retain(|a| a.size > 0);
    out.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_real_project_folders() {
        let root = std::env::temp_dir().join(format!("sweep-proj-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mk = |p: &str| fs::create_dir_all(root.join(p)).unwrap();
        let touch = |p: &str| fs::write(root.join(p), b"x").unwrap();

        mk("web/node_modules/react");
        touch("web/package.json");
        touch("web/node_modules/react/index.js");
        mk("lib/node_modules/x"); // no package.json next to it (global-style install)
        touch("lib/node_modules/x/a.js");
        mk("crate/target/debug");
        touch("crate/Cargo.toml");
        touch("crate/target/debug/bin");
        mk(".hidden/app/node_modules/y"); // hidden parents are skipped
        touch(".hidden/app/package.json");
        touch(".hidden/app/node_modules/y/a.js");
        mk("py/.venv/lib");
        touch("py/.venv/pyvenv.cfg");

        let found = scan(&root, Path::new("/nonexistent"), |_| {});
        fs::remove_dir_all(&root).unwrap();
        let mut kinds: Vec<_> = found.iter().map(|a| a.kind).collect();
        kinds.sort();
        assert_eq!(kinds, ["node", "python", "rust"]);
    }

    /// Read-only benchmark against the real home folder:
    /// cargo test --release -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_home() {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let t = std::time::Instant::now();
        let found = scan(&home, &home, |_| {});
        let total: u64 = found.iter().map(|a| a.size).sum();
        println!("{} folders, {:.2} GB, {:?}", found.len(), total as f64 / 1e9, t.elapsed());
    }
}
