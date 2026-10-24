use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShelfCategory {
    All,
    Apps,
    Cmds,
    Web,
    Settings,
}

impl ShelfCategory {
    pub fn from_label(label: &str) -> Self {
        match label {
            "Apps" => Self::Apps,
            "Cmds" => Self::Cmds,
            "Web" => Self::Web,
            "Settings" => Self::Settings,
            _ => Self::All,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Apps => "Apps",
            Self::Cmds => "Cmds",
            Self::Web => "Web",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionTarget {
    /// Executes a command program with discrete argument list
    SystemProcess {
        program: &'static str,
        args: &'static [&'static str],
    },
    /// Opens a Windows ms-settings URI
    SettingsUri(&'static str),
    /// Opens an HTTP/HTTPS URL in the system default browser
    WebUrl(String),
    /// Launches a local application shortcut (.lnk) or executable (.exe)
    AppPath(PathBuf),
}

#[derive(Debug, Clone)]
pub struct CandidateItem {
    pub title: String,
    pub subtitle: String,
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
    pub display_path: String,
}
