use crate::config::{AppConfig, NoteBangItem};
use crate::engine::expand_env_and_tilde;
use crate::executor::open_path_in_default_app;
use crate::models::{CandidateItem, ExecutionTarget, NoteActionKind, ShelfCategory};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::mem::zeroed;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

const MAX_NOTE_FILE_BYTES: u64 = 2 * 1024 * 1024; // 2 MB cap for inline preview safety
const MAX_PREVIEW_NOTES: usize = 30;
const NOTE_SEPARATOR: &str = "----------------------------------------";

#[repr(C)]
struct Win32SystemTime {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(lp_system_time: *mut Win32SystemTime);
}

#[derive(Clone)]
struct ParsedNoteEntry {
    line_number: usize,
    timestamp: Option<String>,
    content: String,
    search_lower: String,
}

struct CachedNoteFile {
    modified: Option<SystemTime>,
    len: u64,
    entries: Vec<ParsedNoteEntry>,
}

static NOTE_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedNoteFile>>> = OnceLock::new();

fn is_separator_line(trimmed: &str) -> bool {
    trimmed.len() >= 3
        && (trimmed.chars().all(|c| c == '-') || trimmed.chars().all(|c| c == '='))
}

fn parse_timestamp_header(trimmed: &str) -> Option<String> {
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 12 {
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return Some(inner.to_string());
        }
    }
    None
}

fn get_cached_notes(path: &Path) -> Vec<ParsedNoteEntry> {
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
                return entry.entries.clone();
            }
        }

        let Ok(raw) = fs::read_to_string(path) else {
            return Vec::new();
        };

        let mut entries = Vec::new();
        let mut pending_ts: Option<String> = None;

        for (idx, line) in raw.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || is_separator_line(trimmed) {
                continue;
            }
            if let Some(ts) = parse_timestamp_header(trimmed) {
                pending_ts = Some(ts);
                continue;
            }

            let search_lower = match &pending_ts {
                Some(ts) => format!("{} {}", ts.to_ascii_lowercase(), trimmed.to_ascii_lowercase()),
                None => trimmed.to_ascii_lowercase(),
            };

            entries.push(ParsedNoteEntry {
                line_number: idx + 1,
                timestamp: pending_ts.take(),
                content: trimmed.to_string(),
                search_lower,
            });
        }

        if guard.len() >= 16 && !guard.contains_key(path) {
            guard.clear();
        }
        guard.insert(
            path.to_path_buf(),
            CachedNoteFile {
                modified,
                len,
                entries: entries.clone(),
            },
        );
        return entries;
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
            badge: badge_label,
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
                "Appends separator + current date & time + note to {}",
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
            }),
        });

        out.push(CandidateItem {
            title: format!("Overwrite {} with this note", file_label),
            subtitle: "Replaces entire file with separator + current date & time + note".to_string(),
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
                "{} ({}) — Type a note & press Enter to save",
                note.name, file_label
            ),
            subtitle: format!(
                "Automatically adds separator & date/time • File: {}",
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

    // 2. Read cached notes from file (newest first)
    let cached_notes = get_cached_notes(&file_path);
    let mut matched_count = 0usize;

    for entry in cached_notes.iter().rev() {
        if !lower.is_empty() && !entry.search_lower.contains(&lower) {
            continue;
        }
        let subtitle = match &entry.timestamp {
            Some(ts) => format!(
                "🕒 {}  •  Line {} in {}  (Enter to copy)",
                ts, entry.line_number, file_label
            ),
            None => format!(
                "Line {} in {}  •  Press Enter to copy to clipboard",
                entry.line_number, file_label
            ),
        };
        let badge = entry
            .timestamp
            .as_deref()
            .and_then(|ts| ts.split_whitespace().next())
            .map(|d| d.to_string())
            .unwrap_or_else(|| format!("Line {}", entry.line_number));

        out.push(CandidateItem {
            title: entry.content.clone(),
            subtitle,
            badge,
            icon_override: Some("📝".to_string()),
            category: ShelfCategory::Notes,
            can_elevate: false,
            score: 8_500 - (matched_count as i64),
            target: ExecutionTarget::CopyToClipboard(entry.content.clone()),
        });
        matched_count += 1;
        if matched_count >= MAX_PREVIEW_NOTES {
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
        if let Some(last_entry) = cached_notes.last() {
            out.push(CandidateItem {
                title: format!("Delete Last Note: \"{}\"", last_entry.content),
                subtitle: format!("Remove the most recent note block from {}", file_label),
                badge: "Delete Note".to_string(),
                icon_override: Some("⌫".to_string()),
                category: ShelfCategory::Notes,
                can_elevate: false,
                score: 7_100,
                target: ExecutionTarget::NoteAction(NoteActionKind::DeleteLastNote {
                    file_path: file_path.clone(),
                }),
            });

            out.push(CandidateItem {
                title: format!("Clear Entire File ({})", file_label),
                subtitle: "Remove all notes from this file".to_string(),
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

fn format_note_block(content: &str, prepend_newline: bool) -> String {
    let ts = current_local_timestamp();
    if prepend_newline {
        format!("\n{}\n[{}]\n{}\n", NOTE_SEPARATOR, ts, content)
    } else {
        format!("{}\n[{}]\n{}\n", NOTE_SEPARATOR, ts, content)
    }
}

pub fn execute_note_action(action: &NoteActionKind) -> io::Result<bool> {
    match action {
        NoteActionKind::AppendLine { file_path, content } => {
            if let Some(parent) = file_path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    let _ = fs::create_dir_all(parent);
                }
            }

            let file_non_empty = fs::metadata(file_path)
                .map(|m| m.len() > 0)
                .unwrap_or(false);

            let formatted = format_note_block(content, file_non_empty);

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
            let formatted = format_note_block(content, false);
            fs::write(file_path, formatted)?;
            invalidate_cached_file(file_path);
            Ok(true)
        }
        NoteActionKind::DeleteLastNote { file_path } => {
            if let Ok(raw) = fs::read_to_string(file_path) {
                let mut lines: Vec<&str> = raw.lines().collect();
                while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                    lines.pop();
                }
                // Pop the note content line(s) back to the last separator
                while let Some(line) = lines.pop() {
                    if is_separator_line(line.trim()) {
                        break;
                    }
                }
                while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                    lines.pop();
                }

                let new_body = if lines.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", lines.join("\n"))
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

fn current_local_timestamp() -> String {
    unsafe {
        let mut st: Win32SystemTime = zeroed();
        GetLocalTime(&mut st);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            st.w_year, st.w_month, st.w_day, st.w_hour, st.w_minute, st.w_second
        )
    }
}
