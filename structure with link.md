cmdpal-rs/
├── .github/
│   └── workflows/
│       └── build.yml     https://github.com/nayaksudhanshu120999/r/blob/.github/workflows/build.yml       # GitHub Actions CI/CD workflow for Windows x64 release
├── .gitignore            https://github.com/nayaksudhanshu120999/r/blob/main/.gitignore
├── Cargo.toml            https://github.com/nayaksudhanshu120999/r/blob/main/Cargo.toml                   # Rust package & dependency configuration
├── build.rs              https://github.com/nayaksudhanshu120999/r/blob/main/build.rs                     # Slint UI compiler & procedural Windows .ico generator
├── config.toml           https://github.com/nayaksudhanshu120999/r/blob/main/config.toml                  # Default runtime configuration, bangs & extensions
├── EXTENSIONS_GUIDE.md   https://github.com/nayaksudhanshu120999/r/blob/main/EXTENSIONS_GUIDE.md          # Step-by-step Extensions & Notepad guide (also embedded in exe)
├── README.md             https://github.com/nayaksudhanshu120999/r/blob/main/README.md                    # Project documentation
├── ui/
│   └── palette.slint     https://github.com/nayaksudhanshu120999/r/blob/main/ui/palette.slint             # Slint UI for PaletteWindow & 7-tab SettingsWindow
└── src/
    ├── main.rs           https://github.com/nayaksudhanshu120999/r/blob/main/src/main.rs                  # Application entry point, UI state sync & event loop
    ├── models.rs         https://github.com/nayaksudhanshu120999/r/blob/main/src/models.rs                # Core data structures, ShelfCategory & ExecutionOutcome
    ├── config.rs         https://github.com/nayaksudhanshu120999/r/blob/main/src/config.rs                # TOML config loader, normalizer & persistence helpers
    ├── extensions.rs     https://github.com/nayaksudhanshu120999/r/blob/main/src/extensions.rs            # Pluggable Extension Engine, Notepad ('note') & Guide manager
    ├── indexer.rs        https://github.com/nayaksudhanshu120999/r/blob/main/src/indexer.rs               # Start Menu (.lnk) & custom directory file scanner
    ├── engine.rs         https://github.com/nayaksudhanshu120999/r/blob/main/src/engine.rs                # 7-tier smart ranking engine, calculator & bang parser
    ├── executor.rs       https://github.com/nayaksudhanshu120999/r/blob/main/src/executor.rs              # Process/URI/Admin launcher & clipboard integration
    ├── icon.rs           https://github.com/nayaksudhanshu120999/r/blob/main/src/icon.rs                  # Procedural RGBA app & system tray icon renderer
    └── system.rs         https://github.com/nayaksudhanshu120999/r/blob/main/src/system.rs                # Win32 single-instance mutex, tray icon & global hotkey host
