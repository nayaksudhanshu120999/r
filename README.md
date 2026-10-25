# ⚡ cmdpal-rs — Ultra-Lightweight Windows Command Palette & Extension Launcher

**`cmdpal-rs`** is a fast, native Windows Command Palette written in **Rust**, **Slint**, and direct **Win32 APIs**. It combines application launching, a 7-tier smart fuzzy/acronym ranking engine, separated **CmdPal** vs **Windows System Settings**, toggleable inbuilt shortcuts, a **pluggable Extension System** (including an inline **`.txt` / `.md` Notepad & File Viewer** via the `note` bang), an inline scientific/programmer calculator, and customizable web search bangs.

---

## ✨ Key Features

### 1. Separated Categories (`CmdPal` vs `Windows Settings` vs `System Commands` vs `Extensions`)
Unlike launchers that mix internal app preferences with OS settings, `cmdpal-rs` cleanly separates results into 7 priority- customizable shelves:
- **`🧩 EXTENSIONS & PLUGINS` (`Extensions`)** — Interactive extensions like the built-in **`note` Quick Notepad & File Viewer** and custom user plugins.
- **`🎛 CMDPAL SETTINGS & ACTIONS` (`CmdPal`)** — Command Palette's own settings, theme switcher, hotkey presets, startup toggle, default search engine switcher, and index reloader.
- **`⚡ SYSTEM COMMANDS` (`Cmds`)** — 18 inbuilt Windows system/power/admin commands (Task Manager, Device Manager, Services, Registry Editor, Flush DNS, Lock, Sleep, Restart, Shutdown, etc.), inline calculator, and custom shell commands.
- **`⚙ WINDOWS SYSTEM SETTINGS` (`Settings`)** — 19 inbuilt Windows `ms-settings:` shortcuts (Display, Sound, Bluetooth, Wi-Fi, Network, Windows Update, Apps, Storage, Personalization, etc.) and custom Control Panel applets.
- **`❖ APPLICATIONS` (`Apps`)** — Indexed Start Menu applications with per-app visibility toggles.
- **`📄 CUSTOM FILES` (`Files`)** — Direct file/folder paths (`C:\...`, `~/...`, `%APPDATA%\...`) and files indexed from custom scan directories.
- **`🌐 WEB SEARCH` (`Web`)** — Direct domains/URLs, prefix-free bang chips (`yt `, `gh `, `rs `, `ddg `), and fallback search using your chosen **Default Search Engine**.

---

### 2. 7-Tier Smart Suggestion & Ranking Engine
Every query is evaluated across 7 relevance signals so the best match always lands at the top:
1. **Exact Match** (`2400` pts)
2. **Exact Prefix Match** (`1850` pts)
3. **Word-Boundary Prefix Match** (`1520` pts — e.g., `update` immediately matches `Windows Update`)
4. **Acronym / Initialism Match** (`1360–1440` pts — e.g., `vsc` → *Visual Studio Code*, `tm` → *Task Manager*, `dm` → *Device Manager*, `cps` → *Command Palette Settings*)
5. **Multi-Token Out-of-Order Match** (`1220+` pts — e.g., `display setting`, `audio app`)
6. **Contiguous Substring Match** (`960` pts)
7. **Quality-Thresholded SkimV2 Fuzzy Match** (eliminates noisy single-letter false positives)

High-confidence matches automatically bubble their category to the top, while using your configured `category_order` as a tie-breaker.

---

### 3. Pluggable Extension System & Built-in `note` Notepad Editor
- **Quick Notepad (`note` Bang)**:
  - Summon the palette (`Ctrl+Space`) and type **`note `** (`note` + `Space`) to activate the **`📝 NOTE`** pill chip.
  - **Write & Save on `Enter`**: Type any text (e.g., `note Finish Rust PR and buy coffee`) and hit **`Enter`**. The text is appended to your configured `.txt` or `.md` file (`notes.md` by default), the input clears, and the palette refreshes in-place so you immediately see your new line!
  - **Timestamped Notes**: Choose **`🕒 Save Timestamped`** to prepend `[YYYY-MM-DD HH:MM:SS]`.
  - **Live File Viewer in the Palette**: All lines written in your `.txt` or `.md` file are displayed directly inside the palette. Press **`Enter`** on any line to copy it to the clipboard, type `note open` to open the file in your default editor, or `note clear` to clear the file.
- **1-Click Open Buttons in Settings (`Tab 6: Extensions`)**:
  - **`📂 Open Note File`** button: Opens your selected `.txt` or `.md` note file directly from Settings (creating it automatically if it doesn't exist yet).
  - **`📖 Open Extensions Guide`** button: Opens the bundled **`EXTENSIONS_GUIDE.md`** in one click. Even if `EXTENSIONS_GUIDE.md` is deleted from disk, `cmdpal-rs` automatically regenerates it from its embedded copy so the guide can never get lost.
- **Add / Remove / Create Extensions**:
  - Enable, disable (`ON`/`OFF`), or remove any extension (including the built-in `note` extension) in **Settings → Tab 6**, or create new `NotepadFile`, `QuickAppendFile`, `ShellCommand`, and `WebTemplate` extensions via the GUI, `config.toml`, or standalone `.toml` files in `./extensions/*.toml`.

---

### 4. Enable / Disable Inbuilt Shortcuts & Choose Default Search Engine
- **Inbuilt Shortcut Toggles**:
  - **Settings → Tab 3 (`3. System Cmds`)**: Master switch + individual `ON`/`OFF` toggles for all 18 inbuilt System Commands.
  - **Settings → Tab 4 (`4. Win Settings`)**: Master switch + individual `ON`/`OFF` toggles for all 19 inbuilt Windows `ms-settings:` shortcuts.
  - **Settings → Tab 1 (`1. General & Theme`)**: Toggle for showing/hiding CmdPal's own settings shortcuts in search.
- **Configurable Default Search Engine**:
  - **Settings → Tab 7 (`7. Web & Search`)**: Switch your default fallback search engine with 1 click between **Google**, **DuckDuckGo**, **Bing**, **Brave Search**, **Startpage**, **Perplexity**, **YouTube**, or any **Custom Search URL Template (`{q}`)**.

---

## 📁 Project Structure

~~~text
cmdpal-rs/
├── .github/
│   └── workflows/
│       └── build.yml          # GitHub Actions CI/CD workflow for Windows x64 release
├── .gitignore
├── Cargo.toml                 # Rust package & dependency configuration
├── build.rs                   # Slint UI compiler & procedural Windows .ico generator
├── config.toml                # Default runtime configuration, bangs & extensions
├── EXTENSIONS_GUIDE.md        # Step-by-step Extensions & Notepad guide (also embedded in exe)
├── README.md                  # Project documentation
├── ui/
│   └── palette.slint          # Slint UI for PaletteWindow & 7-tab SettingsWindow
└── src/
    ├── main.rs                # Application entry point, UI state sync & event loop
    ├── models.rs              # Core data structures, ShelfCategory & ExecutionOutcome
    ├── config.rs              # TOML config loader, normalizer & persistence helpers
    ├── extensions.rs          # Pluggable Extension Engine, Notepad ('note') & Guide manager
    ├── indexer.rs             # Start Menu (.lnk) & custom directory file scanner
    ├── engine.rs              # 7-tier smart ranking engine, calculator & bang parser
    ├── executor.rs            # Process/URI/Admin launcher & clipboard integration
    ├── icon.rs                # Procedural RGBA app & system tray icon renderer
    └── system.rs              # Win32 single-instance mutex, tray icon & global hotkey host
~~~

---

## ⌨️ Keyboard Shortcuts & Quick Usage

| Shortcut / Input | Action |
| :--- | :--- |
| **`Ctrl + Space`** *(configurable)* | Summon / Hide Command Palette |
| **`↑` / `↓`** | Navigate suggestions |
| **`Enter`** | Execute selected item / Save note in `note` mode |
| **`Ctrl + Shift + Enter`** | Run selected command, app, or file as **Administrator (UAC)** |
| **`Ctrl + ,`** | Open **Command Palette Settings** window |
| **`Backspace`** *(on empty input)* | Clear active Bang Pill Chip (e.g. exit `📝 NOTE` or `🌐 YT` mode) |
| **`Escape`** | Clear active Bang Pill Chip or hide Command Palette |
| **`note <text>` + `Enter`** | Append `<text>` to your `.txt`/`.md` note file & preview inline |
| **`note open` + `Enter`** | Open your `.txt`/`.md` note file in your default external editor |
| **`24 * 7` or `255 in hex`** | Evaluate math / base conversion and copy result on `Enter` |
| **`yt rust slint`** | Activate YouTube bang chip and search YouTube |

---

## 🛠️ Building from Source

### Prerequisites
- **Windows 10 / 11 (x64)**
- **Rust Stable Toolchain** (`rustup` + MSVC build tools)

### Build & Run
~~~bash
# Clone the repository
git clone https://github.com/nayaksudhanshu120999/r.git
cd r

# Run in development mode
cargo run

# Build size-optimized release binary
cargo build --release
~~~

The compiled standalone executable will be generated at:
`target/release/cmdpal-rs.exe`

On first launch, `cmdpal-rs.exe` automatically ensures `config.toml` and `EXTENSIONS_GUIDE.md` exist right next to the executable and registers the system tray icon and global hotkey.
