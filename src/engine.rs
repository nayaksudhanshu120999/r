use crate::config::AppConfig;
use crate::models::{AppEntry, CandidateItem, ExecutionTarget, ShelfCategory};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

const MAX_APP_RESULTS: usize = 24;

struct StaticCommandSpec {
    title: &'static str,
    subtitle: &'static str,
    badge: &'static str,
    category: ShelfCategory,
    triggers: &'static [&'static str],
    target: ExecutionTarget,
}

const PRIORITY_SPECS: &[StaticCommandSpec] = &[
    // Core System Commands
    StaticCommandSpec {
        title: "Shutdown Computer",
        subtitle: "System Command • Executes shutdown /s /t 0",
        badge: "SYSTEM",
        category: ShelfCategory::Cmds,
        triggers: &["shutdown", "power off", "turn off"],
        target: ExecutionTarget::SystemProcess {
            program: "shutdown",
            args: &["/s", "/t", "0"],
        },
    },
    StaticCommandSpec {
        title: "Restart Computer",
        subtitle: "System Command • Executes shutdown /r /t 0",
        badge: "SYSTEM",
        category: ShelfCategory::Cmds,
        triggers: &["restart", "reboot"],
        target: ExecutionTarget::SystemProcess {
            program: "shutdown",
            args: &["/r", "/t", "0"],
        },
    },
    StaticCommandSpec {
        title: "Sleep Computer",
        subtitle: "System Command • Suspends system power state",
        badge: "SYSTEM",
        category: ShelfCategory::Cmds,
        triggers: &["sleep", "suspend", "hibernate"],
        target: ExecutionTarget::SystemProcess {
            program: "rundll32.exe",
            args: &["powrprof.dll,SetSuspendState", "0,1,0"],
        },
    },
    StaticCommandSpec {
        title: "Lock Workstation",
        subtitle: "System Command • Locks current Windows session",
        badge: "SYSTEM",
        category: ShelfCategory::Cmds,
        triggers: &["lock", "lock screen", "lock pc"],
        target: ExecutionTarget::SystemProcess {
            program: "rundll32.exe",
            args: &["user32.dll,LockWorkStation"],
        },
    },
    // Core Windows Settings
    StaticCommandSpec {
        title: "Display Settings",
        subtitle: "Windows Settings • ms-settings:display",
        badge: "SETTINGS",
        category: ShelfCategory::Settings,
        triggers: &["display settings", "display", "resolution", "monitor"],
        target: ExecutionTarget::SettingsUri("ms-settings:display"),
    },
    StaticCommandSpec {
        title: "Network Settings",
        subtitle: "Windows Settings • ms-settings:network",
        badge: "SETTINGS",
        category: ShelfCategory::Settings,
        triggers: &["network settings", "network", "ethernet", "internet"],
        target: ExecutionTarget::SettingsUri("ms-settings:network"),
    },
    StaticCommandSpec {
        title: "Wi-Fi Settings",
        subtitle: "Windows Settings • ms-settings:network-wifi",
        badge: "SETTINGS",
        category: ShelfCategory::Settings,
        triggers: &["wifi", "wi-fi", "wireless", "wifi settings"],
        target: ExecutionTarget::SettingsUri("ms-settings:network-wifi"),
    },
    StaticCommandSpec {
        title: "Windows Update",
        subtitle: "Windows Settings • ms-settings:windowsupdate",
        badge: "SETTINGS",
        category: ShelfCategory::Settings,
        triggers: &["windows update", "update", "check for updates"],
        target: ExecutionTarget::SettingsUri("ms-settings:windowsupdate"),
    },
];

pub struct QueryEngine {
    config: AppConfig,
    apps: Vec<AppEntry>,
    matcher: SkimMatcherV2,
}

impl QueryEngine {
    pub fn new(config: AppConfig, apps: Vec<AppEntry>) -> Self {
        Self {
            config,
            apps,
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn evaluate(&self, raw_input: &str, shelf: ShelfCategory) -> Vec<CandidateItem> {
        let trimmed = raw_input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let normalized = trimmed.to_ascii_lowercase();

        // ====================================================================
        // PRIORITY 1: System Commands & Windows Settings Priority Engine
        // ====================================================================
        if matches!(
            shelf,
            ShelfCategory::All | ShelfCategory::Cmds | ShelfCategory::Settings
        ) {
            let mut priority_matches = self.match_system_and_settings(&normalized, shelf);
            if !priority_matches.is_empty() {
                priority_matches.sort_by(|a, b| b.score.cmp(&a.score));
                return priority_matches;
            }
        }

        // ====================================================================
        // PRIORITY 2: Dynamic Prefix-Free Bang Search Engine
        // ====================================================================
        if matches!(shelf, ShelfCategory::All | ShelfCategory::Web) {
            if let Some(bang_item) = self.match_prefix_free_bang(trimmed) {
                return vec![bang_item];
            }
        }

        // ====================================================================
        // PRIORITY 3: Native App Fuzzy Launcher (Fallback Core)
        // ====================================================================
        let mut results = Vec::new();

        if matches!(shelf, ShelfCategory::All | ShelfCategory::Apps) {
            let mut app_matches: Vec<CandidateItem> = self
                .apps
                .iter()
                .filter_map(|app| {
                    let score = self.matcher.fuzzy_match(&app.name, trimmed)?;
                    Some(CandidateItem {
                        title: app.name.clone(),
                        subtitle: app.display_path.clone(),
                        badge: "APP",
                        category: ShelfCategory::Apps,
                        can_elevate: true,
                        score,
                        target: ExecutionTarget::AppPath(app.path.clone()),
                    })
                })
                .collect();

            app_matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
            app_matches.truncate(MAX_APP_RESULTS);
            results.extend(app_matches);
        }

        // If a specific shelf (Cmds or Settings) is active and exact keyword didn't trigger,
        // allow fuzzy matching against the static system/settings catalog for that shelf.
        if results.is_empty()
            && matches!(shelf, ShelfCategory::Cmds | ShelfCategory::Settings)
        {
            for spec in PRIORITY_SPECS.iter().filter(|s| s.category == shelf) {
                if let Some(score) = self.matcher.fuzzy_match(spec.title, trimmed) {
                    results.push(CandidateItem {
                        title: spec.title.to_string(),
                        subtitle: spec.subtitle.to_string(),
                        badge: spec.badge,
                        category: spec.category,
                        can_elevate: true,
                        score,
                        target: spec.target.clone(),
                    });
                }
            }
            results.sort_by(|a, b| b.score.cmp(&a.score));
        }

        // ====================================================================
        // SECONDARY FALLBACK: Default Global Google Web Search
        // ====================================================================
        if results.is_empty() && matches!(shelf, ShelfCategory::All | ShelfCategory::Web) {
            let encoded = urlencoding::encode(trimmed);
            let google_url = format!("https://www.google.com/search?q={}", encoded);
            results.push(CandidateItem {
                title: format!("Search Google for \"{}\"", trimmed),
                subtitle: google_url.clone(),
                badge: "WEB",
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 1,
                target: ExecutionTarget::WebUrl(google_url),
            });
        }

        results
    }

    fn match_system_and_settings(
        &self,
        normalized_query: &str,
        shelf: ShelfCategory,
    ) -> Vec<CandidateItem> {
        // If input starts with a configured bang shortcut followed by space, do not hijack it
        if let Some((first_token, rest)) = normalized_query.split_once(' ') {
            if !rest.trim().is_empty() && self.config.bangs.contains_key(first_token) {
                return Vec::new();
            }
        }

        let mut matches = Vec::new();

        for spec in PRIORITY_SPECS {
            if shelf != ShelfCategory::All && spec.category != shelf {
                continue;
            }

            let mut best_score: Option<i64> = None;

            for &trigger in spec.triggers {
                if normalized_query == trigger {
                    best_score = Some(1000);
                    break;
                } else if trigger.starts_with(normalized_query) && normalized_query.len() >= 3 {
                    best_score = Some(best_score.unwrap_or(0).max(920));
                } else if normalized_query.contains(trigger) {
                    best_score = Some(best_score.unwrap_or(0).max(880));
                } else if normalized_query
                    .split_whitespace()
                    .any(|word| word == trigger || (word.len() >= 4 && trigger.starts_with(word)))
                {
                    best_score = Some(best_score.unwrap_or(0).max(840));
                }
            }

            if let Some(score) = best_score {
                matches.push(CandidateItem {
                    title: spec.title.to_string(),
                    subtitle: spec.subtitle.to_string(),
                    badge: spec.badge,
                    category: spec.category,
                    can_elevate: true,
                    score,
                    target: spec.target.clone(),
                });
            }
        }

        matches
    }

    fn match_prefix_free_bang(&self, trimmed_input: &str) -> Option<CandidateItem> {
        let (first_token, remaining_payload) = trimmed_input.split_once(' ')?;
        let payload = remaining_payload.trim();
        if payload.is_empty() {
            return None;
        }

        let token_key = first_token.to_ascii_lowercase();
        let url_template = self.config.bangs.get(&token_key)?;

        let encoded_payload = urlencoding::encode(payload);
        let target_url = if url_template.contains("{q}") {
            url_template.replace("{q}", &encoded_payload)
        } else {
            format!("{}{}", url_template, encoded_payload)
        };

        Some(CandidateItem {
            title: format!("[{}] Search: {}", token_key.to_ascii_uppercase(), payload),
            subtitle: target_url.clone(),
            badge: "BANG",
            category: ShelfCategory::Web,
            can_elevate: false,
            score: 950,
            target: ExecutionTarget::WebUrl(target_url),
        })
    }
}
