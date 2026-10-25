use crate::extensions::{
    load_external_extension_manifests, ExtensionEntry, BUILTIN_NOTEPAD_ID,
};
use crate::models::ShelfCategory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_TOML: &str = r##"show_apps = true
run_on_startup = false
hotkey = "Ctrl+Space"
theme = "powertoys"
enable_cmdpal_shortcuts = true
enable_builtin_cmds = true
enable_builtin_settings = true
disabled_builtin_cmds = []
disabled_builtin_settings = []
default_search_engine = "google"
custom_search_url = "https://www.google.com/search?q={q}"
custom_cmd_path = "cmd.exe"
custom_system_settings_path = "ms-settings:"
category_order = ["Extensions", "CmdPal", "Cmds", "Settings", "Apps", "Files", "Web"]
hidden_apps = []
custom_file_scan_paths = []
custom_commands = []
custom_settings = []
removed_builtin_extensions = []

[custom_theme]
window_bg = "#1f1f1f"
surface_bg = "#2b2b2b"
card_bg = "#282828"
selected_bg = "#383838"
border = "#3d3d3d"
accent = "#60cdff"
text_primary = "#ffffff"
text_muted = "#9d9d9d"

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

[[extensions]]
id = "notepad"
name = "Quick Notepad & File Viewer"
description = "Use bang 'note' to save text to a .txt or .md file and view file contents in the palette"
bang = "note"
icon = "📝"
enabled = true
kind = "NotepadFile"
target_file = "notes.md"
command_template = ""
"##;

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
        .iter()
        .map(|c| c.as_str().to_string())
        .collect()
}

fn default_extensions() -> Vec<ExtensionEntry> {
    vec![ExtensionEntry::builtin_notepad()]
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
    #[serde(default)]
    pub bangs: HashMap<String, String>,

    #[serde(default = "default_extensions")]
    pub extensions: Vec<ExtensionEntry>,
    #[serde(default)]
    pub removed_builtin_extensions: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        toml::from_str(DEFAULT_CONFIG_TOML).unwrap_or(Self {
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
            bangs: HashMap::new(),
            extensions: default_extensions(),
            removed_builtin_extensions: Vec::new(),
        })
    }
}

impl AppConfig {
    pub fn resolve_config_path<P: AsRef<Path>>(path: P) -> PathBuf {
        let p = path.as_ref();
        if p.is_absolute() || p.exists() {
            return p.to_path_buf();
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join(p);
            }
        }
        p.to_path_buf()
    }

    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let resolved = Self::resolve_config_path(path);

        if !resolved.exists() {
            let _ = fs::write(&resolved, DEFAULT_CONFIG_TOML);
        }

        let raw_content =
            fs::read_to_string(&resolved).unwrap_or_else(|_| DEFAULT_CONFIG_TOML.to_string());

        let mut parsed: AppConfig = toml::from_str(&raw_content).unwrap_or_default();
        parsed.normalize();
        crate::system::apply_runtime_system_config(parsed.run_on_startup, &parsed.hotkey);
        parsed
    }

    pub fn save<P: AsRef<Path>>(&mut self, path: P) {
        self.normalize();
        let resolved = Self::resolve_config_path(path);
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = fs::write(resolved, serialized);
        }
        crate::system::apply_runtime_system_config(self.run_on_startup, &self.hotkey);
    }

    pub fn normalize(&mut self) {
        self.hotkey = crate::system::normalize_hotkey_str(&self.hotkey);

        self.bangs = self
            .bangs
            .drain()
            .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();

        let t = self.theme.trim().to_ascii_lowercase();
        self.theme = if t == "custom" {
            "custom".to_string()
        } else {
            "powertoys".to_string()
        };

        let engine_norm = self.default_search_engine.trim().to_ascii_lowercase();
        self.default_search_engine = match engine_norm.as_str() {
            "google" | "duckduckgo" | "ddg" | "bing" | "brave" | "startpage" | "perplexity"
            | "youtube" | "custom" => {
                if engine_norm == "ddg" {
                    "duckduckgo".to_string()
                } else {
                    engine_norm
                }
            }
            _ => "google".to_string(),
        };

        if self.custom_search_url.trim().is_empty() {
            self.custom_search_url = default_custom_search_url();
        } else {
            self.custom_search_url = self.custom_search_url.trim().to_string();
        }

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

        let mut ordered = Vec::with_capacity(7);
        for item in &self.category_order {
            let cat = ShelfCategory::from_label(item).as_str().to_string();
            if !ordered.contains(&cat) {
                ordered.push(cat);
            }
        }
        for fallback in ShelfCategory::all_default_order() {
            let s = fallback.as_str().to_string();
            if !ordered.contains(&s) {
                ordered.push(s);
            }
        }
        self.category_order = ordered;

        // Ensure built-in notepad extension is present unless user explicitly removed it
        let notepad_removed = self
            .removed_builtin_extensions
            .iter()
            .any(|id| id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID));
        let has_notepad = self
            .extensions
            .iter()
            .any(|e| e.id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID));
        if !notepad_removed && !has_notepad {
            self.extensions.insert(0, ExtensionEntry::builtin_notepad());
        }

        // Merge any external `./extensions/*.toml` manifests not yet in `self.extensions`
        for ext_manifest in load_external_extension_manifests() {
            let already_exists = self
                .extensions
                .iter()
                .any(|e| e.id.eq_ignore_ascii_case(&ext_manifest.id));
            if !already_exists {
                self.extensions.push(ext_manifest);
            }
        }

        for ext in &mut self.extensions {
            ext.id = ext.id.trim().to_string();
            ext.name = ext.name.trim().to_string();
            ext.bang = ext
                .bang
                .trim()
                .trim_start_matches('!')
                .to_ascii_lowercase();
            if ext.icon.trim().is_empty() {
                ext.icon = "🧩".to_string();
            }
            if ext.id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID) && ext.target_file.trim().is_empty()
            {
                ext.target_file = "notes.md".to_string();
            }
        }
        self.extensions
            .retain(|e| !e.id.is_empty() && !e.name.is_empty() && !e.bang.is_empty());

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

        let mut clean_scans = Vec::new();
        for path in &self.custom_file_scan_paths {
            let trimmed = path.trim();
            if !trimmed.is_empty()
                && !clean_scans
                    .iter()
                    .any(|p: &String| p.eq_ignore_ascii_case(trimmed))
            {
                clean_scans.push(trimmed.to_string());
            }
        }
        self.custom_file_scan_paths = clean_scans;

        self.custom_commands
            .retain(|c| !c.name.trim().is_empty() && !c.command.trim().is_empty());
        self.custom_settings
            .retain(|s| !s.name.trim().is_empty() && !s.path.trim().is_empty());
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

    pub fn set_app_visibility(&mut self, app_name: &str, visible: bool) {
        let trimmed = app_name.trim();
        self.hidden_apps.retain(|h| !h.eq_ignore_ascii_case(trimmed));
        if !visible && !trimmed.is_empty() {
            self.hidden_apps.push(trimmed.to_string());
        }
    }

    pub fn is_builtin_cmd_enabled(&self, id: &str) -> bool {
        self.enable_builtin_cmds
            && !self
                .disabled_builtin_cmds
                .iter()
                .any(|d| d.eq_ignore_ascii_case(id))
    }

    pub fn set_builtin_cmd_enabled(&mut self, id: &str, enabled: bool) {
        self.disabled_builtin_cmds
            .retain(|d| !d.eq_ignore_ascii_case(id));
        if !enabled {
            self.disabled_builtin_cmds.push(id.to_string());
        }
    }

    pub fn is_builtin_setting_enabled(&self, id: &str) -> bool {
        self.enable_builtin_settings
            && !self
                .disabled_builtin_settings
                .iter()
                .any(|d| d.eq_ignore_ascii_case(id))
    }

    pub fn set_builtin_setting_enabled(&mut self, id: &str, enabled: bool) {
        self.disabled_builtin_settings
            .retain(|d| !d.eq_ignore_ascii_case(id));
        if !enabled {
            self.disabled_builtin_settings.push(id.to_string());
        }
    }

    pub fn resolve_default_search_engine(&self) -> (&'static str, String) {
        match self.default_search_engine.as_str() {
            "duckduckgo" => ("DuckDuckGo", "https://duckduckgo.com/?q={q}".to_string()),
            "bing" => ("Bing", "https://www.bing.com/search?q={q}".to_string()),
            "brave" => (
                "Brave Search",
                "https://search.brave.com/search?q={q}".to_string(),
            ),
            "startpage" => (
                "Startpage",
                "https://www.startpage.com/do/dsearch?query={q}".to_string(),
            ),
            "perplexity" => (
                "Perplexity",
                "https://www.perplexity.ai/search?q={q}".to_string(),
            ),
            "youtube" => (
                "YouTube",
                "https://www.youtube.com/results?search_query={q}".to_string(),
            ),
            "custom" => ("Custom Search", self.custom_search_url.clone()),
            _ => ("Google", "https://www.google.com/search?q={q}".to_string()),
        }
    }

    pub fn notepad_extension(&self) -> Option<&ExtensionEntry> {
        self.extensions
            .iter()
            .find(|e| e.id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID))
    }

    pub fn set_notepad_file_path(&mut self, new_path: &str) {
        let clean = new_path.trim();
        if clean.is_empty() {
            return;
        }
        if let Some(ext) = self
            .extensions
            .iter_mut()
            .find(|e| e.id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID))
        {
            ext.target_file = clean.to_string();
        } else {
            let mut np = ExtensionEntry::builtin_notepad();
            np.target_file = clean.to_string();
            self.removed_builtin_extensions
                .retain(|id| !id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID));
            self.extensions.insert(0, np);
        }
    }

    pub fn remove_extension_by_id(&mut self, id: &str) {
        if id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID) {
            if !self
                .removed_builtin_extensions
                .iter()
                .any(|r| r.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID))
            {
                self.removed_builtin_extensions
                    .push(BUILTIN_NOTEPAD_ID.to_string());
            }
        }
        self.extensions.retain(|e| !e.id.eq_ignore_ascii_case(id));
    }

    pub fn restore_builtin_notepad(&mut self) {
        self.removed_builtin_extensions
            .retain(|id| !id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID));
        if !self
            .extensions
            .iter()
            .any(|e| e.id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID))
        {
            self.extensions.insert(0, ExtensionEntry::builtin_notepad());
        }
    }
}
