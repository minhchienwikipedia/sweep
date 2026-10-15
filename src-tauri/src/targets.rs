//! Well-known cache locations, for everyday users and developers.

use crate::size::{path_size, Inodes};
use rayon::prelude::*;
use serde::Serialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
enum Mode {
    /// Delete the listed path itself (it is recreated on demand).
    Whole,
    /// Keep the folder, delete everything inside it.
    Contents,
    /// Delete top-level files with one of these extensions.
    Ext(&'static [&'static str]),
}

pub struct Target {
    pub id: &'static str,
    group: &'static str,
    name: &'static str,
    desc: &'static str,
    default_on: bool,
    mode: Mode,
    paths: Vec<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    id: &'static str,
    group: &'static str,
    name: &'static str,
    desc: &'static str,
    default_on: bool,
}

#[derive(Serialize, Clone)]
pub struct Summary {
    pub id: &'static str,
    pub size: u64,
    pub count: usize,
    pub found: bool,
    pub error: Option<&'static str>,
}

/// Sub-folders of ~/Library/Caches that have their own developer entry.
const OWN_ENTRY: &[&str] = &["Yarn", "CocoaPods", "Homebrew", "pip", "go-build", "com.minhchien.sweep", "sweep"];

pub fn all(h: &Path) -> Vec<Target> {
    let t = |id, group, name, desc, default_on, mode, rel: &[&str]| Target {
        id,
        group,
        name,
        desc,
        default_on,
        mode,
        paths: rel.iter().map(|r| h.join(r)).collect(),
    };
    use Mode::*;
    vec![
        // Everyday
        t("caches", "system", "App caches", "Temporary files apps rebuild automatically", true, Contents, &["Library/Caches"]),
        t("logs", "system", "Logs", "Diagnostic logs and crash reports", true, Contents, &["Library/Logs"]),
        t("installers", "system", "Installers in Downloads", "Leftover .dmg, .pkg and .xip files", false, Ext(&["dmg", "pkg", "mpkg", "xip", "iso"]), &["Downloads"]),
        t("trash", "system", "Trash", "Files you've already deleted", false, Contents, &[".Trash"]),
        // Developer
        t("npm", "dev", "npm cache", "~/.npm download cache and npx installs", true, Whole, &[".npm/_cacache", ".npm/_npx", ".npm/_logs"]),
        t("yarn", "dev", "Yarn cache", "Classic and Berry package caches", true, Whole, &["Library/Caches/Yarn", ".yarn/berry/cache"]),
        t("pnpm", "dev", "pnpm store", "Content-addressable package store", true, Whole, &["Library/pnpm/store", ".local/share/pnpm/store"]),
        t("bun", "dev", "Bun cache", "Bun install cache", true, Whole, &[".bun/install/cache"]),
        t("derived", "dev", "Xcode DerivedData", "Build products and indexes", true, Contents, &["Library/Developer/Xcode/DerivedData"]),
        t("devicesupport", "dev", "Xcode device support", "Symbols copied from connected devices", true, Contents, &[
            "Library/Developer/Xcode/iOS DeviceSupport",
            "Library/Developer/Xcode/watchOS DeviceSupport",
            "Library/Developer/Xcode/tvOS DeviceSupport",
            "Library/Developer/Xcode/visionOS DeviceSupport",
        ]),
        t("simcache", "dev", "Simulator caches", "CoreSimulator dyld and runtime caches", true, Contents, &["Library/Developer/CoreSimulator/Caches"]),
        t("cocoapods", "dev", "CocoaPods cache", "Downloaded pod specs and sources", true, Whole, &["Library/Caches/CocoaPods"]),
        t("gradle", "dev", "Gradle cache", "Dependencies and build caches", true, Whole, &[".gradle/caches"]),
        t("cargo", "dev", "Cargo registry", "Downloaded crates and git checkouts", true, Whole, &[".cargo/registry/cache", ".cargo/registry/src", ".cargo/git/checkouts"]),
        t("pip", "dev", "pip cache", "Python wheels and downloads", true, Whole, &["Library/Caches/pip"]),
        t("go", "dev", "Go caches", "Build cache and module cache", true, Whole, &["Library/Caches/go-build", "go/pkg/mod"]),
        t("homebrew", "dev", "Homebrew cache", "Downloaded bottles and source archives", true, Whole, &["Library/Caches/Homebrew"]),
    ]
}

impl Target {
    pub fn meta(&self) -> Meta {
        Meta { id: self.id, group: self.group, name: self.name, desc: self.desc, default_on: self.default_on }
    }

    /// Resolve the concrete paths this target would delete, and their size.
    pub fn scan(&self, seen: &Inodes) -> (Summary, Vec<PathBuf>) {
        let mut items = Vec::new();
        let mut found = false;
        let mut error = None;

        for root in &self.paths {
            if let Mode::Whole = self.mode {
                if fs::symlink_metadata(root).is_ok() {
                    found = true;
                    items.push(root.clone());
                }
                continue;
            }
            match fs::read_dir(root) {
                Ok(rd) => {
                    found = true;
                    for e in rd.flatten() {
                        let name = e.file_name();
                        let name = name.to_string_lossy();
                        let keep = match self.mode {
                            Mode::Ext(exts) => Path::new(&*name)
                                .extension()
                                .map(|x| x.to_string_lossy().to_ascii_lowercase())
                                .is_some_and(|x| exts.contains(&x.as_str())),
                            _ => !(self.id == "caches" && OWN_ENTRY.contains(&&*name)),
                        };
                        if keep {
                            items.push(e.path());
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                    found = true;
                    error = Some("needs-access");
                }
                Err(_) => {}
            }
        }

        let size = items.par_iter().map(|p| path_size(p, seen)).sum();
        let summary = Summary { id: self.id, size, count: items.len(), found, error };
        (summary, items)
    }
}
