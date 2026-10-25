use crate::config::AppConfig;
use crate::models::{CandidateItem, ExecutionTarget, ExtensionActionKind, ShelfCategory};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BUILTIN_NOTEPAD_ID: &str = "notepad";
pub const EXTENSIONS_GUIDE_FILENAME: &str = "EXTENSIONS_GUIDE.md";

pub const DEFAULT_EXTENSIONS_GUIDE_MD: &str = r#"# 🧩 Command Palette (`cmdpal-rs`) — Extensions & Notepad Guide

> **Note:** This file (`EXTENSIONS_GUIDE.md`) is bundled inside `cmdpal-rs.exe`. Even if this file is deleted or you move the `.exe` to another folder, clicking **"📖 Open Extensions Guide"** in **Settings → Tab 6 (Extensions)** will automatically recreate and open this guide!

---

## 1. Using the Built-in Quick Notepad (`note`) Extension

The **`note`** extension turns your Command Palette into a lightning-fast **Notepad Editor & File Viewer** backed by any `.txt` or `.md` file.

### A. Selecting or Opening Your `.txt` or `.md` File
1. Summon Command Palette (`Ctrl+Space`) and press **`Ctrl+,`** (or click the `⚙` icon) to open **Settings**.
2. Click **Tab 6 (`6. Extensions (Note)`)**.
3. Under **`📝 Quick Notepad ('note' Bang)`**:
   - Type any file path (e.g. `notes.md`, `notes.txt`, `~/Desktop/ideas.md`, or `C:\Users\You\Documents\todo.txt`) or click one of the quick preset buttons (`notes.md`, `notes.txt`, `~/Desktop/notes.md`).
   - Click **`Save File`** to set it as your active note file.
   - Click **`📂 Open Note File`** at any time to open the selected `.txt` or `.md` file directly in your default external editor (Notepad, VS Code, Obsidian, etc.).

### B. Writing & Saving Notes from the Palette
1. Summon Command Palette (`Ctrl+Space`).
2. Type **`note `** (`note` + `Space`). A blue **`📝 NOTE`** pill chip will appear inside the search bar.
3. Type the text you want to save (for example: `Buy groceries and review PR #42`) and press **`Enter`**.
4. **What happens when you press `Enter`**:
   - **Row 1 (`💾 Save to <file>`) — Default**: Appends your text to a new line in your `.txt` or `.md` file, clears the search box, and **keeps the palette open** so you immediately see your new line in the live file preview below!
   - **Row 2 (`🕒 Save Timestamped`)**: Appends your note with a `[YYYY-MM-DD HH:MM:SS]` timestamp (`- **[timestamp]** text` in `.md` files, or `[timestamp] text` in `.txt` files).
   - **Row 3 (`📄 Overwrite <file>`)**: Replaces the entire file content with your typed text.

### C. Viewing, Copying & Managing File Lines Inside the Palette
- Whenever the **`📝 NOTE`** chip is active, the palette reads your `.txt` or `.md` file and displays its lines right inside the suggestion list (`📝 <line content>`).
- **Filter lines**: Start typing while in `note` mode to filter existing lines in the file alongside the Save options.
- **Copy a line**: Click or press **`Enter`** on any `📝` line to copy that line to your clipboard.
- **Delete last line**: When the query box is empty in `note` mode, select **`⌫ Delete Last Line`** and press `Enter`.
- **Open file externally**: Type **`note open`** + `Enter` (or click `📂 Open in External Editor` in the list).
- **Clear entire file**: Type **`note clear`** + `Enter`.

---

## 2. How to Create & Add Your Own Extensions

`cmdpal-rs` supports **4 Extension Types**:

| Extension Kind | What It Does | Target / Template Field |
| :--- | :--- | :--- |
| **`NotepadFile`** | Full interactive palette notepad & inline file viewer for a `.txt` or `.md` file | File path (e.g. `todo.md`, `~/work.txt`) |
| **`QuickAppendFile`** | Appends typed text to a file on `Enter` and immediately closes the palette | File path (e.g. `log.txt`) |
| **`ShellCommand`** | Runs a CLI / PowerShell / Script command where `{q}` is replaced by your query | Command template (e.g. `code "{q}"`) |
| **`WebTemplate`** | Opens a custom URL or URI scheme where `{q}` is replaced by your URL-encoded query | URL template (e.g. `https://chatgpt.com/?q={q}`) |

---

### Method 1: Create an Extension in the Settings GUI (No Coding Required)
1. Open **Settings (`Ctrl+,`)** → **Tab 6 (`6. Extensions (Note)`)**.
2. In the **`Add New Extension`** card:
   - Choose the **Extension Type** (`NotepadFile`, `QuickAppendFile`, `ShellCommand`, or `WebTemplate`).
   - Enter the **Extension Name** (e.g. `Work To-Do List`).
   - Enter the **Bang Trigger** (e.g. `todo` — without spaces).
   - Enter the **Target File Path or Command/URL Template** (e.g. `todos.md`).
3. Click **`+ Add Extension`**.
4. You can now open the palette and type **`todo `** (`todo` + `Space`) to use your new extension! If it is a file extension, a **`📂 Open File`** button will also appear next to it in Settings.

---

### Method 2: Create a Standalone `.toml` Extension in the `extensions/` Folder
You can share or version-control extensions as standalone `.toml` files:
1. Create a folder named **`extensions`** next to `cmdpal-rs.exe`.
2. Inside `extensions/`, create a file such as **`extensions/todo.toml`**:

~~~toml
id = "todo_list"
name = "Project To-Do List"
description = "Manage project tasks in todos.md"
bang = "todo"
icon = "✅"
enabled = true
kind = "NotepadFile"
target_file = "todos.md"
command_template = ""
~~~

3. Click **`↻ Rescan All`** in Settings (or run `Reload Command Palette Config & Indexes` from the palette).

---

### Method 3: Edit `config.toml` Directly
Add an `[[extensions]]` section to `config.toml`:

~~~toml
[[extensions]]
id = "ideas"
name = "Ideas Scratchpad"
description = "Save startup and coding ideas"
bang = "idea"
icon = "💡"
enabled = true
kind = "NotepadFile"
target_file = "ideas.md"
command_template = ""
~~~

---

## 3. How to Disable or Remove an Extension

1. **Temporarily Disable (`ON` / `OFF`)**:
   - Open **Settings (`Ctrl+,`) → Tab 6 (`6. Extensions (Note)`)**.
   - In the **Installed Extensions** list, click the **`ON` / `OFF`** toggle button on any extension row.
2. **Remove an Extension**:
   - Click the red **`Remove`** button on the extension row in **Settings → Tab 6**.
   - If you remove the built-in `note` extension, a **`+ Re-Add 'note' Extension`** button will appear at the top of Tab 6 so you can bring it back anytime with one click.
"#;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ExtensionKind {
    /// Full interactive Notepad & File Viewer (saves notes on Enter + displays file lines inline)
    NotepadFile,
    /// Appends text to a target file and closes palette
    QuickAppendFile,
    /// Executes a shell/CLI command template with `{q}` replaced by query
    ShellCommand,
    /// Opens a URL/URI template with `{q}` replaced by URL-encoded query
    WebTemplate,
}

impl Default for ExtensionKind {
    fn default() -> Self {
        Self::NotepadFile
    }
}

impl ExtensionKind {
    pub fn from_label(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "notepad" | "notepadfile" | "editor" | "note" => Self::NotepadFile,
            "append" | "quickappend" | "quickappendfile" | "file" => Self::QuickAppendFile,
            "shell" | "cmd" | "command" | "shellcommand" | "script" => Self::ShellCommand,
            "web" | "url" | "webtemplate" => Self::WebTemplate,
            _ => Self::ShellCommand,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotepadFile => "NotepadFile",
            Self::QuickAppendFile => "QuickAppendFile",
            Self::ShellCommand => "ShellCommand",
            Self::WebTemplate => "WebTemplate",
        }
    }

    pub fn is_file_backed(&self) -> bool {
        matches!(self, Self::NotepadFile | Self::QuickAppendFile)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtensionEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub bang: String,
    #[serde(default = "default_ext_icon")]
    pub icon: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub kind: ExtensionKind,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub command_template: String,
}

fn default_ext_icon() -> String {
    "🧩".to_string()
}

fn default_true() -> bool {
    true
}

impl ExtensionEntry {
    pub fn builtin_notepad() -> Self {
        Self {
            id: BUILTIN_NOTEPAD_ID.to_string(),
            name: "Quick Notepad & File Viewer".to_string(),
            description: "Type 'note <text>' + Enter to save to your .txt/.md file, or view file contents right in the palette".to_string(),
            bang: "note".to_string(),
            icon: "📝".to_string(),
            enabled: true,
            kind: ExtensionKind::NotepadFile,
            target_file: "notes.md".to_string(),
            command_template: String::new(),
        }
    }
}

/// Ensures `EXTENSIONS_GUIDE.md` always exists next to the app/config so it can never get lost.
pub fn ensure_extensions_guide_file() -> PathBuf {
    let guide_path = AppConfig::resolve_config_path(EXTENSIONS_GUIDE_FILENAME);
    if !guide_path.exists() {
        let _ = fs::write(&guide_path, DEFAULT_EXTENSIONS_GUIDE_MD);
    }
    guide_path
}

/// Ensures `EXTENSIONS_GUIDE.md` exists on disk and opens it in the user's default editor/viewer.
pub fn open_extensions_guide() -> io::Result<()> {
    let guide_path = ensure_extensions_guide_file();
    crate::executor::open_path_in_default_app(&guide_path)
}

/// Resolves a `.txt` or `.md` target file, creates it (and any parent folders) if it doesn't exist yet,
/// and opens it directly in the system's default text/markdown editor.
pub fn open_extension_target_file(raw_path: &str) -> io::Result<PathBuf> {
    let resolved = resolve_extension_file_path(raw_path);
    if let Some(parent) = resolved.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }
    if !resolved.exists() {
        let _ = OpenOptions::new().create(true).write(true).open(&resolved);
    }
    crate::executor::open_path_in_default_app(&resolved)?;
    Ok(resolved)
}

/// Resolves a note/extension target file path. Relative paths (like `"notes.md"` or `"notes.txt"`)
/// are resolved next to the executable/config so they work reliably from anywhere.
pub fn resolve_extension_file_path(raw_path: &str) -> PathBuf {
    let trimmed = raw_path.trim().trim_matches('"');
    let fallback = if trimmed.is_empty() {
        "notes.md"
    } else {
        trimmed
    };

    let expanded = crate::engine::expand_env_and_tilde(fallback);
    let p = Path::new(&expanded);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        AppConfig::resolve_config_path(p)
    }
}

/// Scans optional `./extensions/*.toml` files so users can drop standalone extension manifests
/// into the `extensions/` folder next to the binary or working directory.
pub fn load_external_extension_manifests() -> Vec<ExtensionEntry> {
    let mut dirs = Vec::new();
    dirs.push(PathBuf::from("extensions"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let exe_ext = parent.join("extensions");
            if !dirs.contains(&exe_ext) {
                dirs.push(exe_ext);
            }
        }
    }

    let mut discovered = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("toml"))
                .unwrap_or(false)
            {
                if let Ok(raw) = fs::read_to_string(&path) {
                    if let Ok(ext) = toml::from_str::<ExtensionEntry>(&raw) {
                        if !ext.id.trim().is_empty() && !ext.bang.trim().is_empty() {
                            discovered.push(ext);
                        }
                    }
                }
            }
        }
    }
    discovered
}

/// Evaluates an active extension when its bang chip is active (e.g. `active_bang == "note"`).
pub fn evaluate_extension_bang(ext: &ExtensionEntry, query: &str) -> Vec<CandidateItem> {
    let clean_q = query.trim();
    match ext.kind {
        ExtensionKind::NotepadFile => evaluate_notepad_extension(ext, clean_q),
        ExtensionKind::QuickAppendFile => evaluate_quick_append_extension(ext, clean_q),
        ExtensionKind::ShellCommand => evaluate_shell_extension(ext, clean_q),
        ExtensionKind::WebTemplate => evaluate_web_extension(ext, clean_q),
    }
}

fn evaluate_notepad_extension(ext: &ExtensionEntry, query: &str) -> Vec<CandidateItem> {
    let resolved_path = resolve_extension_file_path(&ext.target_file);
    let file_name = resolved_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("notes.md")
        .to_string();
    let full_path_display = resolved_path.to_string_lossy().to_string();

    let raw_content = fs::read_to_string(&resolved_path).unwrap_or_default();
    let lines: Vec<(usize, String)> = raw_content
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.to_string()))
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();

    let mut out = Vec::new();
    let q_lower = query.to_ascii_lowercase();

    if q_lower == "open" || q_lower == "!open" {
        out.push(CandidateItem {
            title: format!("📂 Open {} in Default Editor", file_name),
            subtitle: format!("Open {} externally", full_path_display),
            badge: "NOTE".to_string(),
            icon_override: Some("📂".to_string()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 10_000,
            target: ExecutionTarget::FilePath(resolved_path.clone()),
        });
    } else if q_lower == "clear" || q_lower == "!clear" {
        out.push(CandidateItem {
            title: format!("🗑 Clear All Notes in {}", file_name),
            subtitle: format!(
                "Delete all {} non-empty lines from {}",
                lines.len(),
                full_path_display
            ),
            badge: "CLEAR".to_string(),
            icon_override: Some("🗑".to_string()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 10_000,
            target: ExecutionTarget::ExtensionAction(ExtensionActionKind::ClearNoteFile {
                file_path: resolved_path.clone(),
            }),
        });
    }

    if !query.is_empty() {
        out.push(CandidateItem {
            title: format!("💾 Save to {}: \"{}\"", file_name, query),
            subtitle: format!(
                "Press Enter to append to {} (updates preview below)",
                full_path_display
            ),
            badge: "SAVE NOTE".to_string(),
            icon_override: Some("💾".to_string()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 9_950,
            target: ExecutionTarget::ExtensionAction(ExtensionActionKind::AppendNote {
                file_path: resolved_path.clone(),
                content: query.to_string(),
                with_timestamp: false,
            }),
        });

        out.push(CandidateItem {
            title: format!("🕒 Save Timestamped to {}: \"{}\"", file_name, query),
            subtitle: format!(
                "Append with [{}] timestamp to {}",
                current_timestamp_label(),
                file_name
            ),
            badge: "TIMESTAMP".to_string(),
            icon_override: Some("🕒".to_string()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 9_900,
            target: ExecutionTarget::ExtensionAction(ExtensionActionKind::AppendNote {
                file_path: resolved_path.clone(),
                content: query.to_string(),
                with_timestamp: true,
            }),
        });

        out.push(CandidateItem {
            title: format!("📄 Overwrite {} with: \"{}\"", file_name, query),
            subtitle: format!(
                "Replace all existing content in {} with this note",
                file_name
            ),
            badge: "OVERWRITE".to_string(),
            icon_override: Some("📄".to_string()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 9_850,
            target: ExecutionTarget::ExtensionAction(ExtensionActionKind::OverwriteNote {
                file_path: resolved_path.clone(),
                content: query.to_string(),
            }),
        });
    }

    if lines.is_empty() {
        out.push(CandidateItem {
            title: format!(
                "✨ {} is empty — Type any text above and press Enter to save a note!",
                file_name
            ),
            subtitle: format!(
                "Target file: {} (Click to open in external editor)",
                full_path_display
            ),
            badge: "EMPTY".to_string(),
            icon_override: Some("📝".to_string()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 9_500,
            target: ExecutionTarget::FilePath(resolved_path.clone()),
        });
    } else {
        let mut displayed = 0usize;
        for (line_num, line_text) in lines.iter().rev() {
            if !query.is_empty()
                && !line_text.to_ascii_lowercase().contains(&q_lower)
                && displayed >= 5
            {
                continue;
            }
            out.push(CandidateItem {
                title: format!("📝 {}", line_text),
                subtitle: format!(
                    "Line #{} in {} • Press Enter to copy line to clipboard",
                    line_num, file_name
                ),
                badge: "FILE LINE".to_string(),
                icon_override: Some("📌".to_string()),
                category: ShelfCategory::Extensions,
                can_elevate: false,
                score: 9_400 - (displayed as i64),
                target: ExecutionTarget::CopyToClipboard(line_text.clone()),
            });
            displayed += 1;
            if displayed >= 14 {
                break;
            }
        }

        if query.is_empty() {
            if let Some((last_line_num, last_text)) = lines.last() {
                let raw_idx = last_line_num.saturating_sub(1);
                out.push(CandidateItem {
                    title: format!("⌫ Delete Last Line (#{}): \"{}\"", last_line_num, last_text),
                    subtitle: format!("Remove line #{} from {}", last_line_num, file_name),
                    badge: "DELETE".to_string(),
                    icon_override: Some("⌫".to_string()),
                    category: ShelfCategory::Extensions,
                    can_elevate: false,
                    score: 8_900,
                    target: ExecutionTarget::ExtensionAction(ExtensionActionKind::DeleteNoteLine {
                        file_path: resolved_path.clone(),
                        line_index: raw_idx,
                    }),
                });
            }

            out.push(CandidateItem {
                title: format!(
                    "📋 Copy Entire {} ({} lines) to Clipboard",
                    file_name,
                    lines.len()
                ),
                subtitle: format!("Copy all text from {}", full_path_display),
                badge: "COPY ALL".to_string(),
                icon_override: Some("📋".to_string()),
                category: ShelfCategory::Extensions,
                can_elevate: false,
                score: 8_850,
                target: ExecutionTarget::CopyToClipboard(raw_content),
            });
        }
    }

    out.push(CandidateItem {
        title: format!("📂 Open {} in External Text/Markdown Editor", file_name),
        subtitle: full_path_display,
        badge: "OPEN FILE".to_string(),
        icon_override: Some("📂".to_string()),
        category: ShelfCategory::Extensions,
        can_elevate: false,
        score: 8_800,
        target: ExecutionTarget::FilePath(resolved_path.clone()),
    });

    if !lines.is_empty() && query.is_empty() {
        out.push(CandidateItem {
            title: format!("🗑 Clear All Content in {}", file_name),
            subtitle: "Erase all saved notes in the selected file".to_string(),
            badge: "CLEAR".to_string(),
            icon_override: Some("🗑".to_string()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 8_750,
            target: ExecutionTarget::ExtensionAction(ExtensionActionKind::ClearNoteFile {
                file_path: resolved_path,
            }),
        });
    }

    out.push(CandidateItem {
        title: format!(
            "⚙ Change Notepad File ({}) or Manage Extensions...",
            file_name
        ),
        subtitle: "Open Command Palette Settings → Extensions tab to select .txt or .md file"
            .to_string(),
        badge: "SETTINGS".to_string(),
        icon_override: Some("⚙".to_string()),
        category: ShelfCategory::Extensions,
        can_elevate: false,
        score: 8_700,
        target: ExecutionTarget::OpenPaletteSettingsTab(5),
    });

    out
}

fn evaluate_quick_append_extension(ext: &ExtensionEntry, query: &str) -> Vec<CandidateItem> {
    let resolved_path = resolve_extension_file_path(&ext.target_file);
    let file_name = resolved_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output.txt")
        .to_string();

    if query.is_empty() {
        vec![CandidateItem {
            title: format!(
                "{} {} — Type text above to append to {}",
                ext.icon, ext.name, file_name
            ),
            subtitle: format!(
                "Target file: {} (Press Enter to open file)",
                resolved_path.display()
            ),
            badge: "EXT".to_string(),
            icon_override: Some(ext.icon.clone()),
            category: ShelfCategory::Extensions,
            can_elevate: false,
            score: 9_800,
            target: ExecutionTarget::FilePath(resolved_path),
        }]
    } else {
        vec![
            CandidateItem {
                title: format!("💾 Append to {}: \"{}\"", file_name, query),
                subtitle: format!(
                    "Extension: {} • Target: {}",
                    ext.name,
                    resolved_path.display()
                ),
                badge: "EXT SAVE".to_string(),
                icon_override: Some(ext.icon.clone()),
                category: ShelfCategory::Extensions,
                can_elevate: false,
                score: 9_900,
                target: ExecutionTarget::ExtensionAction(ExtensionActionKind::AppendNote {
                    file_path: resolved_path.clone(),
                    content: query.to_string(),
                    with_timestamp: false,
                }),
            },
            CandidateItem {
                title: format!("🕒 Append Timestamped to {}: \"{}\"", file_name, query),
                subtitle: format!(
                    "Extension: {} • Target: {}",
                    ext.name,
                    resolved_path.display()
                ),
                badge: "TIMESTAMP".to_string(),
                icon_override: Some("🕒".to_string()),
                category: ShelfCategory::Extensions,
                can_elevate: false,
                score: 9_850,
                target: ExecutionTarget::ExtensionAction(ExtensionActionKind::AppendNote {
                    file_path: resolved_path,
                    content: query.to_string(),
                    with_timestamp: true,
                }),
            },
        ]
    }
}

fn evaluate_shell_extension(ext: &ExtensionEntry, query: &str) -> Vec<CandidateItem> {
    let cmd = if ext.command_template.contains("{q}") {
        ext.command_template.replace("{q}", query)
    } else if query.is_empty() {
        ext.command_template.clone()
    } else {
        format!("{} {}", ext.command_template, query)
    };

    vec![CandidateItem {
        title: if query.is_empty() {
            format!("{} Run {}", ext.icon, ext.name)
        } else {
            format!("{} {}: {}", ext.icon, ext.name, query)
        },
        subtitle: format!("Execute command: {}", cmd),
        badge: "EXT CMD".to_string(),
        icon_override: Some(ext.icon.clone()),
        category: ShelfCategory::Extensions,
        can_elevate: true,
        score: 9_900,
        target: ExecutionTarget::CustomCommand(cmd),
    }]
}

fn evaluate_web_extension(ext: &ExtensionEntry, query: &str) -> Vec<CandidateItem> {
    let encoded = urlencoding::encode(query);
    let url = if ext.command_template.contains("{q}") {
        ext.command_template.replace("{q}", encoded.as_ref())
    } else {
        format!("{}{}", ext.command_template, encoded)
    };

    vec![CandidateItem {
        title: if query.is_empty() {
            format!("{} {} (Type query...)", ext.icon, ext.name)
        } else {
            format!("{} {}: \"{}\"", ext.icon, ext.name, query)
        },
        subtitle: url.clone(),
        badge: "EXT WEB".to_string(),
        icon_override: Some(ext.icon.clone()),
        category: ShelfCategory::Extensions,
        can_elevate: false,
        score: 9_900,
        target: ExecutionTarget::WebUrl(url),
    }]
}

/// Executes an `ExtensionActionKind` and returns whether the palette should stay open and refresh.
pub fn execute_extension_action(action: &ExtensionActionKind) -> io::Result<bool> {
    match action {
        ExtensionActionKind::AppendNote {
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

            let needs_leading_newline = if file_path.exists() {
                let existing = fs::read_to_string(file_path).unwrap_or_default();
                !existing.is_empty() && !existing.ends_with('\n')
            } else {
                false
            };

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)?;

            if needs_leading_newline {
                writeln!(file)?;
            }

            if *with_timestamp {
                let ts = current_timestamp_label();
                if is_md {
                    writeln!(file, "- **[{}]** {}", ts, content)?;
                } else {
                    writeln!(file, "[{}] {}", ts, content)?;
                }
            } else {
                writeln!(file, "{}", content)?;
            }
            file.flush()?;
            Ok(true)
        }
        ExtensionActionKind::OverwriteNote { file_path, content } => {
            if let Some(parent) = file_path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    let _ = fs::create_dir_all(parent);
                }
            }
            fs::write(file_path, format!("{}\n", content))?;
            Ok(true)
        }
        ExtensionActionKind::DeleteNoteLine {
            file_path,
            line_index,
        } => {
            if file_path.exists() {
                let raw = fs::read_to_string(file_path).unwrap_or_default();
                let mut lines: Vec<&str> = raw.lines().collect();
                if *line_index < lines.len() {
                    lines.remove(*line_index);
                    let updated = if lines.is_empty() {
                        String::new()
                    } else {
                        format!("{}\n", lines.join("\n"))
                    };
                    fs::write(file_path, updated)?;
                }
            }
            Ok(true)
        }
        ExtensionActionKind::ClearNoteFile { file_path } => {
            if file_path.exists() {
                fs::write(file_path, "")?;
            }
            Ok(true)
        }
        ExtensionActionKind::ToggleExtension { id, enable } => {
            let mut cfg = AppConfig::load_or_create("config.toml");
            if let Some(ext) = cfg
                .extensions
                .iter_mut()
                .find(|e| e.id.eq_ignore_ascii_case(id))
            {
                ext.enabled = *enable;
            }
            cfg.save("config.toml");
            crate::system::request_config_reload();
            Ok(false)
        }
        ExtensionActionKind::RemoveExtension { id } => {
            let mut cfg = AppConfig::load_or_create("config.toml");
            cfg.remove_extension_by_id(id);
            cfg.save("config.toml");
            crate::system::request_config_reload();
            Ok(false)
        }
        ExtensionActionKind::RestoreBuiltinNotepad => {
            let mut cfg = AppConfig::load_or_create("config.toml");
            cfg.restore_builtin_notepad();
            cfg.save("config.toml");
            crate::system::request_config_reload();
            Ok(false)
        }
    }
}

pub fn current_timestamp_label() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, m, d, hour, minute, second
    )
}
