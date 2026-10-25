use crate::config::{AppConfig, NoteBangItem};
use crate::engine::expand_env_and_tilde;
use crate::executor::open_path_in_default_app;
use crate::models::{CandidateItem, ExecutionTarget, NoteActionKind, ShelfCategory};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_NOTE_FILE_BYTES: u64 = 2 * 1024 * 1024; // 2 MB cap for inline preview safety
const MAX_PREVIEW_LINES: usize = 30;

struct CachedNoteFile {
    modified: Option<SystemTime>,
    len: u64,
    lines: Vec<(usize, String, String)>, // (0-based line_idx, raw_line, lower_line)
}

static NOTE_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedNoteFile>>> = OnceLock::new();

fn get_cached_lines(path: &Path) -> Vec<(usize, String, String)> {
    let Ok(meta) = fs::metadata(path) else {
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > MAX_NOTE_FILE_BYTES {
        return Vec::new();
    }

    let modified = meta.modified().ok();
    let len = meta.len();

    let cache_mutex = NOTE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = cache_mutex.lock() {
        if let Some(entry) = guard.get(path) {
            if entry.modified == modified && entry.len == len {
                return entry.lines.clone();
            }
        }

        let Ok(raw) = fs::read_to_string(path) else {
            return Vec::new();
        };

        let lines: Vec<(usize, String, String)> = raw
            .lines()
            .enumerate()
            .filter_map(|(idx, line)| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some((idx, trimmed.to_string(), trimmed.to_ascii_lowercase()))
                }
            })
            .collect();

        // Keep cache bounded to at most 16 note files
        if guard.len() >= 16 && !guard.contains_key(path) {
            guard.clear();
        }
        guard.insert(
            path.to_path_buf(),
            CachedNoteFile {
                modified,
                len,
                lines: lines.clone(),
            },
        );
        return lines;
    }

    Vec::new()
}

fn invalidate_cached_file(path: &Path) {
    if let Some(cache_mutex) = NOTE_CACHE.get() {
        if let Ok(mut guard) = cache_mutex.lock() {
            guard.remove(path);
        }
    }
}

pub fn resolve_note_file_path(raw_target: &str) -> PathBuf {
    let trimmed = raw_target.trim().trim_matches('"');
    let fallback = if trimmed.is_empty() {
        "notes.md"
    } else {
        trimmed
    };
    let expanded = expand_env_and_tilde(fallback);
    let candidate = PathBuf::from(&expanded);
    if candidate.is_absolute() {
        candidate
    } else {
        AppConfig::resolve_config_path(&expanded)
    }
}

pub fn open_note_target_file(raw_target: &str) -> io::Result<PathBuf> {
    let path = resolve_note_file_path(raw_target);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }
    if !path.exists() {
        let _ = OpenOptions::new().create(true).write(true).open(&path)?;
    }
    open_path_in_default_app(&path)?;
    Ok(path)
}

pub fn evaluate_note_bang(note: &NoteBangItem, remainder: &str) -> Vec<CandidateItem> {
    let file_path = resolve_note_file_path(&note.file_path);
    let file_label = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&note.file_path)
        .to_string();

    let trimmed = remainder.trim();
    let lower = trimmed.to_ascii_lowercase();
    let badge_label = format!("!{}", note.bang);
    let mut out = Vec::with_capacity(16);

    // Quick command: `<bang> open`
    if lower == "open" || lower == "edit" {
        out.push(CandidateItem {
            title: format!("Open {} in Default Editor", file_label),
            subtitle: format!("{} • Launch external editor", file_path.display()),
            badge: badge_label.clone(),
            icon_override: Some("📂".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 10_000,
            target: ExecutionTarget::FilePath(file_path),
        });
        return out;
    }

    // Quick command: `<bang> clear`
    if lower == "clear" {
        out.push(CandidateItem {
            title: format!("Clear all contents of {}", file_label),
            subtitle: format!("{} • Truncate file", file_path.display()),
            badge: badge_label,
            icon_override: Some("🗑".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 10_000,
            target: ExecutionTarget::NoteAction(NoteActionKind::ClearFile { file_path }),
        });
        return out;
    }

    // 1. Write / Save actions when user has typed text
    if !trimmed.is_empty() {
        out.push(CandidateItem {
            title: format!("Save to {}: \"{}\"", file_label, trimmed),
            subtitle: format!(
                "Press Enter to append line to {} and refresh preview",
                file_path.display()
            ),
            badge: "Save Note".to_string(),
            icon_override: Some("💾".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 9_800,
            target: ExecutionTarget::NoteAction(NoteActionKind::AppendLine {
                file_path: file_path.clone(),
                content: trimmed.to_string(),
                with_timestamp: false,
            }),
        });

        out.push(CandidateItem {
            title: format!("Save Timestamped: \"{}\"", trimmed),
            subtitle: format!("Prepend [YYYY-MM-DD HH:MM:SS] in {}", file_label),
            badge: "Timestamp".to_string(),
            icon_override: Some("🕒".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 9_600,
            target: ExecutionTarget::NoteAction(NoteActionKind::AppendLine {
                file_path: file_path.clone(),
                content: trimmed.to_string(),
                with_timestamp: true,
            }),
        });

        out.push(CandidateItem {
            title: format!("Overwrite {} with typed text", file_label),
            subtitle: "Replace entire file content with this line".to_string(),
            badge: "Overwrite".to_string(),
            icon_override: Some("📄".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 9_400,
            target: ExecutionTarget::NoteAction(NoteActionKind::OverwriteFile {
                file_path: file_path.clone(),
                content: trimmed.to_string(),
            }),
        });
    } else {
        out.push(CandidateItem {
            title: format!(
                "{} ({}) — Type text & press Enter to save",
                note.name, file_label
            ),
            subtitle: format!(
                "Active bang: '{}' • File: {}",
                note.bang,
                file_path.display()
            ),
            badge: badge_label.clone(),
            icon_override: Some("📝".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 9_800,
            target: ExecutionTarget::NoOp,
        });
    }

    // 2. Read cached lines from file (newest first)
    let cached_lines = get_cached_lines(&file_path);
    let mut matched_count = 0usize;

    for (line_idx, raw_line, lower_line) in cached_lines.iter().rev() {
        if !lower.is_empty() && !lower_line.contains(&lower) {
            continue;
        }
        out.push(CandidateItem {
            title: raw_line.clone(),
            subtitle: format!(
                "Line {} in {} • Press Enter to copy to clipboard",
                line_idx + 1,
                file_label
            ),
            badge: format!("Line {}", line_idx + 1),
            icon_override: Some("📝".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 8_500 - (matched_count as i64),
            target: ExecutionTarget::CopyToClipboard(raw_line.clone()),
        });
        matched_count += 1;
        if matched_count >= MAX_PREVIEW_LINES {
            break;
        }
    }

    // 3. File management actions
    out.push(CandidateItem {
        title: format!("Open {} in External Editor", file_label),
        subtitle: format!("{}", file_path.display()),
        badge: "Open File".to_string(),
        icon_override: Some("📂".to_string()),
        category: ShelfCategory::Notes,
        can_elevate: false,
        score: 7_200,
        target: ExecutionTarget::FilePath(file_path.clone()),
    });

    if trimmed.is_empty() {
        if let Some((last_idx, last_text, _)) = cached_lines.last() {
            out.push(CandidateItem {
                title: format!("Delete Last Line: \"{}\"", last_text),
                subtitle: format!("Remove line {} from {}", last_idx + 1, file_label),
                badge: "Delete Line".to_string(),
                icon_override: Some("⌫".to_string()),
                category: ShelfCategory::Notes,
                can_elevate: false,
                score: 7_100,
                target: ExecutionTarget::NoteAction(NoteActionKind::DeleteLine {
                    file_path: file_path.clone(),
                    line_index: *last_idx,
                }),
            });

            out.push(CandidateItem {
                title: format!("Clear Entire File ({})", file_label),
                subtitle: "Remove all lines from this note file".to_string(),
                badge: "Clear".to_string(),
                icon_override: Some("🗑".to_string()),
                category: ShelfCategory::Notes,
                can_elevate: false,
                score: 7_000,
                target: ExecutionTarget::NoteAction(NoteActionKind::ClearFile { file_path }),
            });
        }
    }

    out
}

pub fn execute_note_action(action: &NoteActionKind) -> io::Result<bool> {
    match action {
        NoteActionKind::AppendLine {
            file_path,
            content,
            with_timestamp,
        } => {
            if let Some(parent) = file_path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    let _ = fs::create_dir_all(parent);
                }
            }
            let is_md = file_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false);

            let formatted = if *with_timestamp {
                let ts = current_timestamp_label();
                if is_md {
                    format!("- **[{}]** {}\n", ts, content)
                } else {
                    format!("[{}] {}\n", ts, content)
                }
            } else {
                format!("{}\n", content)
            };

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)?;
            file.write_all(formatted.as_bytes())?;
            invalidate_cached_file(file_path);
            Ok(true)
        }
        NoteActionKind::OverwriteFile { file_path, content } => {
            if let Some(parent) = file_path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    let _ = fs::create_dir_all(parent);
                }
            }
            fs::write(file_path, format!("{}\n", content))?;
            invalidate_cached_file(file_path);
            Ok(true)
        }
        NoteActionKind::DeleteLine {
            file_path,
            line_index,
        } => {
            if let Ok(raw) = fs::read_to_string(file_path) {
                let remaining: Vec<&str> = raw
                    .lines()
                    .enumerate()
                    .filter_map(|(i, l)| if i == *line_index { None } else { Some(l) })
                    .collect();
                let new_body = if remaining.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", remaining.join("\n"))
                };
                fs::write(file_path, new_body)?;
                invalidate_cached_file(file_path);
            }
            Ok(true)
        }
        NoteActionKind::ClearFile { file_path } => {
            if file_path.exists() {
                fs::write(file_path, "")?;
                invalidate_cached_file(file_path);
            }
            Ok(true)
        }
    }
}

fn current_timestamp_label() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let min = (rem % 3_600) / 60;
    let sec = rem % 60;

    // Howard Hinnant civil-from-days algorithm
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, m, d, hour, min, sec)
}
