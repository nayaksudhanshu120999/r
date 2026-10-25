use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShelfCategory {
    Notes,
    CmdPal,
    Cmds,
    Settings,
    Apps,
    Files,
    Web,
}

impl ShelfCategory {
    pub fn all_default_order() -> Vec<Self> {
        vec![
            Self::Notes,
            Self::CmdPal,
            Self::Cmds,
            Self::Settings,
            Self::Apps,
            Self::Files,
            Self::Web,
        ]
    }

    pub fn from_label(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "notes" | "note" | "notepad" | "extensions" | "ext" => Some(Self::Notes),
            "cmdpal" | "palette" | "cmdpal_settings" => Some(Self::CmdPal),
            "cmds" | "cmd" | "commands" | "system" => Some(Self::Cmds),
            "settings" | "setting" | "winsettings" | "windows_settings" => Some(Self::Settings),
            "apps" | "app" | "applications" => Some(Self::Apps),
            "files" | "file" | "customfiles" => Some(Self::Files),
            "web" | "search" | "browser" => Some(Self::Web),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Notes => "Notes",
            Self::CmdPal => "CmdPal",
            Self::Cmds => "Cmds",
            Self::Settings => "Settings",
            Self::Apps => "Apps",
            Self::Files => "Files",
            Self::Web => "Web",
        }
    }

    pub fn header_title(&self) -> &'static str {
        match self {
            Self::Notes => "NOTES & TEXT FILES",
            Self::CmdPal => "PALETTE ACTIONS",
            Self::Cmds => "SYSTEM COMMANDS",
            Self::Settings => "WINDOWS SETTINGS",
            Self::Apps => "APPLICATIONS",
            Self::Files => "FILES & FOLDERS",
            Self::Web => "WEB SEARCH",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Notes => "Notes (Note Bangs)",
            Self::CmdPal => "Palette Actions",
            Self::Cmds => "System Commands",
            Self::Settings => "Windows Settings",
            Self::Apps => "Applications",
            Self::Files => "Custom Files",
            Self::Web => "Web Search",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Notes => "Quick note bangs mapped to .txt and .md files",
            Self::CmdPal => "Command Palette preferences, theme, hotkey & index reload",
            Self::Cmds => "Built-in Windows admin tools, calculator & custom shell commands",
            Self::Settings => "Windows ms-settings: pages & Control Panel shortcuts",
            Self::Apps => "Installed Start Menu applications",
            Self::Files => "Direct file paths & files from custom scan folders",
            Self::Web => "Web search bangs, URLs & fallback search engine",
        }
    }

    pub fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Notes => "📝",
            Self::CmdPal => "🎛",
            Self::Cmds => "⚡",
            Self::Settings => "⚙",
            Self::Apps => "❖",
            Self::Files => "📄",
            Self::Web => "🌐",
        }
    }
}

#[derive(Debug, Clone)]
pub enum NoteActionKind {
    AppendLine {
        file_path: PathBuf,
        content: String,
    },
    OverwriteFile {
        file_path: PathBuf,
        content: String,
    },
    DeleteLastNote {
        file_path: PathBuf,
    },
    ClearFile {
        file_path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub enum ConfigMutation {
    ToggleStartup(bool),
    SetHotkey(String),
    SetDefaultSearchEngine(String),
    SwitchTheme(String),
}

#[derive(Debug, Clone)]
pub enum ExecutionTarget {
    SystemProcess {
        program: String,
        args: Vec<String>,
    },
    CustomCommand(String),
    SettingsUri(String),
    WebUrl(String),
    AppPath(PathBuf),
    FilePath(PathBuf),
    CopyToClipboard(String),
    MutateConfig(ConfigMutation),
    ReloadIndex,
    OpenPaletteSettingsTab(i32),
    ActivateBang {
        bang: String,
        label: String,
    },
    NoteAction(NoteActionKind),
    NoOp,
}

#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    HidePalette,
    KeepOpenAndRefresh { clear_query: bool },
    ActivateBang { bang: String, label: String },
    OpenSettingsTab(i32),
    ApplyConfigMutation(ConfigMutation),
    ReloadAll,
}

#[derive(Debug, Clone)]
pub struct CandidateItem {
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub icon_override: Option<String>,
    pub category: ShelfCategory,
    pub can_elevate: bool,
    pub score: i64,
    pub target: ExecutionTarget,
}

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub name_lower: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ScannedFileEntry {
    pub display_name: String,
    pub name_lower: String,
    pub path: PathBuf,
}
