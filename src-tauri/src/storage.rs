//! "What's using my disk": sorts every top-level folder in the home folder (and
//! ~/Library one level down) into exactly one category, so totals never overlap.
//! Whatever is left of the used space is macOS, other users and unreadable data.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Fixed order; the UI colors categories by this order.
pub const ORDER: &[&str] = &["apps", "developer", "documents", "media", "icloud", "caches", "appdata", "trash"];

fn home_category(name: &str) -> &'static str {
    match name {
        ".Trash" => "trash",
        "Applications" => "apps",
        "Pictures" | "Movies" | "Music" => "media",
        // Dot-folders are almost all tool installs and caches (.npm, .cargo, .nvm, .gradle…)
        n if n.starts_with('.') => "developer",
        _ => "documents",
    }
}

fn library_category(name: &str) -> &'static str {
    match name {
        "Developer" => "developer",
        "Caches" | "Logs" => "caches",
        "Mobile Documents" => "icloud",
        _ => "appdata",
    }
}

/// Without Full Disk Access, reading other apps' containers pops up a macOS
/// privacy prompt for each one, so those are left out (and land in "System & other").
const NEEDS_FULL_ACCESS: &[&str] = &["Containers", "Group Containers"];

/// Third-party cloud drives (Google Drive, Dropbox, OneDrive…) list folders over
/// the network, so walking them can take many minutes. Never entered.
const NEVER: &[&str] = &["CloudStorage"];

pub fn has_full_disk_access(home: &Path) -> bool {
    fs::read_dir(home.join(".Trash")).is_ok()
}

pub fn categories(home: &Path, full_access: bool) -> Vec<(&'static str, Vec<PathBuf>)> {
    let mut out: Vec<(&'static str, Vec<PathBuf>)> = ORDER.iter().map(|id| (*id, Vec::new())).collect();
    let mut add = |id: &str, p: PathBuf| {
        if let Some((_, v)) = out.iter_mut().find(|(k, _)| *k == id) {
            v.push(p);
        }
    };

    add("apps", PathBuf::from("/Applications"));
    for e in fs::read_dir(home).into_iter().flatten().flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name != "Library" {
            add(home_category(&name), e.path());
            continue;
        }
        for l in fs::read_dir(e.path()).into_iter().flatten().flatten() {
            let lname = l.file_name();
            let lname = lname.to_string_lossy();
            if NEVER.contains(&&*lname) {
                continue;
            }
            if full_access || !NEEDS_FULL_ACCESS.contains(&&*lname) {
                add(library_category(&lname), l.path());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_folder_has_one_category() {
        assert_eq!(home_category("Documents"), "documents");
        assert_eq!(home_category("Coding"), "documents");
        assert_eq!(home_category(".npm"), "developer");
        assert_eq!(home_category(".Trash"), "trash");
        assert_eq!(home_category("Movies"), "media");
        assert_eq!(library_category("Caches"), "caches");
        assert_eq!(library_category("Mobile Documents"), "icloud");
        assert_eq!(library_category("Application Support"), "appdata");
        for id in ["documents", "developer", "trash", "media", "caches", "icloud", "appdata"] {
            assert!(ORDER.contains(&id));
        }
    }

    /// Read-only: cargo test --release bench_storage -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_storage() {
        use rayon::prelude::*;
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let t = std::time::Instant::now();
        let seen = crate::size::Inodes::default();
        for (id, paths) in categories(&home, has_full_disk_access(&home)) {
            let size: u64 = paths.par_iter().map(|p| crate::size::path_size(p, &seen)).sum();
            println!("{id:>10}  {:6.1} GB", size as f64 / 1e9);
        }
        println!("took {:?}", t.elapsed());
    }
}
