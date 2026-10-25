# 🧩 Command Palette (`cmdpal-rs`) — Extensions & Notepad Guide

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

3. Or create a CLI command extension such as **`extensions/ping.toml`**:

~~~toml
id = "quick_ping"
name = "Ping Host"
description = "Ping a domain or IP in a terminal"
bang = "ping"
icon = "📡"
enabled = true
kind = "ShellCommand"
target_file = ""
command_template = "ping -n 4 {q}"
~~~

4. Click **`↻ Rescan All`** in Settings (or run `Reload Command Palette Config & Indexes` from the palette).

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
