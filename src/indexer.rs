use crate::engine::expand_env_and_tilde;
use crate::models::{AppEntry, ScannedFileEntry};
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn build_start_menu_index() -> Vec<AppEntry> {
    let mut roots = Vec::new();

    if let Ok(appdata) = env::var("APPDATA") {
        roots.push(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    if let Ok(progdata) = env::var("PROGRAMDATA") {
        roots.push(
            PathBuf::from(progdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    let mut seen_names = HashSet::new();
    let mut entries = Vec::new();

    for root in roots {
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let is_lnk = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("lnk"))
                .unwrap_or(false);

            if !is_lnk {
                continue;
            }

            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            let name = stem.trim().to_string();
            if name.is_empty() {
                continue;
            }

            let name_lower = name.to_ascii_lowercase();
            if name_lower.contains("uninstall") {
                continue;
            }

            if seen_names.insert(name_lower.clone()) {
                entries.push(AppEntry {
                    name,
                    name_lower,
                    path: path.to_path_buf(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
    entries
}

pub fn build_custom_files_index(custom_paths: &[String]) -> Vec<ScannedFileEntry> {
    let mut seen_paths = HashSet::new();
    let mut entries = Vec::new();

    for raw in custom_paths {
        let trimmed = raw.trim().trim_matches('"');
        if trimmed.is_empty() {
            continue;
        }

        let expanded = expand_env_and_tilde(trimmed);
        let root = PathBuf::from(&expanded);
        if !root.exists() {
            continue;
        }

        if root.is_file() {
            if let Some(name) = root.file_name().and_then(|s| s.to_str()) {
                let full_lower = root.to_string_lossy().to_ascii_lowercase();
                if seen_paths.insert(full_lower) {
                    entries.push(ScannedFileEntry {
                        display_name: name.to_string(),
                        name_lower: name.to_ascii_lowercase(),
                        path: root,
                    });
                }
            }
            continue;
        }

        for entry in WalkDir::new(&root)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };

            let full_lower = path.to_string_lossy().to_ascii_lowercase();
            if seen_paths.insert(full_lower) {
                entries.push(ScannedFileEntry {
                    display_name: file_name.to_string(),
                    name_lower: file_name.to_ascii_lowercase(),
                    path: path.to_path_buf(),
                });
            }

            if entries.len() >= 4_000 {
                break;
            }
        }
    }

    entries.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
    entries
}

pub fn scan_installed_apps() -> Vec<AppEntry> {
    build_start_menu_index()
}

pub fn scan_custom_files(custom_paths: &[String]) -> Vec<ScannedFileEntry> {
    build_custom_files_index(custom_paths)
}
