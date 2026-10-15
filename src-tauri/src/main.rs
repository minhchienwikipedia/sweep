#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clean;
mod disk;
mod projects;
mod size;
mod storage;
mod targets;
mod ticker;

use rayon::prelude::*;
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, process::Command, sync::Mutex};
use tauri::{async_runtime::spawn_blocking, AppHandle, Emitter, Manager};

/// Paths produced by the last scans, keyed by item id. `clean` only ever
/// deletes paths found here, never arbitrary paths sent from the UI.
#[derive(Default)]
struct Registry(Mutex<HashMap<String, Vec<PathBuf>>>);

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).expect("HOME is not set")
}

#[derive(Serialize)]
struct DiskInfo {
    total: u64,
    free: u64,
    home: String,
}

#[derive(Serialize)]
struct CleanResult {
    freed: u64,
    errors: u64,
}

#[tauri::command]
fn disk_info() -> DiskInfo {
    let h = home();
    let (total, free) = disk::usage(&h);
    DiskInfo { total, free, home: h.to_string_lossy().into_owned() }
}

#[tauri::command]
fn list_targets() -> Vec<targets::Meta> {
    targets::all(&home()).iter().map(|t| t.meta()).collect()
}

/// Sizes every cache target in parallel, emitting `category` as each finishes.
#[tauri::command]
async fn scan_caches(app: AppHandle) -> Result<(), String> {
    spawn_blocking(move || {
        let seen = size::Inodes::default();
        let state = app.state::<Registry>();
        targets::all(&home()).par_iter().for_each(|t| {
            let (summary, paths) = t.scan(&seen);
            state.0.lock().unwrap().insert(t.id.to_string(), paths);
            let _ = app.emit("category", summary);
        });
    })
    .await
    .map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
struct StorageCategory {
    id: &'static str,
    size: u64,
}

/// Sizes each storage category in parallel, emitting `storage` as each finishes.
/// Returns whether Sweep has Full Disk Access (otherwise some data is unreadable).
#[tauri::command]
async fn scan_storage(app: AppHandle) -> Result<bool, String> {
    spawn_blocking(move || {
        let home = home();
        let full_access = storage::has_full_disk_access(&home);
        let seen = size::Inodes::default();
        storage::categories(&home, full_access).par_iter().for_each(|(id, paths)| {
            let size = paths.par_iter().map(|p| size::path_size(p, &seen)).sum();
            let _ = app.emit("storage", StorageCategory { id, size });
        });
        full_access
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn scan_projects(app: AppHandle, root: Option<String>) -> Result<Vec<projects::Artifact>, String> {
    spawn_blocking(move || {
        let home = home();
        let root = root.map(PathBuf::from).unwrap_or_else(|| home.clone());
        let found = projects::scan(&root, &home, |n| {
            let _ = app.emit("scan-progress", n);
        });
        let state = app.state::<Registry>();
        let mut reg = state.0.lock().unwrap();
        reg.retain(|k, _| !k.starts_with("p:"));
        for a in &found {
            reg.insert(a.id.clone(), vec![PathBuf::from(&a.path)]);
        }
        found
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn clean(app: AppHandle, ids: Vec<String>) -> Result<CleanResult, String> {
    spawn_blocking(move || {
        let home = home();
        let paths: Vec<PathBuf> = {
            let state = app.state::<Registry>();
            let mut reg = state.0.lock().unwrap();
            ids.iter().filter_map(|id| reg.remove(id)).flatten().collect()
        };
        let stats = clean::Stats::default();
        ticker::every(
            100,
            || {
                let _ = app.emit("clean-progress", stats.freed());
            },
            || {
                paths
                    .par_iter()
                    .filter(|p| clean::is_safe(p, &home))
                    .for_each(|p| {
                        clean::remove_all(p, &stats);
                    })
            },
        );
        CleanResult { freed: stats.freed(), errors: stats.errors() }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal(path: String) {
    let _ = Command::new("open").arg("-R").arg(path).spawn();
}

#[tauri::command]
fn open_full_disk_access() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .spawn();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Registry::default())
        .invoke_handler(tauri::generate_handler![
            disk_info,
            list_targets,
            scan_caches,
            scan_storage,
            scan_projects,
            clean,
            reveal,
            open_full_disk_access
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sweep");
}
