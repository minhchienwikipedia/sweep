//! Parallel, permanent deletion.

use rayon::prelude::*;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

#[derive(Default)]
pub struct Stats {
    freed: AtomicU64,
    errors: AtomicU64,
}

impl Stats {
    pub fn freed(&self) -> u64 {
        self.freed.load(Relaxed)
    }
    pub fn errors(&self) -> u64 {
        self.errors.load(Relaxed)
    }
}

/// Last line of defence: only delete absolute paths strictly inside the home
/// folder (or on an external volume), never the home folder itself.
pub fn is_safe(p: &Path, home: &Path) -> bool {
    let depth = p.components().count();
    let no_dots = !p.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir));
    let in_home = p.starts_with(home) && p != home;
    let on_volume = p.starts_with("/Volumes") && depth >= 4;
    p.is_absolute() && no_dots && depth >= 4 && (in_home || on_volume)
}

/// Recursively delete `path`, children in parallel. Returns true on success.
/// Failures (files in use, SIP-protected) are counted, not fatal.
pub fn remove_all(path: &Path, st: &Stats) -> bool {
    let Ok(md) = fs::symlink_metadata(path) else { return true };

    if !md.is_dir() {
        // A file with other hard links frees nothing when one link is removed.
        let size = if md.nlink() > 1 { 0 } else { md.blocks() * 512 };
        return match fs::remove_file(path) {
            Ok(()) => {
                st.freed.fetch_add(size, Relaxed);
                true
            }
            Err(_) => {
                st.errors.fetch_add(1, Relaxed);
                false
            }
        };
    }

    // Read-only directories (e.g. Go's module cache) must be made writable
    // before their entries can be unlinked.
    let mode = md.permissions().mode();
    if mode & 0o700 != 0o700 {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o700));
    }

    let children: Vec<PathBuf> = match fs::read_dir(path) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(_) => {
            st.errors.fetch_add(1, Relaxed);
            return false;
        }
    };
    let all_ok = children
        .par_iter()
        .map(|c| remove_all(c, st))
        .reduce(|| true, |a, b| a && b);
    if !all_ok {
        return false;
    }

    match fs::remove_dir(path) {
        Ok(()) => {
            st.freed.fetch_add(md.blocks() * 512, Relaxed);
            true
        }
        Err(_) => {
            st.errors.fetch_add(1, Relaxed);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety() {
        let home = Path::new("/Users/me");
        assert!(!is_safe(home, home));
        assert!(!is_safe(Path::new("/Users/me/.."), home));
        assert!(!is_safe(Path::new("/Users/me/../other/x"), home));
        assert!(!is_safe(Path::new("/Users/other/x"), home));
        assert!(!is_safe(Path::new("/System/Library"), home));
        assert!(!is_safe(Path::new("/Volumes/Disk"), home));
        assert!(is_safe(Path::new("/Users/me/Library/Caches/foo"), home));
        assert!(is_safe(Path::new("/Volumes/Disk/proj/node_modules"), home));
    }

    #[test]
    fn removes_tree_including_readonly_dirs() {
        let root = std::env::temp_dir().join(format!("sweep-test-{}", std::process::id()));
        let ro = root.join("a/b");
        fs::create_dir_all(&ro).unwrap();
        fs::write(ro.join("f.txt"), vec![0u8; 10_000]).unwrap();
        fs::write(root.join("a/g.txt"), b"hi").unwrap();
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();

        let st = Stats::default();
        assert!(remove_all(&root, &st));
        assert!(!root.exists());
        assert!(st.freed() >= 10_000);
        assert_eq!(st.errors(), 0);
    }
}
