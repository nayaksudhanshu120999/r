use crate::models::AppEntry;
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn build_start_menu_index() -> Vec<AppEntry> {
    let mut roots = Vec::with_capacity(2);

    // 1. System-wide Start Menu: C:\ProgramData\Microsoft\Windows\Start Menu\Programs
    let program_data = env::var("PROGRAMDATA")
        .unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let global_start_menu = PathBuf::from(program_data)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    roots.push(global_start_menu);

    // 2. Per-user Start Menu: C:\Users\<User>\AppData\Roaming\Microsoft\Windows\Start Menu\Programs
    if let Ok(appdata) = env::var("APPDATA") {
        let user_start_menu = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        roots.push(user_start_menu);
    } else if let Ok(user_profile) = env::var("USERPROFILE") {
        let user_start_menu = PathBuf::from(user_profile)
            .join("AppData")
            .join("Roaming")
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        roots.push(user_start_menu);
    }

    let mut seen_names = HashSet::with_capacity(256);
    let mut entries = Vec::with_capacity(256);

    for root in roots {
        if !root.exists() {
            continue;
        }

        for dir_entry in WalkDir::new(&root)
            .follow_links(false)
            .max_depth(6)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = dir_entry.path();
            if !path.is_file() || !is_launchable_extension(path) {
                continue;
            }

            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            let clean_name = stem.trim();
            if clean_name.is_empty() || is_uninstaller_noise(clean_name) {
                continue;
            }

            let dedup_key = clean_name.to_ascii_lowercase();
            if seen_names.insert(dedup_key) {
                let path_buf = path.to_path_buf();
                let display_path = path_buf.to_string_lossy().into_owned();
                entries.push(AppEntry {
                    name: clean_name.to_string(),
                    path: path_buf,
                    display_path,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries.shrink_to_fit();
    entries
}

fn is_launchable_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    ext.eq_ignore_ascii_case("lnk") || ext.eq_ignore_ascii_case("exe")
}

fn is_uninstaller_noise(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("uninstall ") || lower == "uninstall"
}
