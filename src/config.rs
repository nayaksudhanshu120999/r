use crate::models::ShelfCategory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const DEFAULT_CONFIG_TOML: &str = r#"show_apps = true
custom_cmd_path = "cmd.exe"
custom_system_settings_path = "ms-settings:"
category_order = ["Cmds", "Settings", "Apps", "Web"]
hidden_apps = []
custom_app_dirs = []
custom_entries = []

[bangs]
yt = "https://www.youtube.com/results?search_query={q}"
g = "https://www.google.com/search?q={q}"
ddg = "https://duckduckgo.com/?q={q}"
gh = "https://github.com/search?q={q}"
rs = "https://docs.rs/releases/search?query={q}"
crates = "https://crates.io/search?q={q}"
wiki = "https://en.wikipedia.org/w/index.php?search={q}"
so = "https://stackoverflow.com/search?q={q}"
r = "https://www.reddit.com/search/?q={q}"
maps = "https://www.google.com/maps/search/{q}"
"#;

fn default_true() -> bool {
    true
}

fn default_cmd_path() -> String {
    "cmd.exe".to_string()
}

fn default_settings_path() -> String {
    "ms-settings:".to_string()
}

fn default_category_order() -> Vec<String> {
    vec![
        "Cmds".to_string(),
        "Settings".to_string(),
        "Apps".to_string(),
        "Web".to_string(),
    ]
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomEntryConfig {
    pub title: String,
    pub category: String, // "Cmds" or "Settings"
    #[serde(default)]
    pub triggers: Vec<String>,
    pub target_path: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_true")]
    pub show_apps: bool,
    #[serde(default = "default_cmd_path")]
    pub custom_cmd_path: String,
    #[serde(default = "default_settings_path")]
    pub custom_system_settings_path: String,
    #[serde(default = "default_category_order")]
    pub category_order: Vec<String>,
    #[serde(default)]
    pub hidden_apps: Vec<String>,
    #[serde(default)]
    pub custom_app_dirs: Vec<String>,
    #[serde(default)]
    pub custom_entries: Vec<CustomEntryConfig>,
    #[serde(default)]
    pub bangs: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        toml::from_str(DEFAULT_CONFIG_TOML).unwrap_or(Self {
            show_apps: true,
            custom_cmd_path: default_cmd_path(),
            custom_system_settings_path: default_settings_path(),
            category_order: default_category_order(),
            hidden_apps: Vec::new(),
            custom_app_dirs: Vec::new(),
            custom_entries: Vec::new(),
            bangs: HashMap::new(),
        })
    }
}

impl AppConfig {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let path_ref = path.as_ref();

        if !path_ref.exists() {
            let _ = fs::write(path_ref, DEFAULT_CONFIG_TOML);
        }

        let raw_content = fs::read_to_string(path_ref)
            .unwrap_or_else(|_| DEFAULT_CONFIG_TOML.to_string());

        let mut parsed: AppConfig = toml::from_str(&raw_content).unwrap_or_default();
        parsed.normalize();
        parsed
    }

    pub fn save<P: AsRef<Path>>(&mut self, path: P) {
        self.normalize();
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = fs::write(path.as_ref(), serialized);
        }
    }

    pub fn normalize(&mut self) {
        self.bangs = self
            .bangs
            .drain()
            .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();

        if self.custom_cmd_path.trim().is_empty() {
            self.custom_cmd_path = default_cmd_path();
        } else {
            self.custom_cmd_path = self.custom_cmd_path.trim().to_string();
        }

        if self.custom_system_settings_path.trim().is_empty() {
            self.custom_system_settings_path = default_settings_path();
        } else {
            self.custom_system_settings_path = self.custom_system_settings_path.trim().to_string();
        }

        // Ensure all 4 categories exist exactly once in preference order
        let mut ordered = Vec::with_capacity(4);
        for item in &self.category_order {
            let cat = ShelfCategory::from_label(item).as_str().to_string();
            if !ordered.contains(&cat) {
                ordered.push(cat);
            }
        }
        for fallback in ["Cmds", "Settings", "Apps", "Web"] {
            let s = fallback.to_string();
            if !ordered.contains(&s) {
                ordered.push(s);
            }
        }
        self.category_order = ordered;

        // Deduplicate hidden_apps case-insensitively
        let mut clean_hidden = Vec::new();
        for app in &self.hidden_apps {
            let trimmed = app.trim();
            if !trimmed.is_empty()
                && !clean_hidden
                    .iter()
                    .any(|h: &String| h.eq_ignore_ascii_case(trimmed))
            {
                clean_hidden.push(trimmed.to_string());
            }
        }
        self.hidden_apps = clean_hidden;
    }

    pub fn ordered_categories(&self) -> Vec<ShelfCategory> {
        self.category_order
            .iter()
            .map(|s| ShelfCategory::from_label(s))
            .collect()
    }

    pub fn is_app_hidden(&self, app_name: &str) -> bool {
        self.hidden_apps
            .iter()
            .any(|h| h.eq_ignore_ascii_case(app_name.trim()))
    }
}
