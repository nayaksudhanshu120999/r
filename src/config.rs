use crate::models::ShelfCategory;
use crate::system::normalize_hotkey_str;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

fn default_hotkey() -> String {
    "Ctrl+Space".to_string()
}

fn default_theme() -> String {
    "powertoys".to_string()
}

fn default_search_engine() -> String {
    "google".to_string()
}

fn default_custom_search_url() -> String {
    "https://www.google.com/search?q={q}".to_string()
}

fn default_cmd_path() -> String {
    "cmd.exe".to_string()
}

fn default_settings_path() -> String {
    "ms-settings:".to_string()
}

fn default_category_order() -> Vec<String> {
    ShelfCategory::all_default_order()
        .into_iter()
        .map(|c| c.as_str().to_string())
        .collect()
}

fn default_notes() -> Vec<NoteBangItem> {
    vec![
        NoteBangItem {
            bang: "note".to_string(),
            name: "Quick Notes".to_string(),
            file_path: "notes.md".to_string(),
        },
        NoteBangItem {
            bang: "todo".to_string(),
            name: "To-Do List".to_string(),
            file_path: "todo.txt".to_string(),
        },
    ]
}

fn default_bangs() -> BTreeMap<String, String> {
    let mut bangs = BTreeMap::new();
    bangs.insert(
        "yt".to_string(),
        "https://www.youtube.com/results?search_query={q}".to_string(),
    );
    bangs.insert(
        "g".to_string(),
        "https://www.google.com/search?q={q}".to_string(),
    );
    bangs.insert(
        "ddg".to_string(),
        "https://duckduckgo.com/?q={q}".to_string(),
    );
    bangs.insert(
        "gh".to_string(),
        "https://github.com/search?q={q}".to_string(),
    );
    bangs.insert(
        "rs".to_string(),
        "https://docs.rs/releases/search?query={q}".to_string(),
    );
    bangs.insert(
        "crates".to_string(),
        "https://crates.io/search?q={q}".to_string(),
    );
    bangs.insert(
        "wiki".to_string(),
        "https://en.wikipedia.org/w/index.php?search={q}".to_string(),
    );
    bangs.insert(
        "so".to_string(),
        "https://stackoverflow.com/search?q={q}".to_string(),
    );
    bangs.insert(
        "r".to_string(),
        "https://www.reddit.com/search/?q={q}".to_string(),
    );
    bangs.insert(
        "maps".to_string(),
        "https://www.google.com/maps/search/{q}".to_string(),
    );
    bangs
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomThemeConfig {
    pub window_bg: String,
    pub surface_bg: String,
    pub card_bg: String,
    pub selected_bg: String,
    pub border: String,
    pub accent: String,
    pub text_primary: String,
    pub text_muted: String,
}

impl Default for CustomThemeConfig {
    fn default() -> Self {
        Self {
            window_bg: "#1f1f1f".to_string(),
            surface_bg: "#2b2b2b".to_string(),
            card_bg: "#282828".to_string(),
            selected_bg: "#383838".to_string(),
            border: "#3d3d3d".to_string(),
            accent: "#60cdff".to_string(),
            text_primary: "#ffffff".to_string(),
            text_muted: "#9d9d9d".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomCommandItem {
    pub name: String,
    pub command: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomSettingItem {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteBangItem {
    pub bang: String,
    pub name: String,
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_true")]
    pub show_apps: bool,
    #[serde(default)]
    pub run_on_startup: bool,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub custom_theme: CustomThemeConfig,

    #[serde(default = "default_true")]
    pub enable_cmdpal_shortcuts: bool,
    #[serde(default = "default_true")]
    pub enable_builtin_cmds: bool,
    #[serde(default = "default_true")]
    pub enable_builtin_settings: bool,
    #[serde(default)]
    pub disabled_builtin_cmds: Vec<String>,
    #[serde(default)]
    pub disabled_builtin_settings: Vec<String>,

    #[serde(default = "default_search_engine")]
    pub default_search_engine: String,
    #[serde(default = "default_custom_search_url")]
    pub custom_search_url: String,

    #[serde(default = "default_cmd_path")]
    pub custom_cmd_path: String,
    #[serde(default = "default_settings_path")]
    pub custom_system_settings_path: String,
    #[serde(default = "default_category_order")]
    pub category_order: Vec<String>,
    #[serde(default)]
    pub hidden_apps: Vec<String>,
    #[serde(default)]
    pub custom_file_scan_paths: Vec<String>,
    #[serde(default)]
    pub custom_commands: Vec<CustomCommandItem>,
    #[serde(default)]
    pub custom_settings: Vec<CustomSettingItem>,
    #[serde(default = "default_bangs")]
    pub bangs: BTreeMap<String, String>,
    #[serde(default = "default_notes")]
    pub notes: Vec<NoteBangItem>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            show_apps: true,
            run_on_startup: false,
            hotkey: default_hotkey(),
            theme: default_theme(),
            custom_theme: CustomThemeConfig::default(),
            enable_cmdpal_shortcuts: true,
            enable_builtin_cmds: true,
            enable_builtin_settings: true,
            disabled_builtin_cmds: Vec::new(),
            disabled_builtin_settings: Vec::new(),
            default_search_engine: default_search_engine(),
            custom_search_url: default_custom_search_url(),
            custom_cmd_path: default_cmd_path(),
            custom_system_settings_path: default_settings_path(),
            category_order: default_category_order(),
            hidden_apps: Vec::new(),
            custom_file_scan_paths: Vec::new(),
            custom_commands: Vec::new(),
            custom_settings: Vec::new(),
            bangs: default_bangs(),
            notes: default_notes(),
        }
    }
}

impl AppConfig {
    pub fn resolve_config_path(rel_path: &str) -> PathBuf {
        let candidate = PathBuf::from(rel_path);
        if candidate.is_absolute() || candidate.exists() {
            return candidate;
        }
        if let Ok(exe) = env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join(rel_path);
            }
        }
        candidate
    }

    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let resolved = Self::resolve_config_path(&path.as_ref().to_string_lossy());
        if let Ok(raw) = fs::read_to_string(&resolved) {
            if let Ok(mut parsed) = toml::from_str::<AppConfig>(&raw) {
                parsed.normalize();
                return parsed;
            }
        }

        let mut cfg = Self::default();
        cfg.normalize();
        if let Ok(serialized) = toml::to_string_pretty(&cfg) {
            let _ = fs::write(&resolved, serialized);
        }
        cfg
    }

    pub fn save<P: AsRef<Path>>(&mut self, path: P) {
        self.normalize();
        let resolved = Self::resolve_config_path(&path.as_ref().to_string_lossy());
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = fs::write(resolved, serialized);
        }
        crate::system::apply_runtime_system_config(self.run_on_startup, &self.hotkey);
    }

    pub fn normalize(&mut self) {
        self.hotkey = normalize_hotkey_str(&self.hotkey);

        let theme_lower = self.theme.trim().to_ascii_lowercase();
        self.theme = if theme_lower == "custom" {
            "custom".to_string()
        } else {
            "powertoys".to_string()
        };

        let engine_lower = self.default_search_engine.trim().to_ascii_lowercase();
        self.default_search_engine = match engine_lower.as_str() {
            "google" | "duckduckgo" | "bing" | "brave" | "startpage" | "perplexity"
            | "youtube" | "custom" => engine_lower,
            _ => "google".to_string(),
        };

        if self.custom_search_url.trim().is_empty() || !self.custom_search_url.contains("{q}") {
            self.custom_search_url = default_custom_search_url();
        }
        if self.custom_cmd_path.trim().is_empty() {
            self.custom_cmd_path = default_cmd_path();
        }
        if self.custom_system_settings_path.trim().is_empty() {
            self.custom_system_settings_path = default_settings_path();
        }

        // Normalize shelf order
        let mut seen_cats = HashSet::new();
        let mut ordered = Vec::with_capacity(7);
        for label in &self.category_order {
            if let Some(cat) = ShelfCategory::from_label(label) {
                if seen_cats.insert(cat) {
                    ordered.push(cat.as_str().to_string());
                }
            }
        }
        for cat in ShelfCategory::all_default_order() {
            if seen_cats.insert(cat) {
                ordered.push(cat.as_str().to_string());
            }
        }
        self.category_order = ordered;

        // Deduplicate & lowercase toggle lists
        Self::dedup_lowercase_vec(&mut self.hidden_apps);
        Self::dedup_lowercase_vec(&mut self.disabled_builtin_cmds);
        Self::dedup_lowercase_vec(&mut self.disabled_builtin_settings);

        // Normalize web bangs
        let mut clean_bangs = BTreeMap::new();
        for (k, v) in &self.bangs {
            let key = k.trim().trim_start_matches('!').to_ascii_lowercase();
            let val = v.trim().to_string();
            if !key.is_empty() && !key.contains(char::is_whitespace) && val.contains("{q}") {
                clean_bangs.insert(key, val);
            }
        }
        self.bangs = clean_bangs;

        // Normalize note bangs (multiple text files with distinct bangs)
        let mut clean_notes = Vec::new();
        let mut seen_note_bangs = HashSet::new();
        for item in &self.notes {
            let bang = item.bang.trim().trim_start_matches('!').to_ascii_lowercase();
            let file_path = item.file_path.trim().trim_matches('"').to_string();
            if bang.is_empty() || bang.contains(char::is_whitespace) || file_path.is_empty() {
                continue;
            }
            let normalized_path = if Path::new(&file_path).extension().is_none() {
                format!("{}.md", file_path)
            } else {
                file_path
            };
            let name = if item.name.trim().is_empty() {
                format!("Notes ({})", normalized_path)
            } else {
                item.name.trim().to_string()
            };
            if seen_note_bangs.insert(bang.clone()) {
                clean_notes.push(NoteBangItem {
                    bang,
                    name,
                    file_path: normalized_path,
                });
            }
        }
        self.notes = clean_notes;
    }

    fn dedup_lowercase_vec(vec: &mut Vec<String>) {
        let mut seen = HashSet::new();
        let mut out = Vec::with_capacity(vec.len());
        for s in vec.iter() {
            let lower = s.trim().to_ascii_lowercase();
            if !lower.is_empty() && seen.insert(lower.clone()) {
                out.push(lower);
            }
        }
        *vec = out;
    }

    pub fn ordered_categories(&self) -> Vec<ShelfCategory> {
        self.category_order
            .iter()
            .filter_map(|s| ShelfCategory::from_label(s))
            .collect()
    }

    pub fn is_app_hidden(&self, app_name_lower: &str) -> bool {
        self.hidden_apps.iter().any(|h| h == app_name_lower)
    }

    pub fn set_app_visibility(&mut self, app_name: &str, visible: bool) {
        let lower = app_name.trim().to_ascii_lowercase();
        self.hidden_apps.retain(|h| h != &lower);
        if !visible {
            self.hidden_apps.push(lower);
        }
    }

    pub fn is_builtin_cmd_enabled(&self, id: &str) -> bool {
        if !self.enable_builtin_cmds {
            return false;
        }
        !self.disabled_builtin_cmds.iter().any(|d| d == id)
    }

    pub fn set_builtin_cmd_enabled(&mut self, id: &str, enabled: bool) {
        let lower = id.trim().to_ascii_lowercase();
        self.disabled_builtin_cmds.retain(|d| d != &lower);
        if !enabled {
            self.disabled_builtin_cmds.push(lower);
        }
    }

    pub fn is_builtin_setting_enabled(&self, id: &str) -> bool {
        if !self.enable_builtin_settings {
            return false;
        }
        !self.disabled_builtin_settings.iter().any(|d| d == id)
    }

    pub fn set_builtin_setting_enabled(&mut self, id: &str, enabled: bool) {
        let lower = id.trim().to_ascii_lowercase();
        self.disabled_builtin_settings.retain(|d| d != &lower);
        if !enabled {
            self.disabled_builtin_settings.push(lower);
        }
    }

    pub fn resolve_default_search_engine(&self) -> (&'static str, &str) {
        match self.default_search_engine.as_str() {
            "duckduckgo" => ("DuckDuckGo", "https://duckduckgo.com/?q={q}"),
            "bing" => ("Bing", "https://www.bing.com/search?q={q}"),
            "brave" => ("Brave Search", "https://search.brave.com/search?q={q}"),
            "startpage" => ("Startpage", "https://www.startpage.com/do/dsearch?query={q}"),
            "perplexity" => ("Perplexity", "https://www.perplexity.ai/search?q={q}"),
            "youtube" => (
                "YouTube",
                "https://www.youtube.com/results?search_query={q}",
            ),
            "custom" => ("Custom Search", self.custom_search_url.as_str()),
            _ => ("Google", "https://www.google.com/search?q={q}"),
        }
    }

    pub fn upsert_note_bang(&mut self, bang: &str, name: &str, file_path: &str) -> bool {
        let clean_bang = bang.trim().trim_start_matches('!').to_ascii_lowercase();
        let clean_file = file_path.trim().trim_matches('"');
        if clean_bang.is_empty()
            || clean_bang.contains(char::is_whitespace)
            || clean_file.is_empty()
        {
            return false;
        }
        let clean_name = if name.trim().is_empty() {
            format!("Notes ({})", clean_file)
        } else {
            name.trim().to_string()
        };

        if let Some(existing) = self.notes.iter_mut().find(|n| n.bang == clean_bang) {
            existing.name = clean_name;
            existing.file_path = clean_file.to_string();
        } else {
            self.notes.push(NoteBangItem {
                bang: clean_bang,
                name: clean_name,
                file_path: clean_file.to_string(),
            });
        }
        self.normalize();
        true
    }

    pub fn remove_note_bang(&mut self, bang: &str) {
        let clean_bang = bang.trim().trim_start_matches('!').to_ascii_lowercase();
        self.notes.retain(|n| n.bang != clean_bang);
    }
}
