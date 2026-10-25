use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShelfCategory {
    Extensions,
    CmdPal,
    Cmds,
    Settings,
    Apps,
    Files,
    Web,
}

impl ShelfCategory {
    pub fn all_default_order() -> [Self; 7] {
        [
            Self::Extensions,
            Self::CmdPal,
            Self::Cmds,
            Self::Settings,
            Self::Apps,
            Self::Files,
            Self::Web,
        ]
    }

    pub fn from_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "extensions" | "extension" | "ext" | "plugins" => Self::Extensions,
            "cmdpal" | "palette" | "cmdpal_settings" | "palettesettings" => Self::CmdPal,
            "cmds" | "cmd" | "commands" | "command" => Self::Cmds,
            "settings" | "setting" | "system_settings" => Self::Settings,
            "apps" | "app" | "applications" => Self::Apps,
            "files" | "file" => Self::Files,
            "web" | "bang" | "search" => Self::Web,
            _ => Self::Apps,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Extensions => "Extensions",
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
            Self::Extensions => "EXTENSIONS & PLUGINS",
            Self::CmdPal => "CMDPAL SETTINGS & ACTIONS",
            Self::Cmds => "SYSTEM COMMANDS",
            Self::Settings => "WINDOWS SYSTEM SETTINGS",
            Self::Apps => "APPLICATIONS",
            Self::Files => "CUSTOM FILES",
            Self::Web => "WEB SEARCH",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Extensions => "Extensions (Notepad, Custom Bangs)",
            Self::CmdPal => "CmdPal Own Settings & Controls",
            Self::Cmds => "System Commands & Calculator",
            Self::Settings => "Windows System Settings (ms-settings)",
            Self::Apps => "Installed Applications",
            Self::Files => "Custom Scanned Files",
            Self::Web => "Web Search & Bangs",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Extensions => {
                "Modular extensions such as Quick Notepad ('note' bang) and custom plugins"
            }
            Self::CmdPal => {
                "Command Palette preferences, theme, hotkey, startup, and search engine"
            }
            Self::Cmds => {
                "Inbuilt Windows admin/power commands, calculator, and custom shell commands"
            }
            Self::Settings => {
                "Inbuilt Windows ms-settings URIs and custom control panel shortcuts"
            }
            Self::Apps => "Start Menu applications enabled in Installed Apps",
            Self::Files => "Direct file/folder paths and files from Custom File Scan paths",
            Self::Web => {
                "Direct URLs, prefix-free bang shortcuts, and fallback default search engine"
            }
        }
    }

    pub fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Extensions => "🧩",
            Self::CmdPal => "🎛",
            Self::Cmds => "⚡",
            Self::Settings => "⚙",
            Self::Apps => "❖",
            Self::Files => "📄",
            Self::Web => "🌐",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ExtensionActionKind {
    AppendNote {
        file_path: PathBuf,
        content: String,
        with_timestamp: bool,
    },
    OverwriteNote {
        file_path: PathBuf,
        content: String,
    },
    DeleteNoteLine {
        file_path: PathBuf,
        line_index: usize,
    },
    ClearNoteFile {
        file_path: PathBuf,
    },
    ToggleExtension {
        id: String,
        enable: bool,
    },
    RemoveExtension {
        id: String,
    },
    RestoreBuiltinNotepad,
}

#[allow(dead_code)]
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
    ToggleStartup(bool),
    SetHotkey(String),
    SetDefaultSearchEngine(String),
    SwitchTheme(String),
    ReloadIndex,
    OpenPaletteSettings,
    OpenPaletteSettingsTab(i32),
    ActivateBang {
        bang: String,
        label: String,
    },
    ExtensionAction(ExtensionActionKind),
    NoOp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome {
    HidePalette,
    KeepOpenAndRefresh { clear_query: bool },
    ActivateBang { bang: String, label: String },
    OpenSettingsTab(i32),
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
