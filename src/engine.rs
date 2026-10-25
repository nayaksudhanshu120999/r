use crate::config::AppConfig;
use crate::models::{
    AppEntry, CandidateItem, ConfigMutation, ExecutionTarget, NoteActionKind, ScannedFileEntry,
    ShelfCategory,
};
use crate::notes::{evaluate_note_bang, resolve_note_file_path};
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
        triggers: &[
            "environment variables",
            "system properties",
            "sysdm.cpl",
            "path",
            "env",
        ],
        program: "SystemPropertiesAdvanced.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "appwiz",
        title: "Programs and Features (Uninstall)",
        subtitle: "appwiz.cpl • Classic uninstaller and Windows features",
        triggers: &["programs and features", "appwiz.cpl", "uninstall program"],
        program: "control.exe",
        args: &["appwiz.cpl"],
    },
    BuiltinCommandSpec {
        id: "ncpa",
        title: "Network Connections (Adapters)",
        subtitle: "ncpa.cpl • Configure Ethernet, Wi-Fi & VPN adapters",
        triggers: &["network connections", "ncpa.cpl", "adapters", "ethernet"],
        program: "control.exe",
        args: &["ncpa.cpl"],
    },
    BuiltinCommandSpec {
        id: "cmd",
        title: "Command Prompt",
        subtitle: "cmd.exe • Windows Command Processor",
        triggers: &["command prompt", "cmd", "terminal"],
        program: "cmd.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "powershell",
        title: "Windows PowerShell",
        subtitle: "powershell.exe • PowerShell console",
        triggers: &["powershell", "ps", "pwsh"],
        program: "powershell.exe",
        args: &[],
    },
    BuiltinCommandSpec {
        id: "flushdns",
        title: "Flush DNS Resolver Cache",
        subtitle: "ipconfig /flushdns • Clear cached DNS records",
        triggers: &["flush dns", "ipconfig", "dns cache", "clear dns"],
        program: "ipconfig.exe",
        args: &["/flushdns"],
    },
    BuiltinCommandSpec {
        id: "lock",
        title: "Lock Workstation",
        subtitle: "rundll32.exe user32.dll,LockWorkStation • Lock screen immediately",
        triggers: &["lock", "lock screen", "lock pc"],
        program: "rundll32.exe",
        args: &["user32.dll,LockWorkStation"],
    },
    BuiltinCommandSpec {
        id: "sleep",
        title: "Sleep PC",
        subtitle: "Suspend system to low-power state",
        triggers: &["sleep", "suspend"],
        program: "rundll32.exe",
        args: &["powrprof.dll,SetSuspendState", "0,1,0"],
    },
    BuiltinCommandSpec {
        id: "restart",
        title: "Restart PC",
        subtitle: "shutdown /r /t 0 • Reboot Windows immediately",
        triggers: &["restart", "reboot"],
        program: "shutdown.exe",
        args: &["/r", "/t", "0"],
    },
    BuiltinCommandSpec {
        id: "shutdown",
        title: "Shut Down PC",
        subtitle: "shutdown /s /t 0 • Power off Windows immediately",
        triggers: &["shutdown", "shut down", "power off"],
        program: "shutdown.exe",
        args: &["/s", "/t", "0"],
    },
];

pub const BUILTIN_SETTINGS: &[BuiltinSettingSpec] = &[
    BuiltinSettingSpec {
        id: "win_settings_home",
        title: "Windows Settings (Home)",
        subtitle: "ms-settings: • Open main Windows Settings app",
        triggers: &["windows settings", "system settings", "ms-settings"],
        uri: "ms-settings:",
    },
    BuiltinSettingSpec {
        id: "display",
        title: "Display Settings",
        subtitle: "ms-settings:display • Resolution, scale, brightness & night light",
        triggers: &[
            "display",
            "resolution",
            "monitor",
            "screen",
            "brightness",
            "scale",
        ],
        uri: "ms-settings:display",
    },
    BuiltinSettingSpec {
        id: "sound",
        title: "Sound & Audio Settings",
        subtitle: "ms-settings:sound • Output devices, microphone & volume mixer",
        triggers: &["sound", "audio", "volume", "microphone", "speaker"],
        uri: "ms-settings:sound",
    },
    BuiltinSettingSpec {
        id: "bluetooth",
        title: "Bluetooth & Devices",
        subtitle: "ms-settings:bluetooth • Pair headphones, mice & keyboards",
        triggers: &["bluetooth", "devices", "pair", "bt"],
        uri: "ms-settings:bluetooth",
    },
    BuiltinSettingSpec {
        id: "wifi",
        title: "Wi-Fi Settings",
        subtitle: "ms-settings:network-wifi • Wireless networks & properties",
        triggers: &["wifi", "wi-fi", "wireless", "wlan"],
        uri: "ms-settings:network-wifi",
    },
    BuiltinSettingSpec {
        id: "network",
        title: "Network & Internet Status",
        subtitle: "ms-settings:network-status • IP, DNS, VPN & proxy status",
        triggers: &["network", "internet", "ethernet", "ip"],
        uri: "ms-settings:network-status",
    },
    BuiltinSettingSpec {
        id: "vpn",
        title: "VPN Settings",
        subtitle: "ms-settings:network-vpn • Virtual private network connections",
        triggers: &["vpn", "virtual private network"],
        uri: "ms-settings:network-vpn",
    },
    BuiltinSettingSpec {
        id: "windowsupdate",
        title: "Windows Update",
        subtitle: "ms-settings:windowsupdate • Check for system updates & drivers",
        triggers: &["windows update", "update", "check for updates", "wu"],
        uri: "ms-settings:windowsupdate",
    },
    BuiltinSettingSpec {
        id: "appsfeatures",
        title: "Installed Apps",
        subtitle: "ms-settings:appsfeatures • Manage or uninstall Windows apps",
        triggers: &["installed apps", "apps and features", "uninstall"],
        uri: "ms-settings:appsfeatures",
    },
    BuiltinSettingSpec {
        id: "defaultapps",
        title: "Default Apps",
        subtitle: "ms-settings:defaultapps • Default browser, file types & links",
        triggers: &["default apps", "default browser", "file associations"],
        uri: "ms-settings:defaultapps",
    },
    BuiltinSettingSpec {
        id: "startupapps",
        title: "Startup Apps (Windows)",
        subtitle: "ms-settings:startupapps • Control apps that run at login",
        triggers: &["startup apps", "login apps", "boot apps"],
        uri: "ms-settings:startupapps",
    },
    BuiltinSettingSpec {
        id: "storage",
        title: "Storage Sense & Disk Space",
        subtitle: "ms-settings:storagesense • Free up disk space & temporary files",
        triggers: &["storage", "disk space", "storage sense"],
        uri: "ms-settings:storagesense",
    },
    BuiltinSettingSpec {
        id: "power",
        title: "Power & Battery Settings",
        subtitle: "ms-settings:powersleep • Power mode, screen timeout & sleep",
        triggers: &["power", "battery", "energy", "sleep timeout"],
        uri: "ms-settings:powersleep",
    },
    BuiltinSettingSpec {
        id: "personalization",
        title: "Personalization & Wallpaper",
        subtitle: "ms-settings:personalization • Background, colors & lock screen",
        triggers: &["personalization", "wallpaper", "background", "colors"],
        uri: "ms-settings:personalization",
    },
    BuiltinSettingSpec {
        id: "taskbar",
        title: "Taskbar Settings",
        subtitle: "ms-settings:taskbar • Taskbar alignment, icons & system tray",
        triggers: &["taskbar", "system tray"],
        uri: "ms-settings:taskbar",
    },
    BuiltinSettingSpec {
        id: "nightlight",
        title: "Night Light Settings",
        subtitle: "ms-settings:nightlight • Blue light filter schedule & strength",
        triggers: &["night light", "blue light", "eye care"],
        uri: "ms-settings:nightlight",
    },
    BuiltinSettingSpec {
        id: "dateandtime",
        title: "Date & Time Settings",
        subtitle: "ms-settings:dateandtime • Time zone, clock sync & region",
        triggers: &["date and time", "time zone", "clock"],
        uri: "ms-settings:dateandtime",
    },
    BuiltinSettingSpec {
        id: "privacy",
        title: "Privacy & Security",
        subtitle: "ms-settings:privacy • Camera, microphone & app permissions",
        triggers: &["privacy", "security", "permissions"],
        uri: "ms-settings:privacy",
    },
    BuiltinSettingSpec {
        id: "about",
        title: "About PC & System Specs",
        subtitle: "ms-settings:about • Processor, RAM, Windows edition & rename PC",
        triggers: &["about pc", "system info", "specs", "pc name"],
        uri: "ms-settings:about",
    },
];

const COMMON_WEB_TLDS: &[&str] = &[
    "com", "org", "net", "edu", "gov", "io", "dev", "rs", "ai", "app", "co", "me", "gg", "tv",
    "xyz", "info", "tech", "cloud", "sh", "uk", "de", "fr", "jp", "in", "au", "ca", "br", "it",
    "nl", "se", "ch", "es",
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

    pub fn extract_bang_prefix(&self, raw_input: &str) -> Option<(String, String, String)> {
        let trimmed_start = raw_input.trim_start();
        let space_idx = trimmed_start.find(' ')?;
        let first_token = &trimmed_start[..space_idx];
        let remainder = &trimmed_start[space_idx + 1..];
        let clean = first_token.trim_start_matches('!').to_ascii_lowercase();
        if clean.is_empty() {
            return None;
        }

        // 1. Check Note Bangs first
        if let Some(note) = self.config.notes.iter().find(|n| n.bang == clean) {
            return Some((
                note.bang.clone(),
                format!("📝 {}", note.bang.to_ascii_uppercase()),
                remainder.to_string(),
            ));
        }

        // 2. Check Web Bangs
        if self.config.bangs.contains_key(&clean) {
            return Some((
                clean.clone(),
                format!("🌐 {}", clean.to_ascii_uppercase()),
                remainder.to_string(),
            ));
        }

        None
    }

    pub fn evaluate_with_bang(&self, active_bang: &str, query: &str) -> Vec<CandidateItem> {
        let bang_clean = active_bang.trim().trim_start_matches('!').to_ascii_lowercase();

        // Active Note Bang Mode
        if !bang_clean.is_empty() {
            if let Some(note) = self.config.notes.iter().find(|n| n.bang == bang_clean) {
                return evaluate_note_bang(note, query);
            }

            if let Some(template) = self.config.bangs.get(&bang_clean) {
                let q_trimmed = query.trim();
                let mut out = Vec::with_capacity(1);
                if q_trimmed.is_empty() {
                    let home_url = template
                        .split("://")
                        .nth(1)
                        .and_then(|rest| rest.split('/').next())
                        .map(|host| format!("https://{}", host))
                        .unwrap_or_else(|| template.replace("{q}", ""));

                    out.push(CandidateItem {
                        title: format!("Open [{}] Homepage", bang_clean),
                        subtitle: format!("{} • Type a query or press Enter", home_url),
                        badge: format!("!{}", bang_clean),
                        icon_override: None,
                        category: ShelfCategory::Web,
                        can_elevate: false,
                        score: 10_000,
                        target: ExecutionTarget::WebUrl(home_url),
                    });
                } else {
                    let encoded = urlencoding::encode(q_trimmed);
                    let target_url = template.replace("{q}", &encoded);
                    out.push(CandidateItem {
                        title: format!("Search [{}] for \"{}\"", bang_clean, q_trimmed),
                        subtitle: target_url.clone(),
                        badge: format!("!{}", bang_clean),
                        icon_override: None,
                        category: ShelfCategory::Web,
                        can_elevate: false,
                        score: 10_000,
                        target: ExecutionTarget::WebUrl(target_url),
                    });
                }
                return out;
            }
        }

        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let q_lower = trimmed.to_ascii_lowercase();
        let mut results: Vec<CandidateItem> = Vec::with_capacity(32);

        // 1. Inline Math Calculator
        if let Some(calc_items) = evaluate_calculator(trimmed) {
            results.extend(calc_items);
        }

        // 2. Direct File/Folder Path Detection
        if let Some(path_item) = self.detect_direct_path(trimmed) {
            results.push(path_item);
        }

        // 3. Direct URL / Domain Detection
        let has_direct_url = if let Some(url_item) = self.detect_direct_url(trimmed) {
            results.push(url_item);
            true
        } else {
            false
        };

        // 4. Exact Bang Activation Chip Suggestion
        let clean_bang_token = q_lower.trim_start_matches('!');
        if !clean_bang_token.contains(char::is_whitespace) {
            if let Some(note) = self.config.notes.iter().find(|n| n.bang == clean_bang_token) {
                results.push(CandidateItem {
                    title: format!("{} — Open '{}' Note Mode", note.name, note.bang),
                    subtitle: format!(
                        "File: {} • Press Space or Enter to write & view lines",
                        note.file_path
                    ),
                    badge: format!("!{}", note.bang),
                    icon_override: Some("📝".to_string()),
                    category: ShelfCategory::Notes,
                    can_elevate: false,
                    score: 3_100,
                    target: ExecutionTarget::ActivateBang {
                        bang: note.bang.clone(),
                        label: format!("📝 {}", note.bang.to_ascii_uppercase()),
                    },
                });
            } else if let Some(url_tmpl) = self.config.bangs.get(clean_bang_token) {
                results.push(CandidateItem {
                    title: format!("Activate !{} Web Search Chip", clean_bang_token),
                    subtitle: format!("Template: {}", url_tmpl),
                    badge: format!("!{}", clean_bang_token),
                    icon_override: None,
                    category: ShelfCategory::Web,
                    can_elevate: false,
                    score: 2_950,
                    target: ExecutionTarget::ActivateBang {
                        bang: clean_bang_token.to_string(),
                        label: format!("🌐 {}", clean_bang_token.to_ascii_uppercase()),
                    },
                });
            }
        }

        // 5. Match across all shelves
        self.match_notes(trimmed, &q_lower, &mut results);
        if self.config.enable_cmdpal_shortcuts {
            self.match_cmdpal_settings(&q_lower, &mut results);
        }
        self.match_commands(&q_lower, &mut results);
        self.match_windows_settings(&q_lower, &mut results);
        if self.config.show_apps {
            self.match_apps(&q_lower, &mut results);
        }
        self.match_custom_files(&q_lower, &mut results);

        // 6. Group & rank by category priority and top score
        let cat_order = self.config.ordered_categories();
        let mut grouped: Vec<(ShelfCategory, usize, i64, Vec<CandidateItem>)> =
            Vec::with_capacity(cat_order.len());

        for (priority_idx, &cat) in cat_order.iter().enumerate() {
            let mut cat_items: Vec<CandidateItem> = results
                .iter()
                .filter(|item| item.category == cat)
                .cloned()
                .collect();

            if cat_items.is_empty() {
                continue;
            }

            cat_items.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then_with(|| a.title.len().cmp(&b.title.len()))
                    .then_with(|| a.title.cmp(&b.title))
            });

            let max_per_shelf = match cat {
                ShelfCategory::Apps => 8,
                ShelfCategory::Files => 6,
                _ => 6,
            };
            cat_items.truncate(max_per_shelf);

            let top_score = cat_items.first().map(|i| i.score).unwrap_or(0);
            grouped.push((cat, priority_idx, top_score, cat_items));
        }

        grouped.sort_by(|a, b| {
            let a_tier = a.2 / 350;
            let b_tier = b.2 / 350;
            b_tier
                .cmp(&a_tier)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| b.2.cmp(&a.2))
        });

        let mut final_list: Vec<CandidateItem> = Vec::with_capacity(MAX_TOTAL_RESULTS);
        for (_, _, _, items) in grouped {
            for item in items {
                if final_list.len() >= MAX_TOTAL_RESULTS - 1 {
                    break;
                }
                final_list.push(item);
            }
        }

        // 7. Always append Default Search Engine fallback
        if !has_direct_url {
            let (engine_name, engine_template) = self.config.resolve_default_search_engine();
            let encoded = urlencoding::encode(trimmed);
            let search_url = engine_template.replace("{q}", &encoded);
            final_list.push(CandidateItem {
                title: format!("Search {} for \"{}\"", engine_name, trimmed),
                subtitle: search_url.clone(),
                badge: engine_name.to_string(),
                icon_override: None,
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 100,
                target: ExecutionTarget::WebUrl(search_url),
            });
        }

        final_list
    }

    fn detect_direct_path(&self, trimmed: &str) -> Option<CandidateItem> {
        let looks_like_path = (trimmed.len() >= 2
            && trimmed.as_bytes()[0].is_ascii_alphabetic()
            && trimmed.as_bytes()[1] == b':')
            || trimmed.starts_with("\\\\")
            || trimmed.starts_with("~/")
            || trimmed.starts_with("~\\")
            || (trimmed.starts_with('%') && trimmed[1..].contains('%'));

        if !looks_like_path {
            return None;
        }

        let expanded = expand_env_and_tilde(trimmed);
        let path = PathBuf::from(&expanded);
        if path.exists() {
            let kind = if path.is_dir() { "Folder" } else { "File" };
            Some(CandidateItem {
                title: format!("Open {}: {}", kind, path.display()),
                subtitle: "Direct filesystem path".to_string(),
                badge: kind.to_string(),
                icon_override: None,
                category: ShelfCategory::Files,
                can_elevate: path.is_file(),
                score: 3_200,
                target: ExecutionTarget::FilePath(path),
            })
        } else {
            None
        }
    }

    fn detect_direct_url(&self, trimmed: &str) -> Option<CandidateItem> {
        if trimmed.contains(char::is_whitespace) {
            return None;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some(CandidateItem {
                title: format!("Open URL: {}", trimmed),
                subtitle: trimmed.to_string(),
                badge: "Open URL".to_string(),
                icon_override: None,
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 3_150,
                target: ExecutionTarget::WebUrl(trimmed.to_string()),
            });
        }

        let host_part = lower.split('/').next().unwrap_or("");
        if let Some(dot_idx) = host_part.rfind('.') {
            let domain_prefix = &host_part[..dot_idx];
            let tld = &host_part[dot_idx + 1..];
            if !domain_prefix.is_empty()
                && domain_prefix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
                && COMMON_WEB_TLDS.contains(&tld)
            {
                let full_url = format!("https://{}", trimmed);
                return Some(CandidateItem {
                    title: format!("Open Website: {}", trimmed),
                    subtitle: full_url.clone(),
                    badge: "Website".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Web,
                    can_elevate: false,
                    score: 2_850,
                    target: ExecutionTarget::WebUrl(full_url),
                });
            }
        }
        None
    }

    /// Zero-allocation smart 7-tier ranking on pre-lowercased strings.
    fn compute_smart_score(&self, query_lower: &str, candidate_lower: &str) -> Option<i64> {
        if query_lower.is_empty() || candidate_lower.is_empty() {
            return None;
        }

        let len_penalty = (candidate_lower.len() as i64).min(60);

        // Tier 1: Exact match
        if candidate_lower == query_lower {
            return Some(2_400);
        }

        // Tier 2: Exact prefix match
        if candidate_lower.starts_with(query_lower) {
            return Some(1_850 - len_penalty);
        }

        // Tier 3: Word-boundary prefix match (zero allocation)
        for (word_idx, word) in candidate_lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .enumerate()
        {
            if word.starts_with(query_lower) {
                return Some(1_520 - (word_idx as i64 * 18) - len_penalty);
            }
        }

        // Tier 4: Acronym / Initialism match (zero allocation)
        if query_lower.len() >= 2 && !query_lower.contains(' ') {
            let q_bytes = query_lower.as_bytes();
            let mut word_initials = candidate_lower
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(|w| w.as_bytes()[0]);

            // Check prefix initialism match
            let mut matched = 0usize;
            let mut total_words = 0usize;
            for init_b in &mut word_initials {
                total_words += 1;
                if matched < q_bytes.len() && init_b == q_bytes[matched] {
                    matched += 1;
                } else if matched == 0 {
                    // keep scanning if acronym starts cleanly
                    break;
                }
            }
            if matched == q_bytes.len() {
                total_words += word_initials.count();
                if total_words == q_bytes.len() {
                    return Some(1_440 - len_penalty);
                }
                return Some(1_360 - len_penalty);
            }
        }

        // Tier 5: Multi-token out-of-order match (zero Vec allocation)
        if query_lower.contains(' ') {
            let mut all_matched = true;
            let mut token_count = 0i64;
            for q_tok in query_lower.split_whitespace() {
                token_count += 1;
                if !candidate_lower.contains(q_tok) {
                    all_matched = false;
                    break;
                }
            }
            if all_matched && token_count > 0 {
                return Some(1_220 + (token_count * 25) - len_penalty);
            }
        }

        // Tier 6: Contiguous substring match
        if let Some(pos) = candidate_lower.find(query_lower) {
            return Some(960 - (pos as i64 * 8) - len_penalty);
        }

        // Tier 7: Quality-thresholded SkimV2 fuzzy match
        if query_lower.len() < 2 {
            return None;
        }
        if let Some(fuzzy_raw) = self.matcher.fuzzy_match(candidate_lower, query_lower) {
            let min_required = (query_lower.len() as i64) * 22;
            if fuzzy_raw >= min_required {
                return Some((400 + fuzzy_raw * 3 - len_penalty).min(890));
            }
        }

        None
    }

    fn score_triggers_and_title(&self, query_lower: &str, title: &str, triggers: &[&str]) -> Option<i64> {
        let title_lower = title.to_ascii_lowercase();
        let mut best = self.compute_smart_score(query_lower, &title_lower);
        for &trig in triggers {
            if let Some(sc) = self.compute_smart_score(query_lower, trig) {
                best = Some(best.map_or(sc, |b| b.max(sc)));
            }
        }
        best
    }

    fn match_notes(&self, raw_query: &str, q_lower: &str, out: &mut Vec<CandidateItem>) {
        for note in &self.config.notes {
            // Direct inline `<bang> <content>` fallback if chip wasn't extracted
            let prefix = format!("{} ", note.bang);
            if q_lower.starts_with(&prefix) {
                let remainder = raw_query[prefix.len()..].trim();
                if !remainder.is_empty() {
                    let file_path = resolve_note_file_path(&note.file_path);
                    out.push(CandidateItem {
                        title: format!("Save to {}: \"{}\"", note.file_path, remainder),
                        subtitle: format!("Append line to {}", file_path.display()),
                        badge: format!("!{}", note.bang),
                        icon_override: Some("💾".to_string()),
                        category: ShelfCategory::Notes,
                        can_elevate: false,
                        score: 3_050,
                        target: ExecutionTarget::NoteAction(NoteActionKind::AppendLine {
                            file_path,
                            content: remainder.to_string(),
                            with_timestamp: false,
                        }),
                    });
                    continue;
                }
            }

            let name_lower = note.name.to_ascii_lowercase();
            let file_lower = note.file_path.to_ascii_lowercase();
            let s1 = self.compute_smart_score(q_lower, &note.bang);
            let s2 = self.compute_smart_score(q_lower, &name_lower);
            let s3 = self.compute_smart_score(q_lower, &file_lower);

            if let Some(score) = [s1, s2, s3].into_iter().flatten().max() {
                if q_lower == note.bang {
                    continue; // Already pushed high-priority exact bang activation
                }
                out.push(CandidateItem {
                    title: format!("{} (!{})", note.name, note.bang),
                    subtitle: format!("Note file: {} • Press Enter to write or view notes", note.file_path),
                    badge: format!("!{}", note.bang),
                    icon_override: Some("📝".to_string()),
                    category: ShelfCategory::Notes,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::ActivateBang {
                        bang: note.bang.clone(),
                        label: format!("📝 {}", note.bang.to_ascii_uppercase()),
                    },
                });
            }
        }
    }

    fn match_cmdpal_settings(&self, q_lower: &str, out: &mut Vec<CandidateItem>) {
        let items: &[(&str, &str, &[&str], ExecutionTarget)] = &[
            (
                "Command Palette Settings",
                "Open Command Palette configuration window (Ctrl+,)",
                &["cmdpal settings", "palette settings", "preferences", "configure", "cps"],
                ExecutionTarget::OpenPaletteSettingsTab(0),
            ),
            (
                "Manage Note Bangs & Text Files",
                "Add or edit note bangs mapped to .txt and .md files",
                &["note settings", "notes", "notepad files", "text files"],
                ExecutionTarget::OpenPaletteSettingsTab(1),
            ),
            (
                "Manage Web Search & Bangs",
                "Change default search engine or edit web search bangs",
                &["search engine", "web bangs", "default search"],
                ExecutionTarget::OpenPaletteSettingsTab(2),
            ),
            (
                "Reload Command Palette Config & Indexes",
                "Rescan config.toml, Start Menu apps, and custom file directories",
                &["reload", "rescan", "refresh index", "reload config"],
                ExecutionTarget::ReloadIndex,
            ),
            (
                "Use Default Fluent Dark Theme",
                "Switch Command Palette appearance to PowerToys Fluent Dark",
                &["dark theme", "powertoys theme", "default theme"],
                ExecutionTarget::MutateConfig(ConfigMutation::SwitchTheme("powertoys".to_string())),
            ),
            (
                "Use Custom Hex Theme",
                "Switch Command Palette appearance to custom hex colors",
                &["custom theme", "hex theme"],
                ExecutionTarget::MutateConfig(ConfigMutation::SwitchTheme("custom".to_string())),
            ),
            (
                if self.config.run_on_startup {
                    "Disable Run on Windows Startup"
                } else {
                    "Enable Run on Windows Startup"
                },
                "Toggle launching Command Palette automatically when signing into Windows",
                &["run on startup", "autostart", "startup toggle"],
                ExecutionTarget::MutateConfig(ConfigMutation::ToggleStartup(!self.config.run_on_startup)),
            ),
            (
                "Set Default Search Engine: Google",
                "Use Google for fallback web searches",
                &["use google", "default google"],
                ExecutionTarget::MutateConfig(ConfigMutation::SetDefaultSearchEngine("google".to_string())),
            ),
            (
                "Set Default Search Engine: DuckDuckGo",
                "Use DuckDuckGo for fallback web searches",
                &["use duckduckgo", "default ddg"],
                ExecutionTarget::MutateConfig(ConfigMutation::SetDefaultSearchEngine("duckduckgo".to_string())),
            ),
            (
                "Set Global Hotkey: Ctrl+Space",
                "Bind activation shortcut to Ctrl+Space",
                &["hotkey ctrl space"],
                ExecutionTarget::MutateConfig(ConfigMutation::SetHotkey("Ctrl+Space".to_string())),
            ),
            (
                "Set Global Hotkey: Alt+Space",
                "Bind activation shortcut to Alt+Space",
                &["hotkey alt space"],
                ExecutionTarget::MutateConfig(ConfigMutation::SetHotkey("Alt+Space".to_string())),
            ),
        ];

        for (title, subtitle, triggers, target) in items {
            if let Some(score) = self.score_triggers_and_title(q_lower, title, triggers) {
                out.push(CandidateItem {
                    title: (*title).to_string(),
                    subtitle: (*subtitle).to_string(),
                    badge: "CmdPal".to_string(),
                    icon_override: None,
                    category: ShelfCategory::CmdPal,
                    can_elevate: false,
                    score,
                    target: target.clone(),
                });
            }
        }
    }

    fn match_apps(&self, q_lower: &str, out: &mut Vec<CandidateItem>) {
        for app in &self.apps {
            if self.config.is_app_hidden(&app.name_lower) {
                continue;
            }
            if let Some(score) = self.compute_smart_score(q_lower, &app.name_lower) {
                out.push(CandidateItem {
                    title: app.name.clone(),
                    subtitle: app.path.to_string_lossy().to_string(),
                    badge: "Application".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Apps,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::AppPath(app.path.clone()),
                });
            }
        }
    }

    fn match_custom_files(&self, q_lower: &str, out: &mut Vec<CandidateItem>) {
        for file in &self.custom_files {
            if let Some(score) = self.compute_smart_score(q_lower, &file.name_lower) {
                out.push(CandidateItem {
                    title: file.display_name.clone(),
                    subtitle: file.path.to_string_lossy().to_string(),
                    badge: "File".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Files,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::FilePath(file.path.clone()),
                });
            }
        }
    }

    fn match_commands(&self, q_lower: &str, out: &mut Vec<CandidateItem>) {
        if self.config.enable_builtin_cmds {
            for spec in BUILTIN_COMMANDS {
                if !self.config.is_builtin_cmd_enabled(spec.id) {
                    continue;
                }
                if let Some(score) = self.score_triggers_and_title(q_lower, spec.title, spec.triggers) {
                    out.push(CandidateItem {
                        title: spec.title.to_string(),
                        subtitle: spec.subtitle.to_string(),
                        badge: "System Cmd".to_string(),
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

        for custom in &self.config.custom_commands {
            let name_lower = custom.name.to_ascii_lowercase();
            let cmd_lower = custom.command.to_ascii_lowercase();
            let s1 = self.compute_smart_score(q_lower, &name_lower);
            let s2 = self.compute_smart_score(q_lower, &cmd_lower);
            if let Some(score) = s1.into_iter().chain(s2).max() {
                out.push(CandidateItem {
                    title: custom.name.clone(),
                    subtitle: custom.command.clone(),
                    badge: "Custom Cmd".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Cmds,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::CustomCommand(custom.command.clone()),
                });
            }
        }
    }

    fn match_windows_settings(&self, q_lower: &str, out: &mut Vec<CandidateItem>) {
        if self.config.enable_builtin_settings {
            for spec in BUILTIN_SETTINGS {
                if !self.config.is_builtin_setting_enabled(spec.id) {
                    continue;
                }
                if let Some(score) = self.score_triggers_and_title(q_lower, spec.title, spec.triggers) {
                    out.push(CandidateItem {
                        title: spec.title.to_string(),
                        subtitle: spec.subtitle.to_string(),
                        badge: "Win Setting".to_string(),
                        icon_override: None,
                        category: ShelfCategory::Settings,
                        can_elevate: false,
                        score,
                        target: ExecutionTarget::SettingsUri(spec.uri.to_string()),
                    });
                }
            }
        }

        for custom in &self.config.custom_settings {
            let name_lower = custom.name.to_ascii_lowercase();
            let path_lower = custom.path.to_ascii_lowercase();
            let s1 = self.compute_smart_score(q_lower, &name_lower);
            let s2 = self.compute_smart_score(q_lower, &path_lower);
            if let Some(score) = s1.into_iter().chain(s2).max() {
                out.push(CandidateItem {
                    title: custom.name.clone(),
                    subtitle: custom.path.clone(),
                    badge: "Custom Setting".to_string(),
                    icon_override: None,
                    category: ShelfCategory::Settings,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::SettingsUri(custom.path.clone()),
                });
            }
        }
    }
}

pub fn expand_env_and_tilde(input: &str) -> String {
    let mut out = input.trim().to_string();
    if out.starts_with("~/") || out.starts_with("~\\") || out == "~" {
        if let Ok(profile) = env::var("USERPROFILE") {
            out = format!("{}{}", profile, &out[1..]);
        }
    }
    while let Some(start) = out.find('%') {
        let Some(end_rel) = out[start + 1..].find('%') else {
            break;
        };
        let end = start + 1 + end_rel;
        let var_name = &out[start + 1..end];
        if let Ok(val) = env::var(var_name) {
            out.replace_range(start..=end, &val);
        } else {
            break;
        }
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

fn evaluate_calculator(raw: &str) -> Option<Vec<CandidateItem>> {
    let trimmed = raw.trim().trim_end_matches('=').trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let (expr_str, base_mode) = if let Some(stripped) = lower
        .strip_suffix(" in hex")
        .or_else(|| lower.strip_suffix(" to hex"))
    {
        (stripped.trim(), TargetBase::Hex)
    } else if let Some(stripped) = lower
        .strip_suffix(" in bin")
        .or_else(|| lower.strip_suffix(" to bin"))
    {
        (stripped.trim(), TargetBase::Bin)
    } else if let Some(stripped) = lower
        .strip_suffix(" in oct")
        .or_else(|| lower.strip_suffix(" to oct"))
    {
        (stripped.trim(), TargetBase::Oct)
    } else if let Some(stripped) = lower
        .strip_suffix(" in dec")
        .or_else(|| lower.strip_suffix(" to dec"))
    {
        (stripped.trim(), TargetBase::Dec)
    } else {
        (lower.as_str(), TargetBase::Auto)
    };

    let has_math_hint = expr_str.chars().any(|c| {
        matches!(
            c,
            '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')' | '&' | '|' | '~'
        )
    }) || expr_str.starts_with("0x")
        || expr_str.starts_with("0b")
        || expr_str.starts_with("0o")
        || expr_str.starts_with("sqrt(")
        || expr_str.starts_with("sin(")
        || expr_str.starts_with("cos(")
        || expr_str.starts_with("tan(")
        || expr_str.starts_with("log(")
        || expr_str.starts_with("ln(")
        || expr_str.starts_with("abs(")
        || base_mode != TargetBase::Auto;

    if !has_math_hint {
        return None;
    }

    let mut parser = MathParser::new(expr_str);
    let val = parser.parse_expr()?;
    parser.skip_whitespace();
    if parser.pos < parser.chars.len() || !val.is_finite() {
        return None;
    }

    // Avoid triggering on plain single numbers unless base conversion was requested
    if base_mode == TargetBase::Auto
        && !expr_str.starts_with("0x")
        && !expr_str.starts_with("0b")
        && !expr_str.starts_with("0o")
        && expr_str.parse::<f64>().is_ok()
    {
        return None;
    }

    let formatted_dec = if (val - val.round()).abs() < 1e-9 && val.abs() < 1e15 {
        format!("{}", val.round() as i64)
    } else {
        format!("{:.8}", val)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    };

    let primary_str = match base_mode {
        TargetBase::Hex => format!("0x{:X}", val.round() as i64),
        TargetBase::Bin => format!("0b{:b}", val.round() as i64),
        TargetBase::Oct => format!("0o{:o}", val.round() as i64),
        TargetBase::Dec | TargetBase::Auto => formatted_dec.clone(),
    };

    let int_val = val.round() as i64;
    let subtitle = if (val - val.round()).abs() < 1e-9 && int_val >= 0 {
        format!(
            "DEC: {}  •  HEX: 0x{:X}  •  BIN: 0b{:b}  (Enter to copy)",
            int_val, int_val, int_val
        )
    } else {
        format!("{} = {}  (Press Enter to copy result)", trimmed, primary_str)
    };

    Some(vec![CandidateItem {
        title: format!("= {}", primary_str),
        subtitle,
        badge: "Calculator".to_string(),
        icon_override: Some("∑".to_string()),
        category: ShelfCategory::Cmds,
        can_elevate: false,
        score: 3_300,
        target: ExecutionTarget::CopyToClipboard(primary_str),
    }])
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
                if op == '*' {
                    left *= right;
                } else if op == '/' {
                    if right == 0.0 {
                        return None;
                    }
                    left /= right;
                } else {
                    if right == 0.0 {
                        return None;
                    }
                    left %= right;
                }
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_power(&mut self) -> Option<f64> {
        let base = self.parse_unary()?;
        if self.peek() == Some('^') {
            self.pos += 1;
            let exp = self.parse_power()?;
            Some(base.powf(exp))
        } else {
            Some(base)
        }
    }

    fn parse_unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some('+') => {
                self.pos += 1;
                self.parse_unary()
            }
            Some('-') => {
                self.pos += 1;
                Some(-self.parse_unary()?)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Option<f64> {
        self.skip_whitespace();
        if self.peek() == Some('(') {
            self.pos += 1;
            let val = self.parse_expr()?;
            if self.peek() != Some(')') {
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
                "sqrt" | "sin" | "cos" | "tan" | "log" | "ln" | "abs" | "floor" | "ceil" => {
                    if self.peek() != Some('(') {
                        return None;
                    }
                    self.pos += 1;
                    let arg = self.parse_expr()?;
                    if self.peek() != Some(')') {
                        return None;
                    }
                    self.pos += 1;
                    return Some(match ident.as_str() {
                        "sqrt" => arg.sqrt(),
                        "sin" => arg.sin(),
                        "cos" => arg.cos(),
                        "tan" => arg.tan(),
                        "log" => arg.log10(),
                        "ln" => arg.ln(),
                        "abs" => arg.abs(),
                        "floor" => arg.floor(),
                        "ceil" => arg.ceil(),
                        _ => unreachable!(),
                    });
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
                        && (matches!(self.chars[self.pos], '0' | '1' | '_'))
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
                        && (matches!(self.chars[self.pos], '0'..='7' | '_'))
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

        let num_str: String = self.chars[start..self.pos]
            .iter()
            .filter(|&&c| c != '_')
            .collect();
        num_str.parse::<f64>().ok()
    }
}
