use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShelfCategory {
    Cmds,
    Settings,
    Apps,
    Files,
    Web,
}

impl ShelfCategory {
    pub fn from_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "cmds" | "cmd" | "commands" | "command" => Self::Cmds,
            "settings" | "setting" => Self::Settings,
            "apps" | "app" | "applications" => Self::Apps,
            "files" | "file" => Self::Files,
            "web" | "bang" | "search" => Self::Web,
            _ => Self::Apps,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cmds => "Cmds",
            Self::Settings => "Settings",
            Self::Apps => "Apps",
            Self::Files => "Files",
            Self::Web => "Web",
        }
    }

    pub fn header_title(&self) -> &'static str {
        match self {
            Self::Cmds => "SYSTEM COMMANDS",
            Self::Settings => "SYSTEM SETTINGS",
            Self::Apps => "APPLICATIONS",
            Self::Files => "CUSTOM FILES",
            Self::Web => "WEB SEARCH",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Cmds => "System Commands",
            Self::Settings => "System Settings",
            Self::Apps => "Installed Applications",
            Self::Files => "Custom Scanned Files",
            Self::Web => "Web Search & Bangs",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Cmds => "Built-in power actions, terminal commands, and custom commands",
            Self::Settings => "Windows ms-settings URIs, Control Panel, and custom settings",
            Self::Apps => "Start Menu applications enabled in the Installed Apps tab",
            Self::Files => "Files indexed exclusively from your Custom File Scan paths",
            Self::Web => "Prefix-free bang shortcuts and fallback web search",
        }
    }

    pub fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Cmds => "⚡",
            Self::Settings => "⚙",
            Self::Apps => "❖",
            Self::Files => "📄",
            Self::Web => "🌐",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionTarget {
    /// Executes a command program with discrete argument list
    SystemProcess {
        program: String,
        args: Vec<String>,
    },
    /// Executes a user-defined custom command string or executable path
    CustomCommand(String),
    /// Opens a Windows ms-settings URI or custom setting path/applet
    SettingsUri(String),
    /// Opens an HTTP/HTTPS URL in the system default browser
    WebUrl(String),
    /// Launches a local application shortcut (.lnk) or executable (.exe)
    AppPath(PathBuf),
    /// Opens a file from a custom scan path using the default system handler
    FilePath(PathBuf),
    /// Opens the built-in Command Palette Settings GUI
    OpenPaletteSettings,
}

#[derive(Debug, Clone)]
pub struct CandidateItem {
    pub title: String,
    pub badge: &'static str,
    pub category: ShelfCategory,
    pub can_elevate: bool,
    pub score: i64,
    pub target: ExecutionTarget,
}

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ScannedFileEntry {
    pub display_name: String,
    pub path: PathBuf,
}
