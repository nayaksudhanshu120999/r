use crate::config::AppConfig;
use crate::models::{AppEntry, CandidateItem, ExecutionTarget, ScannedFileEntry, ShelfCategory};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

const MAX_APP_RESULTS: usize = 14;
const MAX_FILE_RESULTS: usize = 12;

struct StaticCommandSpec {
    title: &'static str,
    badge: &'static str,
    category: ShelfCategory,
    triggers: &'static [&'static str],
    program: &'static str,
    args: &'static [&'static str],
    settings_uri: Option<&'static str>,
    is_palette_settings: bool,
}

const PRIORITY_SPECS: &[StaticCommandSpec] = &[
    StaticCommandSpec {
        title: "Command Palette Settings",
        badge: "SETTINGS",
        category: ShelfCategory::Settings,
        triggers: &[
            "palette settings",
            "preferences",
            "configure palette",
            "cmdpal settings",
            "theme",
        ],
        program: "",
        args: &[],
        settings_uri: None,
        is_palette_settings: true,
    },
    // Core System Commands
    StaticCommandSpec {
        title: "Open Command Prompt (CMD)",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["cmd", "command prompt", "terminal", "shell"],
        program: "cmd.exe",
        args: &[],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Open PowerShell",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["powershell", "pwsh", "ps"],
        program: "powershell.exe",
        args: &[],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Task Manager",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["task manager", "taskmgr", "processes"],
        program: "taskmgr.exe",
        args: &[],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Control Panel",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["control panel", "control"],
        program: "control.exe",
        args: &[],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Shutdown Computer",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["shutdown", "power off", "turn off"],
        program: "shutdown",
        args: &["/s", "/t", "0"],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Restart Computer",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["restart", "reboot"],
        program: "shutdown",
        args: &["/r", "/t", "0"],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Sleep Computer",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["sleep", "suspend", "hibernate"],
        program: "rundll32.exe",
        args: &["powrprof.dll,SetSuspendState", "0,1,0"],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Lock Workstation",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["lock", "lock screen", "lock pc"],
        program: "rundll32.exe",
        args: &["user32.dll,LockWorkStation"],
        settings_uri: None,
        is_palette_settings: false,
    },
    // Core Windows Settings
    StaticCommandSpec {
        title: "System Settings Home",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["system settings", "windows settings", "settings"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Display Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["display settings", "display", "resolution", "monitor"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:display"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Sound & Audio Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["sound settings", "sound", "audio", "volume", "microphone"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:sound"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Network & Internet Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["network settings", "network", "ethernet", "internet"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:network"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Wi-Fi Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["wifi", "wi-fi", "wireless", "wifi settings"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:network-wifi"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Bluetooth & Devices",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["bluetooth", "devices", "bluetooth settings"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:bluetooth"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Installed Apps & Features",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["installed apps", "apps features", "add or remove programs"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:appsfeatures"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Windows Update",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["windows update", "update", "check for updates"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:windowsupdate"),
        is_palette_settings: false,
    },
];

pub struct QueryEngine {
    pub config: AppConfig,
    pub apps: Vec<AppEntry>,
    pub custom_files: Vec<ScannedFileEntry>,
    matcher: SkimMatcherV2,
}

impl QueryEngine {
    pub fn new(
        config: AppConfig,
        apps: Vec<AppEntry>,
        custom_files: Vec<ScannedFileEntry>,
    ) -> Self {
        Self {
            config,
            apps,
            custom_files,
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn evaluate(&self, raw_input: &str) -> Vec<CandidateItem> {
        let trimmed = raw_input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let normalized = trimmed.to_ascii_lowercase();

        let mut cmds_list = self.match_commands(&normalized, trimmed);
        let mut settings_list = self.match_settings(&normalized, trimmed);
        let mut apps_list = self.match_apps(trimmed);
        let mut files_list = self.match_custom_files(trimmed);
        let mut bang_list = Vec::new();

        if let Some(bang_item) = self.match_prefix_free_bang(trimmed) {
            bang_list.push(bang_item);
        }

        cmds_list.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        settings_list.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        apps_list.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        apps_list.truncate(MAX_APP_RESULTS);
        files_list.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        files_list.truncate(MAX_FILE_RESULTS);

        let mut ordered_results = Vec::new();

        let has_explicit_bang = !bang_list.is_empty();
        if has_explicit_bang {
            ordered_results.extend(bang_list.drain(..));
        }

        for category in self.config.ordered_categories() {
            match category {
                ShelfCategory::Cmds => ordered_results.append(&mut cmds_list),
                ShelfCategory::Settings => ordered_results.append(&mut settings_list),
                ShelfCategory::Apps => ordered_results.append(&mut apps_list),
                ShelfCategory::Files => ordered_results.append(&mut files_list),
                ShelfCategory::Web => {
                    if !has_explicit_bang {
                        ordered_results.append(&mut bang_list);
                    }
                }
            }
        }

        if !has_explicit_bang {
            let encoded = urlencoding::encode(trimmed);
            let google_url = format!("https://www.google.com/search?q={}", encoded);
            ordered_results.push(CandidateItem {
                title: format!("Search Google for \"{}\"", trimmed),
                badge: "WEB",
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 1,
                target: ExecutionTarget::WebUrl(google_url),
            });
        }

        ordered_results
    }

    fn match_apps(&self, trimmed: &str) -> Vec<CandidateItem> {
        if !self.config.show_apps {
            return Vec::new();
        }

        self.apps
            .iter()
            .filter(|app| !self.config.is_app_hidden(&app.name))
            .filter_map(|app| {
                let score = self.matcher.fuzzy_match(&app.name, trimmed)?;
                Some(CandidateItem {
                    title: app.name.clone(),
                    badge: "APP",
                    category: ShelfCategory::Apps,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::AppPath(app.path.clone()),
                })
            })
            .collect()
    }

    fn match_custom_files(&self, trimmed: &str) -> Vec<CandidateItem> {
        self.custom_files
            .iter()
            .filter_map(|file| {
                let score = self.matcher.fuzzy_match(&file.display_name, trimmed)?;
                Some(CandidateItem {
                    title: file.display_name.clone(),
                    badge: "FILE",
                    category: ShelfCategory::Files,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::FilePath(file.path.clone()),
                })
            })
            .collect()
    }

    fn match_commands(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        let mut results = Vec::new();

        // 1. Built-in commands
        for spec in PRIORITY_SPECS
            .iter()
            .filter(|s| s.category == ShelfCategory::Cmds)
        {
            if let Some(score) =
                self.score_triggers_and_title(normalized_query, raw_query, spec.title, spec.triggers)
            {
                let program = if spec.program.eq_ignore_ascii_case("cmd.exe")
                    || spec.program.eq_ignore_ascii_case("cmd")
                {
                    self.config.custom_cmd_path.clone()
                } else {
                    spec.program.to_string()
                };

                results.push(CandidateItem {
                    title: spec.title.to_string(),
                    badge: spec.badge,
                    category: ShelfCategory::Cmds,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::SystemProcess {
                        program,
                        args: spec.args.iter().map(|s| (*s).to_string()).collect(),
                    },
                });
            }
        }

        // 2. Simplified user-defined Custom Commands
        for custom in &self.config.custom_commands {
            let trigger_slice = [custom.name.as_str()];
            if let Some(score) = self.score_triggers_and_title(
                normalized_query,
                raw_query,
                &custom.name,
                &trigger_slice,
            ) {
                results.push(CandidateItem {
                    title: custom.name.clone(),
                    badge: "COMMAND",
                    category: ShelfCategory::Cmds,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::CustomCommand(custom.command.clone()),
                });
            }
        }

        results
    }

    fn match_settings(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        let mut results = Vec::new();

        // 1. Built-in settings
        for spec in PRIORITY_SPECS
            .iter()
            .filter(|s| s.category == ShelfCategory::Settings)
        {
            if let Some(score) =
                self.score_triggers_and_title(normalized_query, raw_query, spec.title, spec.triggers)
            {
                let target = if spec.is_palette_settings {
                    ExecutionTarget::OpenPaletteSettings
                } else {
                    let uri = spec.settings_uri.unwrap_or("ms-settings:");
                    let resolved_uri = if uri == "ms-settings:" {
                        self.config.custom_system_settings_path.clone()
                    } else {
                        uri.to_string()
                    };
                    ExecutionTarget::SettingsUri(resolved_uri)
                };

                results.push(CandidateItem {
                    title: spec.title.to_string(),
                    badge: spec.badge,
                    category: ShelfCategory::Settings,
                    can_elevate: false,
                    score,
                    target,
                });
            }
        }

        // 2. Simplified user-defined Custom Settings
        for custom in &self.config.custom_settings {
            let trigger_slice = [custom.name.as_str()];
            if let Some(score) = self.score_triggers_and_title(
                normalized_query,
                raw_query,
                &custom.name,
                &trigger_slice,
            ) {
                results.push(CandidateItem {
                    title: custom.name.clone(),
                    badge: "SETTING",
                    category: ShelfCategory::Settings,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::SettingsUri(custom.path.clone()),
                });
            }
        }

        results
    }

    fn score_triggers_and_title(
        &self,
        normalized_query: &str,
        raw_query: &str,
        title: &str,
        triggers: &[&str],
    ) -> Option<i64> {
        let mut best_score: Option<i64> = None;

        for trigger in triggers {
            let t_low = trigger.to_ascii_lowercase();
            let s = if t_low == normalized_query {
                Some(20_000)
            } else if t_low.starts_with(normalized_query) && normalized_query.len() >= 2 {
                Some(12_000 - (t_low.len() as i64))
            } else if normalized_query.len() >= 3 && t_low.contains(normalized_query) {
                Some(8_000 - (t_low.len() as i64))
            } else {
                None
            };

            if let Some(val) = s {
                best_score = Some(best_score.map_or(val, |curr| curr.max(val)));
            }
        }

        if best_score.is_none() && normalized_query.len() >= 2 {
            if let Some(fuzzy_s) = self.matcher.fuzzy_match(title, raw_query) {
                if fuzzy_s >= 45 {
                    best_score = Some(fuzzy_s);
                }
            }
        }

        best_score
    }

    fn match_prefix_free_bang(&self, trimmed_input: &str) -> Option<CandidateItem> {
        let (first_token, remainder) = trimmed_input.split_once(char::is_whitespace)?;
        let search_payload = remainder.trim();
        if search_payload.is_empty() {
            return None;
        }

        let lookup_key = first_token.to_ascii_lowercase();
        let url_template = self.config.bangs.get(&lookup_key)?;

        let encoded_payload = urlencoding::encode(search_payload);
        let target_url = url_template.replace("{q}", &encoded_payload);

        Some(CandidateItem {
            title: format!(
                "[{}] Search: {}",
                lookup_key.to_ascii_uppercase(),
                search_payload
            ),
            badge: "BANG",
            category: ShelfCategory::Web,
            can_elevate: false,
            score: 15_000,
            target: ExecutionTarget::WebUrl(target_url),
        })
    }
}
