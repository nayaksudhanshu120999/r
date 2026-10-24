use crate::config::AppConfig;
use crate::models::{AppEntry, CandidateItem, ExecutionTarget, ScannedFileEntry, ShelfCategory};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

const MAX_TOTAL_RESULTS: usize = 24;

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
    // Core System Power Commands Only (CMD & PowerShell removed from System Commands)
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

    /// Detects when the user types `<bang> ` (e.g. `"yt "`) to activate the Bang Pill Chip.
    pub fn extract_bang_prefix(&self, raw_input: &str) -> Option<(String, String)> {
        let (first_token, remainder) = raw_input.split_once(char::is_whitespace)?;
        let key = first_token.trim().to_ascii_lowercase();
        if self.config.bangs.contains_key(&key) {
            Some((key, remainder.trim_start().to_string()))
        } else {
            None
        }
    }

    pub fn evaluate_with_bang(&self, active_bang: &str, raw_input: &str) -> Vec<CandidateItem> {
        let bang_key = active_bang.trim().to_ascii_lowercase();

        if !bang_key.is_empty() {
            if let Some(url_template) = self.config.bangs.get(&bang_key) {
                let query = raw_input.trim();
                let encoded = urlencoding::encode(query);
                let target_url = url_template.replace("{q}", &encoded);
                let title = if query.is_empty() {
                    format!(
                        "Type a query to search {}...",
                        bang_key.to_ascii_uppercase()
                    )
                } else {
                    format!("Search {} for \"{}\"", bang_key.to_ascii_uppercase(), query)
                };

                return vec![CandidateItem {
                    title,
                    badge: "BANG",
                    category: ShelfCategory::Web,
                    can_elevate: false,
                    score: 10_000,
                    target: ExecutionTarget::WebUrl(target_url),
                }];
            }
        }

        let trimmed = raw_input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let normalized = trimmed.to_ascii_lowercase();

        let mut all_matches = Vec::with_capacity(32);
        all_matches.extend(self.match_commands(&normalized, trimmed));
        all_matches.extend(self.match_settings(&normalized, trimmed));
        all_matches.extend(self.match_apps(&normalized, trimmed));
        all_matches.extend(self.match_custom_files(&normalized, trimmed));

        let cat_order = self.config.ordered_categories();
        let cat_rank = |cat: ShelfCategory| -> usize {
            cat_order.iter().position(|&c| c == cat).unwrap_or(99)
        };

        all_matches.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| cat_rank(a.category).cmp(&cat_rank(b.category)))
                .then_with(|| a.title.cmp(&b.title))
        });

        all_matches.truncate(MAX_TOTAL_RESULTS);

        let encoded = urlencoding::encode(trimmed);
        let google_url = format!("https://www.google.com/search?q={}", encoded);
        all_matches.push(CandidateItem {
            title: format!("Search Google for \"{}\"", trimmed),
            badge: "WEB",
            category: ShelfCategory::Web,
            can_elevate: false,
            score: 0,
            target: ExecutionTarget::WebUrl(google_url),
        });

        all_matches
    }

    fn compute_fuzzy_score(
        &self,
        candidate_name: &str,
        candidate_lower: &str,
        query_lower: &str,
        raw_query: &str,
    ) -> Option<i64> {
        let mut score = self.matcher.fuzzy_match(candidate_name, raw_query)?;

        if candidate_lower == query_lower {
            score += 500;
        } else if candidate_lower.starts_with(query_lower) {
            score += 220 - (candidate_lower.len() as i64).min(80);
        } else if candidate_lower.contains(query_lower) {
            score += 90 - (candidate_lower.len() as i64).min(50);
        }

        Some(score)
    }

    fn match_apps(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        if !self.config.show_apps {
            return Vec::new();
        }

        self.apps
            .iter()
            .filter(|app| !self.config.is_app_hidden(&app.name))
            .filter_map(|app| {
                let score = self.compute_fuzzy_score(
                    &app.name,
                    &app.name_lower,
                    normalized_query,
                    raw_query,
                )?;
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

    fn match_custom_files(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        self.custom_files
            .iter()
            .filter_map(|file| {
                let score = self.compute_fuzzy_score(
                    &file.display_name,
                    &file.name_lower,
                    normalized_query,
                    raw_query,
                )?;
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

        for spec in PRIORITY_SPECS
            .iter()
            .filter(|s| s.category == ShelfCategory::Cmds)
        {
            if let Some(score) =
                self.score_triggers_and_title(normalized_query, raw_query, spec.title, spec.triggers)
            {
                results.push(CandidateItem {
                    title: spec.title.to_string(),
                    badge: spec.badge,
                    category: ShelfCategory::Cmds,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::SystemProcess {
                        program: spec.program.to_string(),
                        args: spec.args.iter().map(|s| (*s).to_string()).collect(),
                    },
                });
            }
        }

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
        let title_lower = title.to_ascii_lowercase();
        let mut best_score =
            self.compute_fuzzy_score(title, &title_lower, normalized_query, raw_query);

        for &trigger in triggers {
            let t_lower = trigger.to_ascii_lowercase();
            if let Some(s) =
                self.compute_fuzzy_score(trigger, &t_lower, normalized_query, raw_query)
            {
                best_score = Some(best_score.map_or(s, |curr| curr.max(s)));
            }
        }

        best_score.filter(|&s| normalized_query.len() >= 2 || s >= 120)
    }
}
