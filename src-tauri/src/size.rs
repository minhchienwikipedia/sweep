//! Parallel disk-usage calculation.
//!
//! Sizes are *allocated* bytes (st_blocks * 512), which is what actually comes
//! back when a file is deleted. Hard-linked files (pnpm, Time Machine-style
//! stores) are counted once per scan so totals are not inflated.

use rayon::prelude::*;
use std::{collections::HashSet, fs, os::unix::fs::MetadataExt, path::Path, sync::Mutex};

#[derive(Default)]
pub struct Inodes(Mutex<HashSet<(u64, u64)>>);

impl Inodes {
    fn first_sight(&self, dev: u64, ino: u64) -> bool {
        self.0.lock().unwrap().insert((dev, ino))
    }
}

fn file_usage(md: &fs::Metadata, seen: &Inodes) -> u64 {
    if md.nlink() > 1 && !seen.first_sight(md.dev(), md.ino()) {
        return 0;
    }
    md.blocks() * 512
}

/// Total allocated size of a file or directory tree. Never follows symlinks.
pub fn path_size(path: &Path, seen: &Inodes) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(md) if md.is_dir() => md.blocks() * 512 + dir_size(path, seen),
        Ok(md) => file_usage(&md, seen),
        Err(_) => 0,
    }
}

fn dir_size(path: &Path, seen: &Inodes) -> u64 {
    let Ok(rd) = fs::read_dir(path) else { return 0 };
    let entries: Vec<fs::DirEntry> = rd.flatten().collect();
    entries
        .par_iter()
        .map(|e| {
            // DirEntry::metadata does not traverse symlinks on unix.
            let Ok(md) = e.metadata() else { return 0 };
            if md.is_dir() {
                md.blocks() * 512 + dir_size(&e.path(), seen)
            } else {
                file_usage(&md, seen)
            }
        })
        .sum()
}
