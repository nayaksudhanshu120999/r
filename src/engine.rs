use crate::config::AppConfig;
use crate::extensions::evaluate_extension_bang;
use crate::models::{
    AppEntry, CandidateItem, ExecutionTarget, ExtensionActionKind, ScannedFileEntry, ShelfCategory,
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::env;
use std::path::PathBuf;

const MAX_TOTAL_RESULTS: usize = 28;

pub struct BuiltinCommandSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub triggers: &'static [&'static str],
    pub program: &'static str,
    pub args: &'static [&'static str],
}

pub struct BuiltinSettingSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub triggers: &'static [&'static str],
    pub uri: &'static str,
}

pub const BUILTIN_COMMANDS: &[BuiltinCommandSpec] = &[
    BuiltinCommandSpec {
        id: "taskmgr",
        title: "Task Manager",
        subtitle: "taskmgr.exe • Monitor processes, CPU, memory & startup apps",
        triggers: &["task manager", "taskmgr", "processes", "cpu", "ram", "tm"],
        program: "taskmgr.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "control",
        title: "Control Panel",
        subtitle: "control.exe • Classic Windows Control Panel",
        triggers: &["control panel", "control", "cpl"],
        program: "control.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "devmgmt",
        title: "Device Manager",
        subtitle: "devmgmt.msc • Manage hardware drivers & devices",
        triggers: &["device manager", "devmgmt", "drivers", "hardware", "dm"],
        program: "mmc.exe",
        args: &["devmgmt.msc"],
    },
    BuiltinCommandSpec {
        id: "diskmgmt",
        title: "Disk Management",
        subtitle: "diskmgmt.msc • Partition drives and format volumes",
        triggers: &["disk management", "diskmgmt", "partitions", "drives"],
        program: "mmc.exe",
        args: &["diskmgmt.msc"],
    },
    BuiltinCommandSpec {
        id: "services",
        title: "Windows Services",
        subtitle: "services.msc • Start, stop, and configure background services",
        triggers: &["services", "services.msc", "background services"],
        program: "mmc.exe",
        args: &["services.msc"],
    },
    BuiltinCommandSpec {
        id: "regedit",
        title: "Registry Editor",
        subtitle: "regedit.exe • View and edit the Windows system registry",
        triggers: &["registry editor", "regedit", "reg"],
        program: "regedit.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "resmon",
        title: "Resource Monitor",
        subtitle: "resmon.exe • Detailed live CPU, disk, network & memory stats",
        triggers: &["resource monitor", "resmon", "performance monitor"],
        program: "resmon.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "eventvwr",
        title: "Event Viewer",
        subtitle: "eventvwr.exe • Inspect Windows system and application logs",
        triggers: &["event viewer", "eventvwr", "system logs", "logs"],
        program: "eventvwr.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "sysdm",
        title: "System Properties & Environment Variables",
        subtitle: "SystemPropertiesAdvanced.exe • Edit PATH and environment variables",
        triggers: &["environment variables", "env vars", "path", "sysdm", "system properties"],
        program: "SystemPropertiesAdvanced.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "flushdns",
        title: "Flush DNS Cache",
        subtitle: "ipconfig /flushdns • Clear Windows DNS resolver cache",
        triggers: &["flush dns", "flushdns", "clear dns", "ipconfig flushdns"],
        program: "ipconfig.exe",
        args: &["/flushdns"],
    },
    BuiltinCommandSpec {
        id: "snip",
        title: "Snipping Tool / Screen Capture",
        subtitle: "ms-screenclip: • Take a screenshot region",
        triggers: &["snip", "screenshot", "screen capture", "snipping tool"],
        program: "explorer.exe",
        args: &["ms-screenclip:"],
    },
    BuiltinCommandSpec {
        id: "recyclebin",
        title: "Open Recycle Bin",
        subtitle: "shell:RecycleBinFolder • View or restore deleted files",
        triggers: &["recycle bin", "trash", "deleted files"],
        program: "explorer.exe",
        args: &["shell:RecycleBinFolder"],
    },
    BuiltinCommandSpec {
        id: "lock",
        title: "Lock Workstation",
        subtitle: "Lock your Windows session immediately",
        triggers: &["lock", "lock screen", "lock pc", "lock workstation"],
        program: "rundll32.exe",
        args: &["user32.dll,LockWorkStation"],
    },
    BuiltinCommandSpec {
        id: "sleep",
        title: "Sleep Computer",
        subtitle: "Suspend PC into low-power sleep state",
        triggers: &["sleep", "suspend"],
        program: "rundll32.exe",
        args: &["powrprof.dll,SetSuspendState", "0,1,0"],
    },
    BuiltinCommandSpec {
        id: "hibernate",
        title: "Hibernate Computer",
        subtitle: "shutdown /h • Save session to disk and power off",
        triggers: &["hibernate", "hibernation"],
        program: "shutdown.exe",
        args: &["/h"],
    },
    BuiltinCommandSpec {
        id: "signout",
        title: "Sign Out of Windows",
        subtitle: "shutdown /l • Log off current Windows user",
        triggers: &["sign out", "log off", "logoff", "logout"],
        program: "shutdown.exe",
        args: &["/l"],
    },
    BuiltinCommandSpec {
        id: "restart",
        title: "Restart Computer",
        subtitle: "shutdown /r /t 0 • Reboot Windows immediately",
        triggers: &["restart", "reboot", "restart pc"],
        program: "shutdown.exe",
        args: &["/r", "/t", "0"],
    },
    BuiltinCommandSpec {
        id: "shutdown",
        title: "Shutdown Computer",
        subtitle: "shutdown /s /t 0 • Power off Windows immediately",
        triggers: &["shutdown", "power off", "turn off pc"],
        program: "shutdown.exe",
        args: &["/s", "/t", "0"],
    },
];

pub const BUILTIN_SETTINGS: &[BuiltinSettingSpec] = &[
    BuiltinSettingSpec {
        id: "ms_home",
        title: "Windows System Settings Home",
        subtitle: "ms-settings: • Open main Windows Settings app",
        triggers: &["windows settings", "system settings", "ms-settings"],
        uri: "ms-settings:",
    },
    BuiltinSettingSpec {
        id: "ms_display",
        title: "Display & Monitor Settings",
        subtitle: "ms-settings:display • Resolution, scale, brightness & multiple displays",
        triggers: &["display", "resolution", "monitor", "scale", "brightness", "screen"],
        uri: "ms-settings:display",
    },
    BuiltinSettingSpec {
        id: "ms_sound",
        title: "Sound & Audio Settings",
        subtitle: "ms-settings:sound • Output speakers, headphones & input microphone",
        triggers: &["sound", "audio", "volume", "microphone", "speakers", "headphones"],
        uri: "ms-settings:sound",
    },
    BuiltinSettingSpec {
        id: "ms_volume",
        title: "App Volume & Device Preferences",
        subtitle: "ms-settings:apps-volume • Per-application audio mixer",
        triggers: &["volume mixer", "app volume", "audio mixer"],
        uri: "ms-settings:apps-volume",
    },
    BuiltinSettingSpec {
        id: "ms_bluetooth",
        title: "Bluetooth & Devices",
        subtitle: "ms-settings:bluetooth • Pair headphones, mice, keyboards & controllers",
        triggers: &["bluetooth", "devices", "pair", "bt"],
        uri: "ms-settings:bluetooth",
    },
    BuiltinSettingSpec {
        id: "ms_wifi",
        title: "Wi-Fi Settings",
        subtitle: "ms-settings:network-wifi • Wireless networks & connections",
        triggers: &["wifi", "wi-fi", "wireless"],
        uri: "ms-settings:network-wifi",
    },
    BuiltinSettingSpec {
        id: "ms_network",
        title: "Network & Internet Settings",
        subtitle: "ms-settings:network • Ethernet, VPN, proxy & network status",
        triggers: &["network", "internet", "ethernet", "vpn", "ip"],
        uri: "ms-settings:network",
    },
    BuiltinSettingSpec {
        id: "ms_apps",
        title: "Installed Apps & Features",
        subtitle: "ms-settings:appsfeatures • Uninstall or manage Windows applications",
        triggers: &["installed apps", "apps features", "uninstall", "add or remove programs"],
        uri: "ms-settings:appsfeatures",
    },
    BuiltinSettingSpec {
        id: "ms_startup",
        title: "Windows Startup Apps",
        subtitle: "ms-settings:startupapps • Control which apps launch at Windows login",
        triggers: &["startup apps", "windows startup", "boot apps"],
        uri: "ms-settings:startupapps",
    },
    BuiltinSettingSpec {
        id: "ms_defaultapps",
        title: "Default Apps",
        subtitle: "ms-settings:defaultapps • Set default browser, PDF viewer & file associations",
        triggers: &["default apps", "default browser", "file associations"],
        uri: "ms-settings:defaultapps",
    },
    BuiltinSettingSpec {
        id: "ms_update",
        title: "Windows Update",
        subtitle: "ms-settings:windowsupdate • Check for OS patches and driver updates",
        triggers: &["windows update", "update", "check for updates", "wu"],
        uri: "ms-settings:windowsupdate",
    },
    BuiltinSettingSpec {
        id: "ms_battery",
        title: "Power & Battery Settings",
        subtitle: "ms-settings:powersleep • Power mode, battery saver & screen sleep",
        triggers: &["power", "battery", "power plan", "sleep timeout"],
        uri: "ms-settings:powersleep",
    },
    BuiltinSettingSpec {
        id: "ms_storage",
        title: "Storage Sense & Disk Space",
        subtitle: "ms-settings:storagesense • Free up disk space and temporary files",
        triggers: &["storage", "disk space", "free up space", "storage sense"],
        uri: "ms-settings:storagesense",
    },
    BuiltinSettingSpec {
        id: "ms_personalize",
        title: "Personalization, Wallpaper & Themes",
        subtitle: "ms-settings:personalization • Background, colors & lock screen",
        triggers: &["personalization", "wallpaper", "background", "colors", "dark mode"],
        uri: "ms-settings:personalization",
    },
    BuiltinSettingSpec {
        id: "ms_taskbar",
        title: "Taskbar Settings",
        subtitle: "ms-settings:taskbar • Taskbar alignment, tray icons & behavior",
        triggers: &["taskbar", "system tray", "taskbar settings"],
        uri: "ms-settings:taskbar",
    },
    BuiltinSettingSpec {
        id: "ms_nightlight",
        title: "Night Light Settings",
        subtitle: "ms-settings:nightlight • Blue light filter and schedule",
        triggers: &["night light", "blue light", "eye care"],
        uri: "ms-settings:nightlight",
    },
    BuiltinSettingSpec {
        id: "ms_privacy",
        title: "Privacy & Security",
        subtitle: "ms-settings:privacy • Camera, microphone & app permissions",
        triggers: &["privacy", "security", "permissions", "camera", "defender"],
        uri: "ms-settings:privacy",
    },
    BuiltinSettingSpec {
        id: "ms_datetime",
        title: "Date, Time & Time Zone",
        subtitle: "ms-settings:dateandtime • Sync clock, time zone & region",
        triggers: &["date and time", "time zone", "clock"],
        uri: "ms-settings:dateandtime",
    },
    BuiltinSettingSpec {
        id: "ms_about",
        title: "About PC & System Specifications",
        subtitle: "ms-settings:about • CPU, RAM, Windows edition & device rename",
        triggers: &["about pc", "system info", "pc specs", "specifications"],
        uri: "ms-settings:about",
    },
];

const HOTKEY_PRESETS: &[&str] = &[
    "Ctrl+Space",
    "Alt+Space",
    "Ctrl+Shift+Space",
    "Ctrl+Alt+Space",
    "Alt+Q",
];

const COMMON_WEB_TLDS: &[&str] = &[
    "com", "org", "net", "io", "rs", "dev", "ai", "in", "co", "edu", "gov", "me", "app", "gg",
    "tv", "xyz", "info", "sh", "tech", "cloud", "page", "site", "uk", "de", "fr", "jp", "ca", "au",
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

    /// Detects when the user types `<bang> ` (e.g. `"note "` or `"yt "`) to activate the Bang Pill Chip.
    /// Returns `(bang_key, pill_display_label, remainder_query)`.
    pub fn extract_bang_prefix(&self, raw_input: &str) -> Option<(String, String, String)> {
        let (first_token, remainder) = raw_input.split_once(char::is_whitespace)?;
        let key = first_token
            .trim()
            .trim_start_matches('!')
            .to_ascii_lowercase();
        if key.is_empty() {
            return None;
        }

        // 1. Check enabled Extensions first (e.g. `note`)
        if let Some(ext) = self
            .config
            .extensions
            .iter()
            .find(|e| e.enabled && e.bang.eq_ignore_ascii_case(&key))
        {
            let label = format!("{} {}", ext.icon, ext.bang.to_ascii_uppercase());
            return Some((key, label, remainder.trim_start().to_string()));
        }

        // 2. Check Web Search Bangs
        if self.config.bangs.contains_key(&key) {
            let label = format!("🌐 {}", key.to_ascii_uppercase());
            return Some((key, label, remainder.trim_start().to_string()));
        }

        None
    }

    pub fn evaluate_with_bang(&self, active_bang: &str, raw_input: &str) -> Vec<CandidateItem> {
        let bang_key = active_bang.trim().to_ascii_lowercase();
        let trimmed = raw_input.trim();

        // 1. If a Bang Pill Chip is currently active:
        if !bang_key.is_empty() {
            // Check if it's an Extension Bang (such as `note`)
            if let Some(ext) = self
                .config
                .extensions
                .iter()
                .find(|e| e.enabled && e.bang.eq_ignore_ascii_case(&bang_key))
            {
                return evaluate_extension_bang(ext, trimmed);
            }

            // Otherwise check Web Search Bangs
            if let Some(template) = self.config.bangs.get(&bang_key) {
                let encoded = urlencoding::encode(trimmed);
                let target_url = template.replace("{q}", encoded.as_ref());
                let title = if trimmed.is_empty() {
                    format!(
                        "Type a search query for [{}]...",
                        bang_key.to_ascii_uppercase()
                    )
                } else {
                    format!(
                        "Search {} for \"{}\"",
                        bang_key.to_ascii_uppercase(),
                        trimmed
                    )
                };

                return vec![CandidateItem {
                    title,
                    subtitle: target_url.clone(),
                    badge: "BANG".to_string(),
                    icon_override: Some("🌐".to_string()),
                    category: ShelfCategory::Web,
                    can_elevate: false,
                    score: 9_900,
                    target: ExecutionTarget::WebUrl(target_url),
                }];
            }
        }

        if trimmed.is_empty() {
            return Vec::new();
        }

        let mut high_priority = Vec::new();

        // 2. Inline Math & Programmer Calculator
        if let Some(calc_item) = evaluate_calculator(trimmed) {
            high_priority.push(calc_item);
        }

        // 3. Direct File/Folder Path Detection
        if let Some(path_item) = self.detect_direct_path(trimmed) {
            high_priority.push(path_item);
        }

        // 4. Direct URL / Domain Detection
        if let Some(url_item) = self.detect_direct_url(trimmed) {
            high_priority.push(url_item);
        }

        // 5. Gather candidates per separated shelf category
        let q_lower = trimmed.to_ascii_lowercase();

        let ext_matches = self.match_extensions(trimmed, &q_lower);
        let cmdpal_matches = self.match_cmdpal_settings(trimmed, &q_lower);
        let cmd_matches = self.match_commands(trimmed, &q_lower);
        let setting_matches = self.match_windows_settings(trimmed, &q_lower);
        let app_matches = if self.config.show_apps {
            self.match_apps(trimmed, &q_lower)
        } else {
            Vec::new()
        };
        let file_matches = self.match_custom_files(trimmed, &q_lower);

        // 6. Smart Hybrid Ordering:
        // Group by category, order categories by highest relevance tier first (with user's
        // configured `category_order` as tie-breaker), so an exact/prefix match in Apps or
        // Extensions is never buried underneath a weak match in another category.
        let user_order = self.config.ordered_categories();
        let mut category_buckets: Vec<(ShelfCategory, usize, i64, Vec<CandidateItem>)> =
            Vec::with_capacity(7);

        for (user_rank, cat) in user_order.iter().copied().enumerate() {
            let bucket = match cat {
                ShelfCategory::Extensions => ext_matches.clone(),
                ShelfCategory::CmdPal => cmdpal_matches.clone(),
                ShelfCategory::Cmds => cmd_matches.clone(),
                ShelfCategory::Settings => setting_matches.clone(),
                ShelfCategory::Apps => app_matches.clone(),
                ShelfCategory::Files => file_matches.clone(),
                ShelfCategory::Web => Vec::new(),
            };
            if !bucket.is_empty() {
                let top_score = bucket.iter().map(|c| c.score).max().unwrap_or(0);
                category_buckets.push((cat, user_rank, top_score, bucket));
            }
        }

        category_buckets.sort_by(|a, b| {
            let tier_a = a.2 / 350;
            let tier_b = b.2 / 350;
            tier_b
                .cmp(&tier_a)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| b.2.cmp(&a.2))
        });

        let mut results = high_priority;
        for (_, _, _, bucket) in category_buckets {
            for item in bucket {
                if results.len() < MAX_TOTAL_RESULTS.saturating_sub(1) {
                    results.push(item);
                }
            }
        }

        // 7. Fallback Web Search using the user's configured Default Search Engine
        let (engine_name, search_template) = self.config.resolve_default_search_engine();
        let encoded_query = urlencoding::encode(trimmed);
        let fallback_url = if search_template.contains("{q}") {
            search_template.replace("{q}", encoded_query.as_ref())
        } else {
            format!("{}{}", search_template, encoded_query)
        };

        results.push(CandidateItem {
            title: format!("Search {} for \"{}\"", engine_name, trimmed),
            subtitle: format!("Default Search Engine ({}) • {}", engine_name, fallback_url),
            badge: "SEARCH".to_string(),
            icon_override: Some("🌐".to_string()),
            category: ShelfCategory::Web,
            can_elevate: false,
            score: 10,
            target: ExecutionTarget::WebUrl(fallback_url),
        });

        results
    }

    fn detect_direct_path(&self, trimmed: &str) -> Option<CandidateItem> {
        let unquoted = trimmed.trim_matches('"');
        let bytes = unquoted.as_bytes();
        let is_drive_path = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/');
        let is_unc_path = unquoted.starts_with(r"\\");
        let is_env_or_home = unquoted.starts_with('%') || unquoted.starts_with('~');

        if !is_drive_path && !is_unc_path && !is_env_or_home {
            return None;
        }

        let expanded = expand_env_and_tilde(unquoted);
        let path = PathBuf::from(&expanded);
        if path.exists() {
            let kind_badge = if path.is_dir() { "FOLDER" } else { "FILE" };
            Some(CandidateItem {
                title: format!("Open {}", expanded),
                subtitle: format!("Direct {} path on disk", kind_badge.to_ascii_lowercase()),
                badge: kind_badge.to_string(),
                icon_override: None,
                category: ShelfCategory::Files,
                can_elevate: path.is_file(),
                score: 9_600,
                target: ExecutionTarget::FilePath(path),
            })
        } else {
            None
        }
    }

    fn detect_direct_url(&self, trimmed: &str) -> Option<CandidateItem> {
        if trimmed.chars().any(char::is_whitespace) {
            return None;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some(CandidateItem {
                title: format!("Open URL: {}", trimmed),
                subtitle: "Launch directly in default web browser".to_string(),
                badge: "URL".to_string(),
                icon_override: None,
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 9_500,
                target: ExecutionTarget::WebUrl(trimmed.to_string()),
            });
        }

        let host_part = trimmed.split('/').next().unwrap_or(trimmed);
        let host_clean = host_part.split(':').next().unwrap_or(host_part);
        let (_, tld) = host_clean.rsplit_once('.')?;

        if COMMON_WEB_TLDS
            .iter()
            .any(|&valid_tld| valid_tld.eq_ignore_ascii_case(tld))
        {
            let full_url = format!("https://{}", trimmed);
            return Some(CandidateItem {
                title: format!("Open Website: {}", full_url),
                subtitle: "Open domain in default web browser".to_string(),
                badge: "WEBSITE".to_string(),
                icon_override: None,
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 9_300,
                target: ExecutionTarget::WebUrl(full_url),
            });
        }

        None
    }

    /// Multi-signal smart scorer:
    /// 1. Exact match -> 2400
    /// 2. Prefix match -> 1850
    /// 3. Word-boundary prefix match -> 1520
    /// 4. Acronym / initialism match (e.g. `vsc` -> `Visual Studio Code`) -> 1380
    /// 5. Multi-token coverage -> 1220
    /// 6. Contiguous substring match -> 960
    /// 7. Quality-thresholded SkimV2 fuzzy score
    fn compute_smart_score(&self, query_raw: &str, query_lower: &str, target: &str) -> Option<i64> {
        if query_lower.is_empty() || target.is_empty() {
            return None;
        }
        let target_lower = target.to_ascii_lowercase();
        let len_penalty = (target_lower.len().saturating_sub(query_lower.len()) as i64).min(60);

        // 1. Exact Match
        if target_lower == query_lower {
            return Some(2400);
        }

        // 2. Exact Start-of-String Prefix Match
        if target_lower.starts_with(query_lower) {
            return Some(1850 - len_penalty * 2);
        }

        // Split target into alphanumeric words for word-boundary & acronym checks
        let target_words: Vec<&str> = target_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();

        // 3. Word-Boundary Exact or Prefix Match
        for (idx, word) in target_words.iter().enumerate() {
            if *word == query_lower {
                return Some(1680 - (idx as i64 * 25) - len_penalty);
            }
            if word.starts_with(query_lower) {
                return Some(1520 - (idx as i64 * 30) - len_penalty);
            }
        }

        // 4. Acronym / Initialism Match (e.g. "vsc" -> "Visual Studio Code", "tm" -> "Task Manager")
        if query_lower.len() >= 2 && target_words.len() >= query_lower.len() {
            let initials: String = target_words
                .iter()
                .filter_map(|w| w.chars().next())
                .collect();
            if initials == query_lower {
                return Some(1440);
            }
            if initials.starts_with(query_lower) {
                return Some(1360);
            }
        }

        // 5. Multi-Token Out-of-Order Match (e.g. "display setting", "note save")
        let query_tokens: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|t| !t.is_empty())
            .collect();
        if query_tokens.len() > 1 {
            let all_tokens_match = query_tokens.iter().all(|qt| target_lower.contains(qt));
            if all_tokens_match {
                let prefix_hits = query_tokens
                    .iter()
                    .filter(|qt| target_words.iter().any(|tw| tw.starts_with(*qt)))
                    .count() as i64;
                return Some(1220 + prefix_hits * 50 - len_penalty);
            }
        }

        // 6. Contiguous Substring Match
        if let Some(pos) = target_lower.find(query_lower) {
            if query_lower.len() >= 2 {
                return Some(960 - (pos as i64 * 6).min(180) - len_penalty);
            }
        }

        // 7. SkimV2 Fuzzy Match with strict quality threshold to prevent noisy suggestions
        if query_lower.len() >= 2 {
            let raw_fuzzy = self.matcher.fuzzy_match(target, query_raw)?;
            let min_required = if query_lower.len() == 2 {
                55
            } else if query_lower.len() == 3 {
                68
            } else {
                82
            };
            if raw_fuzzy >= min_required {
                return Some(300 + raw_fuzzy - len_penalty);
            }
        }

        None
    }

    fn score_triggers_and_title(
        &self,
        query_raw: &str,
        query_lower: &str,
        title: &str,
        triggers: &[&str],
    ) -> Option<i64> {
        let mut best: Option<i64> = self.compute_smart_score(query_raw, query_lower, title);

        for &trig in triggers {
            if let Some(s) = self.compute_smart_score(query_raw, query_lower, trig) {
                let boosted = s + 25;
                best = Some(best.map_or(boosted, |cur| cur.max(boosted)));
            }
        }

        best
    }

    fn match_extensions(&self, query_raw: &str, query_lower: &str) -> Vec<CandidateItem> {
        let mut items = Vec::new();

        for ext in &self.config.extensions {
            if !ext.enabled {
                continue;
            }

            let bang_trigger = format!("!{}", ext.bang);
            let triggers = [ext.bang.as_str(), bang_trigger.as_str(), "extension", "notepad"];
            if let Some(score) =
                self.score_triggers_and_title(query_raw, query_lower, &ext.name, &triggers)
            {
                let chip_label = format!("{} {}", ext.icon, ext.bang.to_ascii_uppercase());
                let sub = if !ext.target_file.is_empty() {
                    format!(
                        "Bang: '{} ' (Type '{}' + Space or press Enter) • File: {}",
                        ext.bang, ext.bang, ext.target_file
                    )
                } else {
                    format!(
                        "Bang: '{} ' (Type '{}' + Space or press Enter) • {}",
                        ext.bang, ext.bang, ext.description
                    )
                };

                items.push(CandidateItem {
                    title: format!("{} ({} + Space)", ext.name, ext.bang),
                    subtitle: sub,
                    badge: "EXTENSION".to_string(),
                    icon_override: Some(ext.icon.clone()),
                    category: ShelfCategory::Extensions,
                    can_elevate: false,
                    score: score + 40,
                    target: ExecutionTarget::ActivateBang {
                        bang: ext.bang.clone(),
                        label: chip_label,
                    },
                });
            }
        }

        // Open Extensions Guide (EXTENSIONS_GUIDE.md) directly from the palette
        if let Some(s) = self.score_triggers_and_title(
            query_raw,
            query_lower,
            "Open Extensions Guide (EXTENSIONS_GUIDE.md)",
            &[
                "extensions guide",
                "extension guide",
                "guide",
                "how to create extension",
                "notepad guide",
                "help",
            ],
        ) {
            let guide_path = crate::extensions::ensure_extensions_guide_file();
            items.push(CandidateItem {
                title: "📖 Open Extensions Guide (EXTENSIONS_GUIDE.md)".to_string(),
                subtitle: format!(
                    "Step-by-step guide to create, add, use & remove extensions • {}",
                    guide_path.display()
                ),
                badge: "GUIDE".to_string(),
                icon_override: Some("📖".to_string()),
                category: ShelfCategory::Extensions,
                can_elevate: false,
                score: s + 20,
                target: ExecutionTarget::FilePath(guide_path),
            });
        }

        if query_lower.contains("ext")
            || query_lower.contains("plugin")
            || query_lower.contains("note")
        {
            if let Some(s) = self.score_triggers_and_title(
                query_raw,
                query_lower,
                "Manage Extensions (Add / Remove / Configure)",
                &[
                    "extensions",
                    "add extension",
                    "remove extension",
                    "plugins",
                    "notepad file",
                ],
            ) {
                items.push(CandidateItem {
                    title: "Manage Extensions (Add / Remove / Configure)".to_string(),
                    subtitle: "Open Command Palette Settings → Extensions tab".to_string(),
                    badge: "EXT MGR".to_string(),
                    icon_override: Some("🧩".to_string()),
                    category: ShelfCategory::Extensions,
                    can_elevate: false,
                    score: s,
                    target: ExecutionTarget::OpenPaletteSettingsTab(5),
                });
            }

            let has_notepad = self.config.notepad_extension().is_some();
            if !has_notepad {
                items.push(CandidateItem {
                    title: "Add Built-in Quick Notepad ('note') Extension".to_string(),
                    subtitle: "Restore the 'note' bang extension to save and view .txt/.md files"
                        .to_string(),
                    badge: "INSTALL".to_string(),
                    icon_override: Some("📝".to_string()),
                    category: ShelfCategory::Extensions,
                    can_elevate: false,
                    score: 1_300,
                    target: ExecutionTarget::ExtensionAction(
                        ExtensionActionKind::RestoreBuiltinNotepad,
                    ),
                });
            }
        }

        items.sort_by(|a, b| b.score.cmp(&a.score));
        items.truncate(6);
        items
    }

    /// Exclusively matches Command Palette's OWN settings (`ShelfCategory::CmdPal`),
    /// completely separated from Windows System Settings!
    fn match_cmdpal_settings(&self, query_raw: &str, query_lower: &str) -> Vec<CandidateItem> {
        if !self.config.enable_cmdpal_shortcuts {
            return Vec::new();
        }

        let mut items = Vec::new();

        // 1. Open Command Palette Settings Window
        if let Some(score) = self.score_triggers_and_title(
            query_raw,
            query_lower,
            "Command Palette Settings",
            &[
                "cmdpal settings",
                "palette settings",
                "preferences",
                "configure palette",
                "cps",
                "settings",
            ],
        ) {
            items.push(CandidateItem {
                title: "Command Palette Settings (CmdPal)".to_string(),
                subtitle: "Configure theme, hotkeys, inbuilt shortcuts, extensions & bangs (Ctrl+,)"
                    .to_string(),
                badge: "CMDPAL".to_string(),
                icon_override: Some("🎛".to_string()),
                category: ShelfCategory::CmdPal,
                can_elevate: false,
                score: score + 30,
                target: ExecutionTarget::OpenPaletteSettings,
            });
        }

        // 2. Reload Indexes & Config
        if let Some(score) = self.score_triggers_and_title(
            query_raw,
            query_lower,
            "Reload Command Palette Config & Indexes",
            &["reload", "rescan", "refresh index", "reload config"],
        ) {
            items.push(CandidateItem {
                title: "Reload Command Palette Config & Indexes".to_string(),
                subtitle: "Rescan Start Menu apps, custom files, extensions & config.toml"
                    .to_string(),
                badge: "CMDPAL".to_string(),
                icon_override: Some("↻".to_string()),
                category: ShelfCategory::CmdPal,
                can_elevate: false,
                score,
                target: ExecutionTarget::ReloadIndex,
            });
        }

        // 3. Default Search Engine quick-switchers
        if let Some(score) = self.score_triggers_and_title(
            query_raw,
            query_lower,
            "Change Default Search Engine",
            &[
                "search engine",
                "default search",
                "google",
                "duckduckgo",
                "bing",
                "brave",
            ],
        ) {
            let (current_name, _) = self.config.resolve_default_search_engine();
            items.push(CandidateItem {
                title: format!("Configure Default Search Engine (Current: {})", current_name),
                subtitle: "Open CmdPal Settings → Web & Search Engine tab".to_string(),
                badge: "CMDPAL".to_string(),
                icon_override: Some("🌐".to_string()),
                category: ShelfCategory::CmdPal,
                can_elevate: false,
                score,
                target: ExecutionTarget::OpenPaletteSettingsTab(6),
            });

            for (id, label) in [
                ("google", "Google"),
                ("duckduckgo", "DuckDuckGo"),
                ("bing", "Bing"),
                ("brave", "Brave Search"),
            ] {
                if !self.config.default_search_engine.eq_ignore_ascii_case(id) {
                    items.push(CandidateItem {
                        title: format!("Set Default Search Engine: {}", label),
                        subtitle: format!("Switch fallback web search to {}", label),
                        badge: "ENGINE".to_string(),
                        icon_override: Some("🌐".to_string()),
                        category: ShelfCategory::CmdPal,
                        can_elevate: false,
                        score: score.saturating_sub(25),
                        target: ExecutionTarget::SetDefaultSearchEngine(id.to_string()),
                    });
                }
            }
        }

        // 4. Run on Windows Startup toggle (CmdPal's own startup setting)
        let startup_triggers = ["startup", "run on startup", "autostart", "cmdpal startup"];
        if let Some(score) = self.score_triggers_and_title(
            query_raw,
            query_lower,
            "Run Command Palette on Windows Startup",
            &startup_triggers,
        ) {
            let current_state = if self.config.run_on_startup {
                "ON"
            } else {
                "OFF"
            };
            let next_enable = !self.config.run_on_startup;
            let action_word = if next_enable { "Enable" } else { "Disable" };

            items.push(CandidateItem {
                title: format!(
                    "{} Run on Windows Startup (Currently: {})",
                    action_word, current_state
                ),
                subtitle: "Toggle launching Command Palette automatically when Windows starts"
                    .to_string(),
                badge: "CMDPAL".to_string(),
                icon_override: Some("🚀".to_string()),
                category: ShelfCategory::CmdPal,
                can_elevate: false,
                score,
                target: ExecutionTarget::ToggleStartup(next_enable),
            });
        }

        // 5. Global Hotkey Presets
        let hotkey_triggers = ["hotkey", "shortcut", "keybind", "bind"];
        if let Some(base_score) = self.score_triggers_and_title(
            query_raw,
            query_lower,
            "Palette Global Hotkey",
            &hotkey_triggers,
        ) {
            for &preset in HOTKEY_PRESETS {
                let is_active = self.config.hotkey.eq_ignore_ascii_case(preset);
                let title = if is_active {
                    format!("Global Hotkey: {} (Active)", preset)
                } else {
                    format!("Set Global Hotkey: {}", preset)
                };
                items.push(CandidateItem {
                    title,
                    subtitle: "Rebind global shortcut to summon Command Palette".to_string(),
                    badge: "HOTKEY".to_string(),
                    icon_override: Some("⌨".to_string()),
                    category: ShelfCategory::CmdPal,
                    can_elevate: false,
                    score: if is_active {
                        base_score + 10
                    } else {
                        base_score
                    },
                    target: ExecutionTarget::SetHotkey(preset.to_string()),
                });
            }
        }

        items.sort_by(|a, b| b.score.cmp(&a.score));
        items.truncate(6);
        items
    }

    fn match_apps(&self, query_raw: &str, query_lower: &str) -> Vec<CandidateItem> {
        let mut items: Vec<CandidateItem> = self
            .apps
            .iter()
            .filter(|app| !self.config.is_app_hidden(&app.name))
            .filter_map(|app| {
                let score = self.compute_smart_score(query_raw, query_lower, &app.name)?;
                Some(CandidateItem {
                    title: app.name.clone(),
                    subtitle: format!("Application • {}", app.path.display()),
                    badge: "APP".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Apps,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::AppPath(app.path.clone()),
                })
            })
            .collect();

        items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        items.truncate(8);
        items
    }

    fn match_custom_files(&self, query_raw: &str, query_lower: &str) -> Vec<CandidateItem> {
        let mut items: Vec<CandidateItem> = self
            .custom_files
            .iter()
            .filter_map(|file| {
                let score = self.compute_smart_score(query_raw, query_lower, &file.display_name)?;
                Some(CandidateItem {
                    title: file.display_name.clone(),
                    subtitle: file.path.to_string_lossy().to_string(),
                    badge: "FILE".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Files,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::FilePath(file.path.clone()),
                })
            })
            .collect();

        items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        items.truncate(6);
        items
    }

    fn match_commands(&self, query_raw: &str, query_lower: &str) -> Vec<CandidateItem> {
        let mut items = Vec::new();

        // 1. Inbuilt System Commands (only those enabled in Settings!)
        if self.config.enable_builtin_cmds {
            for spec in BUILTIN_COMMANDS {
                if !self.config.is_builtin_cmd_enabled(spec.id) {
                    continue;
                }
                if let Some(score) =
                    self.score_triggers_and_title(query_raw, query_lower, spec.title, spec.triggers)
                {
                    items.push(CandidateItem {
                        title: spec.title.to_string(),
                        subtitle: spec.subtitle.to_string(),
                        badge: "COMMAND".to_string(),
                        icon_override: None,
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
        }

        // 2. User-Configured Custom Commands
        for custom in &self.config.custom_commands {
            let s1 = self.compute_smart_score(query_raw, query_lower, &custom.name);
            let s2 = self.compute_smart_score(query_raw, query_lower, &custom.command);
            if let Some(score) = s1.max(s2) {
                items.push(CandidateItem {
                    title: custom.name.clone(),
                    subtitle: format!("Custom Command • {}", custom.command),
                    badge: "CUSTOM CMD".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Cmds,
                    can_elevate: true,
                    score: score + 20,
                    target: ExecutionTarget::CustomCommand(custom.command.clone()),
                });
            }
        }

        items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        items.truncate(6);
        items
    }

    /// Matches Windows System Settings (`ms-settings:` & custom OS settings)
    fn match_windows_settings(&self, query_raw: &str, query_lower: &str) -> Vec<CandidateItem> {
        let mut items = Vec::new();

        // 1. Inbuilt Windows System Settings (only those enabled in Settings!)
        if self.config.enable_builtin_settings {
            for spec in BUILTIN_SETTINGS {
                if !self.config.is_builtin_setting_enabled(spec.id) {
                    continue;
                }
                if let Some(score) =
                    self.score_triggers_and_title(query_raw, query_lower, spec.title, spec.triggers)
                {
                    let uri = if spec.id == "ms_home" {
                        self.config.custom_system_settings_path.clone()
                    } else {
                        spec.uri.to_string()
                    };

                    items.push(CandidateItem {
                        title: spec.title.to_string(),
                        subtitle: spec.subtitle.to_string(),
                        badge: "WIN SETTING".to_string(),
                        icon_override: None,
                        category: ShelfCategory::Settings,
                        can_elevate: false,
                        score,
                        target: ExecutionTarget::SettingsUri(uri),
                    });
                }
            }
        }

        // 2. User-Configured Custom System Settings
        for custom in &self.config.custom_settings {
            let s1 = self.compute_smart_score(query_raw, query_lower, &custom.name);
            let s2 = self.compute_smart_score(query_raw, query_lower, &custom.path);
            if let Some(score) = s1.max(s2) {
                items.push(CandidateItem {
                    title: custom.name.clone(),
                    subtitle: format!("Custom Setting • {}", custom.path),
                    badge: "SETTING".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Settings,
                    can_elevate: false,
                    score: score + 20,
                    target: ExecutionTarget::SettingsUri(custom.path.clone()),
                });
            }
        }

        items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        items.truncate(6);
        items
    }
}

pub fn expand_env_and_tilde(input: &str) -> String {
    let mut out = input.to_string();

    if out.starts_with('~') {
        if let Ok(profile) = env::var("USERPROFILE") {
            out = format!("{}{}", profile, &out[1..]);
        }
    }

    while let Some(start) = out.find('%') {
        let rest = &out[start + 1..];
        let Some(end_rel) = rest.find('%') else {
            break;
        };
        let var_name = &rest[..end_rel];
        if var_name.is_empty() {
            break;
        }
        let replacement = env::var(var_name).unwrap_or_default();
        let full_span_end = start + 1 + end_rel + 1;
        out.replace_range(start..full_span_end, &replacement);
    }

    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetBase {
    Auto,
    Dec,
    Hex,
    Bin,
    Oct,
}

fn evaluate_calculator(raw: &str) -> Option<CandidateItem> {
    let trimmed = raw.trim().trim_end_matches('=').trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let (expr_part, target_base) = if let Some(e) = lower
        .strip_suffix(" in hex")
        .or_else(|| lower.strip_suffix(" to hex"))
        .or_else(|| lower.strip_suffix(" as hex"))
    {
        (e.trim(), TargetBase::Hex)
    } else if let Some(e) = lower
        .strip_suffix(" in bin")
        .or_else(|| lower.strip_suffix(" to bin"))
        .or_else(|| lower.strip_suffix(" as bin"))
    {
        (e.trim(), TargetBase::Bin)
    } else if let Some(e) = lower
        .strip_suffix(" in oct")
        .or_else(|| lower.strip_suffix(" to oct"))
        .or_else(|| lower.strip_suffix(" as oct"))
    {
        (e.trim(), TargetBase::Oct)
    } else if let Some(e) = lower
        .strip_suffix(" in dec")
        .or_else(|| lower.strip_suffix(" to dec"))
        .or_else(|| lower.strip_suffix(" as dec"))
    {
        (e.trim(), TargetBase::Dec)
    } else {
        (lower.as_str(), TargetBase::Auto)
    };

    let has_operator = expr_part.chars().any(|c| {
        matches!(
            c,
            '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')' | '&' | '|' | '<' | '>'
        )
    });
    let has_base_prefix = expr_part.contains("0x")
        || expr_part.contains("0b")
        || expr_part.contains("0o")
        || expr_part.starts_with("sqrt(")
        || expr_part.starts_with("sin(")
        || expr_part.starts_with("cos(")
        || expr_part.starts_with("tan(")
        || expr_part.starts_with("log(")
        || expr_part.starts_with("ln(")
        || expr_part.starts_with("abs(");

    if !has_operator && !has_base_prefix && target_base == TargetBase::Auto {
        return None;
    }

    let mut parser = MathParser::new(expr_part);
    let val = parser.parse_expr()?;
    parser.skip_whitespace();
    if parser.pos < parser.chars.len() || !val.is_finite() {
        return None;
    }

    let is_integer = (val - val.round()).abs() < 1e-9 && val.abs() < (i64::MAX as f64);
    let int_val = val.round() as i64;

    let (formatted_result, copy_value) = match target_base {
        TargetBase::Hex if is_integer => {
            let s = format!("0x{:X}", int_val);
            (s.clone(), s)
        }
        TargetBase::Bin if is_integer => {
            let s = format!("0b{:b}", int_val);
            (s.clone(), s)
        }
        TargetBase::Oct if is_integer => {
            let s = format!("0o{:o}", int_val);
            (s.clone(), s)
        }
        TargetBase::Dec if is_integer => {
            let s = format!("{}", int_val);
            (s.clone(), s)
        }
        _ => {
            if is_integer {
                let dec = format!("{}", int_val);
                if has_base_prefix && int_val >= 0 {
                    (
                        format!("{} (0x{:X} • 0b{:b})", dec, int_val, int_val),
                        dec,
                    )
                } else {
                    (dec.clone(), dec)
                }
            } else {
                let s = format!("{:.8}", val)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string();
                (s.clone(), s)
            }
        }
    };

    Some(CandidateItem {
        title: format!("{} = {}", trimmed, formatted_result),
        subtitle: format!("Calculator • Press Enter to copy {} to clipboard", copy_value),
        badge: "CALC".to_string(),
        icon_override: Some("🧮".to_string()),
        category: ShelfCategory::Cmds,
        can_elevate: false,
        score: 9_800,
        target: ExecutionTarget::CopyToClipboard(copy_value),
    })
}

struct MathParser {
    chars: Vec<char>,
    pos: usize,
}

impl MathParser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_whitespace();
        self.chars.get(self.pos).copied()
    }

    fn parse_expr(&mut self) -> Option<f64> {
        let mut left = self.parse_term()?;
        while let Some(op) = self.peek() {
            if op == '+' || op == '-' {
                self.pos += 1;
                let right = self.parse_term()?;
                if op == '+' {
                    left += right;
                } else {
                    left -= right;
                }
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut left = self.parse_power()?;
        while let Some(op) = self.peek() {
            if op == '*' || op == '/' || op == '%' {
                self.pos += 1;
                let right = self.parse_power()?;
                match op {
                    '*' => left *= right,
                    '/' => {
                        if right == 0.0 {
                            return None;
                        }
                        left /= right;
                    }
                    '%' => {
                        if right == 0.0 {
                            return None;
                        }
                        left %= right;
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_power(&mut self) -> Option<f64> {
        let base = self.parse_unary()?;
        if let Some('^') = self.peek() {
            self.pos += 1;
            let exp = self.parse_power()?;
            Some(base.powf(exp))
        } else {
            Some(base)
        }
    }

    fn parse_unary(&mut self) -> Option<f64> {
        match self.peek()? {
            '+' => {
                self.pos += 1;
                self.parse_unary()
            }
            '-' => {
                self.pos += 1;
                Some(-self.parse_unary()?)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Option<f64> {
        self.skip_whitespace();
        if self.peek()? == '(' {
            self.pos += 1;
            let val = self.parse_expr()?;
            if self.peek()? != ')' {
                return None;
            }
            self.pos += 1;
            return Some(val);
        }

        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }
        if self.pos > start {
            let ident: String = self.chars[start..self.pos].iter().collect();
            match ident.as_str() {
                "pi" => return Some(std::f64::consts::PI),
                "e" => return Some(std::f64::consts::E),
                "sqrt" | "sin" | "cos" | "tan" | "log" | "ln" | "abs" => {
                    if self.peek()? != '(' {
                        return None;
                    }
                    self.pos += 1;
                    let arg = self.parse_expr()?;
                    if self.peek()? != ')' {
                        return None;
                    }
                    self.pos += 1;
                    return match ident.as_str() {
                        "sqrt" => Some(arg.sqrt()),
                        "sin" => Some(arg.sin()),
                        "cos" => Some(arg.cos()),
                        "tan" => Some(arg.tan()),
                        "log" => Some(arg.log10()),
                        "ln" => Some(arg.ln()),
                        "abs" => Some(arg.abs()),
                        _ => None,
                    };
                }
                _ => return None,
            }
        }

        self.parse_number()
    }

    fn parse_number(&mut self) -> Option<f64> {
        self.skip_whitespace();
        let start = self.pos;
        if start + 2 <= self.chars.len() && self.chars[start] == '0' {
            match self.chars[start + 1] {
                'x' | 'X' => {
                    self.pos += 2;
                    let hex_start = self.pos;
                    while self.pos < self.chars.len()
                        && (self.chars[self.pos].is_ascii_hexdigit()
                            || self.chars[self.pos] == '_')
                    {
                        self.pos += 1;
                    }
                    let s: String = self.chars[hex_start..self.pos]
                        .iter()
                        .filter(|&&c| c != '_')
                        .collect();
                    return i64::from_str_radix(&s, 16).ok().map(|v| v as f64);
                }
                'b' | 'B' => {
                    self.pos += 2;
                    let bin_start = self.pos;
                    while self.pos < self.chars.len()
                        && (self.chars[self.pos] == '0'
                            || self.chars[self.pos] == '1'
                            || self.chars[self.pos] == '_')
                    {
                        self.pos += 1;
                    }
                    let s: String = self.chars[bin_start..self.pos]
                        .iter()
                        .filter(|&&c| c != '_')
                        .collect();
                    return i64::from_str_radix(&s, 2).ok().map(|v| v as f64);
                }
                'o' | 'O' => {
                    self.pos += 2;
                    let oct_start = self.pos;
                    while self.pos < self.chars.len()
                        && (matches!(self.chars[self.pos], '0'..='7')
                            || self.chars[self.pos] == '_')
                    {
                        self.pos += 1;
                    }
                    let s: String = self.chars[oct_start..self.pos]
                        .iter()
                        .filter(|&&c| c != '_')
                        .collect();
                    return i64::from_str_radix(&s, 8).ok().map(|v| v as f64);
                }
                _ => {}
            }
        }

        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_digit()
                || self.chars[self.pos] == '.'
                || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }
        if self.pos == start {
            return None;
        }
        let s: String = self.chars[start..self.pos]
            .iter()
            .filter(|&&c| c != '_')
            .collect();
        s.parse::<f64>().ok()
    }
}
